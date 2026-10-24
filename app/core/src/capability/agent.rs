use super::{Clock, Corpus, Mailbox, Manuals, Render, Repository};
use crate::record::TodoEvent;
use crate::store::Prodrome;

/// What the triage agent may use: it reads attacker-controlled mail, so nothing here
/// sends, notifies, names a peer or performs a card, and its `chain` is a replica it reads.
/// Its writes are cards, filed through the call's [`crate::secretary::filed::TriageCards`].
///
/// ```
/// fn write(agent: suzatary_core::capability::Agent<'_>) { let _ = agent.chain; }
/// ```
///
/// No field reaches the owner's phone, a peer's login, the model, or the inbox a person
/// taps:
///
/// ```compile_fail,E0609
/// fn push(agent: suzatary_core::capability::Agent<'_>) { let _ = agent.notify; }
/// ```
/// ```compile_fail,E0609
/// fn peer(agent: suzatary_core::capability::Agent<'_>) { let _ = agent.whois; }
/// ```
/// ```compile_fail,E0609
/// fn file(agent: suzatary_core::capability::Agent<'_>) { let _ = agent.inbox; }
/// ```
/// ```compile_fail,E0609
/// fn fetch(agent: suzatary_core::capability::Agent<'_>) { let _ = agent.web; }
/// ```
///
/// A caller may swap one of its effects, and may not add one:
///
/// ```
/// use suzatary_core::capability::{Agent, Clock};
/// fn swap<'a>(agent: Agent<'a>, clock: &'a dyn Clock) -> Agent<'a> {
///     Agent { clock, ..agent }
/// }
/// ```
/// ```compile_fail,E0560
/// use suzatary_core::capability::{Agent, Notify};
/// fn widen<'a>(agent: Agent<'a>, push: &'a dyn Notify) -> Agent<'a> {
///     Agent { notify: push, ..agent }
/// }
/// ```
#[derive(Clone, Copy)]
pub struct Agent<'a> {
    /// A replica of the chain, synced before a run, which it reads.
    pub chain: &'a dyn Prodrome<TodoEvent>,
    /// The triage queue and its dedup state.
    pub corpus: &'a dyn Corpus,
    /// The mail index, read-only.
    pub mail: &'a dyn Mailbox,
    /// The repository's gates.
    pub repository: &'a dyn Repository,
    /// The manuals it is handed.
    pub manuals: &'a dyn Manuals,
    /// Typst, for confirmed markup.
    pub render: &'a dyn Render,
    /// The instant every write is stamped with.
    pub clock: &'a dyn Clock,
}
