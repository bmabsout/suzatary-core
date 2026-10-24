//! Mail-in as a port: one account's mail pulled from its server into its maildir.

use crate::connect::Failure;

/// What pulls one account's mail into its maildir: Microsoft's IMAP, mbsync from the Proton
/// Bridge, lieer from Gmail (on lieer's own grant or on the box's Google grant), or
/// [`super::mailroom::Mailroom`] in memory. Every message lands once, as the server holds it, in
/// a leaf's `new/` or `cur/` under the account's maildir, where `notmuch new` indexes it;
/// `conformance::puller` holds any implementation to that.
pub trait Puller {
    /// Pull the account's mail, its INBOX from `since` where the kind searches by date.
    ///
    /// # Errors
    /// [`Unpulled`], whose class names the fix.
    fn pull(&self, since: chrono::NaiveDate) -> Result<String, Unpulled>;
}

/// Why one account's pull failed: its class, and the line the sync's report shows for it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{said}")]
pub struct Unpulled {
    /// The class, which names the fix card.
    pub class: Failure,
    /// What the report shows.
    pub said: String,
}

impl From<super::imap::ImapError> for Unpulled {
    fn from(failed: super::imap::ImapError) -> Self {
        Self {
            class: failed.class(),
            said: failed.to_string(),
        }
    }
}
