//! The secretary's readings a page takes from its replicas of the inbox and the conversations:
//! `suzatary_core::secretary::readings` applied to the objects the classes hold, so a page reads
//! what the box would answer for the same objects. A conversation's reading takes the inbox's
//! class by reference: its objects are already in the module, and nothing crosses again.

use prodrome_core::fpl::datetime_of;
use prodrome_wasm_exports::wire::parse_instant;
use suzatary_core::query;
use suzatary_core::secretary::inbox::Fold;
use suzatary_core::secretary::key_ring::KeyRing;
use suzatary_core::secretary::readings::{self, DEFAULT_TURN_WINDOW, Dress, Window};
use suzatary_core::secretary::store::{ConversationId, Stored};
use suzatary_core::secretary::transcript;
use suzatary_core::wire::MemoryPayload;
use wasm_bindgen::prelude::*;

use crate::{Chains, Conversations, Proposals, json, refused};

#[wasm_bindgen]
impl Proposals {
    /// Every proposal, oldest first, with its state and its value at `at` (ISO), each priced
    /// through `chains` by the todo it serves: what the Inbox lists. A remember card's words are
    /// found in the memory as its route answers it, or read as unread for `undefined`.
    pub fn inbox(
        &self,
        at: &str,
        chains: &Chains,
        memory: Option<String>,
    ) -> Result<String, JsError> {
        let at = parse_instant("at", at).map_err(refused)?;
        let now = datetime_of(at).map_err(|e| refused(format!("at: {e}")))?;
        let memory = memory_of(memory)?;
        json(&readings::inbox(
            &self.fold()?,
            &chains.set(now, None),
            memory.as_ref(),
        ))
    }
}

/// The lines read once with the keys a pull gave: the ring parsed once and every slot opened
/// once, so each question a page asks of one reading reads what was opened. A slot whose key is
/// not given reads as missing, and one an erasure names as erased.
#[wasm_bindgen]
pub struct Opened(Vec<Stored>);

#[wasm_bindgen]
impl Conversations {
    /// These lines opened with `keys`, the pull's `keys` as JSON, `{<name>: <hex>}`.
    pub fn opened(&self, keys: &str) -> Result<Opened, JsError> {
        self.stored(&ring(keys)?).map(Opened)
    }

    /// Every key an erasure of its own conversation names, and every key named in `told` (JSON
    /// `[<name>, …]`, the names the box's pull says an erasure anywhere in the store names, which
    /// may be in a piece not yet held), each with the answer keys of the blocks its words were
    /// rendered to where `keys` (as [`Conversations::opened`] takes them) still opens it, as JSON
    /// `{<name>: [<answer key>, …]}`: what a replica forgets, its rendered blocks first, then the
    /// key, as the box's sweep does.
    pub fn erased(&self, keys: &str, told: &str) -> Result<String, JsError> {
        let ring = ring(keys)?;
        let told: std::collections::BTreeSet<String> =
            serde_json::from_str(told).map_err(|e| refused(format!("told: {e}")))?;
        let read = self.0.read().map_err(refused)?;
        let lines: Vec<_> = read
            .nodes
            .iter()
            .filter_map(|node| node.event().map(|line| (node.name(), line)))
            .collect();
        let named = lines.iter().flat_map(|(_, line)| {
            line.keys()
                .enumerate()
                .filter(|(_, name)| told.contains(name.as_str()))
                .map(move |(slot, name)| (*line, slot, name))
        });
        let mut dead: std::collections::BTreeMap<&str, Vec<String>> =
            std::collections::BTreeMap::new();
        for (line, slot, name) in transcript::dead(lines.iter().copied())
            .into_iter()
            .chain(named)
        {
            let fed = ring.key(name).get().map_or_else(Vec::new, |key| {
                transcript::fed(line, slot, key)
                    .iter()
                    .map(|block| crate::rendering::answer_key(block.as_str()))
                    .collect()
            });
            let blocks = dead.entry(name.as_str()).or_default();
            for block in fed {
                if !blocks.contains(&block) {
                    blocks.push(block);
                }
            }
        }
        json(&dead)
    }
}

#[wasm_bindgen]
impl Opened {
    /// Every conversation, newest first, with what of each stands in `inbox`.
    pub fn list(&self, inbox: &Proposals) -> Result<String, JsError> {
        json(&readings::conversations(&self.0, &inbox.fold()?))
    }

    /// One conversation's window, as the box's route reads it: `before`, `limit` and `about` as
    /// its query spells them, and the memory as its route answers it, or `undefined` for a
    /// memory not read, which reads every receipt as unread. Nothing else beyond the two stores
    /// dresses it: blocks stay their source, and no changeset is drawn.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "wasm-bindgen hands an optional string over by value"
    )]
    pub fn conversation(
        &self,
        id: &str,
        inbox: &Proposals,
        before: Option<String>,
        limit: Option<String>,
        about: Option<String>,
        memory: Option<String>,
    ) -> Result<String, JsError> {
        let id = ConversationId::new(id).map_err(|e| refused(e.to_string()))?;
        let window = Window {
            before: query::turn_before(before.as_deref()).map_err(|r| said(&r))?,
            limit: query::turn_limit(limit.as_deref(), DEFAULT_TURN_WINDOW)
                .map_err(|r| said(&r))?,
            about: query::about(about.as_deref()).map_err(|r| said(&r))?,
        };
        let memory = memory_of(memory)?;
        let stored = self
            .0
            .iter()
            .find(|stored| stored.id == id)
            .ok_or_else(|| refused(format!("no conversation named {id}")))?;
        let dress = Dress {
            memory: memory.as_ref(),
            ..Dress::BARE
        };
        json(&readings::conversation(
            stored,
            &inbox.fold()?,
            &window,
            &dress,
        ))
    }

    /// Every conversation's turns holding `q`, newest first: only words this reading opened.
    pub fn search(&self, q: &str) -> Result<String, JsError> {
        json(&readings::search(&self.0, q).map_err(|r| said(&r))?)
    }
}

/// What an erasure does, as the box's card says it before the owner confirms one.
#[wasm_bindgen]
#[must_use]
pub fn erase_warning() -> String {
    suzatary_core::secretary::erasure::Erasure::FOR_GOOD.to_owned()
}

/// The keys a pull gave, parsed once.
fn ring(keys: &str) -> Result<KeyRing, JsError> {
    serde_json::from_str(keys)
        .map(|given| KeyRing::of(&given))
        .map_err(|e| refused(format!("keys: {e}")))
}

/// The memory as its route answers it, parsed once.
fn memory_of(memory: Option<String>) -> Result<Option<MemoryPayload>, JsError> {
    memory
        .map(|text| serde_json::from_str(&text))
        .transpose()
        .map_err(|e| refused(format!("memory: {e}")))
}

impl Proposals {
    fn fold(&self) -> Result<Fold, JsError> {
        let read = self.0.read().map_err(refused)?;
        Ok(Fold::of(read.nodes))
    }
}

impl Conversations {
    /// Every conversation, newest first, as the box's list reads them.
    fn stored(&self, keys: &KeyRing) -> Result<Vec<Stored>, JsError> {
        let read = self.0.read().map_err(refused)?;
        Ok(transcript::transcripts(
            read.nodes,
            &|name: &suzatary_core::secretary::locked_text::KeyName| keys.key(name),
        )
        .map_err(|e| refused(e.to_string()))?
        .into_values()
        .rev()
        .map(transcript::Transcript::into_stored)
        .collect())
    }
}

fn said(refusal: &suzatary_core::refusal::Refusal) -> JsError {
    refused(refusal.message())
}
