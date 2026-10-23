//! A place where concurrent renames disagree. The reading still answers one tag for every name,
//! the same on every device, and lists each dispute so a person can settle it with one rename.

use std::collections::BTreeSet;

use crate::secretary::tags::alias_event::Alias;
use crate::secretary::tags::tag::Tag;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dispute {
    /// The reading follows `chosen`, the alias of the write with the least object name.
    Aliases {
        tag: Tag,
        chosen: Alias,
        others: Vec<Alias>,
    },
    /// Every name in the ring reads as `spelled`, the least of them.
    Ring { names: BTreeSet<Tag>, spelled: Tag },
}
