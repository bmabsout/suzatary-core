use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::agent::{Call, TriageSurface, answered, refused};
use crate::refusal::Refusal;
use crate::verb::Verb;

/// Ignore a sender wholesale.
pub struct NoiseAdd;

#[derive(Deserialize, JsonSchema)]
pub struct NoiseAddArgs {
    sender: String,
}

impl Verb for NoiseAdd {
    type Surface = TriageSurface;
    type Args = NoiseAddArgs;
    type Answer = Value;
    const NAME: &'static str = "noise_add";
    const DESCRIPTION: &'static str =
        "Add a sender (bare email address) to the wholesale-ignore list.";

    fn run(call: &Call<'_>, args: NoiseAddArgs) -> Result<Value, Refusal> {
        answered(
            &call
                .caps
                .corpus
                .noise_add(&args.sender)
                .map_err(|e| refused(&e))?,
        )
    }
}
