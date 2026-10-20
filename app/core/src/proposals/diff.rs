//! A proposal as a person deciding about it reads it.
//!
//! A triage batch (sealed objects) and a secretary changeset (unsealed acts) are both mapped into
//! [`Proposed`] rows and drawn by one [`shown`], so neither can start eliding a field.
//!
//! Fields are shown as prose, not as `crate::view::authored_diff`'s canonical prints (which are
//! evidence against the bytes): a person accepting a sentence is shown the sentence, and a price
//! is the fulfillment it evaluates to, at the record's own instant (the number the writer meant)
//! under the empty environment (a proposal is not on the chain).

use std::collections::BTreeMap;

use prodrome_core::event::TodoId;
use prodrome_core::fpl;
use prodrome_core::payload::Payload as _;

use crate::believed::records;
use crate::record::{Authored, Envelope, Node, TodoEvent};
use crate::wire;

use super::Batch;

/// One proposed change, whether carried as a sealed object or an unsealed act.
#[derive(Debug, Clone)]
pub struct Proposed<'a> {
    /// An object's hash in a batch; an act's position in a changeset, its only name.
    pub name: String,
    /// The act, or `None` for a merge, which asserts no fact about a todo.
    pub event: Option<&'a TodoEvent>,
    /// What in the same proposal must be taken with it; empty for a changeset, confirmed whole.
    pub needs: Vec<String>,
}

/// One batch, rendered; `nodes` is the target chain, which the "before" column is read from.
#[must_use]
pub fn shown_batch(nodes: &[Node], batch: &Batch) -> wire::ProposedBatch {
    let inside: std::collections::BTreeSet<prodrome_core::event::Hash> =
        batch.objects.iter().map(|(name, _)| name.clone()).collect();
    // A parent the target already holds need not be taken, so `needs` is over this batch only.
    let rows: Vec<Proposed<'_>> = batch
        .objects
        .iter()
        .map(|(name, envelope)| Proposed {
            name: name.as_str().to_owned(),
            event: envelope.event(),
            needs: needs_of(envelope, &inside),
        })
        .collect();
    wire::ProposedBatch {
        head: batch.head.as_str().to_owned(),
        at: batch.at().map(iso).unwrap_or_default(),
        actor: batch.actor().to_owned(),
        objects: shown(nodes, &rows),
    }
}

fn needs_of(
    envelope: &Envelope,
    inside: &std::collections::BTreeSet<prodrome_core::event::Hash>,
) -> Vec<String> {
    prodrome_core::event::parents_of(envelope)
        .into_iter()
        .filter(|parent| inside.contains(parent))
        .map(|parent| parent.as_str().to_owned())
        .collect()
}

/// Every row of one proposal, with the "before" column advancing through it.
///
/// Rows are read in order, so a todo added and then edited shows the first row's after as the
/// second row's before.
#[must_use]
pub fn shown(nodes: &[Node], rows: &[Proposed<'_>]) -> Vec<wire::ProposedObject> {
    // Folded at the latest claimed instant, not a clock, so "before" does not depend on when the
    // page was opened.
    let horizon = nodes
        .iter()
        .filter_map(|node| node.event())
        .map(TodoEvent::at)
        .chain(rows.iter().filter_map(|row| row.event).map(TodoEvent::at))
        .max();
    let mut current: BTreeMap<TodoId, Authored> = horizon
        .map(|at| {
            records(nodes, at)
                .into_iter()
                .filter_map(|(todo, candidates)| Some((todo, candidates.into_iter().next()?)))
                .collect()
        })
        .unwrap_or_default();
    // Fixed before the walk, so an earlier row filling `current` cannot make a later one look
    // settled.
    let on_chain: std::collections::BTreeSet<TodoId> = current.keys().cloned().collect();
    // Filled as the walk goes: acting on a todo this same proposal adds is self-contained.
    let mut authored: std::collections::BTreeSet<TodoId> = std::collections::BTreeSet::new();
    rows.iter()
        .map(|row| shown_object(row, &mut current, &on_chain, &mut authored))
        .collect()
}

/// One row, and the running "before" advanced past it.
fn shown_object(
    row: &Proposed<'_>,
    current: &mut BTreeMap<TodoId, Authored>,
    on_chain: &std::collections::BTreeSet<TodoId>,
    authored: &mut std::collections::BTreeSet<TodoId>,
) -> wire::ProposedObject {
    let name = &row.name;
    let needs = row.needs.clone();
    let Some(event) = row.event else {
        // A merge has nothing to diff, but is still a row: it is an object a pick has to take.
        return wire::ProposedObject {
            hash: name.clone(),
            act: "merge".to_owned(),
            todo: String::new(),
            body: String::new(),
            at: String::new(),
            actor: String::new(),
            note: String::new(),
            changes: Vec::new(),
            needs,
            pending_todo: false,
        };
    };
    let todo = event.todo().clone();
    let (act, changes) = match event {
        TodoEvent::Authored(record) => {
            let before = current.get(&todo).cloned();
            let changes = changed(before.as_ref(), record);
            let act = if before.is_none() {
                "added"
            } else if changes
                .iter()
                .all(|c| matches!(c.field.as_str(), "priority" | "rationale"))
                && changes.iter().any(|c| c.field == "priority")
            {
                // Only the number and its reason moved: the edit `todo_update` refuses without a
                // fresh rationale.
                "repriced"
            } else {
                "edited"
            };
            current.insert(todo.clone(), (**record).clone());
            authored.insert(todo.clone());
            (act, changes)
        }
        TodoEvent::Created(_) => ("created", Vec::new()),
        TodoEvent::Completed(_) => ("completed", Vec::new()),
        TodoEvent::Cancelled(_) => ("cancelled", Vec::new()),
        TodoEvent::Reopened(_) => ("reopened", Vec::new()),
        TodoEvent::Tended(_) => ("tended", Vec::new()),
        TodoEvent::SpecRevised(_) => ("repriced", Vec::new()),
    };
    let note = match event {
        TodoEvent::Created(e) => e.note.clone(),
        TodoEvent::Completed(e)
        | TodoEvent::Cancelled(e)
        | TodoEvent::Reopened(e)
        | TodoEvent::Tended(e) => e.note.clone(),
        TodoEvent::SpecRevised(e) => e.note.clone(),
        TodoEvent::Authored(_) => String::new(),
    };
    wire::ProposedObject {
        hash: name.clone(),
        act: act.to_owned(),
        // The action line as of this object; "" when nothing has authored one.
        body: current
            .get(&todo)
            .map(|record| crate::escape::markup_text(record.payload.body.as_str()))
            .unwrap_or_default(),
        todo: todo.as_str().to_owned(),
        at: iso(event.at()),
        actor: event.actor().as_str().to_owned(),
        note,
        changes,
        needs,
        // True only for a todo that another proposal would put on the chain.
        pending_todo: !on_chain.contains(&todo) && !authored.contains(&todo),
    }
}

/// Every changed field a person prices or reads, before and after.
///
/// `created`, `subtodos`, `notes` and `source` are left to the todo page: they are not what the
/// todo says, or not what this surface writes.
fn changed(before: Option<&Authored>, after: &Authored) -> Vec<wire::ProposedChange> {
    /// A field's name and the total reader that says what it says.
    type Reader = (&'static str, fn(&Authored) -> String);
    let readers: [Reader; 6] = [
        ("body", |r| {
            crate::escape::markup_text(r.payload.body.as_str())
        }),
        ("detail", |r| {
            crate::escape::markup_text(r.payload.detail.as_str())
        }),
        ("priority", price),
        ("rationale", |r| {
            crate::escape::markup_text(&r.payload.rationale.join(" "))
        }),
        ("waiting_on", |r| {
            crate::view::string_text(r.payload.waiting_on.as_str())
        }),
        ("category", |r| {
            r.payload
                .category
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .unwrap_or_default()
        }),
    ];
    readers
        .into_iter()
        .filter_map(|(field, read)| {
            let was = before.map(read).unwrap_or_default();
            let now = read(after);
            (was != now).then(|| wire::ProposedChange {
                field: field.to_owned(),
                before: was,
                after: now,
            })
        })
        .collect()
}

/// A record's spec as its fulfillment at its own instant under the empty environment, plus its
/// recurrence in words: before its first pass a recurrence prices as what it replaced, so a
/// number alone would show no change.
fn price(record: &Authored) -> String {
    let Some(term) = record.payload.spec() else {
        return String::new();
    };
    let now = fpl::instant_of(record.at);
    // A spec that refers to other todos has no value on its own; the diff
    // shows none rather than link against a chain it does not hold.
    let number = fpl::Closed::of(term.clone()).map(|closed| {
        fpl::fulfillment(&closed, now, &fpl::Env::new()).map_or_else(
            || "unpriced".to_owned(),
            |value| format!("{:.0}%", value * 100.0),
        )
    });
    [number, crate::authoring::recurrence_words(term, now)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("; ")
}

/// An instant through `fpl::iso`, the one spelling every `at` on the wire uses.
fn iso(at: prodrome_core::literal::Datetime) -> String {
    fpl::iso(fpl::instant_of(at))
}
