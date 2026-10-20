//! `suzatary proposals accept|reject`: the operator's terminal door to the Inbox's settling.
//!
//! It calls `suzatary_core::proposals::routes` rather than a second implementation of the
//! closure rules.

use std::path::Path;

use anyhow::Context;
use clap::Subcommand;

use crate::instance::Instance;

/// Which act, and on which chain.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Accept or reject.
    #[command(subcommand)]
    action: Action,
    /// Which chain these are proposals against; absent is the instance file's first.
    #[arg(long, global = true, value_name = "NAME")]
    chain: Option<String>,
}

/// The two acts; listing is `suzatary todos proposals`, a reading rather than an act.
#[derive(Debug, Subcommand)]
pub enum Action {
    /// Adopt these objects into the chain; the set must be ancestor-closed within its batch.
    Accept {
        /// The object names, as `todos proposals` prints them.
        #[arg(value_name = "HASH", required = true)]
        objects: Vec<String>,
        /// Who is accepting, the chain's owner when absent; settles any held batch's decision as
        /// this actor.
        #[arg(long)]
        actor: Option<String>,
    },
    /// Remove these descendant-closed objects from the proposal store for good.
    ///
    /// The one deletion allowed, since the objects never entered a chain.
    Reject {
        /// The object names, as `todos proposals` prints them.
        #[arg(value_name = "HASH", required = true)]
        objects: Vec<String>,
        /// Who is rejecting, the chain's owner when absent; settles any held batch's decision as
        /// this actor.
        #[arg(long)]
        actor: Option<String>,
    },
}

/// The box's interpreters and one chain's stores, as the proposal routes read and write them,
/// so the CLI calls the same functions the app does.
fn opened(
    instance: &Instance,
    chain: &crate::instance::ChainDir,
) -> (crate::wiring::Opened, crate::wiring::Stores) {
    (
        crate::wiring::Opened::of(
            instance,
            "the proposals verb: checking whether a declared secret resolves",
        ),
        crate::wiring::Stores::of(chain, &instance.signers()),
    )
}

/// `suzatary proposals <act> <hash>…`.
///
/// # Errors
/// An unknown chain or object, an open pick, or either store failing.
pub fn run(path: &Path, args: &Args) -> anyhow::Result<()> {
    let instance = Instance::open(path)?;
    let chain = instance.chain(args.chain.as_deref())?;
    let (opened, stores) = opened(&instance, chain);
    let who = |actor: &Option<String>| actor.as_deref().unwrap_or(chain.owner.as_str()).to_owned();
    match &args.action {
        Action::Accept { objects, actor } => {
            let picked =
                suzatary_core::proposals::names(objects).map_err(|refusal| said(&refusal))?;
            let done = suzatary_core::proposals::routes::accept(
                &stores.chain,
                &stores.proposals,
                &opened.grounds.render,
                &opened.grounds.clock,
                &chain.policy,
                &picked,
                &who(actor),
            )
            .map_err(|refusal| said(&refusal))?;
            println!(
                "accepted {} object(s) onto {} ({} already held); heads {}",
                done.adopted.len(),
                done.chain,
                done.already.len(),
                done.heads
                    .iter()
                    .map(|head| short(head))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Action::Reject { objects, actor } => {
            let picked =
                suzatary_core::proposals::names(objects).map_err(|refusal| said(&refusal))?;
            let done = suzatary_core::proposals::routes::reject(
                &stores.chain,
                &stores.proposals,
                &opened.grounds.clock,
                &chain.policy,
                &picked,
                &who(actor),
            )
            .map_err(|refusal| said(&refusal))?;
            for name in &done.removed {
                println!("removed {}", short(name));
            }
            println!(
                "{} object(s) removed from {}'s proposal store",
                done.removed.len(),
                done.chain
            );
        }
    }
    Ok(())
}

/// A refusal as the terminal's error, verbatim, since it names what was missing.
fn said(refusal: &suzatary_core::refusal::Refusal) -> anyhow::Error {
    anyhow::anyhow!("{}", refusal.message())
}

/// A name as a line prints one.
fn short(name: &str) -> String {
    name.chars().take(12).collect()
}

/// `suzatary todos proposals`: the batches, as a person reads them.
///
/// # Errors
/// Either store being unreadable.
pub fn list(
    instance: &Instance,
    chain: &crate::instance::ChainDir,
    json: bool,
) -> anyhow::Result<()> {
    let (opened, stores) = opened(instance, chain);
    let listed = suzatary_core::proposals::routes::list(
        &stores.chain,
        &stores.proposals,
        opened.conversations.conversations(),
        &opened.grounds.clock,
        instance.persona.zone,
        &chain.policy,
    )
    .map_err(|refusal| said(&refusal))?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&listed).context("printing the batches")?
        );
        return Ok(());
    }
    if listed.batches.is_empty() {
        println!("no proposals standing against {}", listed.chain);
        return Ok(());
    }
    for batch in &listed.batches {
        println!(
            "\nbatch {} — {} by {}, {} object(s)",
            short(&batch.head),
            if batch.at.is_empty() {
                "—"
            } else {
                &batch.at
            },
            batch.actor,
            batch.objects.len()
        );
        for object in &batch.objects {
            let todo = object.act.todo();
            let pending = if todo.is_some_and(|todo| todo.pending_todo) {
                "  (its todo is itself proposed)"
            } else {
                ""
            };
            println!(
                "  {}  {:<10} {}{pending}",
                short(&object.hash),
                object.act.word(),
                todo.map_or_else(String::new, |todo| if todo.body.is_empty() {
                    todo.todo.clone()
                } else {
                    format!("{} — {}", todo.todo, todo.body)
                })
            );
            if let Some(note) = object.act.note().filter(|note| !note.is_empty()) {
                println!("              note: {note}");
            }
            for change in object.act.changes() {
                println!(
                    "              {}: {} → {}",
                    change.field,
                    shown_field(&change.before),
                    shown_field(&change.after)
                );
            }
        }
    }
    println!(
        "\naccept or reject with: suzatary proposals accept|reject --chain {} <hash>…",
        listed.chain
    );
    Ok(())
}

/// An absent field reads as `—` rather than as nothing.
fn shown_field(text: &str) -> &str {
    if text.is_empty() { "—" } else { text }
}
