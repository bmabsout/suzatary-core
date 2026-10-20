//! One proposal line against the inbox's reading: placed, or held with why not.

use super::{Asking, Standing};
use serde::Serialize;

/// Why the inbox's reading cannot place a conversation's proposal line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub enum Unheld {
    /// The inbox holds nothing of it: on a replica, a record not taken in yet; on the box, which
    /// files the record before the line, a damaged object the store's read leaves out.
    #[error("the inbox holds no record of it")]
    NotHeld,
    /// The inbox holds responses to it and no asking.
    #[error("it is answered and was never asked")]
    Unasked,
}

/// A proposal line the inbox's reading cannot place, held with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unplaced {
    /// The line.
    pub asking: Asking,
    /// Why it has no object.
    pub why: Unheld,
}

/// One proposal line against the inbox's reading: placed with its object, or held as a named
/// failure, as a chain set holds a member that does not read.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    /// Joined with its inbox object.
    Placed(Box<Standing>),
    /// With none, and why.
    Unplaced(Unplaced),
}

impl Line {
    /// The line as its conversation records it.
    #[must_use]
    pub fn asking(&self) -> &Asking {
        match self {
            Line::Placed(standing) => &standing.asking,
            Line::Unplaced(unplaced) => &unplaced.asking,
        }
    }
}
