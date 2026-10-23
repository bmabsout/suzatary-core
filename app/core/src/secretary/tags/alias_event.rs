//! The alias store's schema. A rename writes aliases and no membership: memberships stay under
//! the names they were written with, and the reading follows each name's alias.

use prodrome_core::event::Actor;
use prodrome_core::literal::{Datetime, ProdromeError, Table, Value, Vocabulary};
use prodrome_core::schema::Schema;

use crate::literal::{Fields, Literal as _, Signature, record};
use crate::secretary::tags::alias_registers::{AliasRegister, AliasRegisters};
use crate::secretary::tags::tag::Tag;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Alias {
    Itself,
    /// Never the tag itself: [`AliasEvent::aliased`] refuses that.
    As(Tag),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasEvent {
    tag: Tag,
    at: Datetime,
    actor: Actor,
    alias: Alias,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0} cannot be an alias of itself; it is a name of its own")]
pub struct OfItself(pub Tag);

impl AliasEvent {
    /// Refuses `to == tag`, so "its own name" has one spelling, [`AliasEvent::unaliased`].
    pub fn aliased(tag: Tag, to: Tag, at: Datetime, actor: Actor) -> Result<Self, OfItself> {
        if tag == to {
            return Err(OfItself(tag));
        }
        Ok(Self {
            tag,
            at,
            actor,
            alias: Alias::As(to),
        })
    }

    #[must_use]
    pub fn unaliased(tag: Tag, at: Datetime, actor: Actor) -> Self {
        Self {
            tag,
            at,
            actor,
            alias: Alias::Itself,
        }
    }

    #[must_use]
    pub fn tag(&self) -> &Tag {
        &self.tag
    }

    #[must_use]
    pub fn alias(&self) -> &Alias {
        &self.alias
    }
}

const ALIASED: Signature = ("Aliased", &["tag", "at", "actor", "to"]);
const UNALIASED: Signature = ("Unaliased", &["tag", "at", "actor"]);
const SIGNATURES: &[Signature] = &[ALIASED, UNALIASED];

#[derive(Debug, Clone, Default)]
pub struct AliasVocabulary;

impl Vocabulary for AliasVocabulary {
    fn signature(&self, name: &str) -> Option<prodrome_core::literal::Signature<'_>> {
        Table(SIGNATURES).find(name)
    }
}

impl Schema for AliasEvent {
    type Vocabulary = AliasVocabulary;
    type Key = Tag;
    type Register = AliasRegister;

    fn registers(_: &AliasVocabulary) -> Vec<AliasRegister> {
        vec![AliasRegister]
    }

    type Registers<'a> = AliasRegisters<'a>;

    fn to_value(&self) -> Value {
        let head = vec![
            self.tag.to_value(),
            self.at.to_value(),
            self.actor.to_value(),
        ];
        match &self.alias {
            Alias::As(to) => record(ALIASED, head.into_iter().chain([to.to_value()]).collect()),
            Alias::Itself => record(UNALIASED, head),
        }
    }

    fn from_value(_: &AliasVocabulary, value: &Value) -> Result<Self, ProdromeError> {
        let fields = Fields::among(value, SIGNATURES)?;
        let (tag, at, actor) = (fields.get("tag")?, fields.get("at")?, fields.get("actor")?);
        match fields.name() {
            "Aliased" => Self::aliased(tag, fields.get("to")?, at, actor)
                .map_err(|failed| ProdromeError::invalid(failed.to_string())),
            "Unaliased" => Ok(Self::unaliased(tag, at, actor)),
            other => Err(crate::literal::no_arm("an alias event", other)),
        }
    }

    fn key(&self) -> &Tag {
        &self.tag
    }

    fn at(&self) -> Datetime {
        self.at
    }

    fn actor(&self) -> &Actor {
        &self.actor
    }

    fn writes(&self) -> impl Iterator<Item = AliasRegister> {
        std::iter::once(AliasRegister)
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
    fn a_tag_is_no_alias_of_itself() {
        let at = Datetime::new(2026, 10, 21, 9, 0, 0, 0).expect("an instant");
        let ada = Actor::new("ada").expect("an actor");
        let travel = Tag::new("travel").expect("a tag");
        assert_eq!(
            AliasEvent::aliased(travel.clone(), travel.clone(), at, ada.clone()),
            Err(OfItself(travel.clone()))
        );
        let itself = record(
            ALIASED,
            vec![
                travel.to_value(),
                at.to_value(),
                ada.to_value(),
                travel.to_value(),
            ],
        );
        assert!(AliasEvent::from_value(&AliasVocabulary, &itself).is_err());
    }
}
