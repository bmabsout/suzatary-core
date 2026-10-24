//! The alias store's schema. A rename writes aliases and no membership: memberships stay under
//! the names they were written with, and the reading follows each name's alias.

use prodrome_core::event::Actor;
use prodrome_core::fold::{Discrete, Frontier, RegisterType};
use prodrome_core::literal::{Datetime, ProdromeError, Value};
use prodrome_core::registers::Stamp;

use crate::envelope::{Event, Says, Vocab};
use crate::literal::{Fields, Literal as _, Signature};
use crate::secretary::topics::topic::Topic;
use crate::sole::{One, Only, Sole};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Alias {
    Itself,
    /// Never the topic itself: [`AliasEvent::aliased`] refuses that.
    As(Topic),
}

/// A topic's alias event.
pub type AliasEvent = Event<Alias>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0} cannot be an alias of itself; it is a name of its own")]
pub struct OfItself(pub Topic);

impl AliasEvent {
    /// Refuses `to == topic`, so "its own name" has one spelling, [`AliasEvent::unaliased`].
    pub fn aliased(topic: Topic, to: Topic, at: Datetime, actor: Actor) -> Result<Self, OfItself> {
        if topic == to {
            return Err(OfItself(topic));
        }
        Ok(Self::of(topic, at, actor, Alias::As(to)))
    }

    #[must_use]
    pub fn unaliased(topic: Topic, at: Datetime, actor: Actor) -> Self {
        Self::of(topic, at, actor, Alias::Itself)
    }
}

const ALIASED: Signature = ("Aliased", &["topic", "at", "actor", "to"]);
const UNALIASED: Signature = ("Unaliased", &["topic", "at", "actor"]);
const SIGNATURES: &[Signature] = &[ALIASED, UNALIASED];

/// The alias store's schema.
pub type AliasVocabulary = Vocab<Alias>;

impl Says for Alias {
    type Key = Topic;
    type Register = Only;
    type Registers<'a> = One<'a, AliasEvent>;

    const EVENTS: &'static [Signature] = SIGNATURES;

    fn registers() -> Vec<Only> {
        vec![Only]
    }

    fn writes(_: &AliasEvent) -> impl Iterator<Item = Only> {
        std::iter::once(Only)
    }

    fn body(&self) -> (Signature, Vec<Value>) {
        match self {
            Alias::As(to) => (ALIASED, vec![to.to_value()]),
            Alias::Itself => (UNALIASED, Vec::new()),
        }
    }

    fn parse(fields: &Fields<'_>) -> Result<AliasEvent, ProdromeError> {
        let (topic, at, actor) = (
            fields.get("topic")?,
            fields.get("at")?,
            fields.get("actor")?,
        );
        match fields.name() {
            "Aliased" => AliasEvent::aliased(topic, fields.get("to")?, at, actor)
                .map_err(|failed| ProdromeError::invalid(failed.to_string())),
            "Unaliased" => Ok(AliasEvent::unaliased(topic, at, actor)),
            other => Err(crate::literal::no_arm("an alias event", other)),
        }
    }
}

/// The register's type: an [`Alias`], discrete, so two different aliases written concurrently
/// for one topic are a conflict, which the reading shows and the next rename settles.
#[derive(Debug)]
pub struct AliasType;

impl RegisterType<AliasEvent> for AliasType {
    type Value<'e> = Discrete<&'e Alias>;

    fn value(event: &AliasEvent) -> Option<Discrete<&Alias>> {
        Some(Discrete(event.said()))
    }
}

impl Sole for AliasEvent {
    fn reading<'a>(frontier: &Frontier<'a, Self>) -> Vec<&'a Stamp<Self>> {
        frontier.reading::<AliasType>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_constructor_is_a_term_or_an_envelope() {
        crate::literal::assert_own(SIGNATURES);
    }

    #[test]
    fn a_topic_is_no_alias_of_itself() {
        let at = Datetime::new(2026, 10, 21, 9, 0, 0, 0).expect("an instant");
        let ada = Actor::new("ada").expect("an actor");
        let travel = Topic::new("travel").expect("a topic");
        assert_eq!(
            AliasEvent::aliased(travel.clone(), travel.clone(), at, ada.clone()),
            Err(OfItself(travel.clone()))
        );
        let itself = crate::literal::record(
            ALIASED,
            vec![
                travel.to_value(),
                at.to_value(),
                ada.to_value(),
                travel.to_value(),
            ],
        );
        assert!(
            <AliasEvent as prodrome_core::schema::Schema>::from_value(
                &AliasVocabulary::default(),
                &itself
            )
            .is_err()
        );
    }
}
