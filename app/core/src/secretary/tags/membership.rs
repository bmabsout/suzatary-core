//! The membership store's key, so each (target, tag) pair is a register of its own.

use prodrome_core::fold::Order;

use crate::secretary::tags::tag::Tag;
use crate::secretary::tags::target::Target;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Membership {
    pub target: Target,
    /// As its writer spelled it; an alias may spell it otherwise now.
    pub tag: Tag,
}

/// `On` above `Off`: a removal supersedes the adds its writer saw, and concurrent writes read
/// their greatest, so an add the removal did not see stays on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Presence {
    Off,
    On,
}

impl Order for Presence {
    fn le(&self, other: &Self) -> bool {
        self <= other
    }
}
