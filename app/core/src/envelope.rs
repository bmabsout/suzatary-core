//! One event of an application schema whose events print as
//! `Constructor(<key>, at, actor, <own fields>)`: what it is about, when its writer stamped it
//! (data, never an order), who wrote it, and what it says. A schema says the last part, a
//! [`Says`]; [`Event`] is the rest, and is the [`Schema`] its store is opened at.

use std::fmt::Debug;
use std::marker::PhantomData;

use prodrome_core::event::Actor;
use prodrome_core::fold::Product;
use prodrome_core::literal::{Datetime, ProdromeError, Table, Value, Vocabulary};
use prodrome_core::schema::Schema;

use crate::literal::{Fields, Literal, Signature, record};

/// One event: its key, its instant, its writer and what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event<S: Says> {
    key: S::Key,
    at: Datetime,
    actor: Actor,
    said: S,
}

/// What an event of a schema says of its key, and the rest of the schema.
pub trait Says: Clone + PartialEq + Debug + Send + Sync + Sized + 'static {
    /// What an event is about.
    type Key: Head;
    /// A register a write supersedes in.
    type Register: Copy + Ord + Debug + Send + Sync + 'static;
    /// An entity's registers.
    type Registers<'a>: Product<'a, Schema = Event<Self>>;

    /// Every event constructor, its declared fields the key's, `at` and `actor` first.
    const EVENTS: &'static [Signature];
    /// The constructors nested in an event's fields.
    const NESTED: &'static [Signature] = &[];

    /// Every register a write supersedes in, in the order a reading lists them.
    fn registers() -> Vec<Self::Register>;

    /// The registers `event` writes, each once.
    fn writes(event: &Event<Self>) -> impl Iterator<Item = Self::Register>;

    /// Is `event` about standing? (`Schema::asks`.)
    #[must_use]
    fn asks(_: &Event<Self>) -> bool {
        true
    }

    /// Its constructor, and the values of the fields after the head.
    fn body(&self) -> (Signature, Vec<Value>);

    /// The event `fields` spell, through the schema's smart constructors.
    ///
    /// # Errors
    /// A field a smart constructor refuses.
    fn parse(fields: &Fields<'_>) -> Result<Event<Self>, ProdromeError>;
}

/// A key's fields, as an event's head prints them.
pub trait Head: Clone + Ord + Debug + Send + Sync {
    /// Their values, in order.
    fn head(&self) -> Vec<Value>;
}

/// A key with a literal is one field.
impl<T: Literal + Clone + Ord + Debug + Send + Sync> Head for T {
    fn head(&self) -> Vec<Value> {
        vec![self.to_value()]
    }
}

impl<S: Says> Event<S> {
    /// The event, as a schema's smart constructor makes it.
    pub(crate) fn of(key: S::Key, at: Datetime, actor: Actor, said: S) -> Self {
        Self {
            key,
            at,
            actor,
            said,
        }
    }

    /// What it is about.
    #[must_use]
    pub fn key(&self) -> &S::Key {
        &self.key
    }

    /// What it says.
    #[must_use]
    pub fn said(&self) -> &S {
        &self.said
    }
}

/// A schema's whitelist: its events' constructors, then the nested ones.
pub struct Vocab<S>(PhantomData<fn() -> S>);

impl<S> Default for Vocab<S> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<S> Clone for Vocab<S> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<S> Debug for Vocab<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Vocab")
    }
}

impl<S: Says> Vocabulary for Vocab<S> {
    fn signature(&self, name: &str) -> Option<prodrome_core::literal::Signature<'_>> {
        Table(S::EVENTS)
            .find(name)
            .or_else(|| Table(S::NESTED).find(name))
    }
}

impl<S: Says> Schema for Event<S> {
    type Vocabulary = Vocab<S>;
    type Key = S::Key;
    type Register = S::Register;

    fn registers(_: &Vocab<S>) -> Vec<S::Register> {
        S::registers()
    }

    type Registers<'a> = S::Registers<'a>;

    fn to_value(&self) -> Value {
        let (signature, own) = self.said.body();
        let head = self.key.head();
        record(
            signature,
            head.into_iter()
                .chain([self.at.to_value(), self.actor.to_value()])
                .chain(own)
                .collect(),
        )
    }

    fn from_value(_: &Vocab<S>, value: &Value) -> Result<Self, ProdromeError> {
        S::parse(&Fields::among(value, S::EVENTS)?)
    }

    fn key(&self) -> &S::Key {
        &self.key
    }

    fn at(&self) -> Datetime {
        self.at
    }

    fn actor(&self) -> &Actor {
        &self.actor
    }

    fn writes(&self) -> impl Iterator<Item = S::Register> {
        S::writes(self)
    }

    fn asks(&self) -> bool {
        S::asks(self)
    }
}
