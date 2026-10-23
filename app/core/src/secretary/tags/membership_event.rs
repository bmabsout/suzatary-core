//! The membership store's schema: `Tagged` and `Untagged`, each writing its pair's one register.

use prodrome_core::event::Actor;
use prodrome_core::literal::{Datetime, ProdromeError, Table, Value, Vocabulary};
use prodrome_core::schema::Schema;

use crate::literal::{Fields, Literal as _, Signature, record};
use crate::secretary::tags::membership::{Membership, Presence};
use crate::secretary::tags::membership_registers::{MembershipRegister, MembershipRegisters};
use crate::secretary::tags::tag::Tag;
use crate::secretary::tags::target::Target;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipEvent {
    membership: Membership,
    at: Datetime,
    actor: Actor,
    presence: Presence,
}

impl MembershipEvent {
    #[must_use]
    pub fn tagged(target: Target, tag: Tag, at: Datetime, actor: Actor) -> Self {
        Self::of(Membership { target, tag }, at, actor, Presence::On)
    }

    #[must_use]
    pub fn untagged(target: Target, tag: Tag, at: Datetime, actor: Actor) -> Self {
        Self::of(Membership { target, tag }, at, actor, Presence::Off)
    }

    fn of(membership: Membership, at: Datetime, actor: Actor, presence: Presence) -> Self {
        Self {
            membership,
            at,
            actor,
            presence,
        }
    }

    #[must_use]
    pub fn membership(&self) -> &Membership {
        &self.membership
    }

    #[must_use]
    pub fn presence(&self) -> Presence {
        self.presence
    }
}

const TAGGED: Signature = ("Tagged", &["target", "tag", "at", "actor"]);
const UNTAGGED: Signature = ("Untagged", &["target", "tag", "at", "actor"]);
const SIGNATURES: &[Signature] = &[TAGGED, UNTAGGED];

#[derive(Debug, Clone, Default)]
pub struct MembershipVocabulary;

impl Vocabulary for MembershipVocabulary {
    fn signature(&self, name: &str) -> Option<prodrome_core::literal::Signature<'_>> {
        Table(SIGNATURES).find(name)
    }
}

impl Schema for MembershipEvent {
    type Vocabulary = MembershipVocabulary;
    type Key = Membership;
    type Register = MembershipRegister;

    fn registers(_: &MembershipVocabulary) -> Vec<MembershipRegister> {
        vec![MembershipRegister]
    }

    type Registers<'a> = MembershipRegisters<'a>;

    fn to_value(&self) -> Value {
        let signature = match self.presence {
            Presence::On => TAGGED,
            Presence::Off => UNTAGGED,
        };
        record(
            signature,
            vec![
                self.membership.target.to_value(),
                self.membership.tag.to_value(),
                self.at.to_value(),
                self.actor.to_value(),
            ],
        )
    }

    fn from_value(_: &MembershipVocabulary, value: &Value) -> Result<Self, ProdromeError> {
        let fields = Fields::among(value, SIGNATURES)?;
        let presence = match fields.name() {
            "Tagged" => Presence::On,
            "Untagged" => Presence::Off,
            other => return Err(crate::literal::no_arm("a membership event", other)),
        };
        Ok(Self::of(
            Membership {
                target: fields.get("target")?,
                tag: fields.get("tag")?,
            },
            fields.get("at")?,
            fields.get("actor")?,
            presence,
        ))
    }

    fn key(&self) -> &Membership {
        &self.membership
    }

    fn at(&self) -> Datetime {
        self.at
    }

    fn actor(&self) -> &Actor {
        &self.actor
    }

    fn writes(&self) -> impl Iterator<Item = MembershipRegister> {
        std::iter::once(MembershipRegister)
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
