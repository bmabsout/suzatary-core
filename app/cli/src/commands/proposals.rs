//! `suzatary proposals carry`: the migration that carries every open batch of the triage agent's
//! replica into the Inbox as the agent's card. Run once on the box after the deploy that makes
//! the agent file cards, with the owner's yes; this verb is deleted once it has run.
//!
//! A dry run, the default, prints each batch and what it becomes and writes nothing. `--apply`
//! files each card, completes the decision that held its batch, and drops the batch's objects
//! from the replica, so a second run finds nothing left to carry.

use std::path::Path;

use anyhow::Context;
use clap::Subcommand;
use prodrome_core::event::{Actor, Hash, mk_completed};
use suzatary_core::capability::Clock;
use suzatary_core::record::TodoEvent;
use suzatary_core::reference::{Reference, TodoRef};
use suzatary_core::secretary::batch_cards::{self, Batch};
use suzatary_core::secretary::changes::{self, Act, Building, Changeset};
use suzatary_core::secretary::filed::TriageCards;
use suzatary_core::secretary::store::Reasons;
use suzatary_core::store::{Prodrome, Replica};

use crate::instance::{ChainDir, Instance};

/// The one act.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// What to do.
    #[command(subcommand)]
    action: Action,
}

/// The migration.
#[derive(Debug, Subcommand)]
pub enum Action {
    /// Carry each open triage batch into the Inbox as the agent's card. Prints what it would do
    /// and writes nothing unless `--apply` is given.
    Carry {
        /// Which chain's replica; absent is every chain's.
        #[arg(long, value_name = "NAME")]
        chain: Option<String>,
        /// File the cards and drop the carried objects from the replica.
        #[arg(long)]
        apply: bool,
    },
}

/// `suzatary proposals carry [--chain <name>] [--apply]`.
///
/// # Errors
/// A store that does not read or write, or a chain the instance does not name.
pub fn run(path: &Path, args: &Args) -> anyhow::Result<()> {
    let Action::Carry { chain, apply } = &args.action;
    let instance = Instance::open(path)?;
    let chains: Vec<&ChainDir> = match chain {
        Some(name) => vec![instance.chain(Some(name))?],
        None => instance.chains.iter().collect(),
    };
    let conversations = instance.conversation_stores();
    let inbox = instance.inbox_store();
    let cards = TriageCards::new(conversations.conversations(), &inbox);
    let render = suzatary_io::render::Typst::new(
        instance.code().to_path_buf(),
        instance.paths.markup_cache.clone(),
        suzatary_io::typeset::Closed::this_binary(),
    );
    let mut left = 0usize;
    for chain in chains {
        left += carry(&instance, chain, &cards, &render, *apply)?;
    }
    if *apply && left > 0 {
        anyhow::bail!("{left} batch(es) were not carried: the lines above say why");
    }
    Ok(())
}

/// Carry one chain's batches, answering how many could not be.
fn carry(
    instance: &Instance,
    chain: &ChainDir,
    cards: &TriageCards<'_>,
    render: &suzatary_io::render::Typst,
    apply: bool,
) -> anyhow::Result<usize> {
    let name = chain.policy.name.as_str();
    let target = super::store_of(chain);
    let replica = super::proposals_of(chain);
    let batches = batch_cards::batches(
        &target
            .objects()
            .with_context(|| format!("reading the {name} chain"))?,
        &replica
            .objects()
            .with_context(|| format!("reading the {name} replica"))?,
        &chain.policy.name,
        instance.persona.zone,
    );
    if batches.is_empty() {
        println!("{name}: no open batch");
        return Ok(0);
    }
    let categories: Vec<&str> = instance
        .deployment
        .categories
        .keys()
        .map(String::as_str)
        .collect();
    let clock = suzatary_io::clock::SystemClock;
    let mut left = 0usize;
    for batch in batches {
        let at = clock.now();
        let building = Building {
            now: suzatary_core::zone::Moment {
                at,
                zone: instance.persona.zone,
            },
            by: suzatary_core::secretary::WRITES_AS,
            categories: &categories,
            untrusted: &chain.policy.untrusted,
            typeset: render,
        };
        let check = |changeset: &Changeset| changes::checked(changeset, &target, &building);
        match carried(name, &batch, &check) {
            Err(why) => {
                left += 1;
                println!("{name} {}: NOT CARRIED, {why}", batch.todo.as_str());
            }
            Ok(None) => {
                println!(
                    "{name} {}: nothing to carry; {} object(s) to drop",
                    batch.todo.as_str(),
                    batch.objects.len()
                );
                if apply {
                    replica.discard(&batch.objects)?;
                }
            }
            Ok(Some((changeset, reasons))) => {
                println!(
                    "{name} {}: {} as one card, {} object(s) to drop",
                    batch.todo.as_str(),
                    changeset
                        .acts()
                        .iter()
                        .map(Act::word)
                        .collect::<Vec<_>>()
                        .join(", "),
                    batch.objects.len()
                );
                if !apply {
                    continue;
                }
                let serves =
                    suzatary_core::todos::current(&target.nodes()?, at, batch.todo.as_str()).map(
                        |_| TodoRef {
                            chain: chain.policy.name.clone(),
                            id: batch.todo.clone(),
                        },
                    );
                let card = match cards.file(at, changeset, reasons, serves, &check) {
                    Ok(carded) => carded.card,
                    Err(refusal) => {
                        left += 1;
                        println!(
                            "{name} {}: NOT CARRIED, {}",
                            batch.todo.as_str(),
                            refusal.message()
                        );
                        continue;
                    }
                };
                println!("  filed as {card}");
                close_decisions(instance, chain, &batch.objects, &card.to_string())?;
                replica.discard(&batch.objects)?;
            }
        }
    }
    Ok(left)
}

/// The batch as a card's changeset and reasons, checked against the chain; `None` when nothing
/// in it writes anything.
fn carried(
    chain: &str,
    batch: &Batch,
    check: &dyn Fn(&Changeset) -> Result<(), suzatary_core::refusal::Refusal>,
) -> Result<Option<(Changeset, Reasons)>, String> {
    let acts = batch.acts.clone()?;
    let Ok(changeset) = Changeset::new(chain, acts) else {
        return Ok(None);
    };
    check(&changeset).map_err(|refusal| refusal.message().to_owned())?;
    let reasons = Reasons::new(batch.why.clone(), batch.sources.clone())
        .map_err(|failed| failed.to_string())?;
    Ok(Some((changeset, reasons)))
}

/// Complete every open decision that held one of these objects' batch, naming the card it became.
fn close_decisions(
    instance: &Instance,
    chain: &ChainDir,
    objects: &[Hash],
    card: &str,
) -> anyhow::Result<()> {
    let assistant = Actor::new(suzatary_core::secretary::WRITES_AS.actor())?;
    let writer = Instance::writer_of(chain, &instance.signers(), &assistant);
    let nodes = writer.nodes()?;
    let events = suzatary_core::view::events_of(&nodes);
    let at = suzatary_io::clock::SystemClock.now();
    for decision in held_by(&events, chain.policy.name.as_str(), objects) {
        writer.append(mk_completed(
            &decision,
            at,
            assistant.as_str(),
            &format!("carried to the Inbox as {card}"),
        )?)?;
        println!("  completed the decision {decision}");
    }
    Ok(())
}

/// The open decisions whose `batch:<chain>/<head>` names one of `objects`.
fn held_by(events: &[TodoEvent], chain: &str, objects: &[Hash]) -> Vec<String> {
    let mut open = std::collections::BTreeMap::<String, bool>::new();
    for event in events {
        let todo = event.todo().as_str().to_owned();
        match event {
            TodoEvent::Authored(record) if record.payload.is_decision() => {
                let detail = record.payload.detail.as_str();
                let first = detail.split_once('\n').map_or(detail, |(first, _)| first);
                let names = matches!(
                    first.parse::<Reference>(),
                    Ok(Reference::Batch { chain: named, head })
                        if named.as_str() == chain
                            && objects.iter().any(|object| object.as_str() == head.as_str())
                );
                if names {
                    open.entry(todo).or_insert(true);
                }
            }
            TodoEvent::Completed(_) | TodoEvent::Cancelled(_) => {
                if let Some(state) = open.get_mut(&todo) {
                    *state = false;
                }
            }
            TodoEvent::Reopened(_) => {
                if let Some(state) = open.get_mut(&todo) {
                    *state = true;
                }
            }
            TodoEvent::Authored(_)
            | TodoEvent::Created(_)
            | TodoEvent::Tended(_)
            | TodoEvent::SpecRevised(_) => {}
        }
    }
    open.into_iter()
        .filter_map(|(todo, open)| open.then_some(todo))
        .collect()
}
