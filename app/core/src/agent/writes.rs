//! The three verbs that append, shared with the secretary, which differ only in the [`Writer`]
//! each hands down.
//!
//! A write the authoring rules decline answers `{"ok": false, "error": …}` so the agent can retry;
//! a `Refusal` is a call not performed at all.

use std::collections::BTreeSet;

use crate::believed::{Believed, Preview};
use crate::record::{Node, TodoEvent};
use prodrome_core::event::{mk_cancelled, mk_completed, mk_reopened, mk_tended};
use prodrome_core::fpl;
use prodrome_core::literal::ProdromeError;
use serde_json::{Value, json};

use super::events_of;
use super::priced::{PRICE_SPELLINGS, Priced, day_of};
use super::sourced::Sourced;
use crate::authoring::{self, NewTodo, Writer};
use crate::capability::{Agent, Datetime};
use crate::chains::ChainPolicy;
use crate::deployment::Deployment;
use crate::refusal::{Refusal, shown};
use crate::store::Prodrome;
use crate::todo::TodoPatch;
use crate::todos;
use crate::zone::{Moment, Zone};

/// A `(todo, at, actor, note)` event constructor.
type Build = fn(&str, Datetime, &str, &str) -> Result<TodoEvent, ProdromeError>;

/// The lifecycle kinds either surface may record; repricing is a trusted actor's move.
const RECORD_KINDS: [(&str, Build); 4] = [
    ("cancelled", mk_cancelled),
    ("completed", mk_completed),
    ("reopened", mk_reopened),
    ("tended", mk_tended),
];

/// A new todo as either surface's `todo_add` spells it.
pub(crate) struct Adding {
    pub todo_id: String,
    pub body: String,
    pub rationale: String,
    pub kind: String,
    pub category: Option<String>,
    pub priced: Priced,
    pub detail: String,
    pub waiting_on: Option<String>,
    pub sourced: Sourced,
    /// The day it is created on; absent is today.
    pub date: Option<String>,
    pub parent: Option<String>,
}

/// An edit as either surface's `todo_update` spells it; `None` is untouched.
pub(crate) struct Updating {
    pub todo_id: String,
    pub rationale: Option<String>,
    pub priced: Priced,
    pub body: Option<String>,
    pub detail: Option<String>,
    pub detail_append: Option<String>,
    pub waiting_on: Option<String>,
    pub clear_waiting_on: bool,
}

/// One lifecycle event on an existing todo, at the clock's instant so it cannot be backdated.
pub(crate) fn record(
    caps: &Agent<'_>,
    by: Writer,
    kind: &str,
    todo: &str,
    note: &str,
) -> Result<Value, Refusal> {
    let Some(&(_, build)) = RECORD_KINDS.iter().find(|(name, _)| *name == kind) else {
        return Ok(json!({
            "error": format!(
                "kind must be one of {:?}, got {}",
                RECORD_KINDS.map(|(name, _)| name),
                shown(kind)
            ),
        }));
    };

    // Held from the check to the append, so the id cannot stop existing between them.
    let _held = caps.chain.locked()?;
    let events = events_of(caps.chain)?;
    let known: BTreeSet<&str> = events.iter().map(|e| e.todo().as_str()).collect();
    if !known.contains(todo) {
        return Ok(json!({
            "error": format!(
                "unknown todo id {} — no event in the chain mentions it \
                 (typo? new todos have no Created event until the phase-4 step)",
                shown(todo)
            ),
            "known_ids": known.iter().collect::<Vec<_>>(),
        }));
    }

    let at = caps.clock.now();
    let event =
        build(todo, at, by.actor(), note).map_err(|e| Refusal::bad_request(e.to_string()))?;
    let digest = caps.chain.append(event)?;
    Ok(json!({
        "hash": digest.as_str(),
        "kind": kind,
        "todo": todo,
        "reminder": reminder(by),
    }))
}

/// What a `record` tells its caller about what it just did.
fn reminder(by: Writer) -> &'static str {
    match by {
        Writer::Triage => {
            "recorded as actor 'triage' — PROVISIONAL until the owner confirms; \
             it does not bind evaluation (env_at skips untrusted actors)."
        }
        Writer::Assistant => {
            "recorded as actor 'assistant', which is TRUSTED — this binds the fold \
             now. Only record what the owner has said is done."
        }
    }
}

/// A new todo, rendered and verified before anything is appended.
pub(crate) fn todo_add(
    caps: &Agent<'_>,
    deployment: &Deployment,
    chain: &ChainPolicy,
    by: Writer,
    adding: Adding,
) -> Result<Value, Refusal> {
    let todo_id = adding.todo_id.as_str();
    let _held = caps.chain.locked()?;
    let mut preview = Preview::of(caps.chain.objects()?)
        .map_err(|failed| Refusal::internal(failed.to_string()))?;
    let at = caps.clock.now();
    if todos::current(preview.nodes(), at, todo_id).is_some() {
        return Ok(refused_write(&already_spoken_for(
            caps.chain,
            preview.nodes(),
            todo_id,
        )?));
    }
    let known: Vec<&str> = deployment.categories.keys().map(String::as_str).collect();
    let new = NewTodo {
        todo: todo_id.to_owned(),
        fields: crate::todo::TodoFields {
            body: adding.body,
            rationale: adding.rationale,
            kind: adding.kind,
            category: adding.category.unwrap_or_default(),
            price: match adding.priced.price() {
                Ok(Some(price)) => price,
                Ok(None) => return Ok(refused_write(PRICE_SPELLINGS)),
                Err(refusal) => return Ok(refused_write(refusal.message())),
            },
            detail: adding.detail,
            waiting_on: adding.waiting_on.unwrap_or_default(),
        },
        source: match adding.sourced.source() {
            Ok(source) => source,
            Err(refusal) => return Ok(refused_write(refusal.message())),
        },
        subtodos: Vec::new(),
    };
    // `None` keeps `build_todo`'s midnight: `created` is a day.
    let created = match adding.date.as_deref() {
        None => None,
        Some(text) => {
            let day = match day_of(text, "created") {
                Ok(day) => day,
                Err(refusal) => return Ok(refused_write(refusal.message())),
            };
            match fpl::datetime_of(chrono::NaiveDateTime::from(day)) {
                Ok(day) => Some(day),
                Err(e) => return Ok(refused_write(&e.to_string())),
            }
        }
    };
    let now = Moment {
        at,
        zone: deployment.zone,
    };
    let built = match adding.parent.as_deref() {
        None => authoring::build_todo(&new, now, by.actor(), &known, caps.render)
            .map(|child| (child, None)),
        Some(parent) => {
            let Some(parent) = todos::current(preview.nodes(), at, parent) else {
                return Ok(refused_write(&format!(
                    "no todo with id {} on the chain to be a parent",
                    shown(parent)
                )));
            };
            let believed = Believed::of(preview.nodes(), at, &chain.untrusted)
                .map_err(|e| Refusal::internal(e.to_string()))?;
            authoring::build_subtodo(
                &new,
                &parent,
                &believed.linkable(),
                now,
                by.actor(),
                &known,
                caps.render,
            )
            .map(|(child, revision)| (child, Some(revision)))
        }
    };
    let (child, revision) = match built {
        Ok(built) => built,
        Err(refusal) => return Ok(refused_write(refusal.message())),
    };
    let child = match created {
        None => child,
        Some(day) => with_created(child, day),
    };
    append_authored(caps.chain, chain, &mut preview, child, revision, at, true)
}

/// Changed fields on a todo that exists.
pub(crate) fn todo_update(
    caps: &Agent<'_>,
    zone: Zone,
    chain: &ChainPolicy,
    by: Writer,
    updating: Updating,
) -> Result<Value, Refusal> {
    let todo_id = updating.todo_id.as_str();
    let _held = caps.chain.locked()?;
    let mut preview = Preview::of(caps.chain.objects()?)
        .map_err(|failed| Refusal::internal(failed.to_string()))?;
    let at = caps.clock.now();
    let Some(before) = todos::current(preview.nodes(), at, todo_id) else {
        return Ok(refused_write(&format!(
            "no todo with id {} on the chain",
            shown(todo_id)
        )));
    };
    // Category, checklist and deferral are not arguments of this verb; widening it changes the
    // snapshotted surface.
    let edit = TodoPatch {
        rationale: updating.rationale.into(),
        price: match updating.priced.price() {
            Ok(price) => price.into(),
            Err(refusal) => return Ok(refused_write(refusal.message())),
        },
        body: updating.body.into(),
        detail: updating.detail.into(),
        detail_append: updating.detail_append.into(),
        waiting_on: updating.waiting_on.into(),
        clear_waiting_on: updating.clear_waiting_on,
        ..TodoPatch::default()
    };
    let now = Moment { at, zone };
    let event = match authoring::update_todo(&before, &edit, now, by.actor(), &[], "", caps.render)
    {
        Ok(event) => event,
        Err(refusal) => return Ok(refused_write(refusal.message())),
    };
    append_authored(caps.chain, chain, &mut preview, event, None, at, false)
}

fn refused_write(error: &str) -> Value {
    json!({"ok": false, "error": error})
}

/// Why an id is taken, naming the unsettled batch so a run updates rather than re-proposes.
fn already_spoken_for(
    chain: &dyn Prodrome<TodoEvent>,
    nodes: &[Node],
    todo_id: &str,
) -> Result<String, crate::store::StoreError> {
    let settled = chain.settled()?;
    let latest = crate::view::stream_of(nodes, todo_id)
        .into_iter()
        .rev()
        .find(|(_, event)| matches!(event, TodoEvent::Authored(_)))
        .map(|(name, _)| name);
    let standing = latest.and_then(|name| crate::proposals::batch_of(nodes, &settled, &name));
    Ok(match standing {
        None => format!(
            "todo {} already exists — todo_update it, or choose another id",
            shown(todo_id)
        ),
        Some(head) => format!(
            "todo {} is already STANDING in batch {} — it is proposed and nobody has settled \
             it yet. todo_update it instead, so the Inbox holds ONE proposal for this todo \
             and not the same one twice.",
            shown(todo_id),
            crate::proposals::short(&head)
        ),
    })
}

/// A record's `created` replaced by the day the caller named.
fn with_created(event: TodoEvent, created: Datetime) -> TodoEvent {
    match event {
        TodoEvent::Authored(mut record) => {
            record.payload.created = created;
            TodoEvent::Authored(record)
        }
        other => other,
    }
}

/// Render against the chain with the records on it, verify, then append them all as one act.
fn append_authored(
    store: &dyn Prodrome<TodoEvent>,
    chain: &ChainPolicy,
    preview: &mut Preview,
    event: TodoEvent,
    parent: Option<TodoEvent>,
    at: Datetime,
    first: bool,
) -> Result<Value, Refusal> {
    for written in std::iter::once(&event).chain(&parent) {
        preview
            .ahead(written.clone())
            .map_err(|failed| Refusal::internal(failed.to_string()))?;
    }
    let nodes = preview.nodes();
    let shown = rendered(nodes, at, chain, &event).and_then(|child| {
        let parent = parent
            .as_ref()
            .map(|parent| rendered(nodes, at, chain, parent))
            .transpose()?;
        Ok((child, parent))
    });
    let ((todo_id, block), parent_shown) = match shown {
        Ok(shown) => shown,
        Err(refusal) => return Ok(refused_write(refusal.message())),
    };
    for event in std::iter::once(event).chain(parent) {
        store.append(event)?;
    }
    let mut answer = if first {
        json!({
            "ok": true,
            "todo_id": todo_id,
            "block": block,
            "next": format!("call mark(hash, 'todo', todo_id='{todo_id}') to link the message"),
        })
    } else {
        json!({"ok": true, "todo_id": todo_id, "block": block})
    };
    if let Some((parent, block)) = parent_shown {
        answer["parent"] = json!({"todo_id": parent, "block": block});
    }
    Ok(answer)
}

/// One written record's id, day and block, priced at the record's own instant.
fn rendered(
    nodes: &[Node],
    at: Datetime,
    chain: &ChainPolicy,
    event: &TodoEvent,
) -> Result<(String, String), Refusal> {
    let TodoEvent::Authored(record) = event else {
        return Err(Refusal::internal("authoring answers an Authored"));
    };
    let block = todos::render_current(nodes, at, &chain.untrusted, record)?;
    Ok((record.todo.as_str().to_owned(), block))
}
