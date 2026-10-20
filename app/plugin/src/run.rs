//! What one call of a plugin did: its answer, and what the host recorded while it ran.

use suzatary_core::plugin::{CredentialName, Refusal, Said, Synced};

use crate::Log;

/// Why a call produced nothing. Whatever it repeats of the plugin's words is [`Said`], clipped
/// where the answer crossed into the host, so no error a call returns is longer than the bound.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    /// The plugin answered that a fetch was refused.
    #[error("the plugin reports a refused fetch: {0}")]
    Refused(Refusal),
    /// The plugin could not read what the outside world answered.
    #[error("the plugin could not read its answer: {0}")]
    Malformed(Said),
    /// The plugin did not recognise the cursor.
    #[error("the plugin refused its cursor: {0}")]
    BadCursor(Said),
    /// The component could not be instantiated in a fresh store.
    #[error("the plugin could not be instantiated: {0}")]
    Instantiate(Said),
    /// The call outlived its budget's time and was interrupted.
    #[error("the plugin ran past its time budget and was stopped")]
    Deadline,
    /// The plugin grew past its budget's memory and was stopped.
    #[error("the plugin grew past its memory budget and was stopped")]
    Memory,
    /// The plugin trapped: a panic, an unreachable, a stack overflow.
    #[error("the plugin trapped: {0}")]
    Trapped(Said),
}

/// One fetch the plugin asked for, as the host saw it: the credential it designated, by name,
/// and what came of it. The host's own record, so a reader (the Settings page's "last synced")
/// never rests on what a plugin says of itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetch {
    /// The declared credential the request named, as its URL or for a header; `None` for
    /// neither, or for a name its manifest does not declare.
    pub credential: Option<CredentialName>,
    /// The status it was answered with, or why it was not sent or not handed back.
    pub outcome: Result<u16, Refusal>,
}

/// One call, as the host saw it.
///
/// `refused` is the host's own record of every fetch it refused, so a plugin that swallows a
/// refusal is still seen to have been refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The answer.
    pub result: Result<Synced, RunError>,
    /// Every fetch the host refused during the call, in order.
    pub refused: Vec<Refusal>,
    /// Every fetch asked for, in order, refused or answered.
    pub fetches: Vec<Fetch>,
    /// What the plugin logged.
    pub log: Log,
}
