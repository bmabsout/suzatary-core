//! Parsing the query string, and every refusal it earns, as pure functions over `Option<&str>`.
//!
//! Capped work is the policy: asking for more than a cap gets the cap, not a slow answer that
//! ties up the one process a phone is waiting on. An empty value is absent, not malformed,
//! since a form sends `?t=` for a field nobody filled in.

use prodrome_core::fpl;

use crate::capability::Datetime;
use crate::reference::Reference;
use crate::refusal::{Refusal, shown};

/// A phone screenful of mail and a bit.
pub const MAIL_LIMIT: usize = 50;
/// The most messages one request will read and hash.
pub const MAX_MAIL: usize = 200;
/// A notmuch query is a query and not a document.
pub const MAX_QUERY_CHARS: usize = 500;
/// Every day the assistant has saved a briefing for, capped.
pub const MAX_BRIEFINGS: usize = 400;
/// The most rows a limited `/api/fold` will answer with.
pub const MAX_FOLD_ROWS: usize = 200;

/// An ISO instant, or the default when absent or empty.
pub fn moment(name: &str, raw: Option<&str>, default: Datetime) -> Result<Datetime, Refusal> {
    let Some(text) = raw.filter(|value| !value.is_empty()) else {
        return Ok(default);
    };
    let parsed = fpl::parse_iso(text).map_err(|_| {
        Refusal::bad_request(format!(
            "{name} must be an ISO datetime, got {}",
            shown(text)
        ))
    })?;
    fpl::datetime_of(parsed).map_err(|_| {
        Refusal::bad_request(format!(
            "{name} must be an ISO datetime, got {}",
            shown(text)
        ))
    })
}

/// A mail list's cap: capped, not obeyed, because the ceiling is what one request will read
/// and hash.
pub fn limit(raw: Option<&str>) -> Result<usize, Refusal> {
    let Some(text) = raw.filter(|value| !value.is_empty()) else {
        return Ok(MAIL_LIMIT);
    };
    match text.parse::<usize>() {
        Ok(asked) if asked >= 1 && text.chars().all(|c| c.is_ascii_digit()) => {
            Ok(asked.min(MAX_MAIL))
        }
        _ => Err(Refusal::bad_request(format!(
            "limit must be a positive integer, got {}",
            shown(text)
        ))),
    }
}

/// `/api/fold`'s `limit`: how many of the lowest-fulfillment todos to answer with, capped.
///
/// Unlike the mail's [`limit`], absent means the whole fold, so no existing caller is quietly
/// truncated.
pub fn fold_limit(raw: Option<&str>) -> Result<Option<usize>, Refusal> {
    let Some(text) = raw.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    match text.parse::<usize>() {
        Ok(asked) if asked >= 1 && text.chars().all(|c| c.is_ascii_digit()) => {
            Ok(Some(asked.min(MAX_FOLD_ROWS)))
        }
        _ => Err(Refusal::bad_request(format!(
            "limit must be a positive integer, got {}",
            shown(text)
        ))),
    }
}

/// The most heads one `since=` may name; a replica with more posts first, which joins them.
pub const MAX_SINCE: usize = 256;

/// `/api/objects`'s `since`: a replica's heads, comma-separated, each 64 hex digits; absent or
/// empty is a replica holding nothing.
pub fn since(raw: Option<&str>) -> Result<Vec<prodrome_core::event::Hash>, Refusal> {
    let names: Vec<&str> = raw
        .unwrap_or_default()
        .split(',')
        .filter(|name| !name.is_empty())
        .collect();
    if names.len() > MAX_SINCE {
        return Err(Refusal::bad_request(format!(
            "since names at most {MAX_SINCE} heads, got {}",
            names.len()
        )));
    }
    names
        .into_iter()
        .map(|name| {
            prodrome_core::event::Hash::new(name).map_err(|_| {
                Refusal::bad_request(format!(
                    "since is heads of 64 hex digits, got {}",
                    shown(name)
                ))
            })
        })
        .collect()
}

/// The most hashes one `hashes=` may name: 64 names of 64 hex digits fit a URL of about 4 KiB.
pub const MOST_HASHES: usize = 64;

/// A content answer's `hashes`: one to [`MOST_HASHES`] object names, each 64 hex digits, in
/// ascending order with none twice, so one set of objects has one URL and a cache that kept its
/// answer answers every tab that asks for the same set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashes(Vec<prodrome_core::event::Hash>);

impl Hashes {
    /// The names `raw` spells, comma-separated.
    ///
    /// # Errors
    /// [`Refusal::bad_request`] for none, more than [`MOST_HASHES`], a name that is not 64 hex
    /// digits, and names out of ascending order or named twice.
    pub fn parse(raw: Option<&str>) -> Result<Self, Refusal> {
        let names: Vec<&str> = raw.unwrap_or_default().split(',').collect();
        if names.len() > MOST_HASHES {
            return Err(Refusal::bad_request(format!(
                "hashes names at most {MOST_HASHES} objects, got {}",
                names.len()
            )));
        }
        let hashes = names
            .into_iter()
            .map(|name| {
                prodrome_core::event::Hash::new(name).map_err(|_| {
                    Refusal::bad_request(format!(
                        "hashes is object names of 64 hex digits, comma-separated, got {}",
                        shown(name)
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if hashes.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Refusal::bad_request(
                "hashes names each object once, in ascending order",
            ));
        }
        Ok(Self(hashes))
    }

    /// The names, in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = &prodrome_core::event::Hash> {
        self.0.iter()
    }
}

/// `/api/tree`'s `by`: category when absent or empty, the list's default.
pub fn tree_by(raw: Option<&str>) -> Result<crate::wire::TreeBy, Refusal> {
    use crate::wire::TreeBy;
    match raw.unwrap_or_default() {
        "" | "category" => Ok(TreeBy::Category),
        "urgency" => Ok(TreeBy::Urgency),
        "flat" => Ok(TreeBy::Flat),
        other => Err(Refusal::bad_request(format!(
            "by must be category, urgency or flat, got {}",
            shown(other)
        ))),
    }
}

/// The widest window of turns one conversation request will draw.
pub const MAX_TURN_WINDOW: usize = 200;

/// A conversation page's paging cursor: an absolute turn index, or `None` for the newest
/// window.
pub fn turn_before(raw: Option<&str>) -> Result<Option<usize>, Refusal> {
    let Some(text) = raw.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    match text.parse::<usize>() {
        Ok(value) if text.chars().all(|c| c.is_ascii_digit()) => Ok(Some(value)),
        _ => Err(Refusal::bad_request(format!(
            "before must be a non-negative integer turn index, got {}",
            shown(text)
        ))),
    }
}

/// How many turns one window of a conversation carries, capped; `default` when absent.
pub fn turn_limit(raw: Option<&str>, default: usize) -> Result<usize, Refusal> {
    let Some(text) = raw.filter(|value| !value.is_empty()) else {
        return Ok(default);
    };
    match text.parse::<usize>() {
        Ok(asked) if asked >= 1 && text.chars().all(|c| c.is_ascii_digit()) => {
            Ok(asked.min(MAX_TURN_WINDOW))
        }
        _ => Err(Refusal::bad_request(format!(
            "limit must be a positive integer, got {}",
            shown(text)
        ))),
    }
}

/// `?about=<reference>` on a conversation window: only the rounds that touch one thing, or
/// `None` for the unfiltered window.
///
/// # Errors
/// [`Refusal::bad_request`] naming the text a [`Reference`] refused.
pub fn about(raw: Option<&str>) -> Result<Option<Reference>, Refusal> {
    let Some(text) = raw.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    text.parse::<Reference>()
        .map(Some)
        .map_err(|failed| Refusal::bad_request(format!("about: {failed}")))
}

/// A mail page's cursor: the instant the last page ended at, or `None` for the newest mail.
pub fn before(raw: Option<&str>) -> Result<Option<Datetime>, Refusal> {
    let Some(text) = raw.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let parsed = fpl::parse_iso(text).map_err(|_| {
        Refusal::bad_request(format!(
            "before must be an ISO datetime, got {}",
            shown(text)
        ))
    })?;
    fpl::datetime_of(parsed).map(Some).map_err(|_| {
        Refusal::bad_request(format!(
            "before must be an ISO datetime, got {}",
            shown(text)
        ))
    })
}

/// The notmuch query of `/api/mail/search`.
///
/// Passed through verbatim, so the only check is that it is a query and not a document.
pub fn search_query(raw: Option<&str>) -> Result<String, Refusal> {
    let text = raw.unwrap_or_default().trim();
    if text.is_empty() {
        return Err(Refusal::bad_request(
            "q is required: a notmuch query (plain words, from:, subject:, date: …)",
        ));
    }
    if text.chars().count() > MAX_QUERY_CHARS {
        return Err(Refusal::bad_request(format!(
            "q is capped at {MAX_QUERY_CHARS} characters, got {}",
            text.chars().count()
        )));
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32) -> Datetime {
        Datetime::new(2026, 9, day, 12, 0, 0, 0).expect("a real instant")
    }

    #[test]
    fn an_absent_or_empty_moment_is_the_default_and_a_bad_one_is_a_400() {
        assert_eq!(moment("t", None, at(7)), Ok(at(7)));
        assert_eq!(moment("t", Some(""), at(7)), Ok(at(7)));
        assert_eq!(
            moment("t", Some("2026-09-01T00:00:00"), at(7)),
            Ok(Datetime::new(2026, 9, 1, 0, 0, 0, 0).expect("a real instant"))
        );
        let refused = moment("t", Some("yesterday"), at(7)).unwrap_err();
        assert_eq!(refused.status().code(), 400);
        assert_eq!(
            refused.message(),
            "t must be an ISO datetime, got 'yesterday'"
        );
    }

    /// One set of objects has one spelling: one to the most, ascending, none twice.
    #[test]
    fn hashes_are_one_to_the_most_ascending_and_none_twice() {
        let names: Vec<String> = (0..=MOST_HASHES).map(|n| format!("{n:064x}")).collect();
        let parsed = Hashes::parse(Some(&names[..MOST_HASHES].join(","))).expect("the most");
        assert_eq!(parsed.iter().count(), MOST_HASHES);
        for refused in [
            None,
            Some(String::new()),
            Some(names.join(",")),
            Some(format!("{},{}", names[1], names[0])),
            Some(format!("{},{}", names[0], names[0])),
            Some(format!("{},", names[0])),
            Some("ABC".to_owned()),
        ] {
            let refusal = Hashes::parse(refused.as_deref()).expect_err("refused");
            assert_eq!(refusal.status().code(), 400, "{refused:?}");
        }
    }

    /// Capped, not obeyed.
    #[test]
    fn a_limit_is_capped_not_obeyed() {
        assert_eq!(limit(None), Ok(MAIL_LIMIT));
        assert_eq!(limit(Some("10")), Ok(10));
        assert_eq!(limit(Some("100000")), Ok(MAX_MAIL));
        for bad in ["0", "-1", "ten", "1.5"] {
            assert_eq!(limit(Some(bad)).unwrap_err().status().code(), 400, "{bad}");
        }
    }

    #[test]
    fn a_search_with_no_query_is_a_400_and_not_the_whole_corpus() {
        assert_eq!(
            search_query(None).unwrap_err().message(),
            "q is required: a notmuch query (plain words, from:, subject:, date: …)"
        );
        assert_eq!(search_query(Some("   ")).unwrap_err().status().code(), 400);
        assert_eq!(
            search_query(Some("  from:her  ")),
            Ok("from:her".to_owned())
        );
        let long = "x".repeat(MAX_QUERY_CHARS + 1);
        assert_eq!(
            search_query(Some(&long)).unwrap_err().message(),
            "q is capped at 500 characters, got 501"
        );
    }

    /// The fold's limit defaults to the whole fold, not a screenful.
    #[test]
    fn a_fold_limit_is_absent_by_default_and_capped_when_it_is_not() {
        assert_eq!(fold_limit(None), Ok(None));
        assert_eq!(fold_limit(Some("")), Ok(None));
        assert_eq!(fold_limit(Some("5")), Ok(Some(5)));
        assert_eq!(fold_limit(Some("100000")), Ok(Some(MAX_FOLD_ROWS)));
        for bad in ["0", "-1", "five", "1.5"] {
            assert_eq!(
                fold_limit(Some(bad)).unwrap_err().status().code(),
                400,
                "{bad}"
            );
        }
        assert_eq!(
            fold_limit(Some("five")).unwrap_err().message(),
            "limit must be a positive integer, got 'five'"
        );
    }

    #[test]
    fn a_turn_before_is_absent_or_a_plain_index() {
        assert_eq!(turn_before(None), Ok(None));
        assert_eq!(turn_before(Some("")), Ok(None));
        assert_eq!(turn_before(Some("0")), Ok(Some(0)));
        assert_eq!(turn_before(Some("42")), Ok(Some(42)));
        assert_eq!(
            turn_before(Some("-1")).unwrap_err().message(),
            "before must be a non-negative integer turn index, got '-1'"
        );
        assert_eq!(turn_before(Some("soon")).unwrap_err().status().code(), 400);
    }

    #[test]
    fn a_turn_limit_defaults_and_is_capped_not_obeyed() {
        assert_eq!(turn_limit(None, 60), Ok(60));
        assert_eq!(turn_limit(Some(""), 60), Ok(60));
        assert_eq!(turn_limit(Some("10"), 60), Ok(10));
        assert_eq!(turn_limit(Some("100000"), 60), Ok(MAX_TURN_WINDOW));
        for bad in ["0", "-1", "ten", "1.5"] {
            assert_eq!(
                turn_limit(Some(bad), 60).unwrap_err().status().code(),
                400,
                "{bad}"
            );
        }
    }

    #[test]
    fn about_is_absent_or_a_parsed_reference() {
        assert_eq!(about(None), Ok(None));
        assert_eq!(about(Some("")), Ok(None));
        assert_eq!(
            about(Some("todo:life/x")).expect("it parses"),
            Some("todo:life/x".parse().expect("a reference"))
        );
        assert_eq!(
            about(Some("not-a-reference")).unwrap_err().status().code(),
            400
        );
        assert!(
            about(Some("not-a-reference"))
                .unwrap_err()
                .message()
                .starts_with("about: ")
        );
    }

    #[test]
    fn a_cursor_is_absent_or_an_instant() {
        assert_eq!(before(None), Ok(None));
        assert_eq!(before(Some("")), Ok(None));
        assert_eq!(
            before(Some("2026-09-01T10:00:00")),
            Ok(Some(Datetime::new(2026, 9, 1, 10, 0, 0, 0).expect("real")))
        );
        assert_eq!(
            before(Some("soon")).unwrap_err().message(),
            "before must be an ISO datetime, got 'soon'"
        );
    }
}
