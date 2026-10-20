//! The compactor's one task: the digest's rule at its budget's pace
//! (`secretary::compaction::Paced`), woken at startup and whenever a write moves a
//! conversation's lines, and asleep until the instant a hold frees.
//!
//! Startup is a nudge like any other, so the lines already stored are compacted by the same
//! rule at the same pace as new ones: an import is no special path. One task keeps two firings
//! from writing at once; the engine inside `Paced` fires once per reading, so a nudge that
//! moved nothing performs nothing. A step that failed (the model unreachable, the subscription
//! down to its reserve) waits out a [`Backoff`] that no nudge shortens, doubling from
//! [`RETRY`] to [`RETRY_MOST`] while failures repeat, then steps on a fresh engine, as a
//! restart would. Every call it made was counted before it was made, so retrying spends
//! within the budget like any other step.

use std::sync::Arc;

use suzatary_core::capability::{Compact, Server};
use suzatary_core::secretary::compact_budget::{Budget, Hold};
use suzatary_core::secretary::compacting::Compacting;
use suzatary_core::secretary::compaction::{Compacted, Paced, Step};
use tokio::sync::mpsc;

use super::State;
use super::interpreters::CompactorModel;
use crate::instance::Instance;

/// `state` with the compactor's task started over its first chain's stores when `compacting`
/// runs one; off is the kill switch, with no task, and the lines wait.
pub fn started(
    state: State,
    instance: &Instance,
    live: &super::live::Shared,
    compacting: Compacting,
) -> State {
    match compacting {
        Compacting::Off => state,
        Compacting::On { model, budget } => {
            let model = Arc::new(CompactorModel::new(instance, live, model));
            let caps = state.first.caps;
            state.with_compactor(Compactor::spawn(caps, model, budget))
        }
    }
}

/// How long a first failed step waits before it is tried again.
const RETRY: std::time::Duration = std::time::Duration::from_mins(15);

/// The longest wait between failed steps.
const RETRY_MOST: std::time::Duration = std::time::Duration::from_hours(4);

/// The wait after a failed step: [`RETRY`], doubled by each failure that follows to at most
/// [`RETRY_MOST`], and back to [`RETRY`] once a step compacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Backoff(std::time::Duration);

impl Backoff {
    fn new() -> Self {
        Self(RETRY)
    }

    /// The wait this failure owes, and the next one doubled.
    fn failed(&mut self) -> std::time::Duration {
        let wait = self.0;
        self.0 = (self.0 * 2).min(RETRY_MOST);
        wait
    }
}

/// The compactor's channel: a nudge, never a payload, since every step reads again.
pub struct Compactor {
    nudge: mpsc::Sender<()>,
}

impl Compactor {
    /// Start the task over `caps`'s memory and clock, asking `compact`, held to `budget`; it
    /// takes its first step at once.
    #[must_use]
    pub fn spawn(
        caps: &'static Server<'static>,
        compact: Arc<dyn Compact + Send + Sync>,
        budget: Budget,
    ) -> Self {
        let (nudge, nudged) = mpsc::channel(1);
        tokio::spawn(run(caps, compact, budget, nudged));
        Self { nudge }
    }

    /// A conversation's lines moved: the task steps again.
    pub fn nudge(&self) {
        // Full is fine: a nudge already stands and the next step reads again.
        let _ = self.nudge.try_send(());
    }
}

async fn run(
    caps: &'static Server<'static>,
    compact: Arc<dyn Compact + Send + Sync>,
    budget: Budget,
    mut nudged: mpsc::Receiver<()>,
) {
    let mut paced = Paced::new(compact.as_ref(), budget);
    let mut said: Option<Hold> = None;
    let mut backoff = Backoff::new();
    loop {
        // Drained before the read, so a write landing during the step leaves its nudge.
        let _ = nudged.try_recv();
        let asked = Arc::clone(&compact);
        let stepped = tokio::task::spawn_blocking(move || {
            let step = paced.step(&caps.memories, asked.as_ref(), caps.clock);
            (paced, step)
        })
        .await;
        let step = match stepped {
            Ok((back, step)) => {
                paced = back;
                step
            }
            Err(join_error) => {
                tracing::warn!("compactor: a step did not complete, so it stops: {join_error}");
                return;
            }
        };
        let wait = match step {
            Ok(Step::Compacted(done)) => {
                tracing::info!("compactor: {}", logged(&done));
                said = None;
                backoff = Backoff::new();
                continue;
            }
            Ok(Step::Held(hold)) => {
                if said != Some(hold) {
                    tracing::info!("compactor: {hold}");
                    said = Some(hold);
                }
                let now = prodrome_core::fpl::instant_of(caps.clock.now());
                Some((hold.until() - now).to_std().unwrap_or_default())
            }
            Ok(Step::Idle) => None,
            Err(refused) => {
                let wait = backoff.failed();
                tracing::warn!(
                    "compactor: {}; trying again in {} minutes",
                    refused.message(),
                    wait.as_secs() / 60
                );
                // A minimum: nudges wait in the channel and do not cut it short.
                tokio::time::sleep(wait).await;
                paced = Paced::new(compact.as_ref(), budget);
                continue;
            }
        };
        let woken = match wait {
            Some(until) => tokio::select! {
                () = tokio::time::sleep(until) => true,
                nudge = nudged.recv() => nudge.is_some(),
            },
            None => nudged.recv().await.is_some(),
        };
        if !woken {
            return;
        }
    }
}

/// A firing as a log line names it: the node by its first twelve digits.
fn logged(done: &Compacted) -> String {
    let short = |hash: &prodrome_core::event::Hash| hash.as_str()[..12].to_owned();
    match done {
        Compacted::Summarized { node, object } => {
            format!("summarised {} as {}", short(node), short(object))
        }
        Compacted::Unfit { node, object } => {
            format!("found no line for {} ({})", short(node), short(object))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backoff_doubles_to_its_most_and_starts_again() {
        let mut backoff = Backoff::new();
        let waits: Vec<u64> = (0..7).map(|_| backoff.failed().as_secs() / 60).collect();
        assert_eq!(waits, [15, 30, 60, 120, 240, 240, 240]);
        assert_eq!(Backoff::new().failed(), RETRY);
    }
}
