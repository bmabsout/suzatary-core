//! `suzatary secretary migrate-receipts` over a real `Instance` (`suzatary init`'s own output),
//! whose conversations hold tool lines in the old shape, written as an older build wrote them: a
//! `remember` answer, a refusal, a `remember` answer the owner erased, one a forget erased, a
//! card's filing answer, an answer whose erasure the sweep has not yet acted on, and a model line
//! resting on them all, beside a conversation with no tool line. A dry run writes nothing; a run
//! carries each line into its outcome, locks every carried line's words again, keeps the digest's
//! leaves and verifies; a second run writes nothing; a run stopped part way is finished by the
//! next; a server of this build does not start before it; and once carried, a receipt outlives
//! the owner erasing its turn's words.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use clap::Parser;
use prodrome_core::event::{Actor, Hash};
use prodrome_core::literal::{Datetime, Value, print_literal};
use prodrome_core::schema::Schema as _;
use sha2::{Digest as _, Sha256};
use suzatary_cli::commands::{init, migrate_receipts};
use suzatary_cli::instance::Instance;
use suzatary_core::capability::{ToolCall, Turn};
use suzatary_core::literal::Literal as _;
use suzatary_core::secretary::carried::Carried;
use suzatary_core::secretary::erasure::Erasure;
use suzatary_core::secretary::line_event::{LineEvent, LineSlot};
use suzatary_core::secretary::line_words::LineWords;
use suzatary_core::secretary::locked_text::{FreshKey, KeyName, Locked, Shape};
use suzatary_core::secretary::memory::FactId;
use suzatary_core::secretary::outcome::Outcome;
use suzatary_core::secretary::store::{ConversationId, Entry, Kind};
use suzatary_core::secretary::transcript;
use suzatary_core::secretary::turn_shape::TurnShape;
use suzatary_core::secretary::words::Words;
use suzatary_core::store::Doc as _;
use suzatary_io::conversations::ConversationStores;
use suzatary_io::texts::SystemEntropy;

#[derive(Debug, Parser)]
struct Wrapper {
    #[command(flatten)]
    args: init::Args,
}

fn fresh_instance(name: &str) -> Instance {
    let dir = std::env::temp_dir().join(format!(
        "suzatary-cli-migrate-receipts-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let instance_path = dir.join("instance.toml");
    let repo = dir.to_str().expect("a utf-8 temp path");
    let args = Wrapper::parse_from([
        "suzatary-migrate-test",
        "--name",
        "Ada",
        "--city",
        "London",
        "--zone",
        "Europe/London",
        "--box",
        "ada-box.tailnet.ts.net",
        "--login",
        "her@example.test",
        "--repo",
        repo,
    ])
    .args;
    let env = suzatary_cli::paths::Env::under(
        instance_path.with_file_name("home"),
        instance_path.with_file_name("run"),
    );
    init::run_keyed(&instance_path, args, &env, |_| {
        Ok(suzatary_core::signatures::BoxKeys::of(
            &suzatary_core::cast::keys::root(),
            &suzatary_core::capability::Secret::new(suzatary_core::cast::keys::ROOT),
            &suzatary_core::capability::Secret::new(suzatary_core::cast::keys::AGENT),
        )?)
    })
    .expect("a fresh init");
    Instance::read(&instance_path, &env).expect("it reads back")
}

fn at(minute: u32) -> Datetime {
    Datetime::new(2026, 10, 1, 9, minute, 0, 0).expect("an instant")
}

fn trains() -> ConversationId {
    ConversationId::new("trains").expect("a slug")
}

fn other() -> ConversationId {
    ConversationId::new("other").expect("a slug")
}

fn ada() -> Actor {
    Actor::new("ada").expect("an actor")
}

/// A slot's binding as an older build printed it.
struct Bound(Vec<u8>);

impl Shape for Bound {
    type Words = LineWords;
    const DOMAIN: &'static [u8] = <LineSlot as Shape>::DOMAIN;
    fn print(&self) -> Vec<u8> {
        self.0.clone()
    }
}

/// The name of an object's text.
fn named(text: &str) -> Hash {
    Hash::new(hex::encode(Sha256::digest(text.as_bytes()))).expect("a name")
}

/// Every file under `dir`, by name, with its bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir).map_or_else(
        |_| BTreeMap::new(),
        |entries| {
            entries
                .map(|entry| {
                    let path = entry.expect("an entry").path();
                    (
                        path.file_name()
                            .expect("a name")
                            .to_string_lossy()
                            .into_owned(),
                        std::fs::read(&path).expect("bytes"),
                    )
                })
                .collect()
        },
    )
}

/// The conversations' store as it stands on disk: its objects and its keys.
fn disk(fixture: &Fixture) -> (BTreeMap<String, Vec<u8>>, BTreeMap<String, Vec<u8>>) {
    (files(&fixture.root.join("objects")), files(&fixture.keys))
}

/// What the fixture wrote: where, and the name and keys of each line it wrote by hand.
struct Fixture {
    root: PathBuf,
    keys: PathBuf,
    genesis: Hash,
    /// Each line of `trains` in order, its object's name and the keys of its words.
    lines: Vec<(Hash, Vec<KeyName>)>,
    /// The lines of `other`, which rest on no tool line.
    untouched: BTreeSet<Hash>,
}

impl Fixture {
    /// Write `event` as a change resting on the last line of `trains`.
    fn put(&mut self, event: &Value, keys: Vec<KeyName>) -> Hash {
        let deps = self
            .lines
            .last()
            .map(|(name, _)| vec![Value::str(name.as_str())])
            .unwrap_or_default();
        let text = print_literal(&Value::call(
            "Change",
            vec![
                ("genesis".to_owned(), Value::str(self.genesis.as_str())),
                ("deps".to_owned(), Value::Tuple(deps)),
                ("event".to_owned(), event.clone()),
            ],
        ));
        let name = named(&text);
        std::fs::write(self.root.join("objects").join(format!("{name}.py")), text)
            .expect("an object");
        self.lines.push((name.clone(), keys));
        name
    }

    /// A tool line in the old shape, its words locked under the old binding, as an older build
    /// wrote one.
    fn old_tool(
        &mut self,
        stores: &ConversationStores,
        verb: &str,
        refused: bool,
        answer: &str,
    ) -> (Hash, KeyName) {
        let held = self.lines.len();
        let mut fields = vec![
            ("conversation".to_owned(), trains().to_value()),
            (
                "at".to_owned(),
                at(u32::try_from(held).expect("few lines")).to_value(),
            ),
            ("actor".to_owned(), Value::str("assistant")),
            ("held".to_owned(), held.to_value()),
            ("carried".to_owned(), Value::None),
            (
                "turn".to_owned(),
                Value::str(format!(
                    r#"{{"who":"tool","verb":"{verb}","refused":{refused},"files":[]}}"#
                )),
            ),
            ("by".to_owned(), Value::None),
        ];
        let mut bound = fields.clone();
        bound.push(("slot".to_owned(), 0usize.to_value()));
        let binding = Bound(print_literal(&Value::call("Said", bound)).into_bytes());
        let words = suzatary_core::secretary::turn_shape::split(&Turn::Tool {
            verb: verb.into(),
            answer: answer.to_owned(),
            outcome: Outcome::Answered,
            files: Vec::new(),
        })
        .1
        .expect("words")
        .remove(0);
        let (locked, key) = Locked::lock(
            &binding,
            &words,
            FreshKey::draw(&SystemEntropy).expect("a key"),
        );
        assert_eq!(
            locked.open(&binding, &key),
            Some(words),
            "it opens as it was written"
        );
        stores.keys.set(&key.name(), &key).expect("the key kept");
        fields.push((
            "words".to_owned(),
            Value::Tuple(vec![
                Locked::<LineSlot>::read(locked.key().clone(), locked.sealed().to_vec()).to_value(),
            ]),
        ));
        let name = self.put(&Value::call("Said", fields), vec![key.name()]);
        (name, key.name())
    }

    /// A line written by this build's writer, resting on the last.
    fn line(&mut self, stores: &ConversationStores, entry: &Entry) -> Hash {
        let held = u32::try_from(self.lines.len()).expect("few lines");
        let (line, keys) = LineEvent::write(
            trains(),
            Actor::new("assistant").expect("an actor"),
            held,
            entry,
            &SystemEntropy,
        )
        .expect("a line");
        for key in &keys {
            stores.keys.set(&key.name(), key).expect("the key kept");
        }
        let names = keys
            .iter()
            .map(suzatary_core::secretary::locked_text::TextKey::name)
            .collect();
        self.put(&line.to_value(), names)
    }

    /// An erasure of `key` for `erasure`; `swept` destroys the key, as the sweep would.
    fn erase(&mut self, stores: &ConversationStores, key: &KeyName, erasure: Erasure, swept: bool) {
        let held = u32::try_from(self.lines.len()).expect("few lines");
        let line = LineEvent::erasure(
            trains(),
            at(held),
            Actor::new("assistant").expect("an actor"),
            held,
            BTreeSet::from([key.clone()]),
            erasure,
        );
        self.put(&line.to_value(), Vec::new());
        if swept {
            stores.keys.remove(key).expect("destroyed");
        }
    }
}

/// The objects a store holds, by name, with their text.
fn history(root: &Path) -> Vec<(Hash, String)> {
    files(&root.join("objects"))
        .into_iter()
        .map(|(file, text)| {
            (
                Hash::new(file.trim_end_matches(".py")).expect("a name"),
                String::from_utf8(text).expect("text"),
            )
        })
        .collect()
}

/// The lines of `trains` the fixture writes, by what each is.
struct Old {
    remembered: Hash,
    refused: Hash,
    erased: Hash,
    forgotten: Hash,
    card: Hash,
    pending: (Hash, KeyName),
    noted: Hash,
}

/// A person's turn.
fn said(text: &str) -> Entry {
    Entry::Said {
        at: at(1),
        turn: Turn::Person {
            text: text.to_owned(),
            references: Vec::new(),
            attachments: Vec::new(),
        },
        by: None,
    }
}

/// The fixture over the store, once `other`, a conversation with no tool line, is written by
/// this build's own writer.
fn fixture(stores: &ConversationStores) -> Fixture {
    let conversations = stores.conversations();
    transcript::start(
        conversations,
        &other(),
        &Entry::Opened {
            at: at(0),
            title: Words::kept("Other".to_owned()),
            model: "m".to_owned(),
            kind: Kind::Topic,
        },
    )
    .expect("opened");
    transcript::push(conversations, &other(), &said("what is on today")).expect("said");
    let root = stores.lines.inner().root().to_path_buf();
    let untouched: BTreeSet<Hash> = history(&root).into_iter().map(|(name, _)| name).collect();
    let genesis = history(&root)
        .into_iter()
        .find(|(_, text)| text.starts_with("Genesis("))
        .expect("a genesis")
        .0;
    Fixture {
        keys: stores
            .keys
            .path(&KeyName::new("0".repeat(64)).expect("a name"))
            .parent()
            .expect("the keys directory")
            .to_path_buf(),
        root,
        genesis,
        lines: Vec::new(),
        untouched,
    }
}

/// Conversations as an older build left them.
fn old_conversations(instance: &Instance) -> (Fixture, Old) {
    let stores = instance.conversation_stores();
    let mut fixture = fixture(&stores);
    fixture.line(
        &stores,
        &Entry::Opened {
            at: at(0),
            title: Words::kept("Trains".to_owned()),
            model: "m".to_owned(),
            kind: Kind::Topic,
        },
    );
    fixture.line(&stores, &said("remember that I take the early train"));
    fixture.line(
        &stores,
        &Entry::Said {
            at: at(2),
            turn: Turn::Model {
                text: String::new(),
                calls: vec![ToolCall {
                    verb: "remember".into(),
                    args: [(
                        "fact".to_owned(),
                        serde_json::json!("Takes the early train"),
                    )]
                    .into(),
                }],
            },
            by: None,
        },
    );
    let (remembered, _) = fixture.old_tool(
        &stores,
        "remember",
        false,
        r#"{"remembered": 1, "kind": "owner"}"#,
    );
    let (refused, _) = fixture.old_tool(&stores, "search", true, "no such thread");
    let (erased, key) = fixture.old_tool(
        &stores,
        "remember",
        false,
        r#"{"remembered": 2, "kind": "person"}"#,
    );
    fixture.erase(&stores, &key, Erasure::Asked { by: ada() }, true);
    let (forgotten, key) = fixture.old_tool(
        &stores,
        "remember",
        false,
        r#"{"remembered": 3, "kind": "person"}"#,
    );
    fixture.erase(&stores, &key, Erasure::Forgot { fact: FactId(3) }, true);
    let (card, _) = fixture.old_tool(
        &stores,
        "remember",
        false,
        r#"{"proposed": 0, "kind": "person"}"#,
    );
    let pending = fixture.old_tool(&stores, "todo_show", false, "the permit is due Friday");
    fixture.erase(&stores, &pending.1, Erasure::Asked { by: ada() }, false);
    let noted = fixture.line(
        &stores,
        &Entry::Said {
            at: at(30),
            turn: Turn::Model {
                text: "= Noted\n\nThe early train, from now on.".to_owned(),
                calls: Vec::new(),
            },
            by: None,
        },
    );
    (
        fixture,
        Old {
            remembered,
            refused,
            erased,
            forgotten,
            card,
            pending,
            noted,
        },
    )
}

/// The turns of `trains` as this build reads them.
fn turns(instance: &Instance) -> Vec<Turn> {
    let stores = instance.conversation_stores();
    transcript::read(stores.conversations(), &trains())
        .expect("trains reads")
        .into_stored()
        .turns
        .into_iter()
        .map(|said| said.turn)
        .collect()
}

/// The outcome a turn's shape holds.
fn outcome(turn: &Turn) -> Option<Outcome> {
    match suzatary_core::secretary::turn_shape::shape(turn) {
        TurnShape::Tool { outcome, .. } => Some(outcome),
        TurnShape::Person { .. } | TurnShape::Model { .. } | TurnShape::Act { .. } => None,
    }
}

/// The findings `verify` reports of the conversations.
fn findings(instance: &Instance) -> Vec<String> {
    transcript::findings(
        &instance.conversation_stores().lines,
        &instance.inbox_store(),
    )
}

/// What the carried `trains` must read as: each line's words where they were kept, each tool
/// line's outcome, and the receipt of every `remember` answer but the one the owner erased.
fn assert_carried(instance: &Instance) {
    assert!(findings(instance).is_empty(), "{:?}", findings(instance));
    let turns = turns(instance);
    assert_eq!(turns.len(), 9, "every turn once, none twice: {turns:#?}");
    assert!(
        matches!(&turns[0], Turn::Person { text, .. } if text == "remember that I take the early train")
    );
    assert!(
        matches!(&turns[1], Turn::Model { calls, .. } if calls.len() == 1),
        "a call's arguments opened again"
    );
    assert!(
        matches!(&turns[2], Turn::Tool { answer, .. } if answer.contains("\"remembered\": 1")),
        "a kept answer's words open under their new key: {:?}",
        turns[2]
    );
    assert_eq!(
        turns.iter().map(outcome).collect::<Vec<_>>(),
        vec![
            None,
            None,
            Some(Outcome::Remembered { fact: FactId(1) }),
            Some(Outcome::Refused),
            Some(Outcome::Answered),
            Some(Outcome::Remembered { fact: FactId(3) }),
            Some(Outcome::Answered),
            Some(Outcome::Answered),
            None,
        ],
        "the receipt the owner's erasure took is lost, the one a forget took is its fact"
    );
    assert!(matches!(&turns[4], Turn::Erased { .. }));
    assert!(matches!(&turns[5], Turn::Erased { .. }));
    assert!(
        matches!(&turns[7], Turn::Erased { .. }),
        "an erasure the sweep has not acted on still erases"
    );
    assert!(
        matches!(&turns[8], Turn::Model { text, .. } if text.starts_with("= Noted")),
        "a line resting on the carried ones opens again"
    );
}

/// What a plan reads of each tool line, and which slots it carries as stored.
fn assert_reads(instance: &Instance, fixture: &Fixture, old: &Old) {
    let stores = instance.conversation_stores();
    let plan = migrate_receipts::plan(
        &history(&fixture.root),
        &|name| Ok(stores.keys.get(name)?),
        &SystemEntropy,
    )
    .expect("a plan");
    let reads: BTreeMap<&Hash, &migrate_receipts::Read> =
        plan.tools.iter().map(|(name, read)| (name, read)).collect();
    assert_eq!(reads.len(), 6);
    assert_eq!(
        reads[&old.remembered],
        &migrate_receipts::Read::Outcome(Outcome::Remembered { fact: FactId(1) })
    );
    assert_eq!(
        reads[&old.refused],
        &migrate_receipts::Read::Outcome(Outcome::Refused)
    );
    assert_eq!(reads[&old.erased], &migrate_receipts::Read::ReceiptLost);
    assert_eq!(
        reads[&old.forgotten],
        &migrate_receipts::Read::Outcome(Outcome::Remembered { fact: FactId(3) })
    );
    assert_eq!(
        reads[&old.card],
        &migrate_receipts::Read::Outcome(Outcome::Answered)
    );
    assert_eq!(
        plan.sealed.iter().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            (old.erased.clone(), 0),
            (old.forgotten.clone(), 0),
            (old.pending.0.clone(), 0),
        ]),
        "a slot whose key is gone, or named by an erasure, is carried as stored"
    );
    assert!(
        plan.removed
            .iter()
            .all(|name| !fixture.untouched.contains(name))
    );
}

/// The digest's leaves after the run: every carried line's is the name it had before, renamed
/// over its keys an erasure names, so every node, and every summary of one, keeps its name.
fn assert_leaves(instance: &Instance, fixture: &Fixture, old: &Old) {
    let stores = instance.conversation_stores();
    let conversations = stores.conversations();
    let objects = conversations.lines.objects().expect("the objects");
    let opened = transcript::opened(&objects, &conversations.slots());
    let digest =
        suzatary_core::secretary::digest::digest(&objects, &opened, &[], &BTreeSet::new(), |_| {
            None
        })
        .expect("a digest");
    let gone: BTreeSet<&KeyName> = [&old.pending.1]
        .into_iter()
        .chain(
            fixture
                .lines
                .iter()
                .filter(|(name, _)| *name == old.erased || *name == old.forgotten)
                .flat_map(|(_, keys)| keys),
        )
        .collect();
    let expected: BTreeSet<Hash> = fixture
        .lines
        .iter()
        .map(|(name, keys)| {
            let erased: Vec<Hash> = keys
                .iter()
                .filter(|key| gone.contains(key))
                .map(|key| Hash::new(key.as_str()).expect("a key's name is a hash"))
                .collect();
            if erased.is_empty() {
                name.clone()
            } else {
                prodrome_core::memo::name(b"erased line", std::iter::once(name).chain(&erased))
            }
        })
        .chain(
            fixture
                .untouched
                .iter()
                .filter(|name| **name != fixture.genesis)
                .cloned(),
        )
        .collect();
    let mut leaves = BTreeSet::new();
    let mut stack = vec![digest.tree().expect("a tree").root()];
    while let Some(node) = stack.pop() {
        if node.is_leaf() {
            leaves.insert(prodrome_core::memo::Tree::name(node).clone());
        }
        stack.extend(node.children());
    }
    assert_eq!(
        leaves, expected,
        "the digest's leaves are the lines the store held before"
    );
}

#[test]
fn migrate_receipts_carries_every_tool_line_once() {
    let instance = fresh_instance("carries");
    let (fixture, old) = old_conversations(&instance);
    assert!(
        !findings(&instance).is_empty(),
        "this build does not read a tool line in the old shape"
    );
    let before = disk(&fixture);

    assert_reads(&instance, &fixture, &old);

    migrate_receipts::run(&instance, true).expect("a dry run");
    assert_eq!(disk(&fixture), before, "a dry run writes nothing");

    migrate_receipts::run(&instance, false).expect("a run");
    assert_carried(&instance);
    let after = history(&fixture.root);
    let names: BTreeSet<Hash> = after.iter().map(|(name, _)| name.clone()).collect();
    assert!(
        fixture.untouched.is_subset(&names),
        "a conversation resting on no tool line keeps its names"
    );
    // The opening, the person's turn and the call come before the first tool line.
    let (before_tools, from_tools) = fixture.lines.split_at(3);
    assert!(
        before_tools.iter().all(|(name, _)| names.contains(name)),
        "a line resting on no tool line keeps its name"
    );
    assert!(
        from_tools.iter().all(|(name, _)| !names.contains(name)),
        "a tool line and every line resting on one is carried"
    );
    assert!(!names.contains(&old.noted));
    let keys = files(&fixture.keys);
    for (name, keys_of) in before_tools {
        for key in keys_of {
            assert!(keys.contains_key(key.as_str()), "{name} keeps its key");
        }
    }
    for (name, keys_of) in from_tools {
        for key in keys_of {
            let kept = keys.contains_key(key.as_str());
            if *key == old.pending.1 {
                assert!(kept, "a key an erasure names is the sweep's to destroy");
            } else {
                assert!(!kept, "the old key of {name} went with its line");
            }
        }
    }
    // Every carried line names the object it replaced, the digest's leaf, which keeps its name.
    let carried: BTreeSet<Hash> = after
        .iter()
        .filter_map(|(_, text)| {
            let envelope = prodrome_core::event::parse_envelope::<LineEvent>(
                &suzatary_core::secretary::line_event::LineVocabulary,
                text,
            )
            .ok()?;
            let line = envelope.event()?;
            match line.carried() {
                Carried::From(old) => Some(old.clone()),
                Carried::Written => None,
            }
        })
        .collect();
    let written: BTreeSet<Hash> = from_tools.iter().map(|(name, _)| name.clone()).collect();
    assert_eq!(carried, written);
    assert_leaves(&instance, &fixture, &old);

    let carried = disk(&fixture);
    migrate_receipts::run(&instance, false).expect("a second run");
    assert_eq!(disk(&fixture), carried, "a second run writes nothing");
}

#[test]
fn a_run_stopped_part_way_is_finished_by_the_next() {
    for stop in [
        "after half the objects",
        "after every object",
        "after the old keys",
    ] {
        let instance = fresh_instance(&stop.replace(' ', "-"));
        let (fixture, _) = old_conversations(&instance);
        let stores = instance.conversation_stores();
        let plan = migrate_receipts::plan(
            &history(&fixture.root),
            &|name| Ok(stores.keys.get(name)?),
            &SystemEntropy,
        )
        .expect("a plan");
        let objects = match stop {
            "after half the objects" => plan.objects.len() / 2,
            _ => plan.objects.len(),
        };
        for (name, text, keys) in &plan.objects[..objects] {
            for key in keys {
                stores.keys.set(&key.name(), key).expect("a key");
            }
            std::fs::write(
                fixture.root.join("objects").join(format!("{name}.py")),
                text,
            )
            .expect("written");
        }
        if stop == "after the old keys" {
            for key in &plan.unkeyed {
                stores.keys.remove(key).expect("removed");
            }
        }
        migrate_receipts::uncarried(&instance).expect_err("a stopped run is not done");
        migrate_receipts::run(&instance, false).expect("the next run");
        assert_carried(&instance);
        migrate_receipts::uncarried(&instance).expect("nothing is left to carry");
        let carried = disk(&fixture);
        migrate_receipts::run(&instance, false).expect("a run after");
        assert_eq!(
            disk(&fixture),
            carried,
            "{stop}: then nothing is left to write"
        );
    }
}

/// Once carried, a `remember` turn's receipt is its shape's: the owner erasing the turn's words
/// leaves the fact it names.
#[test]
fn a_receipt_outlives_the_owner_erasing_its_words() {
    let instance = fresh_instance("erased-after");
    let (_, _) = old_conversations(&instance);
    migrate_receipts::run(&instance, false).expect("a run");
    let stores = instance.conversation_stores();
    let conversations = stores.conversations();
    let written = transcript::read(conversations, &trains()).expect("reads");
    let keys: BTreeSet<KeyName> = written
        .lines()
        .iter()
        .find(|line| {
            matches!(
                line.line.shape(),
                suzatary_core::secretary::line_shape::LineShape::Said {
                    turn: TurnShape::Tool {
                        outcome: Outcome::Remembered { fact: FactId(1) },
                        ..
                    },
                    ..
                }
            )
        })
        .expect("the remember answer")
        .line
        .keys()
        .cloned()
        .collect();
    transcript::erase(
        conversations,
        &trains(),
        keys,
        Erasure::Asked { by: ada() },
        at(40),
    )
    .expect("erased");
    let turns = turns(&instance);
    assert!(matches!(&turns[2], Turn::Erased { .. }), "{:?}", turns[2]);
    assert_eq!(
        outcome(&turns[2]),
        Some(Outcome::Remembered { fact: FactId(1) })
    );
}

#[derive(Debug, Parser)]
struct Serve {
    #[command(flatten)]
    args: suzatary_cli::commands::serve::Args,
}

/// A server of this build over conversations still in the old shape does not start, and says
/// which verb to run; once the run is done, it starts.
#[test]
fn a_server_over_uncarried_conversations_does_not_start() {
    let instance = fresh_instance("served");
    let (fixture, _) = old_conversations(&instance);
    let before = disk(&fixture);
    let refused =
        suzatary_cli::commands::serve::serve(&instance, Serve::parse_from(["serve"]).args)
            .expect_err("it does not start");
    assert!(
        refused
            .to_string()
            .contains("run `suzatary secretary migrate-receipts` first"),
        "{refused}"
    );
    assert_eq!(disk(&fixture), before, "the refusal writes nothing");
    migrate_receipts::run(&instance, false).expect("a run");
    migrate_receipts::uncarried(&instance).expect("nothing is left to carry");
    crate::migrate_inbox::assert_starts(&instance);
}
