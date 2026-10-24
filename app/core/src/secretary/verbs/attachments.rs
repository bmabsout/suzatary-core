use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::refusal::Refusal;
use crate::secretary::verb_answer::VerbAnswer;
use crate::secretary::verbs::{kept, mail_refusal};
use crate::secretary::{Call, SecretarySurface};
use crate::verb::Verb;

/// A message's attachments, or one of them read.
pub struct Attachments;

#[derive(Deserialize, JsonSchema)]
pub struct AttachmentsArgs {
    id: String,
    name: Option<String>,
}

impl Verb for Attachments {
    type Surface = SecretarySurface;
    type Args = AttachmentsArgs;
    type Answer = VerbAnswer;
    const NAME: &'static str = "attachments";
    const DESCRIPTION: &'static str = "A message's attachments — the `id` `show` takes. With no `name`: every \
        attachment as {name, mime, size}, nothing read. With `name`: a PDF or an \
        image is handed to you AS ITSELF, attached to this answer (a scanned PDF \
        page by page), so read it there; `text/*` comes back as its text; \
        anything else (`.docx`, a zip) is a sentence naming its type, never \
        opened or run. A PDF whose text names an SSN, ITIN, TIN or EIN, a code, \
        or a card or bank number is not handed over: its text comes back with the \
        number `[redacted]`. Use it to \
        read a salary in an HR letter or a notice's receipt number without the \
        owner opening the file; `propose_email`'s `attach` sends one.";

    fn run(call: &Call<'_>, args: AttachmentsArgs) -> Result<VerbAnswer, Refusal> {
        let id = args.id.as_str();
        match args.name.as_deref() {
            None => {
                let listed = call
                    .caps
                    .mail
                    .attachments(id)
                    .map_err(|failed| mail_refusal(&failed))?;
                Ok(VerbAnswer::from(json!({
                    "id": id,
                    "attachments": listed.iter().map(|one| json!({
                        "name": one.name,
                        "mime": one.mime,
                        "size": one.size,
                    })).collect::<Vec<_>>(),
                    "next": if listed.is_empty() {
                        "No attachments on this message."
                    } else {
                        "Call attachments again with `name` to read one; propose_email's `attach` \
                         sends one."
                    },
                })))
            }
            Some(name) => {
                let read = call
                    .caps
                    .mail
                    .read_attachment(id, name)
                    .map_err(|failed| mail_refusal(&failed))?;
                let mut head = Map::new();
                head.insert("id".to_owned(), Value::from(id));
                head.insert("name".to_owned(), Value::from(name));
                VerbAnswer::reading(head, name, read, |bytes| kept(call.caps, bytes))
            }
        }
    }
}
