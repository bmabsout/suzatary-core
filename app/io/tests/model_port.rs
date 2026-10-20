//! The model's conformance suite over a real interpreter, `OpenAiCompatible`, whose far end is a
//! scripted HTTP transport: the request is built, sent and its answer read as on the wire, and
//! no socket is opened.

use std::sync::{Arc, Mutex, PoisonError};

use suzatary_core::capability::{Conversation, Secret, ToolCall, Turn};
use suzatary_core::conformance::model::{Case, Rig, check};
use suzatary_io::http::{HttpError, Resp, Scripted};
use suzatary_io::openai::{Attestation, OpenAiCompatible};

/// Each model gets its own scripted far end; the rig keeps the last, to say what it heard.
#[derive(Default)]
struct Wire(Mutex<Option<Arc<Scripted>>>);

impl Wire {
    fn model(&self, far: Scripted) -> OpenAiCompatible<Arc<Scripted>> {
        let far = Arc::new(far);
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(&far));
        OpenAiCompatible::with_transport(
            "https://models.example.test/v1",
            "a-model",
            Secret::new("not-a-key"),
            Attestation::Unclaimed,
            far,
        )
    }
}

impl Rig for Wire {
    type Model = OpenAiCompatible<Arc<Scripted>>;

    fn answering(&self, text: &str) -> Self::Model {
        let body = serde_json::json!({"choices": [{"message": {"content": text}}]}).to_string();
        self.model(Scripted::always(Ok(Resp {
            status: 200,
            headers: Vec::new(),
            body: body.into_bytes(),
        })))
    }

    fn unreachable(&self) -> Self::Model {
        self.model(Scripted::always(Err(HttpError::NotSent(
            "no route to the far end".to_owned(),
        ))))
    }

    /// Every string the requests carried, in order: what the far end read.
    fn heard(&self, _model: &Self::Model) -> String {
        let far = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let mut heard = Vec::new();
        for request in far.map(|far| far.sent()).unwrap_or_default() {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
            strings(&body, &mut heard);
        }
        heard.join("\n")
    }
}

fn strings(value: &serde_json::Value, into: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => into.push(text.clone()),
        serde_json::Value::Array(values) => values.iter().for_each(|value| strings(value, into)),
        serde_json::Value::Object(fields) => fields.values().for_each(|value| strings(value, into)),
        _ => {}
    }
}

#[test]
fn the_openai_compatible_interpreter_keeps_every_law_over_a_scripted_far_end() {
    let conversation = Conversation {
        system: "You are the secretary. Propose; never send.".to_owned(),
        turns: vec![
            Turn::Person {
                text: "When does the lease renew?".to_owned(),
                references: Vec::new(),
                attachments: Vec::new(),
            },
            Turn::Model {
                text: "Let me look.".to_owned(),
                calls: vec![ToolCall {
                    verb: "search".into(),
                    args: std::collections::BTreeMap::new(),
                }],
            },
            Turn::Tool {
                verb: "search".into(),
                answer: "one message: the lease renews on 1 October".to_owned(),
                refused: false,
                files: Vec::new(),
            },
            Turn::Act {
                text: "Ada dismissed proposal 1.".to_owned(),
                references: Vec::new(),
            },
        ],
    };
    let cases = [
        Case {
            conversation: conversation.clone(),
            answer: r#"{"say": "It renews on 1 October.", "calls": []}"#.to_owned(),
        },
        Case {
            conversation: conversation.clone(),
            answer: "```json\n{\"say\": \"Checking.\", \"calls\": [{\"verb\": \"context\", \
                     \"args\": {\"day\": \"today\"}}]}\n```"
                .to_owned(),
        },
        Case {
            conversation,
            answer: "Plain prose, which is what was said.".to_owned(),
        },
    ];
    let tools = serde_json::json!([{"name": "context"}, {"name": "search"}]);
    assert_eq!(check(&Wire::default(), &tools, &cases), Ok(()));
}
