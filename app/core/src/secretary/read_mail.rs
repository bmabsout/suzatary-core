//! The messages a conversation read: the only ones a held value it proposes to send may come from.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::capability::Turn;
use crate::held_value::Handle;
use crate::secretary::store::Stored;
use crate::secretary::surface::SecretaryVerb;
use crate::secretary::tool_name::ToolName;

/// The notmuch ids of every message a `show`, a `search` or a reference the person pointed at
/// answered in a conversation, by the answers' `id` fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadMail(BTreeSet<String>);

impl ReadMail {
    /// The messages `stored`'s mail answers name.
    #[must_use]
    pub fn of(stored: &Stored) -> Self {
        let mut read = BTreeSet::new();
        for said in &stored.turns {
            let answer = match &said.turn {
                Turn::Tool {
                    verb:
                        ToolName::Verb(SecretaryVerb::Show | SecretaryVerb::Search)
                        | ToolName::Reference,
                    answer,
                    ..
                } => answer,
                Turn::Tool { .. }
                | Turn::Person { .. }
                | Turn::Model { .. }
                | Turn::Act { .. }
                | Turn::Erased { .. } => continue,
            };
            if let Ok(value) = serde_json::from_str::<Value>(answer) {
                named(&value, &mut read);
            }
        }
        Self(read)
    }

    /// Whether the message `handle` names was read.
    #[must_use]
    pub fn holds(&self, handle: &Handle) -> bool {
        self.reads(handle.message())
    }

    /// Whether the message `message`, a notmuch id, was read.
    #[must_use]
    pub fn reads(&self, message: &str) -> bool {
        self.0.contains(message)
    }
}

/// Every `id` field of `value`: the notmuch id a mail answer names each message by. A
/// placeholder in an answer's text marks nothing read, since a Drive file or a web page may hold
/// one typed by anyone.
fn named(value: &Value, read: &mut BTreeSet<String>) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields {
                if let (true, Value::String(id)) = (key == "id", field) {
                    read.insert(id.clone());
                }
                named(field, read);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| named(item, read)),
        Value::String(_) | Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}
