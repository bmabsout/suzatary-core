//! Whether the compactor runs, and with what: a sum, so a compactor that is off names no model
//! and no budget, and a running one names both.

use crate::models::ModelName;
use crate::secretary::compact_budget::Budget;
use crate::secretary::compaction::Compaction;

/// The compactor as the instance file sets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compacting {
    /// The kill switch: nothing is compacted, and the lines wait.
    Off,
    /// Compact with `model`, within `budget`.
    On {
        /// The model the compactor runs.
        model: ModelName,
        /// What it may spend.
        budget: Budget,
    },
}

impl Compacting {
    /// The rule it runs, none when off.
    #[must_use]
    pub fn rule(&self) -> Option<Compaction> {
        match self {
            Compacting::Off => None,
            Compacting::On { model, .. } => Some(Compaction {
                model: model.clone(),
            }),
        }
    }
}
