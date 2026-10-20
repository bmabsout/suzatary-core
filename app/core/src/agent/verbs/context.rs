use serde_json::Value;

use crate::agent::{Call, TriageSurface, reads};
use crate::refusal::Refusal;
use crate::verb::{NoArgs, Verb};

/// The manuals and the vocabularies, first.
pub struct Context;

impl Verb for Context {
    type Surface = TriageSurface;
    type Args = NoArgs;
    type Answer = Value;
    const NAME: &'static str = "context";
    const DESCRIPTION: &'static str = "START HERE. Everything you need to act with understanding rather than \
        classify messages in isolation: the triage contract (OPERATOR.md) and the \
        exact vocabularies the authoring verbs accept; CLAUDE.md is already your \
        project memory.";

    fn run(call: &Call<'_>, _: NoArgs) -> Result<Value, Refusal> {
        Ok(reads::context(call.caps.manuals, call.deployment))
    }
}
