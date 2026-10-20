//! What a review did about one batch.

use serde::{Deserialize, Serialize};

/// What a review did about one batch.
///
/// Holding is an answer: a batch left standing on purpose is a different fact from one nobody
/// looked at, and the Inbox shows the reason above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// Adopted onto the chain.
    Accepted,
    /// Deleted from the replica: the one place an object is removed.
    Rejected,
    /// Left standing for the owner, with the reason recorded.
    Held,
}

impl Decision {
    /// The word a record line starts with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Decision::Accepted => "accepted",
            Decision::Rejected => "rejected",
            Decision::Held => "held",
        }
    }
}

// Its literal is its text, as a stored line prints it.
crate::literal::text_literal!(
    Decision => |decision| decision.word().to_owned(),
);
