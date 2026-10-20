//! The secretary's two stores as replicas a tab holds: what a replica lacks, and their heads as
//! they move. The inbox and the conversations' lines are written only by the box, so a tab only
//! pulls, a piece at a time; the lines' pull carries the keys of the words it sends.

use axum::Json;
use suzatary_core::capability::Server;
use suzatary_core::refusal::Refusal;
use suzatary_core::store::StoreError;
use suzatary_core::{query, routes, wire};

use super::Chained;
use super::call::Call;
use super::guard::Reader;
use super::params::{Asked, Params};

/// A store a tab holds a replica of, named by its path segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    /// The proposals, `inbox`.
    Inbox,
    /// The conversations' lines, `lines`.
    Lines,
}

impl Store {
    /// The store a segment names; any other is not found.
    ///
    /// # Errors
    /// [`Refusal::not_found`] for a name that is no store's.
    pub fn named(segment: &str) -> Result<Self, Refusal> {
        match segment {
            "inbox" => Ok(Self::Inbox),
            "lines" => Ok(Self::Lines),
            // A page from before the lines asks for the store they replaced: it is told a new version is ready.
            "conversations" => Err(Refusal::new(
                suzatary_core::refusal::Status::UpgradeRequired,
                "the conversations are read from the lines now: a new version of this page is ready, and Reload loads it",
            )),
            other => Err(Refusal::not_found(format!(
                "a tab holds the inbox and the lines, not {}",
                suzatary_core::refusal::shown(other)
            ))),
        }
    }

    /// Its heads, sorted.
    ///
    /// # Errors
    /// The store's own.
    pub fn heads(self, caps: &Server<'_>) -> Result<Vec<String>, StoreError> {
        let tips = match self {
            Self::Inbox => caps.inbox.tips()?,
            Self::Lines => caps.conversations.lines.tips()?,
        };
        let mut heads: Vec<String> = tips.iter().map(|h| h.as_str().to_owned()).collect();
        heads.sort();
        Ok(heads)
    }
}

/// A store, and the heads a replica of it holds.
pub struct Since {
    store: Store,
    since: Vec<prodrome_core::event::Hash>,
}

impl Params for Since {
    fn parse(asked: &Asked, _: &Chained) -> Result<Self, Refusal> {
        Ok(Self {
            store: Store::named(asked.segment("store")?)?,
            since: query::since(asked.query("since"))?,
        })
    }
}

/// `GET /api/stores/<store>/objects?since=<heads>`: a piece of what a replica holding those heads
/// lacks, and how much is left after it.
pub fn objects(call: &Call<Reader, Since, ()>) -> Result<Json<wire::StorePayload>, Refusal> {
    let caps = call.state.first.caps;
    let Since { store, since } = &call.params;
    let (epochs, now) = (call.state.epochs.as_ref(), caps.clock.now());
    let cut = routes::Cut::PIECE;
    match store {
        Store::Inbox => routes::store_objects_since(caps.inbox, "inbox", since, epochs, now, cut),
        Store::Lines => routes::lines_since(caps.conversations, since, epochs, now, cut),
    }
    .map(Json)
}

/// `GET /api/waiting`: the Inbox badge's count, read here so a tab shows it without pulling the
/// conversations. A decision names a batch on any chain this box serves.
pub fn waiting(call: &Call<Reader, (), ()>) -> Result<Json<wire::WaitingPayload>, Refusal> {
    let caps = call.state.first.caps;
    let inbox = suzatary_core::secretary::inbox::fold(caps.inbox)?;
    let read = call.state.chains_read();
    let set = read.set(caps.clock.now());
    let chains: Vec<_> = call
        .state
        .chains
        .values()
        .map(|chained| chained.policy.name.clone())
        .collect();
    let batches = |name: &suzatary_core::chains::ChainName| {
        call.state
            .chains
            .get(name.as_str())
            .map_or(Ok(Vec::new()), |chained| {
                let target = chained.caps.chain.objects()?;
                let replica = chained.caps.proposals.objects()?;
                Ok(suzatary_core::proposals::batches(
                    &suzatary_core::sync::Objects::of(&target),
                    &suzatary_core::sync::Objects::of(&replica),
                ))
            })
    };
    routes::waiting(&inbox, caps.conversations.lines, &set, &chains, &batches).map(Json)
}
