//! What the agent proposes, as a reading of two object sets: what its replica holds that the
//! chain does not is STANDING, and a batch is what stands about one todo. This module decides,
//! without I/O, what accepting or rejecting one means.
//!
//! Every rule is a function of two object sets, so the laws are proptests; the binary performs
//! the effects the answers name and decides nothing. An [`Acceptance`] has the shape of
//! [`crate::sync`]'s outcome because accepting a batch and receiving a replica's post are one act
//! (the Prodrome's `store::accept`, the restricted join), but a pick is refused whole where a post
//! is refused per object: half a choice is not one.
//!
//! A batch is derived, never stored, so a partial accept needs no bookkeeping: the accepted
//! objects fall out of the difference and what is left is the batch. Nothing about a run is
//! kept, since nothing needs one: a change rests on its todo's heads, so what stands about one
//! todo is closed over the chain by itself, and two todos' batches commute.

// `pub(crate)`: a secretary changeset is drawn with the same rows as a batch, since deciding
// either is one job.
pub(crate) mod diff;
pub mod routes;

use std::collections::{BTreeMap, BTreeSet};

use prodrome_core::event::{Hash, TodoId, canonical_envelope, parents_of};

use crate::record::Envelope;
use crate::refusal::{Refusal, shown};
use crate::sync::Objects;

pub use diff::shown_batch;

/// What stands about one todo: every standing object that writes it, and every standing object
/// those rest on, so accepting a whole batch is always admissible.
///
/// A change's deps are its todo's own writes, so a batch of changes is one todo's. An object an
/// earlier writer sealed on another todo's (a legacy run, one object on the last) is in the
/// batch of every todo resting on it, since accepting any of them takes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    /// The todo it is about.
    pub todo: TodoId,
    /// The batch's name: the least of its heads (the writes nothing in it rests on), the object
    /// a `batch:` reference spells. It is a name and decides nothing: every head is in
    /// `objects`, and after a partial accept what is left still has one.
    pub head: Hash,
    /// Every object, parents first, so a reader applies them in an order no store would refuse.
    pub objects: Vec<(Hash, Envelope)>,
}

impl Batch {
    /// Does this batch hold that name?
    #[must_use]
    pub fn holds(&self, name: &Hash) -> bool {
        self.objects.iter().any(|(held, _)| held == name)
    }

    /// The instant the head's event claims.
    #[must_use]
    pub fn at(&self) -> Option<prodrome_core::literal::Datetime> {
        self.event().map(prodrome_core::event::TodoEvent::at)
    }

    /// Who wrote the head's event.
    #[must_use]
    pub fn actor(&self) -> &str {
        self.event().map_or("", |event| event.actor().as_str())
    }

    fn event(&self) -> Option<&crate::record::TodoEvent> {
        self.objects
            .iter()
            .find(|(name, _)| *name == self.head)
            .and_then(|(_, envelope)| envelope.event())
    }
}

/// Every batch standing against `target`, newest head first, ties broken by the head's name.
#[must_use]
pub fn batches(target: &Objects, replica: &Objects) -> Vec<Batch> {
    let mut writes: BTreeMap<TodoId, BTreeSet<Hash>> = BTreeMap::new();
    for name in replica
        .names()
        .into_iter()
        .filter(|name| !target.holds(name))
    {
        if let Some(event) = replica.get(&name).and_then(Envelope::event) {
            writes.entry(event.todo().clone()).or_default().insert(name);
        }
    }
    let mut found: Vec<Batch> = writes
        .into_iter()
        .filter_map(|(todo, writes)| {
            let mut closed: BTreeSet<Hash> = writes
                .iter()
                .flat_map(|name| replica.ancestors(name))
                .filter(|name| !target.holds(name) && replica.holds(name))
                .collect();
            closed.extend(writes);
            let named: BTreeSet<Hash> = closed
                .iter()
                .filter_map(|name| replica.get(name))
                .flat_map(parents_of)
                .collect();
            let head = closed.iter().find(|name| !named.contains(*name))?.clone();
            Some(Batch {
                todo,
                head,
                objects: parents_first(replica, &closed),
            })
        })
        .collect();
    found.sort_by(|left, right| {
        right
            .at()
            .cmp(&left.at())
            .then_with(|| left.head.as_str().cmp(right.head.as_str()))
    });
    found
}

/// The named objects in an order every parent precedes its children.
///
/// Kahn's with the hash as the tiebreak, as `prodrome_core::store::linearise` orders a whole
/// store. A parent outside the set (the target's tip) is not waited for: the reader holds it.
fn parents_first(replica: &Objects, wanted: &BTreeSet<Hash>) -> Vec<(Hash, Envelope)> {
    let mut left: BTreeSet<Hash> = wanted.clone();
    let mut out: Vec<(Hash, Envelope)> = Vec::with_capacity(left.len());
    let mut placed: BTreeSet<Hash> = BTreeSet::new();
    while !left.is_empty() {
        let ready: Vec<Hash> = left
            .iter()
            .filter(|name| {
                replica.get(name).is_some_and(|envelope| {
                    parents_of(envelope)
                        .iter()
                        .all(|parent| !wanted.contains(parent) || placed.contains(parent))
                })
            })
            .cloned()
            .collect();
        if ready.is_empty() {
            // A cycle is impossible on a store `verify` passes; a corrupt one is answered, not
            // hung on.
            out.extend(left.iter().filter_map(|name| {
                replica
                    .get(name)
                    .map(|envelope| (name.clone(), envelope.clone()))
            }));
            break;
        }
        for name in ready {
            if let Some(envelope) = replica.get(&name) {
                out.push((name.clone(), envelope.clone()));
            }
            placed.insert(name.clone());
            left.remove(&name);
        }
    }
    out
}

// --- Accepting ---------------------------------------------------------------

/// What an accept does, decided from the objects alone: take in exactly `taken`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Acceptance {
    /// The objects to take in, parents first, each its name and canonical print.
    pub taken: Vec<(Hash, String)>,
    /// The names the target already held; not an error, which is what makes accepting twice free.
    pub already: Vec<Hash>,
}

/// Take exactly these objects into the target.
///
/// # Errors
/// - A name no batch holds and the target does not either.
/// - An ancestor-open pick, naming every missing ancestor: a completion picked without the
///   `todo_add` it rests on could not be adopted, and this says so in the reader's words.
pub fn accept(target: &Objects, replica: &Objects, picked: &[Hash]) -> Result<Acceptance, Refusal> {
    let standing = batches(target, replica);

    // In the caller's order, deduped, so a client can line the answer up against its own list.
    let mut already: Vec<Hash> = Vec::new();
    let mut taking: BTreeMap<Hash, Envelope> = BTreeMap::new();
    let mut seen: BTreeSet<Hash> = BTreeSet::new();
    for name in picked {
        if !seen.insert(name.clone()) {
            continue;
        }
        if target.holds(name) {
            already.push(name.clone());
            continue;
        }
        let held = standing
            .iter()
            .find_map(|batch| batch.objects.iter().find(|(held, _)| held == name))
            .ok_or_else(|| {
                Refusal::not_found(format!(
                    "no proposal named {} — a batch is what the proposal store holds \
                     about one todo that this chain does not hold",
                    shown(name.as_str())
                ))
            })?;
        taking.insert(name.clone(), held.1.clone());
    }

    // Ancestor-closed: a parent the target holds is fine; one only in the replica is refused.
    let missing: Vec<String> = taking
        .values()
        .flat_map(parents_of)
        .filter(|parent| !target.holds(parent) && !taking.contains_key(parent))
        .map(|parent| short(&parent))
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();
    if !missing.is_empty() {
        return Err(Refusal::bad_request(format!(
            "this pick rests on {} nobody would be taking: {}. \
             Picking a completion means taking the todo it completes.",
            if missing.len() == 1 {
                "an object"
            } else {
                "objects"
            },
            missing.join(", ")
        )));
    }

    // A signature travels with what it signs, so an accepted batch keeps the agent's proof.
    let mut order: BTreeSet<Hash> = taking.keys().cloned().collect();
    order.extend(signatures_of(target, replica, &order));
    Ok(Acceptance {
        taken: parents_first(replica, &order)
            .into_iter()
            .map(|(name, envelope)| (name, canonical_envelope(&envelope)))
            .collect(),
        already,
    })
}

// --- Rejecting ---------------------------------------------------------------

/// Remove exactly these objects from the replica.
///
/// The mirror of [`accept`]: the set must be descendant-closed, since deleting a parent something
/// still names would leave a store `verify` cannot pass.
///
/// # Errors
/// - A name no batch holds, including one already on the chain, which never rewrites.
/// - A descendant-open set, naming what still rests on it.
pub fn reject(target: &Objects, replica: &Objects, picked: &[Hash]) -> Result<Vec<Hash>, Refusal> {
    let standing = batches(target, replica);
    let mut wanted: BTreeSet<Hash> = picked.iter().cloned().collect();

    for name in &wanted {
        if target.holds(name) {
            return Err(Refusal::bad_request(format!(
                "{} is on the chain already — rejecting is DELETION from the \
                 proposal store, and nothing removes an object a chain holds",
                short(name)
            )));
        }
        if !standing.iter().any(|batch| batch.holds(name)) {
            return Err(Refusal::not_found(format!(
                "no proposal named {}",
                shown(name.as_str())
            )));
        }
    }

    let orphaned: Vec<String> = standing
        .iter()
        .flat_map(|batch| &batch.objects)
        .filter(|(name, envelope)| {
            !wanted.contains(name)
                && parents_of(envelope)
                    .iter()
                    .any(|parent| wanted.contains(parent))
        })
        .map(|(name, _)| short(name))
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();
    if !orphaned.is_empty() {
        return Err(Refusal::bad_request(format!(
            "{} still rests on what this would remove: {}. \
             Rejecting a todo means rejecting what was written on top of it.",
            if orphaned.len() == 1 {
                "an object"
            } else {
                "objects"
            },
            orphaned.join(", ")
        )));
    }
    // A signature goes with what it signs, since nothing may rest on a removed object.
    let signatures = signatures_of(target, replica, &wanted);
    wanted.extend(signatures.iter().cloned());
    // In batch order, so the answer reads the way the batch does, the signatures after.
    Ok(standing
        .iter()
        .flat_map(|batch| &batch.objects)
        .filter(|(name, _)| wanted.contains(name))
        .map(|(name, _)| name.clone())
        .chain(signatures)
        .collect())
}

/// The standing signatures of `signed`: the replica's `Signed` objects the target lacks whose
/// object is one of them. A batch shows writes; their signatures are taken and removed with
/// them, never picked alone.
fn signatures_of(target: &Objects, replica: &Objects, signed: &BTreeSet<Hash>) -> Vec<Hash> {
    replica
        .names()
        .into_iter()
        .filter(|name| {
            !target.holds(name)
                && matches!(
                    replica.get(name),
                    Some(Envelope::Signed(signature)) if signed.contains(&signature.object)
                )
        })
        .collect()
}

/// Which batch each picked object belongs to, grouped, in [`batches`]' order.
///
/// Computed before the settle, which removes the objects the batch is derived from. Grouped
/// because a pick may span batches, and filing it under one head would misreport the review. A
/// name in no batch is dropped here and refused by [`accept`]/[`reject`].
#[must_use]
pub fn grouped(target: &Objects, replica: &Objects, picked: &[Hash]) -> Vec<(Hash, Vec<Hash>)> {
    let wanted: BTreeSet<Hash> = picked.iter().cloned().collect();
    batches(target, replica)
        .into_iter()
        .filter_map(|batch| {
            let mine: Vec<Hash> = batch
                .objects
                .iter()
                .map(|(name, _)| name.clone())
                .filter(|name| wanted.contains(name))
                .collect();
            (!mine.is_empty()).then_some((batch.head, mine))
        })
        .collect()
}

/// A name as a refusal spells one: enough to find it, short enough to read.
pub(crate) fn short(name: &Hash) -> String {
    name.as_str().chars().take(12).collect::<String>() + "…"
}

/// Which standing batch an object is in, named as [`Batch::head`] names it, or `None` when the
/// chain holds it: the least standing write to its todo that no standing write to it rests on.
#[must_use]
pub fn batch_of(
    nodes: &[crate::record::Node],
    settled: &BTreeSet<Hash>,
    object: &Hash,
) -> Option<Hash> {
    if settled.contains(object) {
        return None;
    }
    let todo = nodes
        .iter()
        .find(|node| node.name() == object)?
        .event()?
        .todo();
    let writes: Vec<&crate::record::Node> = nodes
        .iter()
        .filter(|node| {
            !settled.contains(node.name()) && node.event().is_some_and(|event| event.todo() == todo)
        })
        .collect();
    let named: BTreeSet<&Hash> = writes.iter().flat_map(|node| node.parents()).collect();
    writes
        .iter()
        .map(|node| node.name())
        .filter(|name| !named.contains(name))
        .min()
        .cloned()
}

/// The names in a body, parsed through `Hash::new`.
///
/// # Errors
/// Anything that is not 64 hex digits, named.
pub fn names(given: &[String]) -> Result<Vec<Hash>, Refusal> {
    given
        .iter()
        .map(|text| {
            Hash::new(text.clone()).map_err(|_| {
                Refusal::bad_request(format!(
                    "an object name is 64 hex digits, got {}",
                    shown(text)
                ))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
