//! `suzatary secretary migrate-receipts [--dry]`: carry every tool line of the conversations into
//! the shape that names how its call ended, once.
//!
//! Before this, a tool line's shape held `refused: bool`, and the fact a `remember` call wrote
//! was read back out of the answer's words, so it went when the owner erased them. The shape now
//! holds an `Outcome` (`Answered`, `Refused`, or `Remembered { fact }`), so this run reads the old
//! flag once, and for each `remember` answer the fact from its words, or from the forget that
//! erased them. A `remember` answer the owner erased names no fact any more: it is carried as
//! answered, and the run says its receipt is lost.
//!
//! A line's words are locked under a binding that holds its whole print but the words, so a line
//! whose shape or provenance changes is locked again: each slot is opened with its key under the
//! old binding and locked under a fresh key bound to the new line. A slot whose key is gone, or
//! named by an erasure the sweep has not yet acted on, cannot or must not be opened, and is
//! carried as stored. The rewrite is the store's image under its own genesis
//! (`suzatary_core::store::image`), so every line resting on a carried one is sealed again too;
//! each such line names the object it replaced (`Carried::From`), which the digest reads as its
//! leaf, so every summary keeps its node, and the turn order and every proposal's id stay.
//!
//! A run writes each new object's keys and then the object, parents first, then removes the old
//! keys it replaced, then the old objects children first. A run stopped part way is finished by
//! running it again: an object whose `Carried::From` names an object still in the store is that
//! object's image, written by the stopped run, and the next run takes it as the image instead of
//! drawing new keys. A run stopped between an object's keys and the object leaves keys no line
//! names, which open nothing, as the store's own writer may (`transcript::write`). A store with
//! nothing to carry writes nothing.
//! This module is the one reader of the old shape and goes with the verb once the box has run it
//! (`docs/MIGRATE-RECEIPTS-2026-10-20.md`).

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context as _;
use prodrome_core::event::{Envelope, EventVocabulary, Hash};
use prodrome_core::literal::{Call, Value, parse_literal, print_literal};
use prodrome_core::schema::Schema as _;
use serde::Deserialize;
use suzatary_core::capability::Entropy;
use suzatary_core::literal::Literal as _;
use suzatary_core::secretary::SecretaryVerb;
use suzatary_core::secretary::carried::Carried;
use suzatary_core::secretary::erasure::Erasure;
use suzatary_core::secretary::line_event::{Carry, LineEvent, LineSlot, LineVocabulary};
use suzatary_core::secretary::line_words::LineWords;
use suzatary_core::secretary::locked_text::{KeyName, Locked, Shape, TextKey};
use suzatary_core::secretary::memory::FactId;
use suzatary_core::secretary::outcome::Outcome;
use suzatary_core::secretary::store::ConversationId;
use suzatary_core::secretary::tool_name::ToolName;
use suzatary_core::secretary::turn_shape::{FileShape, TurnShape};
use suzatary_core::store::{Doc as _, Prodrome as _};

use crate::instance::Instance;

/// A tool line's shape as it was stored before this run: the flag, never the outcome.
#[derive(Deserialize)]
#[serde(tag = "who", rename_all = "lowercase")]
enum Old {
    Tool {
        verb: ToolName,
        refused: bool,
        files: Vec<FileShape>,
    },
}

/// What the run made of one tool line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Read {
    /// Its outcome, from its flag and, for a `remember` answer, its words or its forget.
    Outcome(Outcome),
    /// A `remember` answer whose words no longer open and no forget erased: the owner erased
    /// them, or their key is gone. No fact is left to name, so it is carried as answered, without
    /// a receipt.
    ReceiptLost,
}

/// What a run does to the conversations, in the order it must.
pub struct Plan {
    /// Each tool line carried, by the name of the object that held it.
    pub tools: Vec<(Hash, Read)>,
    /// Each slot carried as stored, by the line that held it and the slot's number.
    pub sealed: Vec<(Hash, usize)>,
    /// The objects to write, parents first, each with the keys that open its words.
    pub objects: Vec<(Hash, String, Vec<TextKey>)>,
    /// The keys of the slots locked again, which nothing opens once the run is done.
    pub unkeyed: Vec<KeyName>,
    /// The objects replaced, children first, so the store never holds one whose parent is gone.
    pub removed: Vec<Hash>,
}

impl Plan {
    /// Whether the run writes or removes anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty() && self.removed.is_empty()
    }
}

/// One slot's binding as it was stored: the old line's print without its words, and the slot's
/// number, under the conversations' own domain.
struct Bound(Vec<u8>);

impl Shape for Bound {
    type Words = LineWords;
    const DOMAIN: &'static [u8] = <LineSlot as Shape>::DOMAIN;
    fn print(&self) -> Vec<u8> {
        self.0.clone()
    }
}

/// One stored object, read as a literal: its call, and the names it rests on.
struct Stored {
    call: Call,
    deps: Vec<Hash>,
}

impl Stored {
    /// Its line's literal, when it is a change.
    fn event(&self) -> Option<&Call> {
        (self.call.name == "Change")
            .then(|| self.call.field("event").and_then(Value::as_call))
            .flatten()
    }
}

/// `text` read as a literal of the conversations' envelopes, past the lines' reader, which
/// refuses a tool line in the old shape.
fn stored(name: &Hash, text: &str) -> anyhow::Result<Stored> {
    let words = EventVocabulary::<LineEvent>::new(&LineVocabulary);
    let Value::Call(call) =
        parse_literal(text, &words).with_context(|| format!("object {name} does not read"))?
    else {
        anyhow::bail!("object {name} is not a constructor call");
    };
    let deps = match call.field("deps") {
        Some(Value::Tuple(deps)) => deps
            .iter()
            .map(|dep| {
                dep.as_str()
                    .and_then(|dep| Hash::new(dep).ok())
                    .with_context(|| format!("object {name} rests on a non-name"))
            })
            .collect::<anyhow::Result<_>>()?,
        _ => Vec::new(),
    };
    Ok(Stored { call, deps })
}

/// The old shape of `event`, a tool line written before this run; `None` for any other line.
fn old(event: &Call) -> Option<Old> {
    if event.name != "Said" {
        return None;
    }
    serde_json::from_str(event.field("turn")?.as_str()?).ok()
}

/// The object `event` was carried from, when it names one this store still holds: the line it is
/// the image of, written by a run that stopped.
fn imaged<'h>(event: &Call, held: &'h BTreeMap<Hash, Stored>) -> Option<&'h Hash> {
    let from = Carried::from_value(event.field("carried")?).ok()?;
    match from {
        Carried::From(object) => held.get_key_value(&object).map(|(name, _)| name),
        Carried::Written => None,
    }
}

/// `event`'s binding for slot `slot`: its print without `words`, with the slot's number.
fn binding(event: &Call, slot: usize) -> Bound {
    let fields = event
        .fields
        .iter()
        .filter(|(name, _)| name != "words")
        .cloned()
        .chain([("slot".to_owned(), slot.to_value())])
        .collect();
    Bound(print_literal(&Value::call(event.name.clone(), fields)).into_bytes())
}

/// The locked slots `event` holds.
fn slots(event: &Call) -> anyhow::Result<Vec<Locked<LineSlot>>> {
    event.field("words").map_or_else(
        || Ok(Vec::new()),
        |words| Vec::<Locked<LineSlot>>::from_value(words).map_err(anyhow::Error::from),
    )
}

/// Every erasure each conversation's lines hold, by the key it names.
fn erasures(held: &BTreeMap<Hash, Stored>) -> BTreeMap<ConversationId, BTreeMap<KeyName, Erasure>> {
    let mut lines: BTreeMap<ConversationId, Vec<(&Hash, LineEvent)>> = BTreeMap::new();
    for (name, object) in held {
        let Some(event) = object.event() else {
            continue;
        };
        // A tool line in the old shape does not read, and is no erasure.
        if let Ok(line) = LineEvent::from_value(&LineVocabulary, &Value::Call(event.clone())) {
            lines
                .entry(line.conversation().clone())
                .or_default()
                .push((name, line));
        }
    }
    lines
        .into_iter()
        .map(|(id, lines)| {
            let named = suzatary_core::secretary::transcript::erasures(
                lines.iter().map(|(name, line)| (*name, line)),
            );
            (id, named)
        })
        .collect()
}

/// What a tool line in the old shape came to: its flag, and for a `remember` answer the fact its
/// words name, or the fact the forget that erased them names.
fn outcome(old: &Old, words: Option<&LineWords>, erased: Option<&Erasure>) -> Read {
    let Old::Tool { verb, refused, .. } = old;
    if *refused {
        return Read::Outcome(Outcome::Refused);
    }
    if verb.verb() != Some(SecretaryVerb::Remember) {
        return Read::Outcome(Outcome::Answered);
    }
    match (words, erased) {
        // A card's answer names the card it filed, and no fact.
        (Some(LineWords::Tool { answer, .. }), _) => Read::Outcome(Outcome::answered(
            serde_json::from_str::<serde_json::Value>(answer.as_str())
                .ok()
                .and_then(|answered| answered.get("remembered")?.as_u64())
                .map(FactId),
        )),
        (_, Some(Erasure::Forgot { fact })) => Read::Outcome(Outcome::Remembered { fact: *fact }),
        (Some(_), _) | (None, Some(Erasure::Asked { .. }) | None) => Read::ReceiptLost,
    }
}

/// One line carried: its image's line, the keys that open it, and what the run read of it.
struct Carrying {
    line: LineEvent,
    keys: Vec<TextKey>,
    tool: Option<Read>,
    sealed: Vec<usize>,
}

/// The line `event`, stored as `name`, carried: each slot opened with its key under the old
/// binding and locked again, or kept as stored where `named` erases its key or no key is left,
/// and a tool line's shape carried into its outcome.
fn carry(
    name: &Hash,
    event: &Call,
    named: Option<&BTreeMap<KeyName, Erasure>>,
    key: &dyn Fn(&KeyName) -> anyhow::Result<Option<TextKey>>,
    entropy: &dyn Entropy,
) -> anyhow::Result<Carrying> {
    let stored = slots(event).with_context(|| format!("line {name}'s words"))?;
    let mut opened = Vec::with_capacity(stored.len());
    for (slot, locked) in stored.iter().enumerate() {
        let erased = named.is_some_and(|named| named.contains_key(locked.key()));
        opened.push(match key(locked.key())? {
            Some(key) if !erased => Some(
                Locked::<Bound>::read(locked.key().clone(), locked.sealed().to_vec())
                    .open(&binding(event, slot), &key)
                    .with_context(|| {
                        format!("slot {slot} of line {name} does not open with its key")
                    })?,
            ),
            Some(_) | None => None,
        });
    }
    let mut event = event.clone();
    let tool = old(&event)
        .map(|old| {
            let read = outcome(
                &old,
                opened.first().and_then(Option::as_ref),
                stored.first().and_then(|locked| named?.get(locked.key())),
            );
            let Old::Tool { verb, files, .. } = old;
            let shape = TurnShape::Tool {
                verb,
                outcome: match read {
                    Read::Outcome(outcome) => outcome,
                    Read::ReceiptLost => Outcome::Answered,
                },
                files,
            };
            let turn = Value::str(serde_json::to_string(&shape)?);
            for (field, value) in &mut event.fields {
                if field == "turn" {
                    *value = turn.clone();
                }
            }
            anyhow::Ok(read)
        })
        .transpose()?;
    let template = LineEvent::from_value(&LineVocabulary, &Value::Call(event))
        .with_context(|| format!("line {name} does not read once carried"))?;
    let from = match template.carried() {
        Carried::From(object) => object.clone(),
        Carried::Written => name.clone(),
    };
    let (line, keys) = template.carried_as(
        Carried::From(from),
        |slot, _| opened[slot].clone().map_or(Carry::Sealed, Carry::Words),
        entropy,
    )?;
    Ok(Carrying {
        line,
        keys,
        tool,
        sealed: (0..opened.len())
            .filter(|slot| opened[*slot].is_none())
            .collect(),
    })
}

/// The lines a run carries: every tool line in the old shape, every line a stopped run already
/// imaged, and everything resting on one, since its image rests on the image; and each image a
/// stopped run wrote, by the line it is the image of.
fn carried(held: &BTreeMap<Hash, Stored>) -> (BTreeSet<&Hash>, BTreeMap<&Hash, &Hash>) {
    let images: BTreeMap<&Hash, &Hash> = held
        .iter()
        .filter_map(|(name, object)| Some((imaged(object.event()?, held)?, name)))
        .collect();
    let mut carried: BTreeSet<&Hash> = held
        .iter()
        .filter(|(_, object)| object.event().and_then(old).is_some())
        .map(|(name, _)| name)
        .chain(images.keys().copied())
        .collect();
    loop {
        let more: Vec<&Hash> = held
            .iter()
            .filter(|(name, object)| {
                !carried.contains(name)
                    && !images.values().any(|image| *image == *name)
                    && object.deps.iter().any(|dep| carried.contains(dep))
            })
            .map(|(name, _)| name)
            .collect();
        if more.is_empty() {
            return (carried, images);
        }
        carried.extend(more);
    }
}

/// `carried` children first: each pass takes every line no line still left rests on, so the
/// store never holds one whose parent is gone.
fn children_first(
    mut left: BTreeSet<&Hash>,
    held: &BTreeMap<Hash, Stored>,
) -> anyhow::Result<Vec<Hash>> {
    let mut removed = Vec::with_capacity(left.len());
    while !left.is_empty() {
        let leaves: Vec<&Hash> = left
            .iter()
            .copied()
            .filter(|name| {
                !left
                    .iter()
                    .any(|other| held[*other].deps.iter().any(|dep| dep == *name))
            })
            .collect();
        anyhow::ensure!(!leaves.is_empty(), "the carried lines rest on each other");
        for leaf in leaves {
            left.remove(leaf);
            removed.push(leaf.clone());
        }
    }
    Ok(removed)
}

/// The genesis `history` begins with.
fn genesis_of(
    history: &[(Hash, String)],
    held: &BTreeMap<Hash, Stored>,
) -> anyhow::Result<prodrome_core::genesis::Genesis> {
    let Some((first, text)) = history
        .iter()
        .find(|(name, _)| held[name].call.name == "Genesis")
    else {
        anyhow::bail!("the conversations hold no genesis, so they are no prodrome of changes");
    };
    let Envelope::Genesis(genesis) =
        prodrome_core::event::parse_envelope::<LineEvent>(&LineVocabulary, text)?
    else {
        anyhow::bail!("object {first} is no genesis");
    };
    Ok(genesis)
}

/// What a run does to `history`, the conversations' objects as stored, each its name and its
/// text; `key` reads a slot's key by name, and new keys are drawn from `entropy`. Nothing to
/// write for a store with nothing to carry.
///
/// # Errors
/// A store that is not one prodrome of changes, or a slot whose key does not open it.
pub fn plan(
    history: &[(Hash, String)],
    key: &dyn Fn(&KeyName) -> anyhow::Result<Option<TextKey>>,
    entropy: &dyn Entropy,
) -> anyhow::Result<Plan> {
    let held: BTreeMap<Hash, Stored> = history
        .iter()
        .map(|(name, text)| Ok((name.clone(), stored(name, text)?)))
        .collect::<anyhow::Result<_>>()?;
    let (carried, images) = carried(&held);
    let mut plan = Plan {
        tools: Vec::new(),
        sealed: Vec::new(),
        objects: Vec::new(),
        unkeyed: Vec::new(),
        removed: Vec::new(),
    };
    if carried.is_empty() {
        return Ok(plan);
    }
    let erased = erasures(&held);
    let mut events: BTreeMap<Hash, (Value, Vec<TextKey>)> = BTreeMap::new();
    for name in &carried {
        let event = held[*name]
            .event()
            .with_context(|| format!("object {name} rests on a line and is no line"))?;
        let carried = if let Some(image) = images.get(name) {
            let line = held[*image]
                .event()
                .with_context(|| format!("object {image} is no line"))?;
            Carrying {
                line: LineEvent::from_value(&LineVocabulary, &Value::Call(line.clone()))
                    .with_context(|| format!("line {image} does not read"))?,
                keys: Vec::new(),
                tool: None,
                sealed: Vec::new(),
            }
        } else {
            let conversation = ConversationId::from_value(
                event
                    .field("conversation")
                    .with_context(|| format!("line {name} names no conversation"))?,
            )?;
            carry(name, event, erased.get(&conversation), key, entropy)?
        };
        let kept: BTreeSet<&KeyName> = carried.line.keys().collect();
        plan.unkeyed.extend(
            slots(event)?
                .iter()
                .map(Locked::key)
                .filter(|old| !kept.contains(old))
                .cloned(),
        );
        plan.tools
            .extend(carried.tool.map(|read| ((*name).clone(), read)));
        plan.sealed
            .extend(carried.sealed.iter().map(|slot| ((*name).clone(), *slot)));
        events.insert((*name).clone(), (carried.line.to_value(), carried.keys));
    }
    let image = suzatary_core::store::image::<LineEvent>(
        &LineVocabulary,
        history.iter().cloned(),
        &genesis_of(history, &held)?,
        |name, event| {
            Ok(events
                .get(name)
                .map_or_else(|| event.clone(), |(value, _)| value.clone()))
        },
    )
    .context("the conversations have no image")?;
    let mut keys: BTreeMap<&Hash, Vec<TextKey>> = events
        .iter_mut()
        .map(|(name, (_, keys))| (&image.names[name], std::mem::take(keys)))
        .collect();
    let mut written = BTreeSet::new();
    plan.objects = image
        .objects
        .iter()
        .filter(|(name, _)| !held.contains_key(name) && written.insert(name.clone()))
        .map(|(name, text)| {
            (
                name.clone(),
                text.clone(),
                keys.remove(name).unwrap_or_default(),
            )
        })
        .collect();
    plan.removed = children_first(carried, &held)?;
    Ok(plan)
}

/// Every object of the store at `root`, its name and its text, read off the files and not
/// through the lines' reader.
fn history(root: &std::path::Path) -> anyhow::Result<Vec<(Hash, String)>> {
    let dir = root.join("objects");
    let mut history = Vec::new();
    // Conversations never written to hold nothing to carry.
    let entries = match std::fs::read_dir(&dir) {
        Err(failed) if failed.kind() == std::io::ErrorKind::NotFound => return Ok(history),
        read => read.with_context(|| format!("reading {}", dir.display()))?,
    };
    for entry in entries {
        let path = entry?.path();
        let Some(name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".py"))
        else {
            continue;
        };
        let name = Hash::new(name).with_context(|| format!("{} is no object", path.display()))?;
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        history.push((name, text));
    }
    history.sort();
    Ok(history)
}

/// Refuse while the conversations still hold a tool line in the old shape, or a stopped run's
/// half, naming the verb to run: what `suzatary serve` asks before it starts, so this build never
/// serves conversations it cannot read. Goes with the verb.
///
/// An object that does not read as a literal at all is left to the lines' own reading.
///
/// # Errors
/// Conversations with something to carry, or ones whose objects cannot be listed.
pub fn uncarried(instance: &Instance) -> anyhow::Result<()> {
    let stores = instance.conversation_stores();
    let root = stores.lines.inner().root().to_path_buf();
    let held: BTreeMap<Hash, Stored> = history(&root)?
        .iter()
        .filter_map(|(name, text)| Some((name.clone(), stored(name, text).ok()?)))
        .collect();
    let uncarried = held
        .values()
        .filter_map(Stored::event)
        .filter(|event| old(event).is_some() || imaged(event, &held).is_some())
        .count();
    anyhow::ensure!(
        uncarried == 0,
        "the conversations at {} hold {uncarried} line(s) this build does not read, or a stopped \
         carry's: stop the timers and run `suzatary secretary migrate-receipts` first, as its \
         run order MIGRATE-RECEIPTS-2026-10-20.md says",
        root.display()
    );
    Ok(())
}

/// `suzatary secretary migrate-receipts [--dry]`: `dry` writes nothing and says what a run would.
/// The run order stops the server and the timers first; a server of this build refuses to start
/// until the run is done ([`uncarried`]). Public so the CLI's tests drive it.
///
/// # Errors
/// A store that is not one prodrome of changes, a slot whose key does not open it, one that does
/// not verify after, and the stores' own.
pub fn run(instance: &Instance, dry: bool) -> anyhow::Result<()> {
    let stores = instance.conversation_stores();
    let held = stores.lines.locked()?;
    let root = stores.lines.inner().root().to_path_buf();
    let plan = plan(
        &history(&root)?,
        &|name| Ok(stores.keys.get(name)?),
        &suzatary_io::texts::SystemEntropy,
    )?;
    for (name, read) in &plan.tools {
        match read {
            Read::Outcome(Outcome::Answered) => println!("answered: {name}"),
            Read::Outcome(Outcome::Refused) => println!("refused: {name}"),
            Read::Outcome(Outcome::Remembered { fact }) => {
                println!("remembered fact {fact}: {name}");
            }
            Read::ReceiptLost => println!(
                "receipt lost: {name}: a remember answer whose words are gone, and no forget \
                 erased them, names no fact"
            ),
        }
    }
    for (name, slot) in &plan.sealed {
        println!("carried as stored: {name} slot {slot}: its words are erased or have no key");
    }
    println!(
        "conversations: {} tool lines carried, {} objects written, {} removed, {} keys replaced",
        plan.tools.len(),
        plan.objects.len(),
        plan.removed.len(),
        plan.unkeyed.len()
    );
    if dry || plan.is_empty() {
        println!("conversations: nothing written");
        return Ok(());
    }
    let objects = root.join("objects");
    for (name, text, keys) in &plan.objects {
        for key in keys {
            stores.keys.set(&key.name(), key)?;
        }
        let path = objects.join(format!("{name}.py"));
        suzatary_io::durable::replace(&path, text.as_bytes(), suzatary_io::durable::Mode::Readable)
            .with_context(|| format!("writing {}", path.display()))?;
    }
    for key in &plan.unkeyed {
        stores.keys.remove(key)?;
    }
    for name in &plan.removed {
        let path = objects.join(format!("{name}.py"));
        suzatary_io::durable::remove(&path)
            .with_context(|| format!("removing {}", path.display()))?;
        println!("removed: {}", path.display());
    }
    drop(held);
    let stores = instance.conversation_stores();
    let findings =
        suzatary_core::secretary::transcript::findings(&stores.lines, &instance.inbox_store());
    for finding in &findings {
        println!("conversations: {finding}");
    }
    anyhow::ensure!(
        findings.is_empty(),
        "the conversations do not verify after the run"
    );
    println!("conversations: carried, and they verify");
    Ok(())
}
