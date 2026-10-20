//! The proposal routes as functions, shared by the web routes and the CLI; the gate is the
//! binary's.

use prodrome_core::event::{Hash, mk_cancelled, mk_completed};

use crate::capability::{Clock, Datetime, Render};
use crate::chains::{ChainName, ChainPolicy};
use crate::record::TodoEvent;
use crate::reference::{BatchHead, Reference};
use crate::refusal::{Answered, Refusal};
use crate::store::{Prodrome, Replica};
use crate::sync::Objects;
use crate::wire;

/// Which way a batch was settled.
#[derive(Debug, Clone, Copy)]
pub enum Settle {
    /// The batch's objects landed on the chain.
    Accepted,
    /// The batch's objects were deleted from the replica.
    Rejected,
}

/// Who is settling a batch's decision, and why.
#[derive(Debug, Clone, Copy)]
pub struct Settling<'a> {
    /// The gate's resolved login, or the secretary's `assistant`.
    pub actor: &'a str,
    /// The note a person reads on the closed record.
    pub why: &'a str,
    /// When the settle happened.
    pub now: Datetime,
}

/// Settle the decision naming an object that just left the proposals, if one stands.
///
/// # Errors
/// The chain's own refusal to append.
pub fn settle_decision_on(
    store: &dyn Prodrome<TodoEvent>,
    chain_name: &str,
    object: &Hash,
    settle: Settle,
    events: &[TodoEvent],
    settling: Settling<'_>,
) -> Result<(), Refusal> {
    let reference = Reference::Batch {
        chain: ChainName::new(chain_name)
            .map_err(|failed| Refusal::internal(failed.to_string()))?,
        head: BatchHead::new(object.as_str())
            .map_err(|failed| Refusal::internal(failed.to_string()))?,
    };
    let Some(settled) = crate::authoring::decision_for(events, &reference) else {
        return Ok(());
    };
    let event = match settle {
        Settle::Accepted => mk_completed::<crate::record::Record>(
            settled.as_str(),
            settling.now,
            settling.actor,
            settling.why,
        ),
        Settle::Rejected => mk_cancelled::<crate::record::Record>(
            settled.as_str(),
            settling.now,
            settling.actor,
            settling.why,
        ),
    }
    .map_err(|failed| Refusal::bad_request(failed.to_string()))?;
    store.append(event)?;
    Ok(())
}

/// Both stores, read once.
fn both(
    store: &dyn Prodrome<TodoEvent>,
    proposals: &dyn Replica<TodoEvent>,
) -> Result<(Objects, Objects), Refusal> {
    let target = store.objects()?;
    let replica = proposals.objects()?;
    Ok((Objects::of(&target), Objects::of(&replica)))
}

/// `GET /api/proposals` and `/api/<chain>/proposals`: every batch standing against this chain,
/// newest first, with today's review lines.
///
/// # Errors
/// Either store being unreadable.
pub fn list(
    store: &dyn Prodrome<TodoEvent>,
    proposals: &dyn Replica<TodoEvent>,
    conversations: crate::secretary::conversations::Conversations<'_>,
    clock: &dyn Clock,
    zone: crate::zone::Zone,
    chain: &ChainPolicy,
) -> Answered<wire::ProposalsPayload> {
    let (target, replica) = both(store, proposals)?;
    let nodes = store.nodes()?;
    let today = zone.day(prodrome_core::fpl::instant_of(clock.now()));
    Ok(wire::ProposalsPayload {
        chain: chain.name.as_str().to_owned(),
        batches: super::batches(&target, &replica)
            .iter()
            .map(|batch| super::shown_batch(&nodes, batch))
            .collect(),
        // A conversation that does not read costs only its own reviews; `verify` names it.
        reviews: crate::secretary::review::reviews(conversations, today)?
            .items
            .into_iter()
            .filter(|(_, review)| review.chain == chain.name.as_str())
            .map(|(conversation, review)| wire::ReviewedBatch {
                batch: review.batch,
                decision: review.decision.word().to_owned(),
                why: review.why.to_string(),
                at: review.at,
                objects: review.objects.len(),
                conversation: conversation.as_str().to_owned(),
            })
            .collect(),
    })
}

/// `POST /api/proposals/accept`: adopt exactly these objects into the chain, under the write
/// lock.
///
/// # Errors
/// A name no batch holds, an ancestor-open pick, or either store failing.
pub fn accept(
    store: &dyn Prodrome<TodoEvent>,
    proposals: &dyn Replica<TodoEvent>,
    render: &dyn Render,
    clock: &dyn Clock,
    chain: &ChainPolicy,
    picked: &[Hash],
    actor: &str,
) -> Answered<wire::AcceptedPayload> {
    let _held = store.locked()?;
    let (target, replica) = both(store, proposals)?;
    // Grouped before the accept moves anything, while the store still remembers each batch.
    let touched = super::grouped(&target, &replica, picked);
    let outcome = super::accept(&target, &replica, picked)?;
    // THE ONE DOOR: a batch enters the chain as a replica's post does, held to the same rules
    // and the same proof, and refused whole where a post is refused per object.
    let prints: Vec<String> = outcome
        .taken
        .iter()
        .map(|(_, print)| print.clone())
        .collect();
    let agent = prodrome_core::event::Actor::new(crate::agent::WRITES_AS.actor())
        .map_err(|e| Refusal::internal(e.to_string()))?;
    let door = crate::sync::receive(&target, &prints, &agent, &chain.root);
    if let Some(refused) = door.refused.first() {
        return Err(Refusal::forbidden(format!(
            "this batch does not enter the chain: {}",
            refused.reason.said()
        )));
    }
    store.accept(&door.taken)?;
    // Only after the join succeeded.
    let events = crate::view::events_of(&store.nodes()?);
    let settling = Settling {
        actor,
        why: "accepted through the Inbox",
        now: clock.now(),
    };
    for object in touched.iter().flat_map(|(_, objects)| objects) {
        settle_decision_on(
            store,
            chain.name.as_str(),
            object,
            Settle::Accepted,
            &events,
            settling,
        )?;
    }
    let fresh: Vec<crate::escape::Escaped> = outcome
        .taken
        .iter()
        .filter_map(|(_, literal)| {
            prodrome_core::event::parse_envelope(&crate::record::TodoVocabulary::default(), literal)
                .ok()
        })
        .filter_map(|envelope| match envelope.event() {
            Some(crate::record::TodoEvent::Authored(record)) => Some(record.clone()),
            _ => None,
        })
        .filter(|record| crate::view::renders(record, &chain.untrusted))
        .flat_map(|record| {
            crate::view::rich_sources(&record)
                .into_iter()
                .map(|(_, source)| source)
                .collect::<Vec<_>>()
        })
        .collect();
    if !fresh.is_empty() {
        render.warm(&fresh);
    }
    Ok(wire::AcceptedPayload {
        chain: chain.name.as_str().to_owned(),
        heads: crate::routes::head_set(store)?,
        adopted: outcome
            .taken
            .iter()
            .map(|(name, _)| name.as_str().to_owned())
            .collect(),
        already: outcome
            .already
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect(),
    })
}

/// `POST /api/proposals/reject`: remove exactly these objects from the replica, the one
/// deletion this application performs.
///
/// # Errors
/// A name no batch holds, a name the chain holds, a descendant-open set, or the store failing.
pub fn reject(
    store: &dyn Prodrome<TodoEvent>,
    proposals: &dyn Replica<TodoEvent>,
    clock: &dyn Clock,
    chain: &ChainPolicy,
    picked: &[Hash],
    actor: &str,
) -> Answered<wire::RejectedPayload> {
    let _held = store.locked()?;
    let (target, replica) = both(store, proposals)?;
    let touched = super::grouped(&target, &replica, picked);
    let going = super::reject(&target, &replica, picked)?;
    let removed = proposals.discard(&going)?;
    let events = crate::view::events_of(&store.nodes()?);
    let settling = Settling {
        actor,
        why: "rejected through the Inbox",
        now: clock.now(),
    };
    for object in touched.iter().flat_map(|(_, objects)| objects) {
        settle_decision_on(
            store,
            chain.name.as_str(),
            object,
            Settle::Rejected,
            &events,
            settling,
        )?;
    }
    Ok(wire::RejectedPayload {
        chain: chain.name.as_str().to_owned(),
        removed: removed
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect(),
    })
}
