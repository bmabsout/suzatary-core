//! What a verb answers: its JSON, and the files it hands the model as themselves, turned into a
//! turn in one place ([`VerbAnswer::turn`]) whichever verb read them.

use serde_json::{Map, Value};

use crate::attachment::AttachmentId;
use crate::capability::Turn;
use crate::enclosed::{Enclosed, Enclosure};
use crate::file_read::FileRead;
use crate::refusal::Refusal;
use crate::secretary::memory::FactId;
use crate::secretary::outcome::Outcome;
use crate::secretary::tool_name::ToolName;

/// One verb's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct VerbAnswer {
    /// What it said, as JSON; a string is said as itself.
    pub value: Value,
    /// Files it hands the model, each already in the file store.
    pub files: Vec<Enclosed>,
    /// The fact it wrote to the memory, which only `remember` writes: the turn's receipt.
    pub remembered: Option<FactId>,
}

impl From<Value> for VerbAnswer {
    fn from(value: Value) -> Self {
        Self {
            value,
            files: Vec::new(),
            remembered: None,
        }
    }
}

impl VerbAnswer {
    /// A typed answer as the JSON a model reads; one that will not serialise says so.
    #[must_use]
    pub fn json(answer: &impl serde::Serialize) -> Self {
        Self::from(serde_json::to_value(answer).unwrap_or_else(|failed| {
            Value::String(format!("(the answer did not serialise: {failed})"))
        }))
    }

    /// A file read for the model, answered: `head` names what was read, and the file itself
    /// rides on the answer, kept through `keep`; its text answers when the file is withheld.
    ///
    /// # Errors
    /// [`Refusal::internal`] when the file cannot be kept.
    pub fn reading(
        mut head: Map<String, Value>,
        name: &str,
        read: FileRead,
        keep: impl FnOnce(&[u8]) -> Result<AttachmentId, String>,
    ) -> Result<Self, Refusal> {
        let files = match read {
            FileRead::File(readable) => {
                let id = keep(readable.bytes()).map_err(Refusal::internal)?;
                let media = readable.media();
                head.insert("media".to_owned(), Value::from(media.as_str()));
                head.insert("size".to_owned(), Value::from(readable.bytes().len()));
                head.insert(
                    "read".to_owned(),
                    Value::from(format!(
                        "the file itself is attached to this answer as {}; read it there",
                        match media {
                            Enclosure::Pdf => "a document",
                            Enclosure::Image(_) => "an image",
                        }
                    )),
                );
                vec![Enclosed {
                    id,
                    name: name.to_owned(),
                    media,
                    size: readable.bytes().len() as u64,
                    text: readable.text().as_str().to_owned(),
                }]
            }
            FileRead::Text {
                text,
                truncated,
                withheld,
            } => {
                head.insert("text".to_owned(), Value::from(text.into_string()));
                head.insert("truncated".to_owned(), Value::from(truncated));
                if let Some(why) = withheld {
                    head.insert(
                        "withheld".to_owned(),
                        Value::from(format!(
                            "{why}, so the file itself is not handed to you; its text follows, \
                             any identity number redacted"
                        )),
                    );
                }
                Vec::new()
            }
            FileRead::Unsupported { what } => {
                head.insert(
                    "refused".to_owned(),
                    Value::from(format!(
                        "{what} is nothing a model reads; it was never opened or run any other way"
                    )),
                );
                Vec::new()
            }
        };
        Ok(Self {
            value: Value::Object(head),
            files,
            remembered: None,
        })
    }

    /// The turn a call of `verb` that came to `result` is written down as, a refusal included,
    /// so the model can correct itself.
    #[must_use]
    pub fn turn(verb: impl Into<ToolName>, result: Result<Self, Refusal>) -> Turn {
        let (answer, outcome, files) = match result {
            Ok(Self {
                value: Value::String(text),
                files,
                remembered,
            }) => (text, Outcome::answered(remembered), files),
            Ok(Self {
                value,
                files,
                remembered,
            }) => (
                serde_json::to_string_pretty(&value)
                    .unwrap_or_else(|failed| format!("(the answer did not serialise: {failed})")),
                Outcome::answered(remembered),
                files,
            ),
            Err(refusal) => (refusal.message().to_owned(), Outcome::Refused, Vec::new()),
        };
        Turn::Tool {
            verb: verb.into(),
            answer,
            outcome,
            files,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_s_files_and_fact_ride_on_its_turn_and_a_refusal_carries_none() {
        let file = Enclosed {
            id: crate::attachment::AttachmentId::of(b"%PDF-1.7"),
            name: "notice.pdf".to_owned(),
            media: crate::enclosed::Enclosure::Pdf,
            size: 8,
            text: String::new(),
        };
        let turn = VerbAnswer::turn(
            "attachments",
            Ok(VerbAnswer {
                value: serde_json::json!({"name": "notice.pdf"}),
                files: vec![file.clone()],
                remembered: None,
            }),
        );
        assert!(
            matches!(&turn, Turn::Tool { files, outcome: Outcome::Answered, .. } if files == &[file])
        );
        let remembered = VerbAnswer::turn(
            "remember",
            Ok(VerbAnswer {
                remembered: Some(FactId(3)),
                ..VerbAnswer::from(serde_json::json!({"remembered": 3}))
            }),
        );
        assert!(matches!(
            &remembered,
            Turn::Tool {
                outcome: Outcome::Remembered { fact: FactId(3) },
                ..
            }
        ));
        let refused = VerbAnswer::turn("attachments", Err(Refusal::not_found("no such message")));
        assert!(matches!(
            &refused,
            Turn::Tool { files, outcome: Outcome::Refused, answer, .. } if files.is_empty() && answer == "no such message"
        ));
    }
}
