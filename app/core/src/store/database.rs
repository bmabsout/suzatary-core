use std::collections::BTreeMap;

use prodrome_core::declared::Declared;

use super::{Doc, Log, Prodrome, Replica};
use crate::attachment::AttachmentId;
use crate::chains::ChainName;
use crate::decisions::DecisionEvent;
use crate::devices::Device;
use crate::mail::local_state::LocalState;
use crate::mail::state::TriageState;
use crate::plugin::PluginName;
use crate::record::TodoEvent;
use crate::secretary::digest_event::DigestEvent;
use crate::secretary::digest_line::Line;
use crate::secretary::inbox_event::InboxEvent;
use crate::secretary::line_event::LineEvent;
use crate::secretary::locked_text::{KeyName, TextKey};
use crate::secretary::memory::Fact;
use crate::secretary::memory_event::MemoryEvent;
use crate::secretary::salted_text::{Text, TextName};
use crate::upload::Upload;

/// The whole store, as one value: every chain beside the agent's replica of it, and the box's
/// own stores.
#[derive(Clone)]
pub struct Database<'a> {
    /// Each chain by name, in the instance file's order of meaning.
    pub chains: BTreeMap<ChainName, Area<'a>>,
    /// The proposals a person taps.
    pub inbox: &'a dyn Prodrome<InboxEvent>,
    /// The secretary's conversations, one prodrome keyed by conversation, every word locked.
    pub conversations: &'a dyn Prodrome<LineEvent>,
    /// The key of each slot of the conversations' words not erased: box-local, vaulted, never
    /// pushed, and destroyed by the lines' sweep.
    pub line_keys: &'a dyn Doc<KeyName, TextKey>,
    /// The secretary's memory: which facts were remembered and forgotten, by id.
    pub memory: &'a dyn Prodrome<MemoryEvent>,
    /// Each standing fact's words, by the name its remembering gives them; forgetting deletes.
    pub memory_texts: &'a dyn Doc<TextName, Text<Fact>>,
    /// The digest: what was said of each node of the conversations' summary tree, by node.
    pub digests: &'a dyn Prodrome<DigestEvent>,
    /// Each summary's words, by the name its event gives them; the sweep deletes the dead.
    pub digest_texts: &'a dyn Doc<TextName, Text<Line>>,
    /// The recorded decisions: designs and plans as items, keyed by decision.
    pub decisions: &'a dyn Prodrome<DecisionEvent>,
    /// What each plugin learned, by its name: a cache-tier replica at the plugin's declared
    /// schema, rebuilt by syncing again.
    pub plugins: BTreeMap<PluginName, &'a dyn Prodrome<Declared>>,
    /// The triage's dedup authority, pushed: decisions under keyed names.
    pub triage: &'a dyn Doc<(), TriageState>,
    /// What the box keeps of its mail beside the decisions, box-local and never pushed.
    pub triage_local: &'a dyn Doc<(), LocalState>,
    /// Each actor's phones.
    pub devices: &'a dyn Log<String, Device>,
    /// The owner's uploads.
    pub uploads: &'a dyn Doc<AttachmentId, Upload>,
    /// The files a verb kept for the model.
    pub kept: &'a dyn Doc<AttachmentId, Vec<u8>>,
}

/// One chain and the agent's replica of it.
#[derive(Clone, Copy)]
pub struct Area<'a> {
    /// The chain.
    pub events: &'a dyn Prodrome<TodoEvent>,
    /// The agent's batches against it.
    pub proposals: &'a dyn Replica<TodoEvent>,
}

impl Database<'_> {
    /// Every prodrome's fsck findings, named `<chain> chain`, `<chain> proposals`, `inbox`,
    /// `conversations`, `memory`, `digests`, `decisions` and `plugin <name>`.
    #[must_use]
    pub fn verify(&self) -> Vec<(String, Vec<String>)> {
        self.chains
            .iter()
            .flat_map(|(name, area)| {
                [
                    (format!("{name} chain"), area.events.verify()),
                    (format!("{name} proposals"), area.proposals.verify()),
                ]
            })
            .chain([
                (
                    "inbox".to_owned(),
                    crate::secretary::inbox::findings(self.inbox),
                ),
                (
                    "conversations".to_owned(),
                    crate::secretary::transcript::findings(self.conversations, self.inbox),
                ),
                ("memory".to_owned(), self.memory.verify()),
                ("digests".to_owned(), self.digests.verify()),
                ("decisions".to_owned(), self.decisions.verify()),
            ])
            .chain(
                self.plugins
                    .iter()
                    .map(|(name, store)| (format!("plugin {name}"), store.verify())),
            )
            .collect()
    }
}
