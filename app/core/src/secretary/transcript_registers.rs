//! A conversation's one register, its transcript: every line, a grow-only set under inclusion.
//!
//! A line's value is the set of lines it rests on and itself, its down-set in the causal order,
//! and the register's join is union. The register holds its frontier, the lines no later line
//! rests on: a new line supersedes exactly those, so the lines are causally ordered, and its
//! down-set contains every superseded line's, so the register only grows. Two heads are not a
//! conflict, since union joins them; the next line rests on both.

use prodrome_core::fold::{Frontier, Product, Register as _};
use prodrome_core::literal::ProdromeError;
use prodrome_core::registers::Stamp;

use prodrome_core::schema::Schema;

/// The register a line supersedes in: the transcript, the only one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TranscriptRegister;

/// One conversation's registers, at a schema of lines `E`.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptRegisters<'a, E: Schema> {
    /// The lines no later line rests on.
    pub heads: Frontier<'a, E>,
}

impl<E: Schema> Default for TranscriptRegisters<'_, E> {
    fn default() -> Self {
        Self {
            heads: Frontier::default(),
        }
    }
}

impl<'a, E: Schema<Register = TranscriptRegister>> Product<'a> for TranscriptRegisters<'a, E> {
    type Schema = E;

    fn registers(&self) -> Vec<TranscriptRegister> {
        vec![TranscriptRegister]
    }

    fn join(&mut self, stamp: &'a Stamp<E>) {
        self.heads.join(stamp);
    }

    fn frontier(&self, _: TranscriptRegister) -> &Frontier<'a, E> {
        &self.heads
    }

    fn frontier_mut(&mut self, _: TranscriptRegister) -> &mut Frontier<'a, E> {
        &mut self.heads
    }

    /// Every head: under inclusion no head's down-set holds another's, so each is maximal, and
    /// the transcript is their union.
    fn reading(&self, _: TranscriptRegister) -> Vec<&'a Stamp<E>> {
        self.heads.writes().to_vec()
    }

    /// A line rests on the whole frontier, so its down-set holds every one it supersedes: there
    /// is no write below the reading to refuse.
    fn grows(&self, _: &E) -> Result<(), ProdromeError> {
        Ok(())
    }

    /// Union joins two heads, so none is a conflict to settle.
    fn conflicts(
        &self,
    ) -> std::collections::BTreeMap<TranscriptRegister, Vec<prodrome_core::event::Hash>> {
        std::collections::BTreeMap::new()
    }
}
