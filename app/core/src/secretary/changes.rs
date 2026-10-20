//! A changeset: chain acts the secretary is not sure of, performed only by the owner's tap.
//!
//! Acts are stored and events are not, because an `Authored` built at proposal time would undo
//! whatever moved on the chain since; each act goes through the same builders a direct write does.

use prodrome_core::literal::{ProdromeError, Value as LiteralValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::agent::priced::{PRICE_SPELLINGS, Priced};
use crate::authoring::{self, NewTodo, Writer};
use crate::believed::Preview;
use crate::capability::{Agent, Datetime, Typeset};
use crate::chains::ChainPolicy;
use crate::literal::{Fields, Literal, Signature, arm, no_arm, record};
use crate::record::{Node, TodoEvent};
use crate::refusal::{Refusal, shown as quoted};
use crate::todo::{Patch, Price, TodoFields, TodoPatch};
use crate::zone::Moment;

/// One chain act the secretary wants made, each arm backed by an existing builder.
///
/// No `Reopen` or `Undo`, since mail never suggests one and the last event may be the owner's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "act", rename_all = "lowercase")]
pub enum Act {
    /// Author a todo that does not exist yet.
    Add {
        /// Its id.
        todo: String,
        /// Everything else.
        #[serde(flatten)]
        fields: TodoFields,
    },
    /// Change fields on a todo that does.
    Update {
        /// Which todo.
        todo: String,
        /// What changes. Only what is named.
        #[serde(flatten)]
        fields: TodoPatch,
    },
    /// Close it.
    Complete {
        /// Which todo.
        todo: String,
        /// What happened, in a sentence. "" for none.
        note: String,
    },
    /// Drop it.
    Cancel {
        /// Which todo.
        todo: String,
        /// Why, in a sentence. "" for none.
        note: String,
    },
    /// A pass at a recurring todo: done again, and still open.
    Tend {
        /// Which todo.
        todo: String,
        /// What happened, in a sentence. "" for none.
        note: String,
    },
    /// Move the number, with a fresh reason for it.
    Reprice {
        /// Which todo.
        todo: String,
        /// The new price.
        priority: Price,
        /// Why that number now.
        rationale: String,
    },
}

/// Which act a model's record names, and on which todo.
#[derive(Deserialize, JsonSchema)]
struct Named {
    todo: String,
    act: String,
}

/// What an `add` reads beside its price.
#[derive(Deserialize, JsonSchema)]
struct Adding {
    body: String,
    rationale: String,
    #[serde(default = "crate::todo::fields::plain_kind")]
    kind: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    detail: String,
    #[serde(default)]
    waiting_on: String,
}

/// What an `update` reads beside its price.
#[derive(Deserialize, JsonSchema)]
struct Updating {
    body: Option<String>,
    detail: Option<String>,
    detail_append: Option<String>,
    rationale: Option<String>,
    waiting_on: Option<String>,
    #[serde(default)]
    clear_waiting_on: bool,
}

/// What `complete`, `cancel` and `tend` read.
#[derive(Deserialize, JsonSchema)]
struct Noted {
    #[serde(default)]
    note: String,
}

/// What a `reprice` reads beside its price.
#[derive(Deserialize, JsonSchema)]
struct Repricing {
    rationale: String,
}

impl Act {
    /// Which todo this act is about.
    #[must_use]
    pub fn todo(&self) -> &str {
        match self {
            Act::Add { todo, .. }
            | Act::Update { todo, .. }
            | Act::Complete { todo, .. }
            | Act::Cancel { todo, .. }
            | Act::Tend { todo, .. }
            | Act::Reprice { todo, .. } => todo,
        }
    }

    /// The word a refusal and a record line name it by.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Act::Add { .. } => "add",
            Act::Update { .. } => "update",
            Act::Complete { .. } => "complete",
            Act::Cancel { .. } => "cancel",
            Act::Tend { .. } => "tend",
            Act::Reprice { .. } => "reprice",
        }
    }

    /// Reads one out of the JSON a model wrote: which act and on which todo, its price in any
    /// spelling, then only the fields that act takes.
    ///
    /// # Errors
    /// [`Refusal::bad_request`] for an unknown act, a bad field, or two prices at once.
    pub fn of(fields: &Map<String, Value>) -> Result<Self, Refusal> {
        let Named { todo, act } = crate::verb::parse(fields)?;
        let priority = crate::verb::parse::<Priced>(fields)?.price()?;
        match act.as_str() {
            "add" => {
                let Adding {
                    body,
                    rationale,
                    kind,
                    category,
                    detail,
                    waiting_on,
                } = crate::verb::parse(fields)?;
                Ok(Act::Add {
                    todo,
                    fields: TodoFields {
                        body,
                        rationale,
                        kind,
                        category,
                        price: priority.ok_or_else(|| Refusal::bad_request(PRICE_SPELLINGS))?,
                        detail,
                        waiting_on,
                    },
                })
            }
            "update" => {
                let Updating {
                    body,
                    detail,
                    detail_append,
                    rationale,
                    waiting_on,
                    clear_waiting_on,
                } = crate::verb::parse(fields)?;
                Ok(Act::Update {
                    todo,
                    fields: TodoPatch {
                        body: body.into(),
                        detail: detail.into(),
                        detail_append: detail_append.into(),
                        rationale: rationale.into(),
                        price: priority.into(),
                        waiting_on: waiting_on.into(),
                        clear_waiting_on,
                        ..TodoPatch::default()
                    },
                })
            }
            "complete" => Ok(Act::Complete {
                todo,
                note: crate::verb::parse::<Noted>(fields)?.note,
            }),
            "cancel" => Ok(Act::Cancel {
                todo,
                note: crate::verb::parse::<Noted>(fields)?.note,
            }),
            "tend" => Ok(Act::Tend {
                todo,
                note: crate::verb::parse::<Noted>(fields)?.note,
            }),
            "reprice" => {
                let priority = priority.ok_or_else(|| {
                    Refusal::bad_request(
                        "a reprice names the new price: `priority` as a percentage, the \
                         `deadline_*` fields, or a cycle (`recur_every_days` or \
                         `periodic_every_days`)",
                    )
                })?;
                Ok(Act::Reprice {
                    todo,
                    priority,
                    rationale: crate::verb::parse::<Repricing>(fields)?.rationale,
                })
            }
            other => Err(Refusal::bad_request(format!(
                "no act named {} — an act is one of add, update, complete, cancel, tend, reprice",
                quoted(other)
            ))),
        }
    }

    /// The event this act is, against `nodes`: the chain plus the changeset's earlier acts.
    ///
    /// # Errors
    /// Whatever the builder refuses.
    pub fn event(
        &self,
        nodes: &[Node],
        now: Moment,
        by: Writer,
        categories: &[&str],
        typeset: &dyn Typeset,
    ) -> Result<TodoEvent, Refusal> {
        let at = now.at;
        match self {
            Act::Add { todo, fields } => {
                if crate::todos::current(nodes, at, todo).is_some() {
                    return Err(Refusal::conflict(format!(
                        "todo {} already exists — update it instead of adding it again",
                        quoted(todo)
                    )));
                }
                authoring::build_todo(
                    &NewTodo {
                        todo: todo.clone(),
                        fields: fields.clone(),
                        source: None,
                        subtodos: Vec::new(),
                    },
                    now,
                    by.actor(),
                    categories,
                    typeset,
                )
            }
            Act::Update { todo, fields } => {
                let before = existing(nodes, at, todo)?;
                authoring::update_todo(&before, fields, now, by.actor(), categories, "", typeset)
            }
            Act::Reprice {
                todo,
                priority,
                rationale,
            } => {
                let before = existing(nodes, at, todo)?;
                authoring::update_todo(
                    &before,
                    &TodoPatch {
                        rationale: Patch::Set(rationale.clone()),
                        price: Patch::Set(priority.clone()),
                        ..TodoPatch::default()
                    },
                    now,
                    by.actor(),
                    categories,
                    "",
                    typeset,
                )
            }
            Act::Complete { todo, note } => lifecycle(nodes, todo, note, |t, n| {
                prodrome_core::event::mk_completed(t, at, by.actor(), n)
            }),
            Act::Cancel { todo, note } => lifecycle(nodes, todo, note, |t, n| {
                prodrome_core::event::mk_cancelled(t, at, by.actor(), n)
            }),
            Act::Tend { todo, note } => lifecycle(nodes, todo, note, |t, n| {
                prodrome_core::event::mk_tended(t, at, by.actor(), n)
            }),
        }
    }
}

/// The record an edit is against, or the refusal that says there is none.
fn existing(nodes: &[Node], at: Datetime, todo: &str) -> Result<crate::record::Authored, Refusal> {
    crate::todos::current(nodes, at, todo)
        .ok_or_else(|| Refusal::not_found(format!("no todo with id {} on the chain", quoted(todo))))
}

/// The typo guard `record` has: some event on the chain must already mention the todo.
fn known(nodes: &[Node], todo: &str) -> Result<(), Refusal> {
    if nodes
        .iter()
        .filter_map(|node| node.event())
        .any(|event| event.todo().as_str() == todo)
    {
        Ok(())
    } else {
        Err(Refusal::not_found(format!(
            "unknown todo id {} — no event on the chain mentions it",
            quoted(todo)
        )))
    }
}

/// A lifecycle act: the typo guard, then the constructor.
fn lifecycle(
    nodes: &[Node],
    todo: &str,
    note: &str,
    build: impl FnOnce(&str, &str) -> Result<TodoEvent, prodrome_core::literal::ProdromeError>,
) -> Result<TodoEvent, Refusal> {
    known(nodes, todo)?;
    build(todo, note).map_err(|failed| Refusal::bad_request(failed.to_string()))
}

/// The acts in order, never empty, deserialisation included.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawChangeset", into = "RawChangeset")]
pub struct Changeset {
    chain: String,
    acts: Vec<Act>,
}

/// The shape on a stored line, and the only thing [`Changeset`] parses from.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawChangeset {
    chain: String,
    acts: Vec<Act>,
}

/// A changeset that changes nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a changeset names at least one act — add, update, complete, cancel, tend or reprice")]
pub struct NoActs;

impl Changeset {
    /// Build one.
    ///
    /// # Errors
    /// [`NoActs`] for an empty list.
    pub fn new(chain: impl Into<String>, acts: Vec<Act>) -> Result<Self, NoActs> {
        if acts.is_empty() {
            return Err(NoActs);
        }
        Ok(Self {
            chain: chain.into(),
            acts,
        })
    }

    /// The one chain these acts are against, named when proposed.
    #[must_use]
    pub fn chain(&self) -> &str {
        &self.chain
    }

    /// The acts. Never empty.
    #[must_use]
    pub fn acts(&self) -> &[Act] {
        &self.acts
    }
}

impl TryFrom<RawChangeset> for Changeset {
    type Error = NoActs;
    fn try_from(raw: RawChangeset) -> Result<Self, NoActs> {
        Self::new(raw.chain, raw.acts)
    }
}

impl From<Changeset> for RawChangeset {
    fn from(changeset: Changeset) -> RawChangeset {
        RawChangeset {
            chain: changeset.chain,
            acts: changeset.acts,
        }
    }
}

/// Every act as an event, the one build pass the diff, the proposal check and the tap share.
///
/// # Errors
/// The first [`Refusal`], naming the act by position and by what it is.
pub fn events_of(
    changeset: &Changeset,
    chain: &Preview,
    now: Moment,
    by: Writer,
    categories: &[&str],
    typeset: &dyn Typeset,
) -> Result<Vec<TodoEvent>, Refusal> {
    let mut preview = chain.clone();
    let mut built = Vec::with_capacity(changeset.acts().len());
    for (index, act) in changeset.acts().iter().enumerate() {
        let event = act
            .event(preview.nodes(), now, by, categories, typeset)
            .map_err(|failed| named(index, act, &failed))?;
        preview
            .ahead(event.clone())
            .map_err(|failed| named(index, act, &Refusal::internal(failed.to_string())))?;
        built.push(event);
    }
    Ok(built)
}

/// One act's refusal, with the act's own name in front of it.
fn named(index: usize, act: &Act, failed: &Refusal) -> Refusal {
    Refusal::new(
        failed.status(),
        format!(
            "act {} ({} {}) was refused, so nothing was appended: {}",
            index + 1,
            act.word(),
            act.todo(),
            failed.message()
        ),
    )
}

/// Why a changeset did not land, by whether anything did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unapplied {
    /// Refused before the first append: nothing is on the chain.
    Refused(Refusal),
    /// An append failed: the acts before it are on the chain, and the one
    /// that failed may be.
    Appending(Refusal),
}

/// Whether `changeset` would build against `chain` as it stands at `at`: every act through the
/// builders a tap uses, each body through the markup door, nothing appended. A proposed
/// changeset the chain would refuse is refused while it is fixable, whoever proposed it.
///
/// # Errors
/// The chain's refusal of the first act it would not build, or a chain that cannot be read.
pub fn checked(
    changeset: &Changeset,
    chain: &dyn crate::store::Prodrome<TodoEvent>,
    now: Moment,
    categories: &[&str],
    typeset: &dyn Typeset,
) -> Result<(), Refusal> {
    let preview =
        Preview::of(chain.objects()?).map_err(|failed| Refusal::internal(failed.to_string()))?;
    events_of(
        changeset,
        &preview,
        now,
        crate::secretary::WRITES_AS,
        categories,
        typeset,
    )
    .map(|_| ())
}

/// Applies a changeset under the write lock, building and verifying every act before any append.
///
/// Not a transaction: an append that fails leaves the earlier ones, and the refusal says how many.
///
/// # Errors
/// [`Unapplied::Refused`] before the first append, [`Unapplied::Appending`] after it.
pub fn apply(
    caps: &Agent<'_>,
    policy: &ChainPolicy,
    categories: &[&str],
    zone: crate::zone::Zone,
    changeset: &Changeset,
    by: Writer,
) -> Result<Vec<prodrome_core::event::Hash>, Unapplied> {
    if changeset.chain() != policy.name.as_str() {
        return Err(Unapplied::Refused(Refusal::internal(format!(
            "this changeset is against {}, and this is {}",
            quoted(changeset.chain()),
            policy.name.as_str()
        ))));
    }
    let _held = caps
        .chain
        .locked()
        .map_err(|failed| Unapplied::Refused(Refusal::internal(failed.to_string())))?;
    let at = caps.clock.now();
    let chain = caps
        .chain
        .objects()
        .map_err(|failed| Refusal::internal(failed.to_string()))
        .and_then(|objects| {
            Preview::of(objects).map_err(|failed| Refusal::internal(failed.to_string()))
        })
        .map_err(Unapplied::Refused)?;
    let built = events_of(
        changeset,
        &chain,
        Moment { at, zone },
        by,
        categories,
        caps.render,
    )
    .map_err(Unapplied::Refused)?;

    // Rendered and read back before anything lands, as a direct write is.
    let mut preview = chain;
    for (index, event) in built.iter().enumerate() {
        preview
            .ahead(event.clone())
            .map_err(|failed| Unapplied::Refused(Refusal::internal(failed.to_string())))?;
        if let TodoEvent::Authored(record) = event {
            crate::todos::render_current(preview.nodes(), at, &policy.untrusted, record).map_err(
                |failed| Unapplied::Refused(named(index, &changeset.acts()[index], &failed)),
            )?;
        }
    }

    let mut names = Vec::with_capacity(built.len());
    for (index, event) in built.into_iter().enumerate() {
        let name = caps.chain.append(event).map_err(|failed| {
            Unapplied::Appending(Refusal::internal(format!(
                "act {} of this changeset could not be appended ({failed}) — {} act(s) \
                 before it are on the chain",
                index + 1,
                index
            )))
        })?;
        names.push(name);
    }
    Ok(names)
}

/// The changeset drawn as a batch's diff, or the refusal of an act that no longer builds.
#[must_use]
pub fn shown(
    changeset: &Changeset,
    chain: &Preview,
    now: Moment,
    by: Writer,
    categories: &[&str],
    typeset: &dyn Typeset,
) -> (Vec<crate::wire::ProposedObject>, String) {
    match events_of(changeset, chain, now, by, categories, typeset) {
        Ok(built) => (
            crate::proposals::diff::shown(
                chain.nodes(),
                &built
                    .iter()
                    .enumerate()
                    .map(|(index, event)| crate::proposals::diff::Proposed {
                        // An act is unsealed, so its position is its only name.
                        name: format!("{}", index + 1),
                        event: Some(event),
                        needs: Vec::new(),
                    })
                    .collect::<Vec<_>>(),
            ),
            String::new(),
        ),
        Err(refusal) => (Vec::new(), refusal.message().to_owned()),
    }
}

/// The `acts` argument: an array of records, since a flat string would be a second grammar.
///
/// # Errors
/// [`Refusal::bad_request`] when it is not an array of objects that are acts.
pub fn acts_of(acts: Option<&Value>) -> Result<Vec<Act>, Refusal> {
    let name = "acts";
    let Some(Value::Array(listed)) = acts else {
        return Err(Refusal::bad_request(format!(
            "{name} must be an array of acts, each one an object with an `act` of add, update, \
             complete, cancel, tend or reprice"
        )));
    };
    listed
        .iter()
        .enumerate()
        .map(|(index, one)| {
            let Value::Object(fields) = one else {
                return Err(Refusal::bad_request(format!(
                    "{name}[{index}] must be an object with an `act` field"
                )));
            };
            Act::of(fields).map_err(|failed| {
                Refusal::new(
                    failed.status(),
                    format!("act {}: {}", index + 1, failed.message()),
                )
            })
        })
        .collect()
}

/// `Changes(chain, acts)`.
pub const CHANGES: Signature = ("Changes", &["chain", "acts"]);
/// `Add(todo, ...)`: a new todo's fields, its price under `priority`.
pub const ADD: Signature = (
    "Add",
    &[
        "todo",
        "body",
        "rationale",
        "kind",
        "category",
        "priority",
        "detail",
        "waiting_on",
    ],
);
/// `Update(todo, ...)`: each field `None` where it is kept.
pub const UPDATE: Signature = (
    "Update",
    &[
        "todo",
        "body",
        "detail",
        "detail_append",
        "rationale",
        "priority",
        "waiting_on",
        "clear_waiting_on",
        "category",
        "subtodos",
        "defer_to",
    ],
);
/// `Complete(todo, note)`.
pub const COMPLETE: Signature = ("Complete", &["todo", "note"]);
/// `Cancel(todo, note)`.
pub const CANCEL: Signature = ("Cancel", &["todo", "note"]);
/// `Tend(todo, note)`.
pub const TEND: Signature = ("Tend", &["todo", "note"]);
/// `Reprice(todo, priority, rationale)`.
pub const REPRICE: Signature = ("Reprice", &["todo", "priority", "rationale"]);

impl Literal for Act {
    fn to_value(&self) -> LiteralValue {
        let noted = |signature, todo: &String, note: &String| {
            record(signature, vec![todo.to_value(), note.to_value()])
        };
        match self {
            Act::Add { todo, fields } => record(
                ADD,
                vec![
                    todo.to_value(),
                    fields.body.to_value(),
                    fields.rationale.to_value(),
                    fields.kind.to_value(),
                    fields.category.to_value(),
                    fields.price.to_value(),
                    fields.detail.to_value(),
                    fields.waiting_on.to_value(),
                ],
            ),
            Act::Update { todo, fields } => record(
                UPDATE,
                vec![
                    todo.to_value(),
                    fields.body.to_value(),
                    fields.detail.to_value(),
                    fields.detail_append.to_value(),
                    fields.rationale.to_value(),
                    fields.price.to_value(),
                    fields.waiting_on.to_value(),
                    fields.clear_waiting_on.to_value(),
                    fields.category.to_value(),
                    fields.subtodos.to_value(),
                    fields.defer_to.to_value(),
                ],
            ),
            Act::Complete { todo, note } => noted(COMPLETE, todo, note),
            Act::Cancel { todo, note } => noted(CANCEL, todo, note),
            Act::Tend { todo, note } => noted(TEND, todo, note),
            Act::Reprice {
                todo,
                priority,
                rationale,
            } => record(
                REPRICE,
                vec![todo.to_value(), priority.to_value(), rationale.to_value()],
            ),
        }
    }

    fn from_value(value: &LiteralValue) -> Result<Self, ProdromeError> {
        let noted = |signature| -> Result<(String, String), ProdromeError> {
            let fields = Fields::of(value, signature)?;
            Ok((fields.get("todo")?, fields.get("note")?))
        };
        Ok(match arm(value)? {
            "Add" => {
                let fields = Fields::of(value, ADD)?;
                Act::Add {
                    todo: fields.get("todo")?,
                    fields: TodoFields {
                        body: fields.get("body")?,
                        rationale: fields.get("rationale")?,
                        kind: fields.get("kind")?,
                        category: fields.get("category")?,
                        price: fields.get("priority")?,
                        detail: fields.get("detail")?,
                        waiting_on: fields.get("waiting_on")?,
                    },
                }
            }
            "Update" => {
                let fields = Fields::of(value, UPDATE)?;
                Act::Update {
                    todo: fields.get("todo")?,
                    fields: TodoPatch {
                        body: fields.get("body")?,
                        detail: fields.get("detail")?,
                        detail_append: fields.get("detail_append")?,
                        rationale: fields.get("rationale")?,
                        price: fields.get("priority")?,
                        waiting_on: fields.get("waiting_on")?,
                        clear_waiting_on: fields.get("clear_waiting_on")?,
                        category: fields.get("category")?,
                        subtodos: fields.get("subtodos")?,
                        defer_to: fields.get("defer_to")?,
                    },
                }
            }
            "Complete" => {
                let (todo, note) = noted(COMPLETE)?;
                Act::Complete { todo, note }
            }
            "Cancel" => {
                let (todo, note) = noted(CANCEL)?;
                Act::Cancel { todo, note }
            }
            "Tend" => {
                let (todo, note) = noted(TEND)?;
                Act::Tend { todo, note }
            }
            "Reprice" => {
                let fields = Fields::of(value, REPRICE)?;
                Act::Reprice {
                    todo: fields.get("todo")?,
                    priority: fields.get("priority")?,
                    rationale: fields.get("rationale")?,
                }
            }
            other => return Err(no_arm("a chain act", other)),
        })
    }
}

impl Literal for Changeset {
    fn to_value(&self) -> LiteralValue {
        record(CHANGES, vec![self.chain.to_value(), self.acts.to_value()])
    }

    fn from_value(value: &LiteralValue) -> Result<Self, ProdromeError> {
        let fields = Fields::of(value, CHANGES)?;
        Changeset::new(fields.get::<String>("chain")?, fields.get("acts")?)
            .map_err(|failed| ProdromeError::invalid(failed.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Parses;

    fn args(text: &str) -> Map<String, Value> {
        serde_json::from_str(text).expect("an object")
    }

    #[test]
    fn an_act_round_trips_and_names_itself() {
        let act = Act::Add {
            todo: "permitfiling".to_owned(),
            fields: TodoFields {
                body: "File the permit renewal".to_owned(),
                rationale: "A real consequence within weeks.".to_owned(),
                kind: "todo".to_owned(),
                category: String::new(),
                price: Price::Flat { percent: 40 },
                detail: String::new(),
                waiting_on: String::new(),
            },
        };
        let printed = serde_json::to_string(&act).expect("an act serialises");
        assert!(printed.contains(r#""act":"add""#), "{printed}");
        assert!(printed.contains(r#""how":"flat""#), "{printed}");
        let read: Act = serde_json::from_str(&printed).expect("and reads back");
        assert_eq!(read, act);
    }

    #[test]
    fn a_changeset_with_no_acts_is_refused_everywhere() {
        assert_eq!(Changeset::new("life", Vec::new()).unwrap_err(), NoActs);
        assert!(serde_json::from_str::<Changeset>(r#"{"chain":"life","acts":[]}"#).is_err());
        let one: Changeset = serde_json::from_str(
            r#"{"chain":"life","acts":[{"act":"complete","todo":"x","note":""}]}"#,
        )
        .expect("one act");
        assert_eq!(one.chain(), "life");
        assert_eq!(one.acts().len(), 1);
    }

    #[test]
    fn an_act_is_read_out_of_the_json_a_model_writes() {
        let listed = acts_of(
            args(
                r#"{"acts": [
                    {"act": "add", "todo": "newthing", "body": "Do it",
                     "rationale": "because", "priority": "45%"},
                    {"act": "complete", "todo": "oldthing", "note": "done"},
                    {"act": "reprice", "todo": "oldthing", "priority": "20%",
                     "rationale": "a real consequence tomorrow"}
                ]}"#,
            )
            .get("acts"),
        )
        .expect("three acts");
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].word(), "add");
        assert_eq!(listed[0].todo(), "newthing");
        assert_eq!(listed[2].word(), "reprice");
        assert!(matches!(
            &listed[2],
            Act::Reprice {
                priority: Price::Flat { percent: 20 },
                ..
            }
        ));
    }

    #[test]
    fn a_tend_builds_a_pass_at_a_known_todo() {
        let listed = acts_of(
            args(r#"{"acts": [{"act": "tend", "todo": "brush", "note": "vinegar"}]}"#).get("acts"),
        )
        .expect("one act");
        assert_eq!(listed[0].word(), "tend");
        let at = Datetime::new(2026, 9, 25, 12, 0, 0, 0).expect("an instant");
        let known = Preview::chain(&[
            prodrome_core::event::mk_created("brush", at, "ada", "", "").expect("an event")
        ]);
        let built = listed[0]
            .event(
                known.nodes(),
                crate::zone::Moment {
                    at,
                    zone: crate::zone::Zone::UTC,
                },
                Writer::Assistant,
                &[],
                &Parses,
            )
            .expect("a pass");
        assert!(
            matches!(&built, TodoEvent::Tended(pass) if pass.note == "vinegar"),
            "{built:?}"
        );
        assert!(
            listed[0]
                .event(
                    &[],
                    crate::zone::Moment {
                        at,
                        zone: crate::zone::Zone::UTC
                    },
                    Writer::Assistant,
                    &[],
                    &Parses
                )
                .is_err()
        );
    }

    #[test]
    fn an_unknown_act_is_refused_by_name() {
        let refused = acts_of(args(r#"{"acts": [{"act": "delete", "todo": "x"}]}"#).get("acts"))
            .expect_err("no such act");
        assert!(refused.message().contains("no act named"), "{refused:?}");
        assert!(refused.message().contains("cancel"), "{refused:?}");
        assert!(acts_of(args(r#"{"acts": "add one"}"#).get("acts")).is_err());
        assert!(acts_of(args("{}").get("acts")).is_err());
    }

    #[test]
    fn an_act_carries_a_flat_price_or_a_deadline_and_never_both() {
        let refused = acts_of(
            args(
                r#"{"acts": [{"act": "add", "todo": "x", "body": "b", "rationale": "r",
                             "priority": "40%", "deadline_end_date": "2026-10-01"}]}"#,
            )
            .get("acts"),
        )
        .expect_err("two prices");
        assert!(refused.message().contains("ONE of"), "{refused:?}");
    }

    /// Reads, stores, builds and draws a `Recur` reprice.
    #[test]
    fn a_reprice_can_make_a_chore_recur() {
        let listed = acts_of(
            args(
                r#"{"acts": [{"act": "reprice", "todo": "brush", "recur_every_days": 60,
                             "cycle_from_priority": "100%", "cycle_to_priority": "30%",
                             "rationale": "Vinegar it every two months."}]}"#,
            )
            .get("acts"),
        )
        .expect("one act");
        let cycle = crate::todo::Cycle {
            days: 60,
            from_percent: 100,
            to_percent: 30,
        };
        assert!(
            matches!(&listed[0], Act::Reprice { priority: Price::Recur(read), .. } if *read == cycle),
            "{listed:?}"
        );
        let printed = serde_json::to_string(&listed[0]).expect("an act serialises");
        assert!(
            printed.contains(r#""how":"recur","days":60,"from_percent":100,"to_percent":30"#),
            "{printed}"
        );
        assert_eq!(
            serde_json::from_str::<Act>(&printed).expect("and reads back"),
            listed[0]
        );

        let at = Datetime::new(2026, 9, 26, 12, 0, 0, 0).expect("an instant");
        let brush = authoring::build_todo(
            &NewTodo {
                todo: "brush".to_owned(),
                fields: TodoFields {
                    body: "Vinegar the toothbrush".to_owned(),
                    rationale: "A habit.".to_owned(),
                    kind: "todo".to_owned(),
                    category: String::new(),
                    price: Price::Flat { percent: 80 },
                    detail: String::new(),
                    waiting_on: String::new(),
                },
                source: None,
                subtodos: Vec::new(),
            },
            crate::zone::Moment {
                at,
                zone: crate::zone::Zone::UTC,
            },
            "ada",
            &[],
            &Parses,
        )
        .expect("a todo");
        let (rows, problem) = shown(
            &Changeset::new("life", listed.clone()).expect("one act"),
            &Preview::chain(std::slice::from_ref(&brush)),
            crate::zone::Moment {
                at,
                zone: crate::zone::Zone::UTC,
            },
            Writer::Assistant,
            &[],
            &Parses,
        );
        assert_eq!(problem, "");
        assert_eq!(rows[0].act.word(), "repriced");
        let priced = rows[0]
            .act
            .changes()
            .iter()
            .find(|change| change.field == "priority")
            .expect("the price moved");
        assert_eq!(priced.before, "80%");
        assert_eq!(
            priced.after,
            "80%; every 60 days after a pass, from 100% to 30%"
        );

        let TodoEvent::Authored(built) = listed[0]
            .event(
                Preview::chain(&[brush]).nodes(),
                crate::zone::Moment {
                    at,
                    zone: crate::zone::Zone::UTC,
                },
                Writer::Assistant,
                &[],
                &Parses,
            )
            .expect("a reprice")
        else {
            panic!("a reprice is an Authored");
        };
        let spec = built.payload.spec;
        assert!(
            matches!(spec.out(), prodrome_core::term::TermF::Recur { todo, .. } if todo == "brush"),
            "{}",
            prodrome_core::fpl::print_term(&spec)
        );
    }

    #[test]
    fn a_cycle_needs_both_ends_and_a_cadence() {
        for body in [
            r#"{"act": "reprice", "todo": "x", "rationale": "r", "periodic_every_days": 30,
                "cycle_from_priority": "90%"}"#,
            r#"{"act": "reprice", "todo": "x", "rationale": "r",
                "cycle_from_priority": "90%", "cycle_to_priority": "40%"}"#,
            r#"{"act": "reprice", "todo": "x", "rationale": "r", "priority": "40%",
                "recur_every_days": 30, "cycle_from_priority": "90%",
                "cycle_to_priority": "40%"}"#,
        ] {
            assert!(
                acts_of(args(&format!(r#"{{"acts": [{body}]}}"#)).get("acts")).is_err(),
                "{body} was accepted"
            );
        }
    }

    #[test]
    fn a_reprice_names_the_new_price() {
        let refused = acts_of(
            args(r#"{"acts": [{"act": "reprice", "todo": "x", "rationale": "r"}]}"#).get("acts"),
        )
        .expect_err("no price");
        assert!(
            refused.message().contains("names the new price"),
            "{refused:?}"
        );
    }
}
