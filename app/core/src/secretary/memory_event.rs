//! The memory's schema: a fact remembered, naming its words by their [`TextName`], and a fact
//! forgotten, naming only its id, each about one [`FactId`].
//!
//! Every event writes the fact's one register, its life ([`LifeRegister`]), declared
//! inflationary: the store refuses a
//! `Remembered` on a fact already forgotten, so once forgotten, always forgotten. The words are
//! never an object: they live in the memory's text store, which forgetting deletes from.

use prodrome_core::event::Actor;
use prodrome_core::fold::{Discrete, Frontier, RegisterType};
use prodrome_core::literal::{Datetime, ProdromeError, Value};
use prodrome_core::registers::Stamp;

use crate::envelope::{Event, Says, Vocab};
use crate::literal::{Fields, Literal as _, Signature};
use crate::sole::{One, Only, Sole};

use crate::secretary::fact_life::Life;
use crate::secretary::memory::{FactId, Kind, Origin, TAPPED, TURN};
use crate::secretary::salted_text::TextName;

/// One event about one fact: which, when, who wrote it, and what became of it.
pub type MemoryEvent = Event<Said>;

/// What an event says of its fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// It was remembered: its words by name, about whom, and who said it.
    Remembered {
        /// The name of its words in the text store.
        text: TextName,
        /// About whom or what.
        kind: Kind,
        /// Who said it.
        origin: Origin,
    },
    /// It was forgotten; `version` is the memory's version this made.
    Forgotten {
        /// One more than every id and version before it.
        version: u64,
    },
}

/// An event that cannot be.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotAMemoryEvent {
    /// A forgetting numbered at or below the fact it forgets, which was remembered first.
    #[error("fact {fact} cannot be forgotten at version {version}, at or below its own number")]
    Before {
        /// The fact.
        fact: FactId,
        /// The version.
        version: u64,
    },
}

impl MemoryEvent {
    /// Fact `fact` remembered by `actor`: its words by `text`, its `kind` and who said it.
    #[must_use]
    pub fn remembered(
        fact: FactId,
        at: Datetime,
        actor: Actor,
        text: TextName,
        kind: Kind,
        origin: Origin,
    ) -> Self {
        Self::of(fact, at, actor, Said::Remembered { text, kind, origin })
    }

    /// Fact `fact` forgotten by `actor`, making memory version `version`.
    ///
    /// # Errors
    /// [`NotAMemoryEvent::Before`] for a version at or below the fact's own number.
    pub fn forgotten(
        fact: FactId,
        at: Datetime,
        actor: Actor,
        version: u64,
    ) -> Result<Self, NotAMemoryEvent> {
        if version <= fact.0 {
            return Err(NotAMemoryEvent::Before { fact, version });
        }
        Ok(Self::of(fact, at, actor, Said::Forgotten { version }))
    }

    /// The memory's version this event made: a remembered fact's number, or a forgetting's.
    #[must_use]
    pub fn version(&self) -> u64 {
        match *self.said() {
            Said::Remembered { .. } => self.key().0,
            Said::Forgotten { version } => version,
        }
    }
}

/// `Remembered(fact, at, actor, text, kind, said)`.
const REMEMBERED: Signature = (
    "Remembered",
    &["fact", "at", "actor", "text", "kind", "said"],
);
/// `Forgotten(fact, at, actor, version)`.
const FORGOTTEN: Signature = ("Forgotten", &["fact", "at", "actor", "version"]);
/// The memory's whitelist: its two events and an origin's two arms, nothing else, since no
/// event nests a term.
pub type MemoryVocabulary = Vocab<Said>;

impl Says for Said {
    type Key = FactId;
    type Register = Only;
    type Registers<'a> = One<'a, MemoryEvent>;

    const EVENTS: &'static [Signature] = &[REMEMBERED, FORGOTTEN];
    const NESTED: &'static [Signature] = &[TURN, TAPPED];

    fn registers() -> Vec<Only> {
        vec![Only]
    }

    fn writes(_: &MemoryEvent) -> impl Iterator<Item = Only> {
        std::iter::once(Only)
    }

    fn body(&self) -> (Signature, Vec<Value>) {
        match self {
            Said::Remembered { text, kind, origin } => (
                REMEMBERED,
                vec![text.to_value(), kind.to_value(), origin.to_value()],
            ),
            Said::Forgotten { version } => (FORGOTTEN, vec![version.to_value()]),
        }
    }

    fn parse(fields: &Fields<'_>) -> Result<MemoryEvent, ProdromeError> {
        let (fact, at, actor) = (fields.get("fact")?, fields.get("at")?, fields.get("actor")?);
        match fields.name() {
            "Remembered" => Ok(MemoryEvent::remembered(
                fact,
                at,
                actor,
                fields.get("text")?,
                fields.get("kind")?,
                fields.get("said")?,
            )),
            "Forgotten" => MemoryEvent::forgotten(fact, at, actor, fields.get("version")?)
                .map_err(|failed| ProdromeError::invalid(failed.to_string())),
            other => Err(crate::literal::no_arm("a memory event", other)),
        }
    }
}

/// The register's type: a [`Life`].
#[derive(Debug)]
pub struct LifeRegister;

impl RegisterType<MemoryEvent> for LifeRegister {
    type Value<'e> = Life<'e>;

    fn value(event: &MemoryEvent) -> Option<Life<'_>> {
        Some(match event.said() {
            Said::Remembered { .. } => Life::Remembered(Discrete(event)),
            Said::Forgotten { .. } => Life::Forgotten,
        })
    }
}

impl Sole for MemoryEvent {
    fn reading<'a>(frontier: &Frontier<'a, Self>) -> Vec<&'a Stamp<Self>> {
        frontier.reading::<LifeRegister>()
    }

    fn grows(frontier: &Frontier<'_, Self>, event: &Self) -> Result<(), ProdromeError> {
        frontier.grows::<LifeRegister>(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_constructor_is_a_term_or_an_envelope() {
        crate::literal::assert_own(&[REMEMBERED, FORGOTTEN, TURN, TAPPED]);
    }

    /// Every count prints as itself and reads back, at the widest it holds.
    #[test]
    fn every_event_round_trips_at_its_widest() {
        use prodrome_core::event::{canonical, parse_event};
        let at = Datetime::new(2026, 10, 11, 9, 0, 0, 0).expect("an instant");
        let ada = Actor::new("ada").expect("an actor");
        let fact = FactId(u64::MAX - 1);
        for event in [
            MemoryEvent::remembered(
                fact,
                at,
                ada.clone(),
                TextName::new("a".repeat(64)).expect("a name"),
                Kind::Owner,
                Origin::Turn {
                    conversation: crate::secretary::store::ConversationId::new("default")
                        .expect("an id"),
                    turn: usize::MAX,
                },
            ),
            MemoryEvent::forgotten(fact, at, ada, u64::MAX).expect("after its fact"),
        ] {
            let print = canonical(&event);
            assert_eq!(
                parse_event(&MemoryVocabulary::default(), &print),
                Ok(event),
                "{print}"
            );
        }
    }

    #[test]
    fn a_forgetting_comes_after_its_fact() {
        let at = Datetime::new(2026, 10, 2, 9, 0, 0, 0).expect("an instant");
        let ada = Actor::new("ada").expect("an actor");
        assert!(MemoryEvent::forgotten(FactId(3), at, ada.clone(), 3).is_err());
        assert!(MemoryEvent::forgotten(FactId(3), at, ada, 4).is_ok());
    }
}
