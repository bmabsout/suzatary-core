//! A place where concurrent renames disagree. The reading still answers one topic for every name,
//! the same on every device, and lists each dispute so a person can settle it with one rename.

use std::collections::BTreeSet;

use crate::secretary::topics::alias_event::Alias;
use crate::secretary::topics::topic::Topic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dispute {
    /// The reading follows `chosen`, the alias of the write with the least object name.
    Aliases {
        topic: Topic,
        chosen: Alias,
        others: Vec<Alias>,
    },
    /// Every name in the ring reads as `spelled`, the least of them.
    Ring {
        names: BTreeSet<Topic>,
        spelled: Topic,
    },
}
