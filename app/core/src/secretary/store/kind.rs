//! What a conversation is for.

use serde::{Deserialize, Serialize};

/// What a conversation is for: the list pins the default, sorts topics by their last turn, and
/// files the box's own rounds behind a filter.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ts_rs::TS,
)]
#[serde(rename_all = "lowercase")]
#[ts(rename = "ConversationKind")]
pub enum Kind {
    /// [`DEFAULT`](super::DEFAULT), the one `#/secretary` always has.
    Default,
    /// One the owner opened, about one thing.
    Topic,
    /// One the box opened for itself: `secretary review`'s and the sweep's.
    Review,
}

impl Kind {
    /// The name a stored line prints.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Default => "default",
            Kind::Topic => "topic",
            Kind::Review => "review",
        }
    }
}

// Its literal is its text, as a stored line prints it.
crate::literal::text_literal!(
    Kind => |kind| kind.as_str().to_owned(),
);
