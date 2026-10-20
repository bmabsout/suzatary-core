//! One message of `notmuch show --format=json`, parsed once at the boundary.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;
use suzatary_core::capability::MailError;

/// One message as notmuch shows it: every field the readers use, typed, and a message without
/// an `id` or a file does not parse, so no reader meets an empty one.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NotmuchMessage {
    /// notmuch's own id, its `Message-ID` without the brackets.
    pub id: String,
    /// Whether the query matched it, rather than its thread bringing it along.
    #[serde(rename = "match")]
    pub matched: bool,
    /// The first file holding it.
    #[serde(rename = "filename", deserialize_with = "first_file")]
    pub file: PathBuf,
    /// Its tags.
    pub tags: Vec<String>,
    /// Its headers by name.
    pub headers: HashMap<String, String>,
    /// Its MIME tree, absent under `--body=false`.
    pub body: Option<Value>,
}

/// notmuch names a message's files as an array, and a message must have one.
fn first_file<'de, D: serde::Deserializer<'de>>(de: D) -> Result<PathBuf, D::Error> {
    Vec::<PathBuf>::deserialize(de)?
        .into_iter()
        .next()
        .ok_or_else(|| serde::de::Error::custom("a message names no file"))
}

/// One node of a thread: a message (`null` where notmuch elides one) and its replies.
#[derive(Debug, Deserialize)]
struct Node(Option<NotmuchMessage>, Vec<Node>);

/// The messages a `notmuch show` matched, in its order, parsed from its whole answer: a list of
/// threads, each a list of nodes.
///
/// One message that does not parse (no id, no file) refuses the whole answer rather than that
/// one row. notmuch mints an id for every message it indexes, so such an answer is not notmuch's
/// show format at all, and none of its rows is to be believed.
pub fn matched(raw: &str) -> Result<Vec<NotmuchMessage>, MailError> {
    fn walk(nodes: Vec<Node>, out: &mut Vec<NotmuchMessage>) {
        for Node(message, replies) in nodes {
            out.extend(message.filter(|m| m.matched));
            walk(replies, out);
        }
    }
    let threads: Vec<Vec<Node>> = serde_json::from_str(raw)
        .map_err(|e| MailError::Unreachable(format!("notmuch's show output did not parse: {e}")))?;
    let mut out = Vec::new();
    for thread in threads {
        walk(thread, &mut out);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: &str, matched: bool) -> serde_json::Value {
        serde_json::json!({
            "id": id, "match": matched, "filename": [format!("/mail/{id}")],
            "tags": ["inbox"], "headers": {"Subject": "a subject"}
        })
    }

    #[test]
    fn a_non_matching_reply_is_walked_into_and_not_yielded() {
        let forest = serde_json::json!([[[
            message("root", true),
            [[message("reply", false), [[message("late", true), []]]]]
        ]]]);
        let ids: Vec<String> = matched(&forest.to_string())
            .expect("parses")
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, vec!["root", "late"]);
    }

    #[test]
    fn an_elided_message_is_a_null_and_its_replies_are_still_read() {
        let forest = serde_json::json!([[[null, [[message("reply", true), []]]]]]);
        let found = matched(&forest.to_string()).expect("parses");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, PathBuf::from("/mail/reply"));
        assert_eq!(found[0].body, None);
    }

    #[test]
    fn a_message_without_an_id_is_refused() {
        let mut nameless = message("x", true);
        nameless.as_object_mut().expect("an object").remove("id");
        let forest = serde_json::json!([[[nameless, []]]]);
        let refused = matched(&forest.to_string()).expect_err("no id");
        assert!(
            refused.to_string().contains("missing field `id`"),
            "{refused}"
        );
    }

    #[test]
    fn a_message_without_a_file_is_refused() {
        let mut fileless = message("x", true);
        fileless["filename"] = serde_json::json!([]);
        let forest = serde_json::json!([[[fileless, []]]]);
        assert!(matched(&forest.to_string()).is_err());
    }

    #[test]
    fn an_answer_of_another_shape_is_refused_not_walked_past() {
        for raw in ["{}", "[[[1, []]]]", "[[[\"x\"]]]", "not json"] {
            assert!(matched(raw).is_err(), "{raw}");
        }
        assert!(matched("[]").expect("no threads").is_empty());
    }
}
