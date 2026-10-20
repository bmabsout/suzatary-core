use serde_json::{Value, json};

use crate::refusal::Refusal;
use crate::secretary::{Bound, Call, SecretarySurface};
use crate::verb::{NoArgs, Verb};
use crate::wire::ProposalsPayload;

/// Every batch the triage agent left standing.
pub struct ProposalsList;

impl Verb for ProposalsList {
    type Surface = SecretarySurface;
    type Args = NoArgs;
    type Answer = Value;
    const NAME: &'static str = "proposals_list";
    const DESCRIPTION: &'static str = "EVERY BATCH THE TRIAGE AGENT HAS LEFT STANDING, on both chains, exactly \
        as the app's Inbox draws them: each batch by its head, with every object \
        as a readable diff — what is being added, completed, cancelled, \
        repriced or reworded, and the before/after of each field.\n\n\
        A batch is ONE RUN's work. `needs` on a row names the objects inside the \
        same batch it rests on, so taking a completion means taking the todo it \
        completes. `pending_todo` means the row records an outcome for a todo \
        only ANOTHER batch would put on the chain.\n\n\
        It takes no `chain`: there is one Inbox and reviewing half of it is not \
        reviewing it.";

    fn run(call: &Call<'_>, _: NoArgs) -> Result<Value, Refusal> {
        Ok(answered(&listed(call.chains, call.deployment.zone)?))
    }
}

/// Every chain's standing batches, as the verb lists them.
///
/// # Errors
/// A chain or its replica that does not read.
pub fn listed(
    chains: &[Bound<'_>],
    zone: crate::zone::Zone,
) -> Result<Vec<ProposalsPayload>, Refusal> {
    chains
        .iter()
        .map(|bound| {
            crate::proposals::routes::list(
                bound.caps.chain,
                bound.caps.proposals,
                bound.caps.conversations,
                bound.caps.clock,
                zone,
                bound.policy,
            )
        })
        .collect()
}

/// What the verb answers over `listed`: the batches, and what to do with them.
#[must_use]
pub fn answered(listed: &[ProposalsPayload]) -> Value {
    json!({
        "chains": listed,
        "next": "Settle the ordinary ones with proposals_accept / proposals_reject, naming the \
                 chain and the object hashes and saying WHY. Leave a batch STANDING when it \
                 changes a real commitment of the owner's — SECRETARY.md has the rule — and say in \
                 your answer which ones you held and why. When in doubt, hold.",
    })
}
