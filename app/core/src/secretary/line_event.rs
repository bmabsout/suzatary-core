//! The conversations' schema since their words were locked: one line of one conversation, its
//! shape in the clear and each slot of its words locked under a key of its own.
//!
//! A line is `Kind(conversation, at, actor, held, carried, <shape fields>, words)`: the head every
//! line has, the shape of its kind ([`LineShape`]), and `words`, one `Locked(key, box)` per slot,
//! the key's name and the ciphertext in base64. Each slot's associated data is the line's own
//! print without its words and with the slot's number ([`LineSlot`]), so a ciphertext moved to
//! another slot, line or store does not open. Every line writes its conversation's one register,
//! the transcript, so a line rests on the lines its writer held and the lines are causally
//! ordered. Nothing in an object is words: the redaction door is applied to the words before they
//! are locked and again when they are opened.

use std::collections::BTreeSet;

use base64::Engine as _;
use prodrome_core::event::Actor;
use prodrome_core::literal::{Datetime, ProdromeError, Table, Value, Vocabulary, print_literal};
use prodrome_core::schema::Schema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::capability::Entropy;
use crate::literal::{Fields, Literal, Signature, no_arm, record};
use crate::secretary::carried::Carried;
use crate::secretary::erasure::Erasure;
use crate::secretary::line_shape::{self, LineShape};
use crate::secretary::line_words::LineWords;
use crate::secretary::locked_text::{FreshKey, KeyName, Locked, Shape, TextKey};
use crate::secretary::slots::Slots;
use crate::secretary::store::{ConversationId, Entry};
use crate::secretary::transcript_registers::{TranscriptRegister, TranscriptRegisters};

/// One line: which conversation, when, who wrote it, how many lines its writer held, where it
/// came from, its shape, and its words locked.
///
/// `held` tells a line said twice from a line written twice: the Prodrome writes nothing for an
/// event it already holds, and the same words said twice in one second are two lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineEvent {
    conversation: ConversationId,
    at: Datetime,
    actor: Actor,
    held: u32,
    carried: Carried,
    shape: LineShape,
    words: Vec<Locked<LineSlot>>,
}

/// One slot of one line, as its words are bound: the line's print without its words, and the
/// slot's number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineSlot(Vec<u8>);

impl Shape for LineSlot {
    type Words = LineWords;
    const DOMAIN: &'static [u8] = b"suzatary lines v1";
    fn print(&self) -> Vec<u8> {
        self.0.clone()
    }
}

/// An entry that cannot be a stored line.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotALine {
    /// Some of its words are already gone: a line is written with its words.
    #[error("a line is written with its words, and some of these are gone")]
    Gone,
    /// Its words do not pass the redaction door whole.
    #[error("its words do not pass the redaction door: {0}")]
    Unscrubbable(String),
    /// The generator gave no key.
    #[error("no key for its words: {0}")]
    NoKey(String),
}

impl LineEvent {
    /// Line `entry` of `conversation`, written here by `actor` holding `held` lines, each slot of
    /// its words through the redaction door and locked under a key drawn from `entropy`; the keys
    /// that open them are the only thing to keep.
    ///
    /// # Errors
    /// [`NotALine`] for words already gone, words the door refuses, or no key.
    pub fn write(
        conversation: ConversationId,
        actor: Actor,
        held: u32,
        entry: &Entry,
        entropy: &dyn Entropy,
    ) -> Result<(Self, Vec<TextKey>), NotALine> {
        let at = entry.at();
        let (shape, words) = line_shape::split(entry);
        let words = words.ok_or(NotALine::Gone)?;
        let mut line = Self {
            conversation,
            at,
            actor,
            held,
            carried: Carried::Written,
            shape,
            words: Vec::new(),
        };
        let mut keys = Vec::with_capacity(words.len());
        for (slot, words) in words.iter().enumerate() {
            let words = words
                .scrubbed()
                .map_err(|failed| NotALine::Unscrubbable(failed.to_string()))?;
            let key = FreshKey::draw(entropy).map_err(NotALine::NoKey)?;
            let (locked, opens) = Locked::lock(&line.slot(slot), &words, key);
            line.words.push(locked);
            keys.push(opens);
        }
        Ok((line, keys))
    }

    /// An erasure of `keys` in `conversation`, at `at`, written by `actor` holding `held` lines.
    #[must_use]
    pub fn erasure(
        conversation: ConversationId,
        at: Datetime,
        actor: Actor,
        held: u32,
        keys: BTreeSet<KeyName>,
        erasure: Erasure,
    ) -> Self {
        Self {
            conversation,
            at,
            actor,
            held,
            carried: Carried::Written,
            shape: LineShape::Erased { keys, erasure },
            words: Vec::new(),
        }
    }

    /// The conversation.
    #[must_use]
    pub fn conversation(&self) -> &ConversationId {
        &self.conversation
    }

    /// How many lines its writer held.
    #[must_use]
    pub fn held(&self) -> u32 {
        self.held
    }

    /// Where it came from.
    #[must_use]
    pub fn carried(&self) -> &Carried {
        &self.carried
    }

    /// Its shape.
    #[must_use]
    pub fn shape(&self) -> &LineShape {
        &self.shape
    }

    /// The name of each slot's key, in slot order.
    pub fn keys(&self) -> impl Iterator<Item = &KeyName> {
        self.words.iter().map(Locked::key)
    }

    /// Slot `slot`'s words as locked.
    #[must_use]
    pub fn locked(&self, slot: usize) -> Option<&Locked<LineSlot>> {
        self.words.get(slot)
    }

    /// Slot `slot`'s words, opened by `key`; `None` for a slot the line does not hold or a key
    /// that does not open it.
    #[must_use]
    pub fn open(&self, slot: usize, key: &TextKey) -> Option<LineWords> {
        self.words.get(slot)?.open(&self.slot(slot), key)
    }

    /// The line as read: each slot read by `slots` by its key's name, opened with this line's
    /// binding for that slot.
    #[must_use]
    pub fn entry(&self, slots: &impl Slots) -> Entry {
        let words = self
            .words
            .iter()
            .enumerate()
            .map(|(slot, locked)| {
                let binding = self.slot(slot);
                slots.words(locked.key(), &binding.0, locked.sealed(), &|key| {
                    locked.open(&binding, key)
                })
            })
            .collect();
        line_shape::join(&self.conversation, self.shape.clone(), self.at, words)
    }

    /// Slot `slot`'s binding: this line's print without its words, and the slot's number.
    fn slot(&self, slot: usize) -> LineSlot {
        let ((name, fields), mut values) = self.bare();
        // The binding is the bare line with `slot` in place of `words`: no stored constructor.
        let named: Vec<&str> = fields
            .iter()
            .copied()
            .filter(|field| *field != "words")
            .chain(["slot"])
            .collect();
        values.push(slot.to_value());
        LineSlot(
            print_literal(&Value::call(
                name,
                named.into_iter().map(str::to_owned).zip(values).collect(),
            ))
            .into_bytes(),
        )
    }

    fn head(&self) -> Vec<Value> {
        vec![
            self.conversation.to_value(),
            self.at.to_value(),
            self.actor.to_value(),
            self.held.to_value(),
            self.carried.to_value(),
        ]
    }

    /// The line's print without its words: its head and its shape, under its signature.
    fn bare(&self) -> (Signature, Vec<Value>) {
        let mut values = self.head();
        values.extend(shape_fields(&self.shape));
        (signature(&self.shape), values)
    }
}

/// The fields a shape prints after the head, in signature order.
fn shape_fields(shape: &LineShape) -> Vec<Value> {
    match shape {
        LineShape::Opened { model, kind } => vec![model.to_value(), kind.to_value()],
        LineShape::Titled { by } => vec![json(by)],
        LineShape::Said { turn, by } => vec![json(turn), by.to_value()],
        LineShape::Proposed { object, revises } => vec![object.to_value(), revises.to_value()],
        LineShape::Reviewed {
            chain,
            batch,
            decision,
        } => vec![
            chain.to_value(),
            batch.to_value(),
            decision.word().to_owned().to_value(),
            decision.objects().to_vec().to_value(),
        ],
        LineShape::ReviewFailed { standing } => vec![standing.to_value()],
        LineShape::Stopped { by } => vec![by.to_value()],
        LineShape::Rewound { to, by } => vec![to.to_value(), by.to_value()],
        LineShape::MemorySeen { version } => vec![version.to_value()],
        LineShape::Session {
            session,
            model,
            cwd,
            surface,
        } => vec![
            session.to_value(),
            model.to_value(),
            cwd.to_value(),
            surface.to_value(),
        ],
        LineShape::Unheard { session, model } => vec![session.to_value(), model.to_value()],
        LineShape::Kept {
            session,
            model,
            cwd,
            surface,
            from,
        } => vec![
            session.to_value(),
            model.to_value(),
            cwd.to_value(),
            surface.to_value(),
            from.to_value(),
        ],
        LineShape::Scoped { scope } => vec![scope.to_value()],
        LineShape::Coding { proposal, session } => vec![proposal.to_value(), json(session)],
        LineShape::Erased { keys, erasure } => vec![keys.to_value(), json(erasure)],
    }
}

/// The shape a stored line spells.
fn shape_of(fields: Fields<'_>) -> Result<LineShape, ProdromeError> {
    Ok(match fields.name() {
        "Opened" => LineShape::Opened {
            model: fields.get("model")?,
            kind: fields.get("kind")?,
        },
        "Titled" => LineShape::Titled {
            by: parsed(fields, "by")?,
        },
        "Said" => LineShape::Said {
            turn: parsed(fields, "turn")?,
            by: fields.get("by")?,
        },
        "Proposed" => LineShape::Proposed {
            object: fields.get("object")?,
            revises: fields.get("revises")?,
        },
        "Reviewed" => LineShape::Reviewed {
            chain: fields.get("chain")?,
            batch: fields.get("batch")?,
            decision: crate::secretary::store::Decision::read(
                &fields.get::<String>("decision")?,
                &fields.get::<Vec<String>>("objects")?,
            )?,
        },
        "ReviewFailed" => LineShape::ReviewFailed {
            standing: fields.get("standing")?,
        },
        "Stopped" => LineShape::Stopped {
            by: fields.get("by")?,
        },
        "Rewound" => LineShape::Rewound {
            to: fields.get("to")?,
            by: fields.get("by")?,
        },
        "MemorySeen" => LineShape::MemorySeen {
            version: fields.get("version")?,
        },
        "Session" => LineShape::Session {
            session: fields.get("session")?,
            model: fields.get("model")?,
            cwd: fields.get("cwd")?,
            surface: fields.get("surface")?,
        },
        "Unheard" => LineShape::Unheard {
            session: fields.get("session")?,
            model: fields.get("model")?,
        },
        "Kept" => LineShape::Kept {
            session: fields.get("session")?,
            model: fields.get("model")?,
            cwd: fields.get("cwd")?,
            surface: fields.get("surface")?,
            from: fields.get("from")?,
        },
        "Scoped" => LineShape::Scoped {
            scope: fields.get("scope")?,
        },
        "Coding" => LineShape::Coding {
            proposal: fields.get("proposal")?,
            session: parsed(fields, "session")?,
        },
        "Erased" => LineShape::Erased {
            keys: fields.get("keys")?,
            erasure: parsed(fields, "erasure")?,
        },
        other => return Err(no_arm("a conversation line", other)),
    })
}

/// The signature a line of `shape` is stored under.
fn signature(shape: &LineShape) -> Signature {
    match shape {
        LineShape::Opened { .. } => OPENED,
        LineShape::Titled { .. } => TITLED,
        LineShape::Said { .. } => SAID,
        LineShape::Proposed { .. } => PROPOSED,
        LineShape::Reviewed { .. } => REVIEWED,
        LineShape::ReviewFailed { .. } => REVIEW_FAILED,
        LineShape::Stopped { .. } => STOPPED,
        LineShape::Rewound { .. } => REWOUND,
        LineShape::MemorySeen { .. } => MEMORY_SEEN,
        LineShape::Session { .. } => SESSION,
        LineShape::Unheard { .. } => UNHEARD,
        LineShape::Kept { .. } => KEPT,
        LineShape::Scoped { .. } => SCOPED,
        LineShape::Coding { .. } => CODING,
        LineShape::Erased { .. } => ERASED,
    }
}

/// `Locked(key, box)`: a slot's words as stored, the key's name and the ciphertext in base64.
const LOCKED: Signature = ("Locked", &["key", "box"]);

/// A slot's words as stored.
impl Literal for Locked<LineSlot> {
    fn to_value(&self) -> Value {
        record(
            LOCKED,
            vec![
                self.key().to_value(),
                base64::engine::general_purpose::STANDARD
                    .encode(self.sealed())
                    .to_value(),
            ],
        )
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        let fields = Fields::of(value, LOCKED)?;
        let sealed = base64::engine::general_purpose::STANDARD
            .decode(fields.get::<String>("box")?)
            .map_err(|failed| ProdromeError::invalid(format!("Locked.box: {failed}")))?;
        Ok(Locked::read(fields.get("key")?, sealed))
    }
}

/// Whether a line of `signature` carries `words`: every kind that may hold any.
fn has_words(signature: Signature) -> bool {
    signature.1.contains(&"words")
}

/// `Opened(conversation, at, actor, held, carried, model, kind, words)`.
const OPENED: Signature = (
    "Opened",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "model",
        "kind",
        "words",
    ],
);
/// `Titled(conversation, at, actor, held, carried, by, words)`.
const TITLED: Signature = (
    "Titled",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "by",
        "words",
    ],
);
/// `Said(conversation, at, actor, held, carried, turn, by, words)`.
const SAID: Signature = (
    "Said",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "turn",
        "by",
        "words",
    ],
);
/// `Proposed(conversation, at, actor, held, carried, object, revises)`.
const PROPOSED: Signature = (
    "Proposed",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "object",
        "revises",
    ],
);
/// `Reviewed(conversation, at, actor, held, carried, chain, batch, decision, objects, words)`.
const REVIEWED: Signature = (
    "Reviewed",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "chain",
        "batch",
        "decision",
        "objects",
        "words",
    ],
);
/// `ReviewFailed(conversation, at, actor, held, carried, standing, words)`.
const REVIEW_FAILED: Signature = (
    "ReviewFailed",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "standing",
        "words",
    ],
);
/// `Stopped(conversation, at, actor, held, carried, by)`.
const STOPPED: Signature = (
    "Stopped",
    &["conversation", "at", "actor", "held", "carried", "by"],
);
/// `Rewound(conversation, at, actor, held, carried, to, by)`.
const REWOUND: Signature = (
    "Rewound",
    &["conversation", "at", "actor", "held", "carried", "to", "by"],
);
/// `MemorySeen(conversation, at, actor, held, carried, version)`.
const MEMORY_SEEN: Signature = (
    "MemorySeen",
    &["conversation", "at", "actor", "held", "carried", "version"],
);
/// `Session(conversation, at, actor, held, carried, session, model, cwd, surface)`.
const SESSION: Signature = (
    "Session",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "session",
        "model",
        "cwd",
        "surface",
    ],
);
/// `Unheard(conversation, at, actor, held, carried, session, model)`.
const UNHEARD: Signature = (
    "Unheard",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "session",
        "model",
    ],
);
/// `Kept(conversation, at, actor, held, carried, session, model, cwd, surface, from)`.
const KEPT: Signature = (
    "Kept",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "session",
        "model",
        "cwd",
        "surface",
        "from",
    ],
);
/// `Scoped(conversation, at, actor, held, carried, scope)`.
const SCOPED: Signature = (
    "Scoped",
    &["conversation", "at", "actor", "held", "carried", "scope"],
);
/// `Coding(conversation, at, actor, held, carried, proposal, session, words)`.
const CODING: Signature = (
    "Coding",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "proposal",
        "session",
        "words",
    ],
);
/// `Erased(conversation, at, actor, held, carried, keys, erasure)`.
const ERASED: Signature = (
    "Erased",
    &[
        "conversation",
        "at",
        "actor",
        "held",
        "carried",
        "keys",
        "erasure",
    ],
);

/// The lines' constructors, in print order.
const LINES: &[Signature] = &[
    OPENED,
    TITLED,
    SAID,
    PROPOSED,
    REVIEWED,
    REVIEW_FAILED,
    STOPPED,
    REWOUND,
    MEMORY_SEEN,
    SESSION,
    UNHEARD,
    KEPT,
    SCOPED,
    CODING,
    ERASED,
];

/// The lines' whitelist: every line's constructor, and `Locked`.
const SIGNATURES: &[Signature] = &[
    OPENED,
    TITLED,
    SAID,
    PROPOSED,
    REVIEWED,
    REVIEW_FAILED,
    STOPPED,
    REWOUND,
    MEMORY_SEEN,
    SESSION,
    UNHEARD,
    KEPT,
    SCOPED,
    CODING,
    ERASED,
    LOCKED,
];

/// The lines' whitelist: their constructors and `Locked`, nothing else.
#[derive(Debug, Clone, Default)]
pub struct LineVocabulary;

impl Vocabulary for LineVocabulary {
    fn signature(&self, name: &str) -> Option<prodrome_core::literal::Signature<'_>> {
        Table(SIGNATURES).find(name)
    }
}

impl Schema for LineEvent {
    type Vocabulary = LineVocabulary;
    type Key = ConversationId;
    type Register = TranscriptRegister;

    fn registers(_: &LineVocabulary) -> Vec<TranscriptRegister> {
        vec![TranscriptRegister]
    }

    type Registers<'a> = TranscriptRegisters<'a, Self>;

    fn to_value(&self) -> Value {
        let (signature, mut values) = self.bare();
        if has_words(signature) {
            values.push(self.words.to_value());
        }
        record(signature, values)
    }

    fn from_value(_: &LineVocabulary, value: &Value) -> Result<Self, ProdromeError> {
        let fields = Fields::among(value, LINES)?;
        let shape = shape_of(fields)?;
        let words: Vec<Locked<LineSlot>> = if has_words(signature(&shape)) {
            fields.get("words")?
        } else {
            Vec::new()
        };
        if words.len() != shape.slots() {
            return Err(ProdromeError::invalid(format!(
                "{} holds {} slots of words, and its shape {}",
                fields.name(),
                words.len(),
                shape.slots()
            )));
        }
        Ok(Self {
            conversation: fields.get("conversation")?,
            at: fields.get("at")?,
            actor: fields.get("actor")?,
            held: fields.get("held")?,
            carried: fields.get("carried")?,
            shape,
            words,
        })
    }

    fn key(&self) -> &ConversationId {
        &self.conversation
    }

    fn at(&self) -> Datetime {
        self.at
    }

    fn actor(&self) -> &Actor {
        &self.actor
    }

    fn writes(&self) -> impl Iterator<Item = TranscriptRegister> {
        std::iter::once(TranscriptRegister)
    }
}

/// A part of a line still spelled as JSON inside its literal: a turn's shape, who named a
/// conversation, a coding session, an erasure. Changing it moves each slot's binding, so it
/// changes only with a carry that locks every slot again.
fn json(value: &impl Serialize) -> Value {
    Value::str(serde_json::to_string(value).expect("a line's parts are plain data"))
}

/// The JSON part `name` of a stored line, read back.
fn parsed<T: DeserializeOwned>(fields: Fields<'_>, name: &str) -> Result<T, ProdromeError> {
    serde_json::from_str(&fields.get::<String>(name)?)
        .map_err(|failed| ProdromeError::invalid(format!("{}.{name}: {failed}", fields.name())))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU8, Ordering};

    use prodrome_core::event::{canonical, parse_event};

    use super::*;
    use crate::capability::{ToolCall, Turn};
    use crate::secretary::words::{Gone, Words};

    /// Each key one more than the last.
    #[derive(Default)]
    struct Counting(AtomicU8);

    impl Entropy for Counting {
        fn fill(&self, bytes: &mut [u8]) -> Result<(), String> {
            bytes.fill(self.0.fetch_add(1, Ordering::Relaxed));
            Ok(())
        }
    }

    fn at() -> Datetime {
        Datetime::new(2026, 10, 9, 9, 0, 0, 0).expect("an instant")
    }

    fn id() -> ConversationId {
        ConversationId::new("2026-10-09-1").expect("a slug")
    }

    fn model() -> Entry {
        Entry::Said {
            at: at(),
            turn: Turn::Model {
                text: "Noted: Mira is the accountant.".to_owned(),
                calls: vec![ToolCall {
                    verb: "remember".into(),
                    args: [(
                        "fact".to_owned(),
                        serde_json::json!("Mira is the accountant"),
                    )]
                    .into(),
                }],
            },
            by: None,
        }
    }

    fn written(entry: &Entry) -> (LineEvent, Vec<TextKey>) {
        LineEvent::write(
            id(),
            Actor::new("assistant").expect("an actor"),
            3,
            entry,
            &Counting::default(),
        )
        .expect("a line")
    }

    fn opened_by(keys: &[TextKey]) -> impl Fn(&KeyName) -> Words<TextKey> + '_ {
        |name| {
            keys.iter()
                .find(|key| &key.name() == name)
                .and_then(|key| TextKey::parse(name, key.bytes()))
                .map_or_else(|| Gone::Missing.into(), Words::kept)
        }
    }

    #[test]
    fn a_line_prints_no_words_reads_back_and_opens_to_its_entry() {
        let (line, keys) = written(&model());
        assert_eq!(
            keys.len(),
            2,
            "the prose and the one call, each its own key"
        );
        let print = canonical(&line);
        assert!(
            !print.contains("Mira") && !print.contains("accountant"),
            "{print}"
        );
        let read: LineEvent = parse_event(&LineVocabulary, &print).expect("parses");
        assert_eq!(read, line);
        assert_eq!(read.entry(&opened_by(&keys)), model());
    }

    /// A line's instant is the entry's, to the microsecond: nothing about an instant is
    /// refused on the way in, and it reads back as the same value.
    #[test]
    fn a_line_keeps_its_entrys_instant() {
        let at = Datetime::new(2026, 10, 9, 9, 0, 1, 250_000).expect("an instant");
        let entry = Entry::Stopped {
            at,
            by: "ada".to_owned(),
        };
        let (line, keys) = written(&entry);
        let read: LineEvent = parse_event(&LineVocabulary, &canonical(&line)).expect("parses");
        let back = read.entry(&opened_by(&keys));
        assert_eq!(back.at(), at);
        assert_eq!(back, entry);
    }

    /// One slot's key destroyed erases that slot alone: the prose stays, the call's words go.
    #[test]
    fn one_slot_gone_leaves_the_others() {
        let (line, keys) = written(&model());
        let prose = &keys[..1];
        let Entry::Said {
            turn: Turn::Erased { words, shape },
            ..
        } = line.entry(&opened_by(prose))
        else {
            panic!("a turn with a slot gone is erased");
        };
        assert_eq!(shape.slots(), 2);
        assert!(words[0].get().is_some());
        assert_eq!(words[1].gone(), Some(&Gone::Missing));
    }

    /// A slot's ciphertext opens only in its own slot of its own line.
    #[test]
    fn a_ciphertext_moved_does_not_open() {
        let (line, keys) = written(&model());
        let (other, _) = LineEvent::write(
            id(),
            Actor::new("assistant").expect("an actor"),
            4,
            &model(),
            &Counting::default(),
        )
        .expect("a line");
        let moved = line.locked(0).expect("a slot");
        assert!(
            moved.open(&other.slot(0), &keys[0]).is_none(),
            "another line"
        );
        assert!(
            moved.open(&line.slot(1), &keys[0]).is_none(),
            "another slot"
        );
        assert!(moved.open(&line.slot(0), &keys[0]).is_some(), "its own");
    }

    /// A slot moved into another conversation opens to nothing through the cache too, after
    /// its own line was read and kept: the cache is keyed by the binding opening reads.
    #[test]
    fn a_moved_slot_reads_missing_after_the_original_was_read() {
        let (line, keys) = written(&model());
        let mut moved = line.clone();
        moved.conversation = ConversationId::new("2026-10-09-elsewhere").expect("a slug");
        let opened = crate::secretary::opened::Opened::default();
        let cached = crate::secretary::opened::Cached {
            slots: opened_by(&keys),
            opened: &opened,
        };
        assert_eq!(line.entry(&cached), model(), "the original reads whole");
        let Entry::Said {
            turn: Turn::Erased { words, .. },
            ..
        } = moved.entry(&cached)
        else {
            panic!("a moved slot does not open");
        };
        assert!(words.iter().all(|slot| slot.gone() == Some(&Gone::Missing)));
    }

    /// Every count prints as itself and reads back, at the widest it holds.
    #[test]
    fn every_count_round_trips_at_its_widest() {
        for entry in [
            Entry::Rewound {
                to: usize::MAX,
                at: at(),
                by: "ada".to_owned(),
            },
            Entry::MemorySeen {
                at: at(),
                version: u64::MAX,
            },
            Entry::Proposed {
                at: at(),
                object: crate::secretary::inbox::ProposalId::new("p0").expect("an id"),
                revises: Some(usize::MAX),
            },
            Entry::ReviewFailed {
                at: at(),
                reason: Words::kept("down".to_owned()),
                standing: usize::MAX,
            },
        ] {
            let (line, _) = LineEvent::write(
                id(),
                Actor::new("assistant").expect("an actor"),
                u32::MAX,
                &entry,
                &Counting::default(),
            )
            .expect("a line");
            let print = canonical(&line);
            assert_eq!(parse_event(&LineVocabulary, &print), Ok(line), "{print}");
        }
    }

    #[test]
    fn an_erasure_prints_and_reads_back() {
        let line = LineEvent::erasure(
            id(),
            Datetime::new(2026, 10, 9, 9, 1, 0, 0).expect("an instant"),
            Actor::new("assistant").expect("an actor"),
            5,
            [KeyName::new("a".repeat(64)).expect("a name")].into(),
            Erasure::Forgot {
                fact: crate::secretary::memory::FactId(2),
            },
        );
        let read: LineEvent = parse_event(&LineVocabulary, &canonical(&line)).expect("parses");
        assert_eq!(read, line);
    }

    #[test]
    fn no_constructor_is_a_term_or_an_envelope() {
        crate::literal::assert_own(SIGNATURES);
    }
}
