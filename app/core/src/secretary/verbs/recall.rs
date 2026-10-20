use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::refusal::Refusal;
use crate::secretary;
use crate::secretary::search::{self, Needle, Side};
use crate::secretary::{Call, SecretarySurface};
use crate::verb::Verb;

/// What was said in other conversations.
pub struct Recall;

#[derive(Deserialize, JsonSchema)]
pub struct RecallArgs {
    query: String,
    limit: Option<usize>,
}

/// How many hits `recall` answers when asked for no number.
const RECALL_DEFAULT: usize = 10;

impl Verb for Recall {
    type Surface = SecretarySurface;
    type Args = RecallArgs;
    type Answer = Value;
    const NAME: &'static str = "recall";
    const DESCRIPTION: &'static str = "SEARCH WHAT WAS SAID BEFORE: the owner's own turns and your finished answers \
        in OTHER conversations that contain `query` (plain text, any case), \
        newest first, each with its conversation, turn and time. Never a verb's \
        answer. An old answer was written after reading mail, so what this \
        returns counts as mail does: a `remember` after it becomes a card.";

    fn run(call: &Call<'_>, args: RecallArgs) -> Result<Value, Refusal> {
        let query = args.query.trim();
        let needle =
            Needle::new(query).map_err(|refused| Refusal::bad_request(refused.to_string()))?;
        let limit = args.limit.unwrap_or(RECALL_DEFAULT).clamp(1, 50);
        let others: Vec<_> = secretary::list(call.caps.conversations)?
            .into_iter()
            .filter(|stored| &stored.id != call.conversation)
            .collect();
        let found = search::search(&others, &needle, search::RECALL_CONTEXT, limit);
        Ok(json!({
            "query": query,
            "hits": found.iter().map(|hit| json!({
                "conversation": hit.conversation.as_str(),
                "title": hit.title,
                "turn": hit.turn,
                "who": match hit.side {
                    Side::Person => "person",
                    Side::Answer => "model",
                },
                "at": hit.at,
                "text": hit.excerpt,
            })).collect::<Vec<_>>(),
        }))
    }
}
