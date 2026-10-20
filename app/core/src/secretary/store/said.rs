//! One stored turn, with its instant.

use crate::capability::{Datetime, Turn};
use crate::secretary::stamp::Stamp;

/// One stored turn, with the instant its `said` line carried, so it can be placed on a day or
/// against a proposal's `after_turn`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// When it was said.
    pub at: Datetime,
    /// The turn itself.
    pub turn: Turn,
    /// Who stamped it: [`Entry::Said`](super::Entry::Said)'s own `by`.
    pub by: Option<Stamp>,
}
