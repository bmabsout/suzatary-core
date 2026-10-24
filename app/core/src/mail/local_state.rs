//! What the box knows of its mail beside the decisions: never pushed, carried by the vault.
//!
//! `local/mail.json` in the box-local tier, keyed by the canonical content hash, which may be
//! named here because nothing here leaves the box: where each decided message lives (a notmuch
//! locator, so a hash can be shown without rehashing the corpus), the agent's one-line note,
//! and the senders whose mail is ignored wholesale. Every message the box decides about has an
//! entry, so these keys are also the hashes behind the pushed file's keyed names.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// The version this program reads and writes.
pub const SCHEMA_VERSION: u64 = 1;

/// What the box keeps of one decided message; field order is the stored key order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kept {
    /// A notmuch locator, recorded when the message was marked: a shortcut back to the mail,
    /// never its identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<String>,
    /// A one-line why, when the agent left one; `""` is absent and omitted.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// `local/mail.json` could not be read.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum LocalError {
    /// Not JSON of its shape.
    #[error("the mail's box-local file is not JSON of its shape: {0}")]
    Unparseable(String),
    /// Another version.
    #[error("the mail's box-local file is version {0:?}; this program reads {SCHEMA_VERSION}")]
    Version(Option<u64>),
}

/// A hash prefix matched more than one marked message.
///
/// It names the matches, since "use more characters" alone is a dead end.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error(
    "hash prefix {prefix:?} matches {} marked messages ({}) — use more characters",
    matches.len(),
    matches.iter().map(|h| h.chars().take(12).collect::<String>()).collect::<Vec<_>>().join(", ")
)]
pub struct Ambiguous {
    /// What was asked for.
    pub prefix: String,
    /// Every hash it matched, sorted.
    pub matches: Vec<String>,
}

/// The box-local mail state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalState {
    /// Senders whose mail is ignored wholesale, lower-cased.
    pub noise_senders: BTreeSet<String>,
    /// What is kept of each decided message, by its canonical hash.
    pub messages: BTreeMap<String, Kept>,
}

/// The file's shape, in the order it is written.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u64,
    noise_senders: BTreeSet<String>,
    messages: BTreeMap<String, Kept>,
}

impl LocalState {
    /// Read the file.
    ///
    /// # Errors
    /// [`LocalError`].
    pub fn parse(text: &str) -> Result<Self, LocalError> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| LocalError::Unparseable(e.to_string()))?;
        let version = value.get("version").and_then(serde_json::Value::as_u64);
        if version != Some(SCHEMA_VERSION) {
            return Err(LocalError::Version(version));
        }
        let document: Document =
            serde_json::from_value(value).map_err(|e| LocalError::Unparseable(e.to_string()))?;
        Ok(Self {
            noise_senders: document.noise_senders,
            messages: document.messages,
        })
    }

    /// The file's exact text, trailing newline included.
    #[must_use]
    #[expect(
        clippy::missing_panics_doc,
        reason = "unreachable: every field is a string, a u64 or a map with string keys"
    )]
    pub fn print(&self) -> String {
        let document = Document {
            version: SCHEMA_VERSION,
            noise_senders: self.noise_senders.clone(),
            messages: self.messages.clone(),
        };
        let mut text =
            serde_json::to_string_pretty(&document).expect("a local document always serialises");
        text.push('\n');
        text
    }

    /// Is this sender ignored wholesale?
    #[must_use]
    pub fn is_noise(&self, sender: &str) -> bool {
        self.noise_senders.contains(&sender.to_lowercase())
    }

    /// Add a sender to the wholesale-ignore list.
    #[must_use]
    pub fn add_noise(mut self, sender: &str) -> Self {
        self.noise_senders.insert(sender.to_lowercase());
        self
    }

    /// Keep `kept` for the message whose canonical hash is `hash`.
    #[must_use]
    pub fn keep(mut self, hash: &str, kept: Kept) -> Self {
        self.messages.insert(hash.to_owned(), kept);
        self
    }

    /// The one decided message a hash prefix names; `None` when nothing matches, which is an
    /// answer, not an error.
    ///
    /// # Errors
    /// [`Ambiguous`] naming every match.
    pub fn resolve(&self, prefix: &str) -> Result<Option<(&String, &Kept)>, Ambiguous> {
        let matches: Vec<(&String, &Kept)> = self
            .messages
            .iter()
            .filter(|(hash, _)| hash.starts_with(prefix))
            .collect();
        match matches.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(*one)),
            many => Err(Ambiguous {
                prefix: prefix.to_owned(),
                matches: many.iter().map(|(hash, _)| (*hash).clone()).collect(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_printed_document_round_trips() {
        let state = LocalState::default()
            .add_noise("Noreply@Example.Test")
            .keep(
                "bbbb",
                Kept {
                    native_id: Some("msg-2".to_owned()),
                    note: "renewal".to_owned(),
                },
            )
            .keep("aaaa", Kept::default());
        let printed = state.print();
        assert_eq!(
            printed,
            concat!(
                "{\n",
                "  \"version\": 1,\n",
                "  \"noise_senders\": [\n",
                "    \"noreply@example.test\"\n",
                "  ],\n",
                "  \"messages\": {\n",
                "    \"aaaa\": {},\n",
                "    \"bbbb\": {\n",
                "      \"native_id\": \"msg-2\",\n",
                "      \"note\": \"renewal\"\n",
                "    }\n",
                "  }\n",
                "}\n",
            )
        );
        assert_eq!(LocalState::parse(&printed), Ok(state));
        assert_eq!(LocalState::parse("{}"), Err(LocalError::Version(None)));
    }
}
