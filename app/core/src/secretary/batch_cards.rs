//! The migration that carries the triage agent's open batches into the Inbox as its cards: each
//! batch's events read back into the acts that write them. Its verb and this module are deleted
//! once it has run on the box.

use std::collections::{BTreeMap, BTreeSet};

use prodrome_core::event::{Hash, TodoId};

use crate::authoring::Composed;
use crate::chains::ChainName;
use crate::record::{Authored, Envelope, TodoEvent};
use crate::reference::{Reference, TodoRef};
use crate::secretary::changes::{Act, Provenance};
use crate::todo::{Patch, Price, TodoFields, TodoPatch};
use crate::zone::Zone;

/// One open batch: the objects the replica holds about one todo that the chain does not, and
/// the acts they read back into, or why they do not.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    /// The todo it is about.
    pub todo: TodoId,
    /// Every object it holds, its signatures included: what the replica drops once it is carried.
    pub objects: Vec<Hash>,
    /// The acts, in the order the agent wrote them, or the reason they cannot be written.
    pub acts: Result<Vec<Act>, String>,
    /// What the card says the acts are for: the agent's rationales and notes.
    pub why: String,
    /// What the card rests on: the messages its records name, or the todo itself.
    pub sources: Vec<Reference>,
}

/// Every batch `replica` holds against `target`, in the order the replica first wrote to each
/// todo. A pure function of the two object sets, so a second run over the same stores reads the
/// same batches, and none once each was carried and dropped.
#[must_use]
pub fn batches(
    target: &[(Hash, Envelope)],
    replica: &[(Hash, Envelope)],
    chain: &ChainName,
    zone: Zone,
) -> Vec<Batch> {
    let held: BTreeSet<&Hash> = target.iter().map(|(name, _)| name).collect();
    let nodes = crate::believed::Preview::of(target.to_vec())
        .map(|preview| preview.nodes().to_vec())
        .unwrap_or_default();
    let standing: Vec<&(Hash, Envelope)> = replica
        .iter()
        .filter(|(name, _)| !held.contains(name))
        .collect();
    let mut order: Vec<TodoId> = Vec::new();
    let mut found: BTreeMap<TodoId, Read> = BTreeMap::new();
    let mut current: BTreeMap<TodoId, Authored> = BTreeMap::new();
    let mut adds: Vec<(TodoId, usize)> = Vec::new();
    for (name, envelope) in &standing {
        let Some(event) = envelope.event() else {
            continue;
        };
        let todo = event.todo().clone();
        if !found.contains_key(&todo) {
            order.push(todo.clone());
        }
        let read = found.entry(todo.clone()).or_default();
        read.objects.push(name.clone());
        let before = current
            .get(&todo)
            .cloned()
            .or_else(|| crate::todos::current(&nodes, event.at(), todo.as_str()));
        match act_of(event, before.as_ref(), zone) {
            Err(why) => read.refused.push(why),
            Ok(Carried::Nothing) => {}
            Ok(Carried::Act(boxed, said)) => {
                if matches!(*boxed, Act::Add { .. }) {
                    adds.push((todo.clone(), read.acts.len()));
                }
                read.acts.push(*boxed);
                read.said.extend(said);
            }
            Ok(Carried::Parent(children)) => parent(&mut found, &adds, &todo, children),
        }
        if let TodoEvent::Authored(record) = event {
            let mail = record.payload.source.as_ref().and_then(|source| {
                format!("mail:{}", crate::view::string_text(source.hash.as_str()))
                    .parse::<Reference>()
                    .ok()
            });
            if let (Some(mail), Some(read)) = (mail, found.get_mut(&todo))
                && !read.sources.contains(&mail)
            {
                read.sources.push(mail);
            }
            current.insert(todo, (**record).clone());
        }
    }
    // A signature goes with what it signs.
    for (name, envelope) in &standing {
        if let Envelope::Signed(signed) = envelope
            && let Some(read) = found
                .values_mut()
                .find(|read| read.objects.contains(&signed.object))
        {
            read.objects.push(name.clone());
        }
    }
    order
        .into_iter()
        .filter_map(|todo| Some(found.remove(&todo)?.batch(todo, chain)))
        .collect()
}

/// Give each child's add this parent, refusing a child no add of this migration writes.
fn parent(
    found: &mut BTreeMap<TodoId, Read>,
    adds: &[(TodoId, usize)],
    todo: &TodoId,
    children: Vec<String>,
) {
    for child in children {
        let add = adds
            .iter()
            .find(|(added, _)| added.as_str() == child)
            .and_then(|(added, index)| found.get_mut(added)?.acts.get_mut(*index));
        if let Some(Act::Add { from, .. }) = add
            && from.parent.is_none()
        {
            from.parent = Some(todo.as_str().to_owned());
        } else {
            found.entry(todo.clone()).or_default().refused.push(format!(
                "it adds {child} as a subtodo, which only an add of {child} can write"
            ));
        }
    }
}

/// What one todo's standing events read back into so far.
#[derive(Default)]
struct Read {
    objects: Vec<Hash>,
    acts: Vec<Act>,
    refused: Vec<String>,
    said: Vec<String>,
    sources: Vec<Reference>,
}

impl Read {
    /// The batch this reading is, about `todo` on `chain`.
    fn batch(self, todo: TodoId, chain: &ChainName) -> Batch {
        let sources = if self.sources.is_empty() {
            vec![Reference::Todo(TodoRef {
                chain: chain.clone(),
                id: todo.clone(),
            })]
        } else {
            self.sources
        };
        let acts = if self.refused.is_empty() {
            Ok(self.acts)
        } else {
            Err(self.refused.join("; "))
        };
        Batch {
            todo,
            objects: self.objects,
            acts,
            why: format!(
                "The triage agent asked for this before its writes became cards. {}",
                self.said.join(" ")
            )
            .trim()
            .to_owned(),
            sources,
        }
    }
}

/// An act carried, and what the agent said it was for.
fn carried(act: Act, said: Option<String>) -> Carried {
    Carried::Act(Box::new(act), said)
}

/// What one event carries into the card.
enum Carried {
    /// An act, and what the agent said it was for.
    Act(Box<Act>, Option<String>),
    /// No act: a revision that changes nothing an act writes.
    Nothing,
    /// A parent's revision adding these subtodos, written by their adds.
    Parent(Vec<String>),
}

/// The act that writes `event` over `before`, the todo's record as it stood.
fn act_of(event: &TodoEvent, before: Option<&Authored>, zone: Zone) -> Result<Carried, String> {
    let todo = event.todo().as_str().to_owned();
    let noted = |note: &str| (!note.is_empty()).then(|| note.to_owned());
    match event {
        TodoEvent::Authored(record) => match before {
            None => added(record, zone),
            Some(before) => updated(before, record, zone),
        },
        TodoEvent::Completed(life) => Ok(carried(
            Act::Complete {
                todo,
                note: life.note.clone(),
            },
            noted(&life.note),
        )),
        TodoEvent::Cancelled(life) => Ok(carried(
            Act::Cancel {
                todo,
                note: life.note.clone(),
            },
            noted(&life.note),
        )),
        TodoEvent::Reopened(life) => Ok(carried(
            Act::Reopen {
                todo,
                note: life.note.clone(),
            },
            noted(&life.note),
        )),
        TodoEvent::Tended(life) => Ok(carried(
            Act::Tend {
                todo,
                note: life.note.clone(),
            },
            noted(&life.note),
        )),
        TodoEvent::Created(_) => Err(format!("{todo} has a Created event, which no act writes")),
        TodoEvent::SpecRevised(_) => Err(format!(
            "{todo} has a SpecRevised event, which no act writes"
        )),
    }
}

/// A todo's own price, read back into the price a verb writes.
fn own_price(record: &Authored, zone: Zone) -> Result<(Price, Vec<String>), String> {
    let composed = Composed::of(&record.payload.spec);
    let price = Price::read(&composed.own, record.todo.as_str(), zone).ok_or_else(|| {
        format!(
            "{}'s price is not one the verbs write: {}",
            record.todo.as_str(),
            prodrome_core::fpl::print_term(&composed.own)
        )
    })?;
    Ok((price, composed.children))
}

/// A record's waiting-on as the text it was written from.
fn waiting(record: &Authored) -> String {
    crate::view::string_text(record.payload.waiting_on.as_str())
}

/// A new todo's add.
fn added(record: &Authored, zone: Zone) -> Result<Carried, String> {
    let (price, children) = own_price(record, zone)?;
    if !children.is_empty() {
        return Err(format!(
            "{} is added as a parent of subtodos, which an add cannot write",
            record.todo.as_str()
        ));
    }
    let payload = &record.payload;
    let rationale = payload.rationale.join(" ");
    Ok(carried(
        Act::Add {
            todo: record.todo.as_str().to_owned(),
            fields: TodoFields {
                body: payload.body.as_str().to_owned(),
                rationale: rationale.clone(),
                kind: payload.kind.as_str().to_owned(),
                category: payload
                    .category
                    .as_ref()
                    .map(|name| name.as_str().to_owned())
                    .unwrap_or_default(),
                price,
                detail: payload.detail.as_str().to_owned(),
                waiting_on: waiting(record),
            },
            from: Provenance {
                source: payload.source.clone(),
                created: Some(payload.created),
                parent: None,
            },
        },
        Some(rationale),
    ))
}

/// An existing todo's update: each field the record changes, set.
fn updated(before: &Authored, after: &Authored, zone: Zone) -> Result<Carried, String> {
    let (was, old_children) = (
        Composed::of(&before.payload.spec),
        Composed::of(&before.payload.spec).children,
    );
    let (now, new_children) = (
        Composed::of(&after.payload.spec),
        Composed::of(&after.payload.spec).children,
    );
    let added: Vec<String> = match new_children.strip_prefix(old_children.as_slice()) {
        Some(added) => added.to_vec(),
        None => {
            return Err(format!(
                "{} loses a subtodo, which no act writes",
                after.todo.as_str()
            ));
        }
    };
    let (b, a) = (&before.payload, &after.payload);
    let set = |was: &str, now: &str| {
        if was == now {
            Patch::Keep
        } else {
            Patch::Set(now.to_owned())
        }
    };
    let repriced = was.own != now.own;
    let price = if repriced {
        Patch::Set(own_price(after, zone)?.0)
    } else {
        Patch::Keep
    };
    let rationale = a.rationale.join(" ");
    let (waiting_on, clear_waiting_on) = match (waiting(before), waiting(after)) {
        (was, now) if was == now => (Patch::Keep, false),
        (_, now) if now.is_empty() => (Patch::Keep, true),
        (_, now) => (Patch::Set(now), false),
    };
    let patch = TodoPatch {
        body: set(b.body.as_str(), a.body.as_str()),
        detail: set(b.detail.as_str(), a.detail.as_str()),
        // A reprice carries its reason, as the verb that wrote it required.
        rationale: if repriced || b.rationale != a.rationale {
            Patch::Set(rationale.clone())
        } else {
            Patch::Keep
        },
        price,
        waiting_on,
        clear_waiting_on,
        category: set(
            b.category.as_ref().map_or("", |name| name.as_str()),
            a.category.as_ref().map_or("", |name| name.as_str()),
        ),
        subtodos: if b.subtodos == a.subtodos {
            Patch::Keep
        } else {
            Patch::Set(a.subtodos.clone())
        },
        ..TodoPatch::default()
    };
    match (patch.is_empty(), added.is_empty()) {
        (true, true) => Ok(Carried::Nothing),
        (true, false) => Ok(Carried::Parent(added)),
        (false, true) => Ok(carried(
            Act::Update {
                todo: after.todo.as_str().to_owned(),
                fields: patch,
            },
            Some(rationale),
        )),
        (false, false) => Err(format!(
            "{} is reworded and gains a subtodo in one write, which no one act writes",
            after.todo.as_str()
        )),
    }
}
