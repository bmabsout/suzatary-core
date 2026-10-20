//! A node's one register: what was last said of it, discrete.
//!
//! Every digest event about a node supersedes the writes it descends from, so a retraction
//! supersedes the summary it read and a summary written again supersedes the retraction. Two
//! writes no later write descends from are a conflict, shown whole: two replicas that each
//! summarised one node hold both, and neither is picked by when it was written.

use prodrome_core::fold::{Frontier, Product, Register as _};
use prodrome_core::literal::ProdromeError;
use prodrome_core::registers::Stamp;

use crate::secretary::digest_event::DigestEvent;

/// The register every digest event supersedes in: the node's, the only one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeRegister;

/// One node's registers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeRegisters<'a> {
    /// The writes no later write to this node rests on.
    pub said: Frontier<'a, DigestEvent>,
}

impl<'a> NodeRegisters<'a> {
    /// The register's reading under the discrete order: one write, or a conflict's candidates,
    /// twins counted once.
    #[must_use]
    pub fn reading(&self) -> Vec<&'a Stamp<DigestEvent>> {
        self.said.candidates()
    }
}

impl<'a> Product<'a> for NodeRegisters<'a> {
    type Schema = DigestEvent;

    fn registers(&self) -> Vec<NodeRegister> {
        vec![NodeRegister]
    }

    fn join(&mut self, stamp: &'a Stamp<DigestEvent>) {
        self.said.join(stamp);
    }

    fn frontier(&self, _: NodeRegister) -> &Frontier<'a, DigestEvent> {
        &self.said
    }

    fn frontier_mut(&mut self, _: NodeRegister) -> &mut Frontier<'a, DigestEvent> {
        &mut self.said
    }

    fn reading(&self, _: NodeRegister) -> Vec<&'a Stamp<DigestEvent>> {
        NodeRegisters::reading(self)
    }

    /// Discrete, so not inflationary: a retraction may follow a summary and a summary a
    /// retraction.
    fn grows(&self, _: &DigestEvent) -> Result<(), ProdromeError> {
        Ok(())
    }
}
