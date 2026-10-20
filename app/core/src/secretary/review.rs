//! The secretary reviews the agent's batches, applying `SECRETARY.md`'s rule by the model.
//!
//! A hold is derived from what is still standing when the round ends, and a failed review leaves
//! every batch standing.

use super::Bound;
use super::daily::{Daily, daily_round};
use super::found::{Found, Unreadable};
use super::store::{ConversationId, Decision, Entry, Review, Stored, stamp};
use super::surface::SecretaryVerb;
use super::verbs::{answered, listed};
use crate::authoring::NewDecision;
use crate::capability::{Secretary, Turn};
use crate::chains::ChainName;
use crate::deployment::Deployment;
use crate::reference::{BatchHead, Reference};
use crate::refusal::Refusal;
use crate::secretary::transcript;
use crate::wire::ProposalsPayload;

/// What one review did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reviewed {
    /// The conversation it happened in, as it now stands.
    pub stored: Stored,
    /// How many times the model was asked; zero when nothing was standing.
    pub rounds: usize,
    /// The lines this review wrote, in order: accepted, rejected and held with the reason.
    pub decided: Vec<Review>,
    /// How many batches are still standing when it ends.
    pub standing: usize,
}

impl Reviewed {
    /// The lines with one decision, for a caller printing three lists.
    #[must_use]
    pub fn with(&self, decision: Decision) -> Vec<&Review> {
        self.decided
            .iter()
            .filter(|review| review.decision == decision)
            .collect()
    }
}

/// The question the review asks, as a person turn; the rule itself stays in `SECRETARY.md`.
const ASK: &str = "Review the triage agent's standing batches, below. Settle the ordinary ones \
                   with proposals_accept and proposals_reject — naming the chain, the object \
                   hashes and WHY — and leave standing anything that changes a real commitment \
                   of mine, per \"Reviewing the agent's batches\" in your brief. Then tell me, \
                   in a few sentences, what you accepted, what you rejected and what you are \
                   holding for me and why. When in doubt, hold.";

/// A hold's reason when the model said nothing about the rest.
const UNSPOKEN: &str = "The secretary left this standing and did not say why.";

/// Review every standing batch, on every chain, in one round, then write a `held` line for what
/// still stands.
///
/// The seed is `proposals_list`'s own answer as a `Turn::Tool`, and spends no rounds.
///
/// # Errors
/// The stores' own failures, the model's, and an unreadable conversation.
pub fn review(
    caps: &Secretary<'_>,
    deployment: &Deployment,
    settables: &crate::settings::Settables,
    chains: &[Bound<'_>],
    system: &str,
) -> Result<Reviewed, Refusal> {
    let round = daily_round(caps, deployment.zone, Daily::Review)?;
    let id = round.id.clone();
    let standing = listed(chains, deployment.zone)?;
    if heads(&standing).is_empty() {
        // Nothing standing opens no conversation and spawns no model.
        return Ok(Reviewed {
            stored: round.idle(caps),
            rounds: 0,
            decided: Vec::new(),
            standing: 0,
        });
    }
    let already = round.open(caps)?.reviews.len();

    transcript::push(
        caps.conversations,
        &id,
        &Entry::Said {
            at: stamp(caps.clock.now()),
            turn: Turn::Person {
                text: ASK.to_owned(),
                references: Vec::new(),
                attachments: Vec::new(),
            },
            by: None,
        },
    )?;
    transcript::push(
        caps.conversations,
        &id,
        &Entry::Said {
            at: stamp(caps.clock.now()),
            turn: Turn::Tool {
                verb: SecretaryVerb::ProposalsList.into(),
                answer: serde_json::to_string_pretty(&answered(&standing))
                    .unwrap_or_else(|failed| format!("(the batches did not serialise: {failed})")),
                refused: false,
                files: Vec::new(),
            },
            by: None,
        },
    )
    .map_err(|failed| Refusal::internal(failed.to_string()))?;

    // A review that could not run is written down, so an unsettled Inbox is distinguishable from a
    // settled one.
    let halt = crate::capability::Halt::new();
    let quiet = crate::conversation::round::Sink::quiet(&halt);
    let answered = match super::answer(
        caps,
        || crate::capability::Pick::Turn(Box::new(caps.model)),
        deployment,
        settables,
        chains,
        &id,
        system,
        &quiet,
    )
    .and_then(|answered| {
        answered
            .as_ref()
            .map_or(Ok(()), |answered| answered.ending.ran())
            .map(|()| answered)
    }) {
        Ok(answered) => answered,
        Err(refusal) => {
            // Best effort: the refusal below is the failure the caller prints.
            let _ = transcript::push(
                caps.conversations,
                &id,
                &Entry::ReviewFailed {
                    at: stamp(caps.clock.now()),
                    reason: crate::secretary::words::Words::kept(refusal.message().to_owned()),
                    standing: heads(&standing).len(),
                },
            );
            return Err(refusal);
        }
    };
    // No round ran only when nothing waited, so every batch is held.
    let (after, rounds) = match answered {
        Some(answered) => (answered.stored, answered.rounds),
        None => (super::read(caps.conversations, &id)?, 0),
    };
    let standing = hold_the_rest(caps, deployment, chains, &id, &after)?;

    let stored = super::read(caps.conversations, &id)?;
    Ok(Reviewed {
        decided: stored.reviews[already.min(stored.reviews.len())..].to_vec(),
        rounds,
        stored,
        standing,
    })
}

/// Name in today's review every conversation the store holds that does not read, once a day for
/// each set of them: the push to page with when this pass is the first TODAY to see that set,
/// else none. The note is kept in the day's round, so a set that stays unreadable is named and
/// paged again each day, as a finding still standing should be.
///
/// # Errors
/// The store's own.
pub fn note_unreadable(
    caps: &Secretary<'_>,
    zone: crate::zone::Zone,
) -> Result<Option<crate::capability::Push>, Refusal> {
    let unreadable = every(caps.conversations)?.unreadable;
    let Some(count) = std::num::NonZeroUsize::new(unreadable.len()) else {
        return Ok(None);
    };
    let round = daily_round(caps, zone, Daily::Review)?;
    let id = round.id.clone();
    let stored = round.open(caps)?;
    let text = format!(
        "{} the box cannot read, so their reviews are left out of the rest: {}",
        if count.get() == 1 {
            "One conversation".to_owned()
        } else {
            format!("{count} conversations")
        },
        unreadable
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    );
    let said = stored
        .turns
        .iter()
        .any(|said| matches!(&said.turn, Turn::Act { text: noted, .. } if *noted == text));
    if said {
        return Ok(None);
    }
    transcript::push(
        caps.conversations,
        &id,
        &Entry::noted(stamp(caps.clock.now()), text, Vec::new()),
    )?;
    Ok(Some(crate::capability::Push::Unreadable {
        conversations: count,
        round: id,
    }))
}

/// Write down what is still standing, one line per batch per day, with the secretary's own words
/// as the reason.
fn hold_the_rest(
    caps: &Secretary<'_>,
    deployment: &Deployment,
    chains: &[Bound<'_>],
    id: &ConversationId,
    answered: &Stored,
) -> Result<usize, Refusal> {
    let why = said(answered);
    let left = listed(chains, deployment.zone)?;
    let known: std::collections::BTreeSet<String> = answered
        .reviews
        .iter()
        .filter(|review| review.decision == Decision::Held)
        .map(|review| review.batch.clone())
        .collect();
    let at = stamp(caps.clock.now());
    let held = heads(&left);
    for (chain, head) in &held {
        // Authoring is idempotent, and a batch held without a decision is invisible in the Inbox.
        author_batch_decision(
            chains,
            chain,
            head,
            &why,
            crate::zone::Moment {
                at: caps.clock.now(),
                zone: deployment.zone,
            },
        )?;
        if known.contains(head) {
            continue;
        }
        transcript::push(
            caps.conversations,
            id,
            &Entry::Reviewed {
                at: at.clone(),
                chain: chain.clone(),
                batch: head.clone(),
                decision: Decision::Held,
                // A hold is a decision about the whole batch, so it names no object.
                objects: Vec::new(),
                why: crate::secretary::words::Words::kept(why.clone()),
            },
        )?;
    }
    Ok(held.len())
}

/// Author the decision a held batch is, once per `batch:<chain>/<head>`, at the default priority.
fn author_batch_decision(
    chains: &[Bound<'_>],
    chain: &str,
    head: &str,
    why: &str,
    now: crate::zone::Moment,
) -> Result<(), Refusal> {
    let Some(bound) = super::bound_named(chains, chain) else {
        // Unreachable, since `heads` read these chains a moment ago.
        return Ok(());
    };
    let chain_name =
        ChainName::new(chain).map_err(|failed| Refusal::internal(failed.to_string()))?;
    let batch_head =
        BatchHead::new(head).map_err(|failed| Refusal::internal(failed.to_string()))?;
    let performs = Reference::Batch {
        chain: chain_name,
        head: batch_head,
    };
    let nodes = bound.caps.chain.nodes()?;
    let events = crate::view::events_of(&nodes);
    if crate::authoring::decision_for(&events, &performs).is_some() {
        return Ok(());
    }
    let known: std::collections::BTreeSet<String> = events
        .iter()
        .map(|event| event.todo().as_str().to_owned())
        .collect();
    let new = NewDecision {
        body: format!("Settle the triage batch {head} on {chain}"),
        why: why.to_owned(),
        performs,
        detail: why.to_owned(),
        priority: None,
        category: String::new(),
        source: String::new(),
    };
    let event = crate::authoring::build_decision(&new, now, super::WRITES_AS.actor(), &[], &known)?;
    bound.caps.chain.append(event)?;
    Ok(())
}

/// The last thing the model said, which is what a hold's reason is.
///
/// Read as prose, because a decision's rationale is stored escaped and markup would show as
/// source.
fn said(stored: &Stored) -> String {
    stored
        .turns
        .iter()
        .rev()
        .find_map(|said| match &said.turn {
            Turn::Model { text, .. } => {
                Some(super::answer::prose(text)).filter(|words| !words.is_empty())
            }
            _ => None,
        })
        .unwrap_or_else(|| UNSPOKEN.to_owned())
}

/// Every standing batch, by chain and head.
fn heads(listed: &[ProposalsPayload]) -> Vec<(String, String)> {
    listed
        .iter()
        .flat_map(|payload| {
            payload
                .batches
                .iter()
                .map(|batch| (payload.chain.clone(), batch.head.clone()))
        })
        .collect()
}

/// Every conversation: each that reads as it stands, and each that does not, named.
///
/// # Errors
/// The store's own failure to list them.
pub fn every(
    conversations: crate::secretary::conversations::Conversations<'_>,
) -> Result<Found<Stored>, Refusal> {
    let mut found = Found {
        items: Vec::new(),
        unreadable: Vec::new(),
    };
    for (id, read) in transcript::each(conversations)? {
        match read {
            Ok(read) => found.items.push(read.into_stored()),
            Err(failed) => found.unreadable.push(Unreadable {
                id,
                why: failed.to_string(),
            }),
        }
    }
    Ok(found)
}

/// Every review line this box holds since a day, oldest first, from every conversation that
/// reads, and every one that does not.
///
/// # Errors
/// The store's own failure to list them.
pub fn reviews(
    conversations: crate::secretary::conversations::Conversations<'_>,
    since: chrono::NaiveDate,
) -> Result<Found<(ConversationId, Review)>, Refusal> {
    let mut found = every(conversations)?.flat_map(|stored| {
        stored
            .reviews
            .iter()
            .filter(|review| on_or_after(&review.at, since))
            .map(|review| (stored.id.clone(), review.clone()))
            .collect::<Vec<_>>()
    });
    found
        .items
        .sort_by(|left, right| left.1.at.cmp(&right.1.at));
    Ok(found)
}

/// Every review that could not run since a day, oldest first, kept apart from [`reviews`] because
/// these are not decisions.
///
/// # Errors
/// As [`reviews`].
pub fn failures(
    conversations: crate::secretary::conversations::Conversations<'_>,
    since: chrono::NaiveDate,
) -> Result<Found<(ConversationId, crate::secretary::store::ReviewFailure)>, Refusal> {
    let mut found = every(conversations)?.flat_map(|stored| {
        stored
            .failures
            .iter()
            .filter(|failure| on_or_after(&failure.at, since))
            .map(|failure| (stored.id.clone(), failure.clone()))
            .collect::<Vec<_>>()
    });
    found
        .items
        .sort_by(|left, right| left.1.at.cmp(&right.1.at));
    Ok(found)
}

/// How long the review has been down today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Outage {
    /// Failed passes since the last review that ran; zero when it is up.
    pub consecutive: usize,
    /// Batches standing at the latest failure.
    pub standing: usize,
}

impl Outage {
    /// The push to send exactly when this outage reaches `after` passes, so one outage pages once.
    #[must_use]
    pub fn page(self, after: usize) -> Option<crate::capability::Push> {
        let passes = std::num::NonZeroUsize::new(after)?;
        (self.consecutive == after).then_some(crate::capability::Push::ReviewDown {
            standing: self.standing,
            passes,
        })
    }
}

/// Today's [`Outage`], read from the conversations that read.
///
/// # Errors
/// As [`reviews`]: a store that does not list says so, and is never an outage of none.
pub fn outage(
    conversations: crate::secretary::conversations::Conversations<'_>,
    today: chrono::NaiveDate,
) -> Result<Outage, Refusal> {
    let failed = failures(conversations, today)?.items;
    let ran = reviews(conversations, today)?.items;
    let last_ran = ran.last().map(|(_, review)| review.at.clone());
    let since: Vec<_> = failed
        .iter()
        .filter(|(_, failure)| last_ran.as_ref().is_none_or(|at| &failure.at > at))
        .collect();
    Ok(Outage {
        consecutive: since.len(),
        standing: since.last().map_or(0, |(_, failure)| failure.standing),
    })
}

/// Is this stored instant on or after that day? Parsed rather than sliced, to survive a hand edit.
fn on_or_after(at: &str, since: chrono::NaiveDate) -> bool {
    at.get(..10)
        .and_then(|day| chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .is_some_and(|day| day >= since)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secretary::store::Kind;

    /// A conversation that does not read is a finding that names it, and costs only itself:
    /// every other conversation's reviews, failures and outage read as they did without it.
    #[test]
    fn an_unreadable_conversation_is_named_and_costs_only_itself() {
        let store = crate::secretary::conversations::InMemory::default();
        let conversations = store.conversations();
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 11).expect("a day");
        let at = stamp(
            prodrome_core::literal::Datetime::new(2026, 10, 11, 9, 0, 0, 0).expect("an instant"),
        );
        let open = |id: &ConversationId| {
            transcript::start(
                conversations,
                id,
                &Entry::Opened {
                    at: at.clone(),
                    title: crate::secretary::words::Words::kept("Review".to_owned()),
                    model: "m".to_owned(),
                    kind: Kind::Review,
                },
            )
            .expect("opened");
        };
        let good = ConversationId::daily(Daily::Review, day);
        open(&good);
        transcript::push(
            conversations,
            &good,
            &Entry::Reviewed {
                at: at.clone(),
                chain: "life".to_owned(),
                batch: "a".repeat(64),
                decision: Decision::Held,
                objects: Vec::new(),
                why: crate::secretary::words::Words::kept("it changes a commitment".to_owned()),
            },
        )
        .expect("written");
        let before = (
            reviews(conversations, day).expect("lists"),
            failures(conversations, day).expect("lists"),
            outage(conversations, day).expect("lists"),
        );
        assert_eq!(before.0.items.len(), 1);
        assert!(before.0.unreadable.is_empty());
        // A rewind to a turn never said does not fold.
        let bad = ConversationId::new("2026-10-11-1").expect("a slug");
        open(&bad);
        transcript::push(
            conversations,
            &bad,
            &Entry::Rewound {
                to: 9,
                at,
                by: "ada".to_owned(),
            },
        )
        .expect("written");
        let after = reviews(conversations, day).expect("lists");
        assert_eq!(after.items, before.0.items, "the rest reads as it did");
        assert_eq!(
            after
                .unreadable
                .iter()
                .map(|one| &one.id)
                .collect::<Vec<_>>(),
            vec![&bad]
        );
        let failed = failures(conversations, day).expect("lists");
        assert_eq!(failed.items, before.1.items);
        assert_eq!(failed.unreadable, after.unreadable);
        assert_eq!(outage(conversations, day).expect("lists"), before.2);
    }

    /// Each daily round's id is a slug, one a day, and two rounds never share one.
    #[test]
    fn a_daily_round_is_named_by_its_day_and_kind() {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 11).expect("a day");
        for daily in [Daily::Review, Daily::Sweep] {
            let id = ConversationId::daily(daily, day);
            assert_eq!(ConversationId::new(id.as_str()), Ok(id.clone()));
            assert_eq!(id.as_str(), format!("2026-10-11-{}", daily.word()));
        }
        assert_ne!(
            ConversationId::daily(Daily::Review, day),
            ConversationId::daily(Daily::Sweep, day)
        );
    }

    #[test]
    fn an_outage_pages_once_exactly_at_the_threshold() {
        let at = |consecutive| Outage {
            consecutive,
            standing: 2,
        };
        assert_eq!(at(11).page(12), None);
        assert_eq!(
            at(12).page(12),
            Some(crate::capability::Push::ReviewDown {
                standing: 2,
                passes: std::num::NonZeroUsize::new(12).expect("some"),
            }),
            "the twelfth failure pages"
        );
        assert_eq!(at(13).page(12), None, "and only once per outage");
        assert_eq!(at(0).page(0), None, "a zero threshold never pages");
    }
}
