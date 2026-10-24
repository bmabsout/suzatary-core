//! The registers of a schema with one register, [`Only`], which its events write: one frontier,
//! read and climbed as the schema says. The memory, the digest, the conversations' lines and the
//! two topic stores are such schemas, so each implements [`Sole`] and its product is [`One`].

use std::collections::BTreeMap;

use prodrome_core::event::Hash;
use prodrome_core::fold::{Frontier, Product, Register as _};
use prodrome_core::literal::ProdromeError;
use prodrome_core::registers::Stamp;
use prodrome_core::schema::Schema;

/// The one register's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Only;

/// A schema whose events write one register, [`Only`].
pub trait Sole: Schema<Register = Only> {
    /// The register's reading: one write, or a conflict's candidates.
    fn reading<'a>(frontier: &Frontier<'a, Self>) -> Vec<&'a Stamp<Self>>;

    /// The append's refusal of a write below the reading; none where the register is not
    /// inflationary.
    ///
    /// # Errors
    /// A write whose value falls below, or beside, the reading it supersedes.
    fn grows(_: &Frontier<'_, Self>, _: &Self) -> Result<(), ProdromeError> {
        Ok(())
    }

    /// Whether the frontier is a conflict to settle: more than one write, unless the
    /// register's join is union.
    #[must_use]
    fn conflict(frontier: &Frontier<'_, Self>) -> bool {
        frontier.is_conflict()
    }
}

/// One entity's registers at a [`Sole`] schema: the writes no later write to it rests on.
#[derive(Debug, Clone, PartialEq)]
pub struct One<'a, E: Schema>(pub Frontier<'a, E>);

impl<E: Schema> Default for One<'_, E> {
    fn default() -> Self {
        Self(Frontier::default())
    }
}

impl<'a, E: Sole> One<'a, E> {
    /// The register's reading.
    #[must_use]
    pub fn reading(&self) -> Vec<&'a Stamp<E>> {
        E::reading(&self.0)
    }
}

impl<'a, E: Sole> Product<'a> for One<'a, E> {
    type Schema = E;

    fn registers(&self) -> Vec<Only> {
        vec![Only]
    }

    fn join(&mut self, stamp: &'a Stamp<E>) {
        self.0.join(stamp);
    }

    fn frontier(&self, _: Only) -> &Frontier<'a, E> {
        &self.0
    }

    fn frontier_mut(&mut self, _: Only) -> &mut Frontier<'a, E> {
        &mut self.0
    }

    fn reading(&self, _: Only) -> Vec<&'a Stamp<E>> {
        E::reading(&self.0)
    }

    fn grows(&self, event: &E) -> Result<(), ProdromeError> {
        E::grows(&self.0, event)
    }

    fn conflicts(&self) -> BTreeMap<Only, Vec<Hash>> {
        if E::conflict(&self.0) {
            BTreeMap::from([(Only, self.0.names())])
        } else {
            BTreeMap::new()
        }
    }
}
