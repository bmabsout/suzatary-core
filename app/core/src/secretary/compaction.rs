//! The compactor's rule: a node waiting for its summary is owed one ("pending non-empty ⇒
//! compact the most pending node, append its outcome"), fired once per reading by the box's
//! engine like the conversation's rules, and discharged by the outcome it records.
//!
//! The most pending node is the first the Prodrome's `pending` offers (smallest, then
//! leftmost) that this rule's model has not already found unfit. The outcome is an object of the
//! digest's history, a summary or an `Unfit`, so it moves the reading's name and the next firing
//! takes the next node; an attempt that failed records nothing and is not tried again on the same
//! reading. An import of the conversations that already exist is no special path: it is this rule
//! over those lines, at the budget's pace.
//!
//! Every call is stamped at its own instant and the outcome at the instant it came, read from the
//! clock each time, since five tries of a slow model span minutes and a budget's windows fold the
//! instants. The model call is made outside the memory's hold, so a slow model never keeps the
//! owner from remembering; the write is made under it ([`crate::secretary::memories::Held::summarize`]),
//! which asks again whether the node still waits, and every step refuses while a store's read
//! left anything out.

use prodrome_core::event::{Envelope, Hash};
use prodrome_core::fpl::instant_of;
use prodrome_core::literal::Datetime;

use crate::capability::{Clock, Compact};
use crate::models::ModelName;
use crate::refusal::Refusal;
use crate::rule::{Engine, Named, ReadingName, Rule};
use crate::secretary::compact_ask::Ask;
use crate::secretary::compact_budget::{Budget, Hold};
use crate::secretary::compacting::Compacting;
use crate::secretary::compactor::{self, Outcome};
use crate::secretary::digest::{Digest, Written};
use crate::secretary::digest_event::DigestEvent;
use crate::secretary::digest_line::NODE_BYTES;
use crate::secretary::digest_spent::Spent;
use crate::secretary::digests::{Digests, compactor};
use crate::secretary::line_event::LineEvent;
use crate::secretary::memories::Memories;
use crate::secretary::store::Entry;

/// The one digest the box keeps, as a rule's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TheDigest;

impl std::fmt::Display for TheDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the digest")
    }
}

/// What the rule reads: the digest, and the lines it is of, read whole.
#[derive(Debug, Clone)]
pub struct Reading {
    digest: Digest,
    lines: Vec<(Hash, Envelope<LineEvent>)>,
    opened: std::collections::BTreeMap<Hash, Entry>,
}

impl Reading {
    /// The digest as `digests` holds it now.
    ///
    /// # Errors
    /// [`Digests::whole`]'s, and the stores' own.
    pub fn of(digests: &Digests<'_>) -> Result<Self, Refusal> {
        digests.whole()?;
        let lines = digests.lines.lines.objects()?;
        let opened = digests.opened(&lines);
        Ok(Self {
            digest: digests.read_over(&lines, &opened)?,
            lines,
            opened,
        })
    }

    /// The digest.
    #[must_use]
    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    /// The lines the digest is of, in the store's order.
    #[must_use]
    pub fn lines(&self) -> &[(Hash, Envelope<LineEvent>)] {
        &self.lines
    }

    /// Each line as read, by its name.
    #[must_use]
    pub fn entries(&self) -> std::collections::BTreeMap<&Hash, &Entry> {
        self.opened.iter().collect()
    }

    /// The question for `node` and what its summary would be written from.
    ///
    /// # Errors
    /// [`Refusal::internal`] for a node whose children do not read, which a pending node's do.
    fn ask(&self, node: &Hash) -> Result<(Ask, Vec<Hash>), Refusal> {
        Ask::of(&self.digest, &self.entries(), node).ok_or_else(|| {
            Refusal::internal(format!(
                "node {node} is pending and its children do not read"
            ))
        })
    }

    /// What `rule`'s next call would spend: the first question for its most pending node.
    /// `None` when nothing is owed.
    ///
    /// # Errors
    /// As [`Reading::ask`].
    fn next_call(&self, rule: &Compaction) -> Result<Option<Spent>, Refusal> {
        rule.due(&self.digest)
            .map(|node| Ok(Spent::call(self.ask(&node)?.0.bytes())))
            .transpose()
    }
}

/// Named by the digest's own name, which every attempt's outcome moves.
impl Named for Reading {
    fn name(&self) -> ReadingName {
        self.digest.name()
    }
}

/// Compact with `model`: the rule's own field, since what is owed is what that model has not
/// already found unfit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compaction {
    /// The model the compactor runs.
    pub model: ModelName,
}

impl Compaction {
    /// Every node owed a summary, most pending first: what `pending` offers, less each this
    /// model found unfit at today's [`NODE_BYTES`] or above, so another model, or a larger
    /// limit, is owed it again.
    #[must_use]
    pub fn owed(&self, digest: &Digest) -> Vec<Hash> {
        digest
            .pending()
            .into_iter()
            .filter(|node| {
                !digest.candidates(node).iter().any(|candidate| {
                    matches!(
                        &candidate.said,
                        Written::Unfit { model, limit, .. }
                            if model == &self.model
                                && usize::try_from(*limit).unwrap_or(usize::MAX) >= NODE_BYTES
                    )
                })
            })
            .collect()
    }

    /// The most pending node.
    #[must_use]
    pub fn due(&self, digest: &Digest) -> Option<Hash> {
        self.owed(digest).into_iter().next()
    }
}

impl Rule for Compaction {
    type Key = TheDigest;
    type Reading = Reading;
    type Witness = ();

    fn name(&self) -> &'static str {
        "compact"
    }

    fn holds(&self, reading: &Reading) -> bool {
        self.due(&reading.digest).is_some()
    }
}

/// What one firing did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compacted {
    /// A summary of `node`, recorded as `object`.
    Summarized {
        /// The node.
        node: Hash,
        /// Its event.
        object: Hash,
    },
    /// No line fitted `node`; recorded as `object`.
    Unfit {
        /// The node.
        node: Hash,
        /// Its event.
        object: Hash,
    },
}

/// Perform the rule on `reading`: ask `compact` for the most pending node's line and record
/// the outcome at the instant it came; a question no day's cap admits is recorded unfit,
/// asking nothing. Every call is appended as an `Asked` stamped at its own
/// instant before it is made, and refused once `budget` holds then, so no call goes uncounted
/// and none is made past a cap. `None` when nothing is owed.
///
/// # Errors
/// The budget's hold, as a [`Refusal::conflict`]; [`Refusal::unprocessable`] for a compactor
/// that did not answer; the write's refusals (the node no longer waits, a store's read left
/// something out) and the stores' own.
pub fn perform(
    memories: &Memories<'_>,
    compact: &dyn Compact,
    reading: &Reading,
    budget: Budget,
    clock: &dyn Clock,
) -> Result<Option<Compacted>, Refusal> {
    let rule = Compaction {
        model: compact.model().clone(),
    };
    let Some(node) = rule.due(&reading.digest) else {
        return Ok(None);
    };
    let (ask, over) = reading.ask(&node)?;
    if !budget.admits(Spent::call(ask.bytes())) {
        // No call is made: a question larger than a whole day's cap would wait forever.
        return Ok(Some(Compacted::Unfit {
            object: memories.hold()?.unfit(&node, rule.model, 0, clock.now())?,
            node,
        }));
    }
    let mut spent: Vec<(Datetime, Spent)> = reading.digest.attempts().collect();
    let history = memories.digests.history;
    let outcome = compactor::compact(compact, ask, &mut |sent| {
        let now = clock.now();
        if let Some(hold) = budget.hold(&spent, Spent::call(sent), instant_of(now)) {
            return Err(Refusal::conflict(hold.to_string()));
        }
        history.append(DigestEvent::asked(node.clone(), now, compactor(), sent))?;
        spent.push((now, Spent::call(sent)));
        Ok(())
    })?;
    let now = clock.now();
    let held = memories.hold()?;
    let model = rule.model;
    Ok(Some(match outcome {
        Outcome::Fits { line } => Compacted::Summarized {
            object: held.summarize(&node, &line, over, model, now)?,
            node,
        },
        Outcome::Unfit { shortest } => Compacted::Unfit {
            object: held.unfit(&node, model, shortest, now)?,
            node,
        },
    }))
}

/// What one step of the paced rule came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The rule fired and recorded its outcome.
    Compacted(Compacted),
    /// Work is owed and the budget holds it back until [`Hold::until`].
    Held(Hold),
    /// Nothing is owed, or the rule already fired on this reading.
    Idle,
}

/// The rule at the budget's pace: its engine, which fires it once per reading, and the caps it
/// is held to. The box keeps one for as long as it runs; a restart's fresh engine fires nothing
/// already recorded, since every outcome moves the reading.
#[derive(Debug, Clone)]
pub struct Paced {
    engine: Engine<Compaction>,
    budget: Budget,
}

impl Paced {
    /// The rule for `compact`'s model, held to `budget`.
    #[must_use]
    pub fn new(compact: &dyn Compact, budget: Budget) -> Self {
        Self {
            engine: Engine::new(vec![Compaction {
                model: compact.model().clone(),
            }]),
            budget,
        }
    }

    /// One step at `clock`'s instant: read the digest whole, and if work is owed, either say
    /// what holds its next call back or fire the rule and perform it. A hold consumes no firing,
    /// so the same reading is owed again once the hold frees.
    ///
    /// # Errors
    /// [`Reading::of`]'s and [`perform`]'s; a firing that failed is not made again on its
    /// reading.
    pub fn step(
        &mut self,
        memories: &Memories<'_>,
        compact: &dyn Compact,
        clock: &dyn Clock,
    ) -> Result<Step, Refusal> {
        let now = clock.now();
        let reading = Reading::of(&memories.digests)?;
        let owed = Compaction {
            model: compact.model().clone(),
        };
        let budget = self.budget;
        let hold_at = |reading: &Reading, at: Datetime| -> Result<Option<Hold>, Refusal> {
            let spent: Vec<_> = reading.digest.attempts().collect();
            Ok(reading
                .next_call(&owed)?
                .filter(|next| budget.admits(*next))
                .and_then(|next| budget.hold(&spent, next, instant_of(at))))
        };
        if !owed.holds(&reading) {
            return Ok(Step::Idle);
        }
        if let Some(hold) = hold_at(&reading, now)? {
            return Ok(Step::Held(hold));
        }
        if self.engine.due(&TheDigest, &reading).is_none() {
            return Ok(Step::Idle);
        }
        match perform(memories, compact, &reading, self.budget, clock) {
            Ok(done) => Ok(done.map_or(Step::Idle, Step::Compacted)),
            // A cap reached partway through an attempt is a hold, not a failure.
            Err(refused) => match hold_at(&Reading::of(&memories.digests)?, clock.now())? {
                Some(hold) => Ok(Step::Held(hold)),
                None => Err(refused),
            },
        }
    }
}

/// Summaries owed that the compactor is not writing now, and why: what the briefing says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    /// How many nodes are owed one now.
    pub nodes: usize,
    /// What keeps them waiting, if anything does.
    pub why: Option<Why>,
    /// How many waiting nodes the compactor's model found no line for; every node above one
    /// waits for good, until another model or a larger limit is owed it.
    pub unfit: usize,
}

/// What keeps owed summaries waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// The kill switch.
    Off,
    /// A cap, until its instant.
    Held(Hold),
}

impl std::fmt::Display for Waiting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.why {
            Some(Why::Off) => write!(
                f,
                "the compactor is off, and {} summaries wait for it",
                self.nodes
            )?,
            Some(Why::Held(hold)) => {
                write!(f, "{hold}, and {} summaries wait for it", self.nodes)?;
            }
            None => write!(f, "{} summaries wait", self.nodes)?,
        }
        if self.unfit > 0 {
            write!(
                f,
                "; {} found no line of {NODE_BYTES} bytes, so nothing above them is summarised \
                 until another model or a larger limit is tried",
                self.unfit
            )?;
        }
        Ok(())
    }
}

/// What holds the compactor back at `now`, as `digests` reads: a hold or the switch, and the
/// nodes its model found unfit; `None` when nothing waits on either.
///
/// # Errors
/// [`Reading::of`]'s.
pub fn waiting(digests: &Digests<'_>, now: Datetime) -> Result<Option<Waiting>, Refusal> {
    let reading = Reading::of(digests)?;
    let pending = reading.digest.pending().len();
    let (owed, why) = match digests.compacting {
        Compacting::Off => (pending, Some(Why::Off)),
        Compacting::On { model, budget } => {
            let spent: Vec<_> = reading.digest.attempts().collect();
            let rule = Compaction {
                model: model.clone(),
            };
            let held = reading
                .next_call(&rule)?
                .filter(|next| budget.admits(*next))
                .and_then(|next| budget.hold(&spent, next, instant_of(now)));
            (rule.owed(&reading.digest).len(), held.map(Why::Held))
        }
    };
    let unfit = match digests.compacting {
        Compacting::Off => 0,
        Compacting::On { .. } => pending - owed,
    };
    let waits = (why.is_some() && owed > 0) || unfit > 0;
    Ok(waits.then_some(Waiting {
        nodes: owed,
        why: why.filter(|_| owed > 0),
        unfit,
    }))
}
