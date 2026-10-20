//! One batch's answer, as a reader gets it.

use super::Decision;
use crate::secretary::words::Words;

/// One batch's answer, as a reader gets it: the fold of one [`Entry::Reviewed`](super::Entry::Reviewed). A review is a
/// fact about a moment, so re-reviewing a head later is another line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    /// When it was decided.
    pub at: String,
    /// Which chain the batch was against.
    pub chain: String,
    /// The batch, by its head.
    pub batch: String,
    /// What was decided.
    pub decision: Decision,
    /// The objects settled; empty for a hold.
    pub objects: Vec<String>,
    /// Why.
    pub why: Words<String>,
}

impl std::fmt::Display for Review {
    /// The record line, one shape for all three decisions, so every reader quotes one sentence.
    ///
    /// `accepted 3 objects of batch a1b2c3d4e5f6… on life: <why>`, and
    /// `held batch a1b2c3d4e5f6… on life: <why>` (a hold settles no object).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let head: String = self.batch.chars().take(12).collect();
        match self.decision {
            Decision::Held => write!(f, "held batch {head}… on {}: {}", self.chain, self.why),
            settled => write!(
                f,
                "{} {} object{} of batch {head}… on {}: {}",
                settled.word(),
                self.objects.len(),
                if self.objects.len() == 1 { "" } else { "s" },
                self.chain,
                self.why
            ),
        }
    }
}
