//! `suzatary secretary migrate-inbox [--dry]`: carry the inbox's stored withdrawals and prices
//! into the forms the program reads, once.
//!
//! Before this, a card the box or the secretary took back was stored as a `Dismissed` like the
//! owner's no, told apart only by its words: `superseded by proposal <n>` for a card a later one
//! replaced, and any other reason written as the secretary (`assistant`) for a card nothing
//! needed any more. The inbox now stores `Superseded` and `NoLongerNeeded(why)`, so this run
//! reads those words once, here, and nowhere else does. An owner's `Dismissed` is kept as it is.
//!
//! An asking that served a todo could also carry a spec of its own, which nothing read since a
//! proposal is priced by the todo it serves. The inbox now refuses that pair, so this run drops
//! the spec of such an asking.
//!
//! The rewrite is the store's image under its own genesis (`suzatary_core::store::image`): every
//! other object keeps its bytes and its name, and every object resting on a rewritten one is
//! sealed again with its new deps. The new objects are written first, then the old ones removed
//! children first, so a run stopped part way is finished by running it again. A store with
//! nothing to carry writes nothing. This module is the one reader of the old forms and goes with
//! the verb once the box has run it (`docs/MIGRATE-INBOX-2026-10-19.md`).

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context as _;
use prodrome_core::event::{Envelope, Hash};
use prodrome_core::literal::{Value, print_literal};
use suzatary_core::secretary::inbox_event::{InboxEvent, InboxVocabulary};
use suzatary_core::store::Prodrome as _;

use crate::instance::Instance;

/// The words every dismissal that superseded a card began with, followed by the new card's
/// index.
const SUPERSEDED: &str = "superseded by proposal ";

/// The words the box put before its own reason when it withdrew a card a tap could never
/// perform.
const THE_BOX_WITHDREW: &str = "the box withdrew it: ";

/// What the run carries one stored event into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carried {
    /// A dismissal that named the card replacing it, now `Superseded`.
    Superseded,
    /// A dismissal the secretary wrote, now `NoLongerNeeded` with its words.
    NoLongerNeeded {
        /// The words it keeps.
        why: String,
    },
    /// An asking that served a todo and carried a spec of its own, now without the spec.
    SpecDropped,
}

/// What a run does to the inbox, in the order it must.
#[derive(Debug)]
pub struct Plan {
    /// Each event carried, by the name of the object that held it, with what it became.
    pub carried: Vec<(Hash, Carried)>,
    /// The objects to write, parents first.
    pub objects: Vec<(Hash, String)>,
    /// The objects replaced, children first, so the store never holds one whose parent is gone.
    pub removed: Vec<Hash>,
}

/// What `event`, a stored event's literal, is carried into, and how; `None` for one that stays.
fn carry(event: &Value, secretary: &str) -> Option<(Value, Carried)> {
    let Value::Call(call) = event else {
        return None;
    };
    let field = |name: &str| call.field(name).cloned().unwrap_or(Value::None);
    match call.name.as_str() {
        "Dismissed" => {
            let reason = call.field("reason")?.as_str()?;
            let head = vec![
                ("proposal".to_owned(), field("proposal")),
                ("at".to_owned(), field("at")),
                ("actor".to_owned(), field("actor")),
            ];
            // Only the secretary and the box wrote as `secretary`; anyone else's dismissal is a
            // person's no, whatever its words.
            if call.field("actor")?.as_str()? != secretary {
                return None;
            }
            if reason
                .strip_prefix(SUPERSEDED)
                .is_some_and(|index| index.parse::<usize>().is_ok())
            {
                return Some((Value::call("Superseded", head), Carried::Superseded));
            }
            let why = reason.strip_prefix(THE_BOX_WITHDREW).unwrap_or(reason);
            let mut fields = head;
            fields.push(("why".to_owned(), Value::str(why)));
            Some((
                Value::call("NoLongerNeeded", fields),
                Carried::NoLongerNeeded {
                    why: why.to_owned(),
                },
            ))
        }
        "Asked" | "Noticed" | "Offered"
            if !matches!(call.field("serves"), None | Some(Value::None))
                && !matches!(call.field("spec"), None | Some(Value::None)) =>
        {
            let fields = call
                .fields
                .iter()
                .map(|(name, value)| {
                    let value = if name == "spec" {
                        Value::None
                    } else {
                        value.clone()
                    };
                    (name.clone(), value)
                })
                .collect();
            Some((Value::call(call.name.clone(), fields), Carried::SpecDropped))
        }
        _ => None,
    }
}

/// One stored object, read as a literal: its call, and the names it rests on.
struct Stored {
    call: prodrome_core::literal::Call,
    deps: Vec<Hash>,
}

/// `text` read as a literal of the inbox's envelopes, without asking the inbox's reader, which
/// refuses an asking in the old form.
fn stored(name: &Hash, text: &str) -> anyhow::Result<Stored> {
    let words = prodrome_core::event::EventVocabulary::<InboxEvent>::new(&InboxVocabulary);
    let Value::Call(call) = prodrome_core::literal::parse_literal(text, &words)
        .with_context(|| format!("object {name} does not read"))?
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

/// What a run does to `history`, the inbox's objects as stored, each its name and its text;
/// `secretary` is the actor the secretary and the box write as. Nothing to write for a store
/// with nothing to carry.
///
/// # Errors
/// A store that is not one prodrome of changes.
pub fn plan(history: &[(Hash, String)], secretary: &str) -> anyhow::Result<Plan> {
    let read: BTreeMap<&Hash, Stored> = history
        .iter()
        .map(|(name, text)| Ok((name, stored(name, text)?)))
        .collect::<anyhow::Result<_>>()?;
    let mut carried = Vec::new();
    let mut by_print: BTreeMap<String, Value> = BTreeMap::new();
    for (name, object) in &read {
        let Some(event) = object
            .call
            .field("event")
            .filter(|_| object.call.name == "Change")
        else {
            continue;
        };
        if let Some((value, how)) = carry(event, secretary) {
            by_print.insert(print_literal(event), value);
            carried.push(((*name).clone(), how));
        }
    }
    if carried.is_empty() {
        return Ok(Plan {
            carried,
            objects: Vec::new(),
            removed: Vec::new(),
        });
    }
    let Some((first, text)) = history
        .iter()
        .find(|(name, _)| read[name].call.name == "Genesis")
    else {
        anyhow::bail!("the inbox holds no genesis, so it is no prodrome of changes");
    };
    let Envelope::Genesis(genesis) =
        prodrome_core::event::parse_envelope::<InboxEvent>(&InboxVocabulary, text)?
    else {
        anyhow::bail!("object {first} is no genesis");
    };
    let image = suzatary_core::store::image::<InboxEvent>(
        &InboxVocabulary,
        history.iter().cloned(),
        &genesis,
        |event| {
            Ok(by_print
                .get(&print_literal(event))
                .cloned()
                .unwrap_or_else(|| event.clone()))
        },
    )
    .context("the inbox has no image")?;
    let mut written = BTreeSet::new();
    let objects: Vec<(Hash, String)> = image
        .objects
        .into_iter()
        .filter(|(name, _)| !read.contains_key(name) && written.insert(name.clone()))
        .collect();
    // Children first: each pass takes every replaced object that no replaced object still left
    // rests on.
    let mut left: BTreeSet<&Hash> = read
        .keys()
        .copied()
        .filter(|name| image.names.get(*name) != Some(*name))
        .collect();
    let mut removed = Vec::with_capacity(left.len());
    while !left.is_empty() {
        let leaves: Vec<&Hash> = left
            .iter()
            .copied()
            .filter(|name| {
                !left
                    .iter()
                    .any(|other| read[*other].deps.iter().any(|dep| dep == *name))
            })
            .collect();
        anyhow::ensure!(
            !leaves.is_empty(),
            "the replaced objects rest on each other"
        );
        for leaf in leaves {
            left.remove(leaf);
            removed.push(leaf.clone());
        }
    }
    Ok(Plan {
        carried,
        objects,
        removed,
    })
}

/// Every object of the store at `root`, its name and its text, read off the files and not
/// through the inbox's reader.
fn history(root: &std::path::Path) -> anyhow::Result<Vec<(Hash, String)>> {
    let dir = root.join("objects");
    let mut history = Vec::new();
    // An inbox never written to holds nothing to carry.
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

/// Refuse while the inbox still holds an event this run would carry, naming the verb to run:
/// what `suzatary serve` asks before it starts, so this build never serves an inbox it reads
/// wrong (an old asking left out, a box withdrawal read as the owner's no). Goes with the verb.
///
/// An object that does not read as a literal at all is left to the inbox's own reading, which
/// leaves it out, as it does today.
///
/// # Errors
/// An inbox with something to carry, or one whose objects cannot be listed.
pub fn uncarried(instance: &Instance) -> anyhow::Result<()> {
    let root = instance.inbox_store().inner().root().to_path_buf();
    let secretary = suzatary_core::secretary::WRITES_AS.actor();
    let carried = history(&root)?
        .iter()
        .filter_map(|(name, text)| stored(name, text).ok())
        .filter(|object| {
            object.call.name == "Change"
                && object
                    .call
                    .field("event")
                    .is_some_and(|event| carry(event, secretary).is_some())
        })
        .count();
    anyhow::ensure!(
        carried == 0,
        "the inbox at {} holds {carried} event(s) in a form this build no longer reads: stop the \
         timers and run `suzatary secretary migrate-inbox` first, as its run order \
         MIGRATE-INBOX-2026-10-19.md says",
        root.display()
    );
    Ok(())
}

/// `suzatary secretary migrate-inbox [--dry]`: `dry` writes nothing and says what a run would.
/// The run order stops the server and the timers first; a server of this build refuses to start
/// until the run is done ([`uncarried`]). Public so the CLI's tests drive it.
///
/// # Errors
/// A store that is not one prodrome of changes, one that does not verify after, and the store's
/// own.
pub fn run(instance: &Instance, dry: bool) -> anyhow::Result<()> {
    let store = instance.inbox_store();
    let held = store.locked()?;
    let root = store.inner().root().to_path_buf();
    let plan = plan(
        &history(&root)?,
        suzatary_core::secretary::WRITES_AS.actor(),
    )?;
    for (name, carried) in &plan.carried {
        match carried {
            Carried::Superseded => println!("superseded: {name}"),
            Carried::NoLongerNeeded { why } => println!("no longer needed: {name}: {why}"),
            Carried::SpecDropped => println!("spec dropped: {name}"),
        }
    }
    println!(
        "inbox: {} events carried, {} objects written, {} removed",
        plan.carried.len(),
        plan.objects.len(),
        plan.removed.len()
    );
    if dry || plan.removed.is_empty() {
        println!("inbox: nothing written");
        return Ok(());
    }
    let objects = root.join("objects");
    for (name, text) in &plan.objects {
        let path = objects.join(format!("{name}.py"));
        suzatary_io::durable::replace(&path, text.as_bytes(), suzatary_io::durable::Mode::Readable)
            .with_context(|| format!("writing {}", path.display()))?;
    }
    for name in &plan.removed {
        let path = objects.join(format!("{name}.py"));
        suzatary_io::durable::remove(&path)
            .with_context(|| format!("removing {}", path.display()))?;
        println!("removed: {}", path.display());
    }
    drop(held);
    let store = instance.inbox_store();
    let findings = suzatary_core::secretary::inbox::findings(&store);
    for finding in &findings {
        println!("inbox: {finding}");
    }
    anyhow::ensure!(
        findings.is_empty(),
        "the inbox does not verify after the run"
    );
    println!("inbox: carried, and it verifies");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(text: &str) -> Value {
        prodrome_core::literal::parse_literal(
            text,
            &prodrome_core::event::EventVocabulary::<InboxEvent>::new(&InboxVocabulary),
        )
        .expect("a literal")
    }

    fn printed(value: &Value) -> String {
        print_literal(value)
    }

    /// Each old form is read once, by its writer and then its words: only what the secretary
    /// wrote is a withdrawal, and a person's no is kept as it is, even in a supersession's words.
    #[test]
    fn each_old_form_is_carried_by_its_words_and_its_writer() {
        let dismissed = |actor: &str, reason: &str| {
            event(&format!(
                "Dismissed(proposal='p0', at=datetime(2026, 10, 1, 9, 0, 0), actor='{actor}', \
                 reason='{reason}')"
            ))
        };
        let (superseded, how) = carry(
            &dismissed("assistant", "superseded by proposal 4"),
            "assistant",
        )
        .expect("carried");
        assert_eq!(how, Carried::Superseded);
        assert_eq!(
            printed(&superseded),
            "Superseded(proposal='p0', at=datetime(2026, 10, 1, 9, 0, 0), actor='assistant')"
        );
        let (unneeded, how) = carry(
            &dismissed(
                "assistant",
                "the box withdrew it: the file reads another provider",
            ),
            "assistant",
        )
        .expect("carried");
        assert_eq!(
            how,
            Carried::NoLongerNeeded {
                why: "the file reads another provider".to_owned()
            }
        );
        assert!(
            printed(&unneeded).starts_with("NoLongerNeeded("),
            "{}",
            printed(&unneeded)
        );
        assert!(carry(&dismissed("ada", "not this week"), "assistant").is_none());
        assert!(
            carry(&dismissed("ada", "superseded by proposal 4"), "assistant").is_none(),
            "a person's no in a supersession's words stays a no"
        );
        let (unindexed, how) = carry(
            &dismissed("assistant", "superseded by proposal x"),
            "assistant",
        )
        .expect("carried");
        assert!(printed(&unindexed).starts_with("NoLongerNeeded("));
        assert_eq!(
            how,
            Carried::NoLongerNeeded {
                why: "superseded by proposal x".to_owned()
            }
        );
    }

    /// An asking that served a todo loses the spec nothing read; one that serves none keeps its
    /// own.
    #[test]
    fn a_serving_asking_loses_its_unread_spec() {
        let asked = |serves: &str| {
            event(&format!(
                "Asked(proposal='p0', at=datetime(2026, 10, 1, 9, 0, 0), actor='assistant', \
                 act=Email(account='ada@example.com', to=(), cc=(), subject='s', body='b', \
                 attachments=()), why='w', sources=('mail:abcdef012345',), \
                 asked='proposal:default/0', serves={serves}, spec=Flat(value=0.4))"
            ))
        };
        let (carried, how) = carry(&asked("'todo:life/reply'"), "assistant").expect("carried");
        assert_eq!(how, Carried::SpecDropped);
        assert!(printed(&carried).ends_with("serves='todo:life/reply', spec=None)"));
        assert!(carry(&asked("None"), "assistant").is_none());
    }
}
