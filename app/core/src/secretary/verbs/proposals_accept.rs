use serde_json::Value;

use crate::refusal::Refusal;
use crate::secretary::store::Settle;
use crate::secretary::verbs::{SettleArgs, settle};
use crate::secretary::{Call, SecretarySurface};
use crate::verb::Verb;

/// Adopt the triage agent's objects onto the chain.
pub struct ProposalsAccept;

impl Verb for ProposalsAccept {
    type Surface = SecretarySurface;
    type Args = SettleArgs;
    type Answer = Value;
    const NAME: &'static str = "proposals_accept";
    const DESCRIPTION: &'static str = "ADOPT these objects onto the chain — the triage agent's proposal becomes \
        The owner's todo. This is a TRUSTED act and it is yours because you have \
        read the sources; it is exactly what they would otherwise tap.\n\n\
        `objects` is a comma-separated list of the object hashes from \
        `proposals_list`, and the set must be ANCESTOR-CLOSED within its batch — \
        picking a completion means also picking the todo it completes. An open \
        pick is refused, naming what is missing.\n\n\
        `why` is REQUIRED and is recorded: one sentence saying what in the \
        sources made this ordinary. It is what the owner and the briefing read back, \
        so write the reason and not the act.\n\n\
        ⚠️ ACCEPT ORDINARY WORK THAT CARRIES ITS EVIDENCE. SECRETARY.md has the \
        rule and the list of things to HOLD for them instead — anything priced at \
        or under 45, anything closing a todo they wrote themselves, anything about \
        identity documents, money, legal or medical. When in doubt, hold: the \
        cost of a held batch is one tap and the cost of a wrong accept is a \
        chain that does not rewrite.";

    fn run(call: &Call<'_>, args: SettleArgs) -> Result<Value, Refusal> {
        settle(call, &args, Settle::Accepted)
    }
}
