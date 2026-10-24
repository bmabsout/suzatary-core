//! The membership store's schema: `Added` and `Removed`, each writing its pair's one register.

use prodrome_core::event::Actor;
use prodrome_core::fold::{Frontier, RegisterType};
use prodrome_core::literal::{Datetime, ProdromeError, Value};
use prodrome_core::registers::Stamp;

use crate::envelope::{Event, Head, Says, Vocab};
use crate::literal::{Fields, Literal as _, Signature};
use crate::secretary::topics::membership::{Membership, Presence};
use crate::secretary::topics::target::Target;
use crate::secretary::topics::topic::Topic;
use crate::sole::{One, Only, Sole};

/// A topic put on a target or taken off it.
pub type MembershipEvent = Event<Presence>;

impl MembershipEvent {
    #[must_use]
    pub fn added(target: Target, topic: Topic, at: Datetime, actor: Actor) -> Self {
        Self::of(Membership { target, topic }, at, actor, Presence::On)
    }

    #[must_use]
    pub fn removed(target: Target, topic: Topic, at: Datetime, actor: Actor) -> Self {
        Self::of(Membership { target, topic }, at, actor, Presence::Off)
    }

    #[must_use]
    pub fn presence(&self) -> Presence {
        *self.said()
    }
}

const ADDED: Signature = ("Added", &["target", "topic", "at", "actor"]);
const REMOVED: Signature = ("Removed", &["target", "topic", "at", "actor"]);
const SIGNATURES: &[Signature] = &[ADDED, REMOVED];

/// The membership store's schema.
pub type MembershipVocabulary = Vocab<Presence>;

/// A membership's key is two fields, its target's and its topic's.
impl Head for Membership {
    fn head(&self) -> Vec<Value> {
        vec![self.target.to_value(), self.topic.to_value()]
    }
}

impl Says for Presence {
    type Key = Membership;
    type Register = Only;
    type Registers<'a> = One<'a, MembershipEvent>;

    const EVENTS: &'static [Signature] = SIGNATURES;

    fn registers() -> Vec<Only> {
        vec![Only]
    }

    fn writes(_: &MembershipEvent) -> impl Iterator<Item = Only> {
        std::iter::once(Only)
    }

    fn body(&self) -> (Signature, Vec<Value>) {
        match self {
            Presence::On => (ADDED, Vec::new()),
            Presence::Off => (REMOVED, Vec::new()),
        }
    }

    fn parse(fields: &Fields<'_>) -> Result<MembershipEvent, ProdromeError> {
        let presence = match fields.name() {
            "Added" => Presence::On,
            "Removed" => Presence::Off,
            other => return Err(crate::literal::no_arm("a membership event", other)),
        };
        let membership = Membership {
            target: fields.get("target")?,
            topic: fields.get("topic")?,
        };
        Ok(MembershipEvent::of(
            membership,
            fields.get("at")?,
            fields.get("actor")?,
            presence,
        ))
    }
}

/// The register's type: a [`Presence`]. It is not inflationary: a removal after an add goes
/// down, and only causal order decides that it does.
#[derive(Debug)]
pub struct PresenceRegister;

impl RegisterType<MembershipEvent> for PresenceRegister {
    type Value<'e> = Presence;

    fn value(event: &MembershipEvent) -> Option<Presence> {
        Some(event.presence())
    }
}

impl Sole for MembershipEvent {
    fn reading<'a>(frontier: &Frontier<'a, Self>) -> Vec<&'a Stamp<Self>> {
        frontier.reading::<PresenceRegister>()
    }
}

impl One<'_, MembershipEvent> {
    /// `Off` where nothing wrote it.
    #[must_use]
    pub fn presence(&self) -> Presence {
        self.reading()
            .iter()
            .map(|stamp| stamp.event.presence())
            .max()
            .unwrap_or(Presence::Off)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_constructor_is_a_term_or_an_envelope() {
        crate::literal::assert_own(SIGNATURES);
    }
}
