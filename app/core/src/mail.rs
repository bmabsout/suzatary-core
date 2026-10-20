//! The mail corpus's values: bodies are always text, a cut body says so, and triage's
//! disposition is a badge, never a filter.

pub mod account;
pub mod account_email;
pub mod address;
pub mod cache;
pub mod dedup_key;
pub mod folders;
pub mod gone;
pub mod grants;
pub mod health;
pub mod idle;
pub mod imap;
pub mod junk;
pub mod known;
pub mod local_state;
pub mod oauth;
pub mod older;
pub mod outbox;
pub mod removal;
pub mod send;
pub mod state;
pub mod sync_now;
pub mod transport;
pub mod triage;
pub mod verbs;

use serde::Serialize;
use ts_rs::TS;

use crate::capability::Datetime;

/// A link as it appeared in the mail; the label is what tells lookalike links apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct MailLink {
    /// Where it goes, filtered to http/https/mailto before it reaches a page.
    pub url: String,
    /// The text it stood behind, "" for a bare URL.
    pub label: String,
}

/// One header of a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct MailHeader {
    /// The header's name.
    pub name: String,
    /// Its value.
    pub value: String,
}

/// One row of the app's mail list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct MailRow {
    /// The canonical content hash, of which a todo's `source.hash` is a prefix.
    pub hash: String,
    /// The notmuch locator, for a second read.
    pub id: String,
    /// The From address, lower-cased.
    pub sender: String,
    /// The Subject, RFC 2047 decoded.
    pub subject: String,
    /// When it arrived, `None` when the Date header would not parse (`""` on the wire).
    #[serde(with = "date")]
    #[ts(type = "string")]
    pub date: Option<Datetime>,
    /// The maildir folder it lives in.
    pub container: String,
    /// The account's canonical address, "" for a maildir not of this deployment.
    pub account: String,
    /// What triage decided about this hash.
    pub disposition: Disposed,
}

/// What triage decided about a message, as a row shows it: nothing yet, or its decision, the
/// todo only beside the one decision that names one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Disposed {
    /// Triage has decided nothing about it.
    Undecided,
    /// Actionable: a todo was authored for it.
    Todo {
        /// The todo, by id.
        todo: String,
    },
    /// Automated, marketing or bot mail.
    Noise,
    /// Seen, not actionable, not noise.
    Skip,
}

impl From<Option<&state::Decision>> for Disposed {
    fn from(decision: Option<&state::Decision>) -> Self {
        match decision {
            None => Self::Undecided,
            Some(state::Decision::Todo { todo_id }) => Self::Todo {
                todo: todo_id.as_str().to_owned(),
            },
            Some(state::Decision::Noise) => Self::Noise,
            Some(state::Decision::Skip) => Self::Skip,
        }
    }
}

/// One message as a page; which todos it produced is the chain's fact, joined by the route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct MailMessage {
    /// Its list row.
    pub row: MailRow,
    /// notmuch's conversation id, "" when the corpus names none.
    pub thread: String,
    /// The whitelisted headers it carries, in reading order.
    pub headers: Vec<MailHeader>,
    /// The body as text.
    pub body: String,
    /// Whether the body was cut at the cap.
    pub truncated: bool,
    /// The links it holds.
    pub links: Vec<MailLink>,
}

/// The cap on a body's characters; past it the wire says `truncated`.
pub const MAX_BODY_CHARS: usize = 200_000;

/// One attachment's identity (name, MIME type, decoded size) with no content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentMeta {
    /// Its filename from the disposition or the content type, `""` when neither names one.
    pub name: String,
    /// `type/subtype`, lower-cased, `""` when the part carried none.
    pub mime: String,
    /// Its decoded size in bytes, not the wire encoding's length.
    pub size: usize,
}

/// One attachment whole: what the message calls it, its type and its decoded bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentFile {
    /// Its filename, as [`AttachmentMeta::name`] lists it.
    pub name: String,
    /// `type/subtype`, lower-cased, `""` when the part carried none.
    pub mime: String,
    /// Its decoded bytes.
    pub content: Vec<u8>,
}

/// The cap on an attachment's extracted characters, read for one fact rather than whole.
pub const MAX_ATTACHMENT_CHARS: usize = 100_000;

/// The headers a message page prints, in reading order; a whitelist.
pub const SHOWN_HEADERS: &[&str] = &["From", "To", "Cc", "Subject", "Date", "Reply-To"];

/// The schemes a link may carry to a page.
pub const LINK_SCHEMES: &[&str] = &["http://", "https://", "mailto:"];

/// A source hash prefix shorter than this names todos by accident, so it is not evidence.
pub const MIN_SOURCE_PREFIX: usize = 6;

/// `text` cut to at most `chars` characters, on a char boundary, never panicking.
#[must_use]
pub fn shorten(text: &str, chars: usize) -> &str {
    match text.char_indices().nth(chars) {
        Some((at, _)) => &text[..at],
        None => text,
    }
}

/// `text` cut to `cap` characters, and whether it was cut.
#[must_use]
pub fn clip(text: &str, cap: usize) -> (String, bool) {
    let short = shorten(text, cap);
    let truncated = short.len() != text.len();
    (short.to_owned(), truncated)
}

/// At most `len` characters of `text` starting at character `start`: one page of a longer text.
#[must_use]
pub fn page(text: &str, start: usize, len: usize) -> &str {
    let from = shorten(text, start).len();
    shorten(&text[from..], len)
}

/// Only the links a page may carry.
#[must_use]
pub fn web_links(links: Vec<MailLink>) -> Vec<MailLink> {
    links
        .into_iter()
        .filter(|link| {
            let lowered = link.url.to_lowercase();
            LINK_SCHEMES
                .iter()
                .any(|scheme| lowered.starts_with(scheme))
        })
        .collect()
}

/// The whitelisted headers this message actually carries, in reading order.
#[must_use]
pub fn headers_of(headers: &[(String, String)]) -> Vec<MailHeader> {
    SHOWN_HEADERS
        .iter()
        .filter_map(|name| {
            headers
                .iter()
                .find(|(held, value)| held == name && !value.is_empty())
                .map(|(held, value)| MailHeader {
                    name: held.clone(),
                    value: value.clone(),
                })
        })
        .collect()
}

mod date {
    use prodrome_core::fpl::{instant_of, iso};
    use serde::Serializer;

    use crate::capability::Datetime;

    #[expect(clippy::ref_option, reason = "serde's `with` signature")]
    pub fn serialize<S: Serializer>(
        at: &Option<Datetime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&at.map_or_else(String::new, |at| iso(instant_of(at))))
    }
}

/// Was this todo authored from the message with this canonical hash (the todo holds a prefix)?
#[must_use]
pub fn sourced_from(source_hash: &str, message_hash: &str) -> bool {
    source_hash.len() >= MIN_SOURCE_PREFIX && message_hash.starts_with(source_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_at_the_cap_is_whole_and_one_past_it_is_cut() {
        assert_eq!(clip("abcde", 5), ("abcde".to_owned(), false));
        assert_eq!(clip("abcdef", 5), ("abcde".to_owned(), true));
        let (body, cut) = clip("héllo wörld", 5);
        assert!(cut);
        assert_eq!(body, "héllo");
    }

    /// Every byte offset of a multi-byte name is a safe cut.
    #[test]
    fn shorten_never_panics_on_a_multi_byte_name_at_any_cut() {
        let name = "Nora Vale (诺拉)";
        for chars in 0..=name.chars().count() + 2 {
            let cut = shorten(name, chars);
            assert!(name.is_char_boundary(cut.len()), "{chars} chars: {cut:?}");
        }
    }

    /// A 4-byte emoji and a combining mark cut on `char` boundaries.
    #[test]
    fn shorten_holds_at_a_four_byte_emoji_and_a_combining_mark() {
        let emoji = "a🎉b"; // 🎉 is U+1F389, 4 bytes.
        for chars in 0..=4 {
            let cut = shorten(emoji, chars);
            assert!(emoji.is_char_boundary(cut.len()), "{chars} chars: {cut:?}");
        }
        assert_eq!(shorten(emoji, 1), "a");
        assert_eq!(shorten(emoji, 2), "a🎉");

        let combining = "e\u{301}f"; // "é" as `e` + COMBINING ACUTE ACCENT.
        for chars in 0..=3 {
            let cut = shorten(combining, chars);
            assert!(
                combining.is_char_boundary(cut.len()),
                "{chars} chars: {cut:?}"
            );
        }
        // One glyph but two `char`s, so this cut is valid.
        assert_eq!(shorten(combining, 1), "e");
    }

    /// Pages join back to the whole, and a page past the end is empty.
    #[test]
    fn page_of_covers_the_whole_text_in_order() {
        let text = "abcdefghij";
        assert_eq!(page(text, 0, 4), "abcd");
        assert_eq!(page(text, 4, 4), "efgh");
        assert_eq!(page(text, 8, 4), "ij");
        assert_eq!(page(text, 10, 4), "");
        assert_eq!(page(text, 100, 4), "");
    }

    /// A multi-byte name paged in 4-character pieces joins back exactly.
    #[test]
    fn page_of_a_multi_byte_name_reassembles_to_the_whole() {
        let name = "Nora Vale (诺拉)";
        let mut joined = String::new();
        let mut at = 0;
        loop {
            let piece = page(name, at, 4);
            if piece.is_empty() {
                break;
            }
            joined.push_str(piece);
            at += 4;
        }
        assert_eq!(joined, name);
    }

    #[test]
    fn only_three_schemes_reach_a_page() {
        let links = vec![
            MailLink {
                url: "https://example.test/a".into(),
                label: "a".into(),
            },
            MailLink {
                url: "JAVASCRIPT:alert(1)".into(),
                label: "b".into(),
            },
            MailLink {
                url: "mailto:her@example.test".into(),
                label: String::new(),
            },
            MailLink {
                url: "data:text/html,<script>".into(),
                label: String::new(),
            },
        ];
        let kept: Vec<String> = web_links(links).into_iter().map(|l| l.url).collect();
        assert_eq!(
            kept,
            vec![
                "https://example.test/a".to_owned(),
                "mailto:her@example.test".to_owned()
            ]
        );
    }

    /// Reading order, and nothing outside the whitelist.
    #[test]
    fn headers_come_back_whitelisted_and_in_reading_order() {
        let held = vec![
            ("Subject".to_owned(), "hello".to_owned()),
            ("X-Spam-Score".to_owned(), "9".to_owned()),
            ("From".to_owned(), "her@example.test".to_owned()),
            ("Cc".to_owned(), String::new()),
        ];
        assert_eq!(
            headers_of(&held),
            vec![
                MailHeader {
                    name: "From".to_owned(),
                    value: "her@example.test".to_owned(),
                },
                MailHeader {
                    name: "Subject".to_owned(),
                    value: "hello".to_owned(),
                },
            ]
        );
    }

    /// A row with no date prints it as `""`; its disposition is a tagged value, and only a todo
    /// decision carries a todo.
    #[test]
    fn a_row_prints_its_disposition_as_a_tagged_value() {
        let row = MailRow {
            hash: "abc".to_owned(),
            id: "id@x".to_owned(),
            sender: "her@example.test".to_owned(),
            subject: "hello".to_owned(),
            date: None,
            container: "inbox".to_owned(),
            account: "me@example.test".to_owned(),
            disposition: Disposed::Undecided,
        };
        assert_eq!(
            serde_json::to_value(&row).expect("serialises"),
            serde_json::json!({"hash": "abc", "id": "id@x", "sender": "her@example.test",
                "subject": "hello", "date": "", "container": "inbox",
                "account": "me@example.test", "disposition": {"kind": "undecided"}})
        );
        let decided = |decision: &state::Decision| {
            serde_json::to_value(MailRow {
                date: Some(Datetime::new(2026, 9, 1, 8, 30, 0, 0).expect("a real instant")),
                disposition: Disposed::from(Some(decision)),
                ..row.clone()
            })
            .expect("serialises")
        };
        let json = decided(&state::Decision::Noise);
        assert_eq!(json["date"], "2026-09-01T08:30:00");
        assert_eq!(json["disposition"], serde_json::json!({"kind": "noise"}));
        let todo = state::Decision::parse("todo", Some("renewcard")).expect("a decision");
        assert_eq!(
            decided(&todo)["disposition"],
            serde_json::json!({"kind": "todo", "todo": "renewcard"})
        );
    }

    /// A prefix shorter than six names no todo.
    #[test]
    fn the_reverse_index_is_one_directional_and_has_a_floor() {
        assert!(sourced_from("abc123", "abc123def456"));
        assert!(!sourced_from("abc12", "abc123def456"));
        assert!(!sourced_from("", "abc123def456"));
        assert!(!sourced_from("abc123def456", "abc123"));
    }

    use proptest::prelude::*;

    proptest! {
        /// `shorten` never panics and never splits a `char` over arbitrary text.
        #[test]
        fn shorten_never_panics_and_never_splits_a_char(text: String, chars in 0usize..200) {
            let short = shorten(&text, chars);
            prop_assert!(text.is_char_boundary(short.len()));
        }

        /// `shorten` answers at most `chars` characters.
        #[test]
        fn shorten_never_answers_more_than_asked(text: String, chars in 0usize..200) {
            prop_assert!(shorten(&text, chars).chars().count() <= chars);
        }

        /// `clip`'s flag is exactly whether `shorten` cut anything.
        #[test]
        fn clip_flags_truncation_exactly_when_shorten_cuts(text: String, cap in 0usize..200) {
            let (clipped, truncated) = clip(&text, cap);
            prop_assert_eq!(truncated, clipped != text);
            prop_assert_eq!(clipped.as_str(), shorten(&text, cap));
        }

        /// `page` never panics, including a `start` past the end.
        #[test]
        fn page_never_panics_and_never_splits_a_char(
            text: String,
            start in 0usize..200,
            len in 0usize..200,
        ) {
            let piece = page(&text, start, len);
            prop_assert!(piece.chars().count() <= len);
        }

        /// Paging in fixed steps from 0 recovers the whole text.
        #[test]
        fn paging_in_steps_reassembles_the_whole_text(text: String, step in 1usize..50) {
            let mut joined = String::new();
            let mut at = 0;
            loop {
                let piece = page(&text, at, step);
                if piece.is_empty() {
                    break;
                }
                joined.push_str(piece);
                at += step;
            }
            prop_assert_eq!(joined, text);
        }
    }
}
