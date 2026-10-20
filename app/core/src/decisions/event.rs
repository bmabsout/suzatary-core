use std::collections::BTreeSet;

use prodrome_core::event::Actor;
use prodrome_core::literal::{Datetime, ProdromeError, Table, Value, Vocabulary};
use prodrome_core::schema::Schema;

use super::registers::{DecisionRegister, DecisionRegisters};
use super::{DecisionId, NotADecision, Statement};
use crate::literal::{Fields, Literal, Signature, record};
use crate::record::MarkupSource;

/// One event about one decision: which, when its writer recorded it (data, never an order), who,
/// and what was said of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionEvent {
    decision: DecisionId,
    at: Datetime,
    actor: Actor,
    said: Said,
}

/// What an event says of its decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// It was decided: its statement, and the decisions it is the newer word on.
    Decided {
        /// What it says.
        statement: Statement,
        /// What it supersedes.
        supersedes: BTreeSet<DecisionId>,
    },
    /// It was restated, superseding its earlier statement; its links join the earlier ones.
    Revised {
        /// What it says now.
        statement: Statement,
        /// What it supersedes, beside what it already did.
        supersedes: BTreeSet<DecisionId>,
    },
    /// It no longer holds, and nothing replaces it.
    Withdrawn {
        /// Why.
        reason: String,
    },
}

impl DecisionEvent {
    /// Decision `decision` decided by `actor`.
    ///
    /// # Errors
    /// [`NotADecision::Itself`] where it names itself among what it supersedes.
    pub fn decided(
        decision: DecisionId,
        at: Datetime,
        actor: Actor,
        statement: Statement,
        supersedes: BTreeSet<DecisionId>,
    ) -> Result<Self, NotADecision> {
        Self::stated(decision, at, actor, supersedes, |supersedes| {
            Said::Decided {
                statement,
                supersedes,
            }
        })
    }

    /// Decision `decision` restated by `actor`.
    ///
    /// # Errors
    /// [`NotADecision::Itself`] where it names itself among what it supersedes.
    pub fn revised(
        decision: DecisionId,
        at: Datetime,
        actor: Actor,
        statement: Statement,
        supersedes: BTreeSet<DecisionId>,
    ) -> Result<Self, NotADecision> {
        Self::stated(decision, at, actor, supersedes, |supersedes| {
            Said::Revised {
                statement,
                supersedes,
            }
        })
    }

    /// Decision `decision` withdrawn by `actor`, for `reason`.
    ///
    /// # Errors
    /// [`NotADecision::Empty`] for an empty or untrimmed reason.
    pub fn withdrawn(
        decision: DecisionId,
        at: Datetime,
        actor: Actor,
        reason: &str,
    ) -> Result<Self, NotADecision> {
        if reason.is_empty() || reason != reason.trim() {
            return Err(NotADecision::Empty("reason"));
        }
        Ok(Self {
            decision,
            at,
            actor,
            said: Said::Withdrawn {
                reason: reason.to_owned(),
            },
        })
    }

    fn stated(
        decision: DecisionId,
        at: Datetime,
        actor: Actor,
        supersedes: BTreeSet<DecisionId>,
        said: impl FnOnce(BTreeSet<DecisionId>) -> Said,
    ) -> Result<Self, NotADecision> {
        if supersedes.contains(&decision) {
            return Err(NotADecision::Itself(decision.to_string()));
        }
        Ok(Self {
            decision,
            at,
            actor,
            said: said(supersedes),
        })
    }

    /// The decision.
    #[must_use]
    pub fn decision(&self) -> &DecisionId {
        &self.decision
    }

    /// What was said of it.
    #[must_use]
    pub fn said(&self) -> &Said {
        &self.said
    }

    /// The statement a `Decided` or `Revised` makes.
    #[must_use]
    pub fn statement(&self) -> Option<&Statement> {
        match &self.said {
            Said::Decided { statement, .. } | Said::Revised { statement, .. } => Some(statement),
            Said::Withdrawn { .. } => None,
        }
    }

    /// The links a `Decided` or `Revised` makes; none for a withdrawal.
    #[must_use]
    pub fn supersedes(&self) -> Option<&BTreeSet<DecisionId>> {
        match &self.said {
            Said::Decided { supersedes, .. } | Said::Revised { supersedes, .. } => Some(supersedes),
            Said::Withdrawn { .. } => None,
        }
    }
}

/// `Decided(decision, at, actor, title, body, rationale, supersedes)`.
const DECIDED: Signature = (
    "Decided",
    &[
        "decision",
        "at",
        "actor",
        "title",
        "body",
        "rationale",
        "supersedes",
    ],
);
/// `Revised(decision, at, actor, title, body, rationale, supersedes)`.
const REVISED: Signature = (
    "Revised",
    &[
        "decision",
        "at",
        "actor",
        "title",
        "body",
        "rationale",
        "supersedes",
    ],
);
/// `Withdrawn(decision, at, actor, reason)`.
const WITHDRAWN: Signature = ("Withdrawn", &["decision", "at", "actor", "reason"]);
/// The constructors, in print order.
const SIGNATURES: &[Signature] = &[DECIDED, REVISED, WITHDRAWN];

/// The decisions' whitelist: three constructors and nothing else, since no event nests a term.
#[derive(Debug, Clone, Default)]
pub struct DecisionVocabulary;

impl Vocabulary for DecisionVocabulary {
    fn signature(&self, name: &str) -> Option<prodrome_core::literal::Signature<'_>> {
        Table(SIGNATURES).find(name)
    }
}

/// A decision's literal is its id.
impl Literal for DecisionId {
    fn to_value(&self) -> Value {
        Value::str(self.as_str())
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        DecisionId::new(String::from_value(value)?).map_err(invalid)
    }
}

fn invalid(failed: impl std::fmt::Display) -> ProdromeError {
    ProdromeError::invalid(failed.to_string())
}

/// A statement's fields and its links, as `Decided` and `Revised` print them.
fn stated(statement: &Statement, supersedes: &BTreeSet<DecisionId>) -> Vec<Value> {
    vec![
        statement.title().to_owned().to_value(),
        statement.body().as_str().to_owned().to_value(),
        statement.rationale().to_owned().to_value(),
        supersedes.to_value(),
    ]
}

impl Schema for DecisionEvent {
    type Vocabulary = DecisionVocabulary;
    type Key = DecisionId;
    type Register = DecisionRegister;

    fn registers(_: &DecisionVocabulary) -> Vec<DecisionRegister> {
        vec![DecisionRegister::Content, DecisionRegister::Life]
    }

    type Registers<'a> = DecisionRegisters<'a>;

    fn to_value(&self) -> Value {
        let head = [
            self.decision.to_value(),
            self.at.to_value(),
            self.actor.to_value(),
        ];
        let (signature, own) = match &self.said {
            Said::Decided {
                statement,
                supersedes,
            } => (DECIDED, stated(statement, supersedes)),
            Said::Revised {
                statement,
                supersedes,
            } => (REVISED, stated(statement, supersedes)),
            Said::Withdrawn { reason } => (WITHDRAWN, vec![reason.to_value()]),
        };
        record(signature, head.into_iter().chain(own).collect())
    }

    fn from_value(_: &DecisionVocabulary, value: &Value) -> Result<Self, ProdromeError> {
        let fields = Fields::among(value, SIGNATURES)?;
        let (decision, at, actor) = (
            fields.get("decision")?,
            fields.get("at")?,
            fields.get("actor")?,
        );
        let statement = || -> Result<Statement, ProdromeError> {
            Statement::new(
                &fields.get::<String>("title")?,
                MarkupSource::from_chain(fields.get::<String>("body")?),
                &fields.get::<String>("rationale")?,
            )
            .map_err(invalid)
        };
        match fields.name() {
            "Decided" => {
                DecisionEvent::decided(decision, at, actor, statement()?, fields.get("supersedes")?)
            }
            "Revised" => {
                DecisionEvent::revised(decision, at, actor, statement()?, fields.get("supersedes")?)
            }
            "Withdrawn" => {
                DecisionEvent::withdrawn(decision, at, actor, &fields.get::<String>("reason")?)
            }
            other => return Err(crate::literal::no_arm("a decision event", other)),
        }
        .map_err(invalid)
    }

    fn key(&self) -> &DecisionId {
        &self.decision
    }

    fn at(&self) -> Datetime {
        self.at
    }

    fn actor(&self) -> &Actor {
        &self.actor
    }

    /// A statement supersedes in both registers, so a restatement of a withdrawn decision is
    /// refused by the life's order; a withdrawal only in the life. The links accumulate and
    /// supersede nothing.
    fn writes(&self) -> impl Iterator<Item = DecisionRegister> {
        let statement = matches!(self.said, Said::Decided { .. } | Said::Revised { .. })
            .then_some(DecisionRegister::Content);
        statement.into_iter().chain([DecisionRegister::Life])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_constructor_is_a_term_or_an_envelope() {
        crate::literal::assert_own(SIGNATURES);
    }

    /// A set of links has one spelling: sorted, each once; any other print is not this event's.
    #[test]
    fn links_read_back_only_sorted_and_once() {
        use prodrome_core::event::{canonical, parse_event};
        let at = Datetime::new(2026, 10, 3, 9, 0, 0, 0).expect("an instant");
        let statement =
            Statement::new("Plugins", MarkupSource::escaped("body"), "why").expect("a statement");
        let id = |text: &str| DecisionId::new(text).expect("an id");
        let event = DecisionEvent::decided(
            id("plugins"),
            at,
            Actor::new("ada").expect("an actor"),
            statement,
            BTreeSet::from([id("aaa"), id("bbb")]),
        )
        .expect("an event");
        let print = canonical(&event);
        assert_eq!(parse_event(&DecisionVocabulary, &print), Ok(event));
        for other in ["('bbb', 'aaa')", "('aaa', 'aaa', 'bbb')"] {
            let moved = print.replace("('aaa', 'bbb')", other);
            assert_ne!(moved, print);
            assert!(parse_event::<DecisionEvent>(&DecisionVocabulary, &moved).is_err());
        }
    }

    #[test]
    fn a_decision_cannot_supersede_itself() {
        let at = Datetime::new(2026, 10, 3, 9, 0, 0, 0).expect("an instant");
        let id = DecisionId::new("plugins").expect("an id");
        let statement =
            Statement::new("Plugins", MarkupSource::escaped("body"), "why").expect("a statement");
        let refused = DecisionEvent::decided(
            id.clone(),
            at,
            Actor::new("ada").expect("an actor"),
            statement,
            BTreeSet::from([id]),
        );
        assert!(matches!(refused, Err(NotADecision::Itself(_))));
    }
}
