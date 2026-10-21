//! A history's image under a map of its events: the same DAG, begun by another `Genesis`, each
//! `Change`'s event rewritten by the map and every name it rests on renamed to that object's
//! image.
//!
//! Nothing is linearised or replayed, so concurrent writes stay concurrent and the image of a
//! reading is the reading of the image; under the history's own genesis the identity map is the
//! history, byte for byte, and images compose as their maps do. It works on the stored literals,
//! so a map may carry objects of a form no reader admits any longer into the form one does; the
//! store that takes the image admits each object through its own reader.

use std::collections::BTreeMap;

use prodrome_core::event::{EventVocabulary, Hash};
use prodrome_core::genesis::Genesis;
use prodrome_core::literal::{Call, Value, parse_literal, print_literal};
use prodrome_core::schema::Schema;
use sha2::{Digest, Sha256};

/// A history's image: its objects, and the name each object of the history became.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Every object, its name beside its bytes, each after every object it rests on.
    pub objects: Vec<(Hash, String)>,
    /// Each object of the history, by name, to its image's.
    pub names: BTreeMap<Hash, Hash>,
}

/// A history that has no image: it is not one prodrome of `Change`s, or the map refused it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotAnImage {
    /// An object whose bytes do not hash to its name.
    #[error("object {0} is not named by its bytes")]
    Misnamed(String),
    /// An object that does not read as a literal of the schema's envelopes.
    #[error("object {object} does not read: {why}")]
    Unread {
        /// Its name.
        object: String,
        /// The reader's words.
        why: String,
    },
    /// Not exactly one `Genesis`: a legacy store, an empty one, or several prodromes.
    #[error("a history of one prodrome has one Genesis; this one has {0}")]
    Geneses(usize),
    /// An object that is neither the genesis nor one of its prodrome's changes.
    #[error("object {object} is {what}, which a history of changes does not hold")]
    Foreign {
        /// Its name.
        object: String,
        /// What it is instead.
        what: String,
    },
    /// A change resting on an object the history does not hold.
    #[error("object {object} rests on {missing}, which the history does not hold")]
    Missing {
        /// Its name.
        object: String,
        /// The name it rests on.
        missing: String,
    },
    /// Changes that rest on each other.
    #[error("object {0} rests, through others, on itself")]
    Cycle(String),
    /// The map refused an event.
    #[error("the map refused object {object}: {why}")]
    Refused {
        /// Its name.
        object: String,
        /// The map's words.
        why: String,
    },
}

/// The name of a print: the SHA-256 of its bytes, as every object is named.
fn named(print: &str) -> Hash {
    Hash::new(hex::encode(Sha256::digest(print.as_bytes()))).expect("a SHA-256 in hex is a name")
}

/// One change of the history, as stored.
struct Old<'h> {
    deps: Vec<Hash>,
    event: &'h Value,
}

impl<'h> Old<'h> {
    /// A `Change` of the prodrome `genesis` begins, or why the object is not one.
    fn of(object: &Hash, call: &'h Call, genesis: &Hash) -> Result<Self, NotAnImage> {
        let foreign = |what: String| NotAnImage::Foreign {
            object: object.as_str().to_owned(),
            what,
        };
        if call.name != "Change" {
            return Err(foreign(format!("a {}", call.name)));
        }
        let unread = |why: &str| NotAnImage::Unread {
            object: object.as_str().to_owned(),
            why: why.to_owned(),
        };
        if call.field("genesis").and_then(Value::as_str) != Some(genesis.as_str()) {
            return Err(foreign("a change of another prodrome".to_owned()));
        }
        let deps = call
            .field("deps")
            .and_then(Value::as_tuple)
            .ok_or_else(|| unread("Change.deps is not a tuple"))?
            .iter()
            .map(|dep| {
                dep.as_str()
                    .and_then(|name| Hash::new(name).ok())
                    .ok_or_else(|| unread("Change.deps holds a non-name"))
            })
            .collect::<Result<_, _>>()?;
        let event = call
            .field("event")
            .ok_or_else(|| unread("Change has no event"))?;
        Ok(Self { deps, event })
    }
}

/// The image of `history`, each object its name and its stored text, read at the schema `E`:
/// begun by `genesis`, each change's event rewritten by `event`, which is told the object's name.
///
/// # Errors
/// [`NotAnImage`], naming the object.
pub fn image<E: Schema>(
    vocabulary: &E::Vocabulary,
    history: impl IntoIterator<Item = (Hash, String)>,
    genesis: &Genesis,
    event: impl Fn(&Hash, &Value) -> Result<Value, String>,
) -> Result<Image, NotAnImage> {
    let words = EventVocabulary::<E>::new(vocabulary);
    let mut calls = BTreeMap::new();
    for (object, text) in history {
        if named(&text) != object {
            return Err(NotAnImage::Misnamed(object.as_str().to_owned()));
        }
        let unread = |why: String| NotAnImage::Unread {
            object: object.as_str().to_owned(),
            why,
        };
        match parse_literal(&text, &words).map_err(|failed| unread(failed.to_string()))? {
            Value::Call(call) => calls.insert(object, call),
            _ => return Err(unread("not a constructor call".to_owned())),
        };
    }
    let roots: Vec<&Hash> = calls
        .iter()
        .filter(|(_, call)| call.name == "Genesis")
        .map(|(object, _)| object)
        .collect();
    let [old] = roots[..] else {
        return Err(NotAnImage::Geneses(roots.len()));
    };
    let mut pending = calls
        .iter()
        .filter(|(object, _)| *object != old)
        .map(|(object, call)| Ok((object, Old::of(object, call, old)?)))
        .collect::<Result<Vec<_>, NotAnImage>>()?;
    for (object, change) in &pending {
        if let Some(missing) = change.deps.iter().find(|dep| !calls.contains_key(*dep)) {
            return Err(NotAnImage::Missing {
                object: object.as_str().to_owned(),
                missing: missing.as_str().to_owned(),
            });
        }
    }
    let begun = print_literal(&genesis.to_value());
    let mut names = BTreeMap::from([(old.clone(), named(&begun))]);
    let mut objects = vec![(names[old].clone(), begun)];
    let new_genesis = Value::str(names[old].as_str());
    // Parents first: each pass takes every change whose deps all have images, so a pass that
    // takes none has found a cycle, which names that are content hashes cannot make.
    while !pending.is_empty() {
        let before = pending.len();
        let mut waiting = Vec::with_capacity(before);
        for (object, change) in pending {
            let Some(mut deps) = change
                .deps
                .iter()
                .map(|dep| names.get(dep).cloned())
                .collect::<Option<Vec<Hash>>>()
            else {
                waiting.push((object, change));
                continue;
            };
            deps.sort();
            let mapped = event(object, change.event).map_err(|why| NotAnImage::Refused {
                object: object.as_str().to_owned(),
                why,
            })?;
            let print = print_literal(&Value::call(
                "Change",
                vec![
                    ("genesis".to_owned(), new_genesis.clone()),
                    (
                        "deps".to_owned(),
                        Value::Tuple(deps.iter().map(|dep| Value::str(dep.as_str())).collect()),
                    ),
                    ("event".to_owned(), mapped),
                ],
            ));
            let name = named(&print);
            names.insert(object.clone(), name.clone());
            objects.push((name, print));
        }
        if let Some((object, _)) = waiting.first().filter(|_| waiting.len() == before) {
            return Err(NotAnImage::Cycle(object.as_str().to_owned()));
        }
        pending = waiting;
    }
    Ok(Image { objects, names })
}
