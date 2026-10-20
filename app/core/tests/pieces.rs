//! A store's pull by pieces (`routes::store_objects_since`, `routes::lines_since`): asked again
//! and again from what the last piece left, while the store moves between the asks, each piece
//! joined to what the caller holds holds everything it rests on (its parents and the genesis of
//! its prodrome), forks and the merges after them included; the pieces together are what one
//! answer of the whole would send; no piece carries a key an erasure anywhere in the store names;
//! and every piece names every such key.

use std::collections::{BTreeMap, BTreeSet};

use prodrome_core::event::{Actor, Envelope, Hash, parents_of};
use prodrome_core::literal::Datetime;
use proptest::prelude::*;

use suzatary_core::capability::Turn;
use suzatary_core::routes::{self, Cut};
use suzatary_core::secretary::conversations::InMemory;
use suzatary_core::secretary::erasure::Erasure;
use suzatary_core::secretary::line_event::LineEvent;
use suzatary_core::secretary::store::{ConversationId, Entry, Kind, stamp};
use suzatary_core::secretary::transcript;
use suzatary_core::secretary::words::Words;
use suzatary_core::store::{Doc as _, Prodrome as _};

fn at(n: u32) -> String {
    stamp(Datetime::new(2026, 9, 30, 9, n / 60, n % 60, 0).expect("an instant"))
}

fn now() -> Datetime {
    Datetime::new(2026, 9, 30, 12, 0, 0, 0).expect("an instant")
}

fn id(n: usize) -> ConversationId {
    ConversationId::new(format!("2026-09-30-{n}")).expect("a slug")
}

/// One write to one of a few conversations: a turn of some length, the erasure of the words of
/// its latest line that has any, or a fork: two replicas each writing a turn from the same lines,
/// joined, so the next line of that conversation rests on both.
#[derive(Debug, Clone)]
enum Write {
    Said { conversation: usize, words: usize },
    Erased { conversation: usize },
    Fork { conversation: usize, words: usize },
}

fn write() -> impl Strategy<Value = Write> {
    prop_oneof![
        4 => (0..3usize, 1..600usize).prop_map(|(conversation, words)| Write::Said { conversation, words }),
        1 => (0..3usize).prop_map(|conversation| Write::Erased { conversation }),
        1 => (0..3usize, 1..600usize).prop_map(|(conversation, words)| Write::Fork { conversation, words }),
    ]
}

/// A store the writes move, as the box writes it: every line a `Change` over its conversation's
/// heads, its keys kept first. An erasure's keys are kept, as a sweep that could not destroy them
/// leaves them, so only the pull's own reading keeps them out.
struct Writer {
    lines: InMemory,
    written: BTreeMap<usize, u32>,
    second: u32,
}

impl Writer {
    fn new() -> Self {
        Self {
            lines: InMemory::default(),
            written: BTreeMap::new(),
            second: 0,
        }
    }

    fn line(&mut self, conversation: usize, entry: &Entry) -> LineEvent {
        let held = self.written.entry(conversation).or_insert(0);
        let (line, keys) = LineEvent::write(
            id(conversation),
            Actor::new("assistant").expect("an actor"),
            *held,
            entry,
            &self.lines.entropy,
        )
        .expect("a line");
        *held += 1;
        for key in &keys {
            self.lines.keys.set(&key.name(), key).expect("kept");
        }
        line
    }

    fn said(&mut self, conversation: usize, words: usize) -> Entry {
        self.second += 1;
        if !self.written.contains_key(&conversation) {
            let opened = Entry::Opened {
                at: at(self.second),
                title: Words::kept(format!("Topic {conversation}")),
                model: "claude-cli/haiku".to_owned(),
                kind: Kind::Topic,
            };
            let line = self.line(conversation, &opened);
            self.lines.lines.append(line).expect("opened");
        }
        let turn = Turn::Person {
            text: "a".repeat(words),
            references: Vec::new(),
            attachments: Vec::new(),
        };
        Entry::Said {
            at: at(self.second),
            turn,
            by: None,
        }
    }

    fn apply(&mut self, write: &Write) {
        match write {
            Write::Said {
                conversation,
                words,
            } => {
                let said = self.said(*conversation, *words);
                let line = self.line(*conversation, &said);
                self.lines.lines.append(line).expect("said");
            }
            Write::Fork {
                conversation,
                words,
            } => {
                let said = self.said(*conversation, *words);
                // The other replica holds what this one does, and each writes a line from it.
                let other = suzatary_core::store::InMemory::<LineEvent>::default();
                let held: Vec<(Hash, String)> = self
                    .lines
                    .lines
                    .objects()
                    .expect("reads")
                    .iter()
                    .map(|(name, object)| {
                        (
                            name.clone(),
                            prodrome_core::event::canonical_envelope(object),
                        )
                    })
                    .collect();
                other.accept(&held).expect("copied");
                let theirs = self.line(*conversation, &said);
                self.written.entry(*conversation).and_modify(|n| *n -= 1);
                let ours = self.line(*conversation, &said);
                other.append(theirs).expect("theirs");
                self.lines.lines.append(ours).expect("ours");
                let forked: Vec<(Hash, String)> = other
                    .objects()
                    .expect("reads")
                    .iter()
                    .map(|(name, object)| {
                        (
                            name.clone(),
                            prodrome_core::event::canonical_envelope(object),
                        )
                    })
                    .collect();
                self.lines.lines.accept(&forked).expect("joined");
            }
            Write::Erased { conversation } => {
                let objects = self.lines.lines.objects().expect("reads");
                let Some(keys) = objects
                    .iter()
                    .rev()
                    .filter_map(|(_, object)| object.event())
                    .filter(|line| *line.conversation() == id(*conversation))
                    .map(|line| line.keys().cloned().collect::<BTreeSet<_>>())
                    .find(|keys| !keys.is_empty())
                else {
                    return;
                };
                self.second += 1;
                let erased = Entry::Erased {
                    at: at(self.second),
                    keys,
                    erasure: Erasure::Asked {
                        by: Actor::new("ada").expect("an actor"),
                    },
                };
                let line = self.line(*conversation, &erased);
                self.lines.lines.append(line).expect("erased");
            }
        }
    }
}

/// Everything an object rests on, as named in it: its parents, and a change's genesis.
fn rests_on(object: &Envelope<LineEvent>) -> Vec<Hash> {
    let mut on = parents_of(object);
    if let Envelope::Change(change) = object {
        on.push(change.genesis.clone());
    }
    on
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Pieces asked one after another from what the last left, the store moving between them:
    /// each holds nothing already held, fits the budget unless it is one object, holds with what
    /// came before everything it rests on, carries no key an erasure in the store names, and
    /// names every such key; the pull ends holding the store as it is then, and, where nothing
    /// moved, the pieces are the whole answer, keys included.
    #[test]
    fn law_the_pieces_union_is_the_whole_and_each_is_ancestor_closed(
        script in prop::collection::vec(write(), 1..24),
        moves in prop::collection::vec(prop::option::of(write()), 0..6),
        held_first in 0..64usize,
        budget in 1..3000usize,
    ) {
        let mut store = Writer::new();
        for write in &script {
            store.apply(write);
        }
        let epochs = suzatary_core::epoch::Epochs::remembered();
        let first = store.lines.lines.objects().expect("reads");
        // A replica holds a down-set: a prefix of the store's order.
        let mut held: BTreeSet<Hash> =
            first.iter().take(held_first).map(|(name, _)| name.clone()).collect();
        let since: Vec<Hash> = held.iter().cloned().collect();
        let whole = routes::lines_since(store.lines.conversations(), &since, &epochs, now(), Cut::Whole).expect("the whole");
        prop_assert_eq!(whole.rest, 0);
        let mut sent = BTreeSet::new();
        let mut keys = BTreeMap::new();
        let mut moved = false;
        for asked in 0usize.. {
            prop_assert!(asked <= first.len() + moves.len() * 4 + 1, "the pull ends");
            let objects = store.lines.lines.objects().expect("reads");
            let by_name: BTreeMap<&Hash, &Envelope<LineEvent>> = objects.iter().map(|(n, o)| (n, o)).collect();
            let erased: Vec<String> = transcript::erased(&objects).keys().map(|k| k.as_str().to_owned()).collect();
            let since: Vec<Hash> = held.iter().cloned().collect();
            let piece = routes::lines_since(store.lines.conversations(), &since, &epochs, now(), Cut::Bytes(budget)).expect("a piece");
            let lacked = objects.len() - held.len();
            prop_assert_eq!(piece.objects.objects.is_empty(), lacked == 0);
            prop_assert_eq!(piece.rest, lacked - piece.objects.objects.len());
            prop_assert_eq!(&piece.erased, &erased);
            let bytes: usize = piece.objects.objects.iter().map(|o| o.literal.len()).sum();
            prop_assert!(bytes <= budget || piece.objects.objects.len() == 1, "{} bytes over {}", bytes, budget);
            for object in &piece.objects.objects {
                let name = Hash::new(object.hash.clone()).expect("a name");
                prop_assert!(held.insert(name), "{} sent again", object.hash);
                sent.insert(object.hash.clone());
            }
            for object in &piece.objects.objects {
                let name = Hash::new(object.hash.clone()).expect("a name");
                for on in rests_on(by_name[&name]) {
                    prop_assert!(held.contains(&on), "{} came before {} it rests on", object.hash, on.as_str());
                }
            }
            for name in piece.keys.keys() {
                prop_assert!(!erased.contains(name), "the erased key {} was sent", name);
            }
            keys.extend(piece.keys);
            if piece.rest == 0 {
                break;
            }
            if let Some(Some(write)) = moves.get(asked) {
                store.apply(write);
                moved = true;
            }
        }
        prop_assert_eq!(held.len(), store.lines.lines.objects().expect("reads").len());
        if !moved {
            let all: BTreeSet<String> = whole.objects.objects.iter().map(|o| o.hash.clone()).collect();
            prop_assert_eq!(sent, all);
            prop_assert_eq!(keys, whole.keys);
        }
    }
}

/// The writes make what the law needs: a fork leaves a conversation two heads, and the next line
/// rests on both.
#[test]
fn a_fork_then_a_line_is_a_merge() {
    let mut store = Writer::new();
    for write in [
        Write::Said {
            conversation: 0,
            words: 3,
        },
        Write::Fork {
            conversation: 0,
            words: 3,
        },
        Write::Said {
            conversation: 0,
            words: 3,
        },
    ] {
        store.apply(&write);
    }
    let objects = store.lines.lines.objects().expect("reads");
    let merges = objects
        .iter()
        .filter(|(_, object)| matches!(object, Envelope::Change(change) if change.deps.len() > 1))
        .count();
    assert_eq!(merges, 1);
}
