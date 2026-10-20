use serde_json::Value;

use crate::refusal::Refusal;
use crate::secretary::store::Decision;
use crate::secretary::verbs::{SettleArgs, settle};
use crate::secretary::{Call, SecretarySurface};
use crate::verb::Verb;

/// Delete the triage agent's objects from the proposal store.
pub struct ProposalsReject;

impl Verb for ProposalsReject {
    type Surface = SecretarySurface;
    type Args = SettleArgs;
    type Answer = Value;
    const NAME: &'static str = "proposals_reject";
    const DESCRIPTION: &'static str = "DELETE these objects from the proposal store. The one deletion this \
        system performs, allowed because those objects never entered a chain — \
        and their names go into a receipt file, so the same bytes cannot come \
        back.\n\n\
        `objects` is a comma-separated list of hashes and the set must be \
        DESCENDANT-CLOSED within its batch: rejecting a todo means rejecting what \
        was written on top of it. `why` is REQUIRED and recorded.\n\n\
        Reject a DUPLICATE (the same todo already standing in an earlier batch, \
        or already on the chain — keep the newest and reject the older), noise, a \
        todo that restates one already open, and a reprice with no new fact. \
        A rejection is cheap: the agent will propose it again tomorrow if it \
        still matters, and it cannot come back unchanged.";

    fn run(call: &Call<'_>, args: SettleArgs) -> Result<Value, Refusal> {
        settle(call, &args, Decision::Rejected)
    }
}
