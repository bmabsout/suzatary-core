//! US identity numbers never reach a transcript, and neither does a link's credential.
//!
//! [`Redacted`] is the only type a document read answers in, and [`redact`] is its only
//! constructor, so text that skipped the pass does not typecheck. [`Scrubbed`] is the same
//! promise for a stored conversation line: every string in it went through [`redact`] and the
//! link pass, and [`Scrubbed::of`] is its only constructor.
//!
//! Two shapes of identity number are replaced with [`REDACTED`]:
//!
//! - an SSN or ITIN written out, `ddd-dd-dddd` or `ddd dd dddd`, anywhere;
//! - nine digits in a row (single hyphens or spaces between them allowed, so
//!   an EIN's `dd-ddddddd` counts) within [`NEAR`] characters of a label:
//!   SSN, social security, TIN, ITIN or EIN.
//!
//! The link pass replaces, anywhere in the text:
//!
//! - the value of a query or fragment parameter whose name ends in one of
//!   [`CREDENTIAL_PARAMETERS`], percent-encoded separators included, since a
//!   sign-in link is often wrapped inside a tracking link's own parameter;
//! - any JWT, three or more base64url segments the first of which opens `eyJ`;
//! - the password of a `scheme://user:password@host` link.
//!
//! A document read keeps its links: a sign-in link is not an identity number, and a PDF is
//! withheld on what [`redact`] finds. A conversation line is stored and pushed, so it loses them;
//! the mail it quotes keeps the working link.
//!
//! Over-redaction is the safe failure: a number that looks like one of these
//! and is not costs the owner a look at the file; one that slips through costs
//! the number. A `zipcode=` parameter loses its value for the same reason.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// What a redacted number becomes.
pub const REDACTED: &str = "[redacted]";

/// How close, in characters, a nine-digit run must be to a label.
pub const NEAR: usize = 20;

/// Text with every identity number this module recognises replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redacted(String);

impl Redacted {
    /// The text, safe to show.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The text, owned.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

/// `ddd-dd-dddd` or `ddd dd dddd`, one separator kind throughout, not part
/// of a longer run of digits.
static WRITTEN_OUT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[0-9]{3}(?:-[0-9]{2}-|\x20[0-9]{2}\x20)[0-9]{4}\b").expect("a valid pattern")
});

/// Nine digits, single hyphens or spaces allowed between them, not part of
/// a longer run.
static NINE_DIGITS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[0-9](?:[- ]?[0-9]){8}\b").expect("a valid pattern"));

/// The labels that make a nearby nine-digit run an identity number.
static LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:ssn|social\s+security|itin|tin|ein)\b").expect("a valid pattern")
});

/// The endings, compared without case, of a parameter name that carries a credential: `token`
/// covers `access_token`, `id_token` and `X-Amz-Security-Token`, `code` an OAuth or reset code.
pub const CREDENTIAL_PARAMETERS: &[&str] = &[
    "token",
    "code",
    "key",
    "sig",
    "signature",
    "secret",
    "password",
    "passwd",
    "pwd",
    "auth",
    "session",
    "sessionid",
    "session_id",
    "sid",
    "nonce",
    "otp",
    "ticket",
    "jwt",
    "hash",
    "hmac",
    "credential",
    "assertion",
];

/// A credential parameter: a separator (`?`, `&`, `#`, `;`, or one percent-encoded), the name,
/// `=` (or `%3D`), then the value up to the next separator, quote, bracket or space.
static CREDENTIAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?i)((?:[?&#;]|%3f|%26|%23)[a-z0-9_.\-]*(?:{})(?:=|%3d))[^&#;\s"'`<>()\[\]\\]+"#,
        CREDENTIAL_PARAMETERS.join("|")
    ))
    .expect("a valid pattern")
});

/// A JWT, or the first three parts of a JWE: base64url segments joined by dots, the header a JSON
/// object, so it opens `eyJ`.
static JWT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\beyJ[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+").expect("a valid pattern")
});

/// The password of a link that carries one: `scheme://user:` kept, the password replaced.
static USERINFO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b([a-z][a-z0-9+.\-]*://[^\s/:@]+:)[^\s/@]+@").expect("a valid pattern")
});

/// Replace every link credential this module recognises in `text`.
pub(crate) fn unlink(text: &str) -> String {
    let text = USERINFO.replace_all(text, format!("${{1}}{REDACTED}@").as_str());
    let text = CREDENTIAL.replace_all(&text, format!("${{1}}{REDACTED}").as_str());
    JWT.replace_all(&text, REDACTED).into_owned()
}

/// A value every string of which, object keys included, went through [`redact`] and the link
/// pass: what a conversation line must be before a store takes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scrubbed<T>(T);

/// A value whose scrubbed strings no longer make one: a replacement landed in a field that
/// refuses it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the value cannot be stored with its credentials redacted: {0}")]
pub struct Unscrubbable(pub String);

impl<T: Serialize + DeserializeOwned> Scrubbed<T> {
    /// Scrub every string of `value`, through its own serde shape.
    ///
    /// # Errors
    /// [`Unscrubbable`] when the scrubbed strings do not parse back, which refuses the value
    /// rather than store it unscrubbed.
    pub fn of(value: &T) -> Result<Self, Unscrubbable> {
        let tree = serde_json::to_value(value).map_err(|e| Unscrubbable(e.to_string()))?;
        serde_json::from_value(scrubbed(tree))
            .map(Self)
            .map_err(|e| Unscrubbable(e.to_string()))
    }
}

impl<T> Scrubbed<T> {
    /// The value, safe to store.
    #[must_use]
    pub fn get(&self) -> &T {
        &self.0
    }

    /// The value, owned.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.0
    }
}

fn scrubbed(tree: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    let text = |text: &str| unlink(redact(text).as_str());
    match tree {
        Value::String(string) => Value::String(text(&string)),
        Value::Array(items) => Value::Array(items.into_iter().map(scrubbed).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (text(&key), scrubbed(value)))
                .collect(),
        ),
        other => other,
    }
}

/// Which identity number a span of text is, by the label beside it or, with none, by its shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A Social Security number.
    Ssn,
    /// An Individual Taxpayer Identification Number.
    Itin,
    /// A Taxpayer Identification Number of no narrower kind.
    Tin,
    /// An Employer Identification Number.
    Ein,
}

impl Kind {
    /// The kind as a person writes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Kind::Ssn => "SSN",
            Kind::Itin => "ITIN",
            Kind::Tin => "TIN",
            Kind::Ein => "EIN",
        }
    }

    fn of_label(label: &str) -> Self {
        let label = label.to_ascii_lowercase();
        if label == "itin" {
            Kind::Itin
        } else if label == "tin" {
            Kind::Tin
        } else if label == "ein" {
            Kind::Ein
        } else {
            Kind::Ssn
        }
    }
}

/// One identity number in a text: its byte range and its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    /// Where it starts.
    pub start: usize,
    /// Where it ends.
    pub end: usize,
    /// What it is.
    pub kind: Kind,
}

/// Every SSN, ITIN, TIN and EIN this module recognises in `text`, in order and never
/// overlapping: two matches that overlap are one, from the first one's start to the later end.
#[must_use]
pub fn identity_numbers(text: &str) -> Vec<Found> {
    let labels: Vec<(usize, usize, Kind)> = LABEL
        .find_iter(text)
        .map(|found| (found.start(), found.end(), Kind::of_label(found.as_str())))
        .collect();
    // The kind of the closest label within `NEAR` characters, if any.
    let labelled = |start: usize, end: usize| {
        labels
            .iter()
            .filter_map(|&(label_start, label_end, kind)| {
                let gap = if label_end <= start {
                    &text[label_end..start]
                } else if end <= label_start {
                    &text[end..label_start]
                } else {
                    ""
                };
                let gap = gap.chars().count();
                (gap <= NEAR).then_some((gap, kind))
            })
            .min_by_key(|&(gap, _)| gap)
            .map(|(_, kind)| kind)
    };
    let mut spans: Vec<Found> = WRITTEN_OUT
        .find_iter(text)
        .map(|found| Found {
            start: found.start(),
            end: found.end(),
            kind: labelled(found.start(), found.end()).unwrap_or(
                if found.as_str().starts_with('9') {
                    Kind::Itin
                } else {
                    Kind::Ssn
                },
            ),
        })
        .collect();
    spans.extend(NINE_DIGITS.find_iter(text).filter_map(|found| {
        labelled(found.start(), found.end()).map(|kind| Found {
            start: found.start(),
            end: found.end(),
            kind,
        })
    }));
    spans.sort_unstable_by_key(|found| (found.start, found.end));
    let mut merged: Vec<Found> = Vec::with_capacity(spans.len());
    for found in spans {
        match merged.last_mut() {
            Some(last) if found.start < last.end => last.end = last.end.max(found.end),
            Some(_) | None => merged.push(found),
        }
    }
    merged
}

/// Replace every SSN, ITIN, TIN and EIN this module recognises in `text`.
#[must_use]
pub fn redact(text: &str) -> Redacted {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for found in identity_numbers(text) {
        out.push_str(&text[at..found.start]);
        out.push_str(REDACTED);
        at = found.end;
    }
    out.push_str(&text[at..]);
    Redacted(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(text: &str) -> String {
        redact(text).into_string()
    }

    /// Each number found says what it is: by its label, the closest one, or by its shape.
    #[test]
    fn each_number_found_names_its_kind() {
        let kinds = |text: &str| -> Vec<(String, Kind)> {
            identity_numbers(text)
                .into_iter()
                .map(|found| (text[found.start..found.end].to_owned(), found.kind))
                .collect()
        };
        assert_eq!(
            kinds("000-12-3456"),
            vec![("000-12-3456".to_owned(), Kind::Ssn)]
        );
        assert_eq!(
            kinds("912-70-1234"),
            vec![("912-70-1234".to_owned(), Kind::Itin)]
        );
        assert_eq!(
            kinds("EIN: 00-0000001 and TIN 000000002"),
            vec![
                ("00-0000001".to_owned(), Kind::Ein),
                ("000000002".to_owned(), Kind::Tin)
            ]
        );
        assert_eq!(
            kinds("ITIN 000-12-3456"),
            vec![("000-12-3456".to_owned(), Kind::Itin)]
        );
        assert!(kinds("call 555 0100 on 2026-10-21").is_empty());
    }

    #[test]
    fn a_written_out_ssn_or_itin_is_redacted_anywhere() {
        assert_eq!(shown("123-45-6789"), "[redacted]");
        assert_eq!(
            shown("Number: 912 70 1234, issued 2019."),
            "Number: [redacted], issued 2019."
        );
        assert_eq!(
            shown("a 123-45-6789 and a 987-65-4321"),
            "a [redacted] and a [redacted]"
        );
    }

    #[test]
    fn a_nine_digit_run_is_redacted_next_to_a_label() {
        for (text, expected) in [
            ("SSN: 123456789", "SSN: [redacted]"),
            ("ssn 123456789.", "ssn [redacted]."),
            (
                "Social Security Number 123456789",
                "Social Security Number [redacted]",
            ),
            (
                "Social  security no. is 123456789",
                "Social  security no. is [redacted]",
            ),
            ("ITIN 912701234", "ITIN [redacted]"),
            ("TIN: 123456789", "TIN: [redacted]"),
            ("Employer EIN 12-3456789", "Employer EIN [redacted]"),
            ("EIN: 12 3456789", "EIN: [redacted]"),
            ("123456789 (SSN)", "[redacted] (SSN)"),
        ] {
            assert_eq!(shown(text), expected, "{text}");
        }
    }

    #[test]
    fn a_nine_digit_run_far_from_any_label_stays() {
        let far = format!("SSN{}123456789", " ".repeat(NEAR + 1));
        assert_eq!(shown(&far), far);
        let near = format!("SSN{}123456789", " ".repeat(NEAR));
        assert_eq!(shown(&near), format!("SSN{}[redacted]", " ".repeat(NEAR)));
        assert_eq!(shown("Order 123456789 shipped"), "Order 123456789 shipped");
    }

    /// Near-misses: shapes an official letter is full of that are not identity numbers.
    #[test]
    fn phone_numbers_dates_and_receipt_numbers_are_not_redacted() {
        for text in [
            "Call 617-555-1234 or (617) 555-1234",
            "+1 617 555 1234",
            "Dated 2026-09-25, due 09/30/2026",
            "Receipt IOE0912345678 and EAC2190012345",
            "Receipt number: IOE0912345678",
            "A-Number A123456789",
            "Case 1234567890 (ten digits)",
            "123-456-789",
            "12-345-6789",
            "1234-56-7890",
            "The tinted window, a Martin, being here",
            "SSN field left blank; ref 12345678",
            "ITIN: 1234567890",
        ] {
            assert_eq!(shown(text), text, "{text}");
        }
    }

    #[test]
    fn labels_match_whole_words_only() {
        assert_eq!(shown("destination 123456789"), "destination 123456789");
        assert_eq!(shown("Martin 123456789"), "Martin 123456789");
    }

    #[test]
    fn text_with_nothing_to_redact_is_untouched() {
        let letter = "Dear Ada,\n\nYour permit renewal was approved.\n";
        assert_eq!(redact(letter).as_str(), letter);
        assert_eq!(shown(""), "");
    }

    #[test]
    fn multibyte_text_around_a_number_survives() {
        assert_eq!(
            shown("Numéro SSN — 123456789 — fin"),
            "Numéro SSN — [redacted] — fin"
        );
    }

    fn unlinked(text: &str) -> String {
        unlink(text)
    }

    /// The sign-in links a scan of the live transcripts found, in the shapes they arrive.
    #[test]
    fn a_links_credential_parameters_are_redacted() {
        for (text, expected) in [
            (
                "Reset: https://example.test/reset?token=a1b2c3d4&lang=en",
                "Reset: https://example.test/reset?token=[redacted]&lang=en",
            ),
            (
                "https://example.test/login?uid=42&access_token=abc.def",
                "https://example.test/login?uid=42&access_token=[redacted]",
            ),
            (
                "https://example.test/cb#id_token=xyz&state=s1",
                "https://example.test/cb#id_token=[redacted]&state=s1",
            ),
            (
                "https://example.test/auth?oobCode=Zz-9_&mode=resetPassword",
                "https://example.test/auth?oobCode=[redacted]&mode=resetPassword",
            ),
            (
                "https://bucket.example.test/f.pdf?X-Amz-Credential=AK%2F20260930&X-Amz-Signature=9f8e",
                "https://bucket.example.test/f.pdf?X-Amz-Credential=[redacted]&X-Amz-Signature=[redacted]",
            ),
            (
                "https://example.test/a?api_key=k1;session=s2",
                "https://example.test/a?api_key=[redacted];session=[redacted]",
            ),
            (
                "(see https://example.test/v?code=123456).",
                "(see https://example.test/v?code=[redacted]).",
            ),
            (
                r#"{"url":"https://example.test/i?sig=abc","n":1}"#,
                r#"{"url":"https://example.test/i?sig=[redacted]","n":1}"#,
            ),
        ] {
            assert_eq!(unlinked(text), expected, "{text}");
        }
    }

    /// A tracking link wraps the sign-in link percent-encoded; its tail goes with the token, which
    /// is the safe side.
    #[test]
    fn a_percent_encoded_credential_is_redacted_inside_another_link() {
        assert_eq!(
            unlinked(
                "https://l.example.test/?u=https%3A%2F%2Fexample.test%2Flogin%3Ftoken%3Dsecret1%26next%3D%2F&e=1"
            ),
            "https://l.example.test/?u=https%3A%2F%2Fexample.test%2Flogin%3Ftoken%3D[redacted]&e=1"
        );
    }

    #[test]
    fn a_jwt_is_redacted_anywhere() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhZGEifQ.c2lnbmF0dXJl";
        assert_eq!(
            unlinked(&format!("Bearer {jwt} expires soon")),
            "Bearer [redacted] expires soon"
        );
        assert_eq!(
            unlinked(&format!("https://example.test/invite/{jwt}")),
            "https://example.test/invite/[redacted]"
        );
        assert_eq!(
            unlinked(&format!("https://example.test/v?t={jwt}&x=1")),
            "https://example.test/v?t=[redacted]&x=1"
        );
    }

    #[test]
    fn a_links_password_is_redacted() {
        assert_eq!(
            unlinked("fetch https://ada:hunter2@example.test/feed"),
            "fetch https://ada:[redacted]@example.test/feed"
        );
    }

    /// Links that carry nothing to redact, and words that look like a parameter but are not one.
    #[test]
    fn a_link_without_a_credential_is_untouched() {
        for text in [
            "https://example.test/permit?id=42&lang=en#section-2",
            "https://example.test/search?q=token+code+key",
            "https://example.test/a?tokenize=1&keyboard=us&signal=3",
            "The code is on the letter, and the key is under the mat.",
            "token=abc with no separator before it",
            "eyJ alone is not a token, nor is eyJabc",
            "mailto:ada@example.test",
            "p0123456789abcdef0123456789abcdef and 2026-09-30T09:00:00.123456",
        ] {
            assert_eq!(unlinked(text), text, "{text}");
        }
    }

    /// Every ending in the list is a credential, in any case, as a whole name or a suffix.
    #[test]
    fn every_listed_name_is_a_credential_whole_or_as_an_ending() {
        for name in CREDENTIAL_PARAMETERS {
            for spelled in [(*name).to_owned(), name.to_uppercase(), format!("x_{name}")] {
                assert_eq!(
                    unlinked(&format!("https://example.test/?a=1&{spelled}=v4lue&b=2")),
                    format!("https://example.test/?a=1&{spelled}=[redacted]&b=2"),
                    "{spelled}"
                );
            }
        }
    }

    /// Redacting twice is redacting once, so a line read back and stored again does not move.
    #[test]
    fn the_link_pass_is_idempotent() {
        let text = "https://ada:pw@example.test/r?token=a&code=b eyJa.eyJb.c";
        assert_eq!(unlinked(&unlinked(text)), unlinked(text));
    }

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Said {
        text: String,
        args: std::collections::BTreeMap<String, Vec<String>>,
        count: u32,
    }

    #[test]
    fn a_scrubbed_value_has_every_string_scrubbed_and_keys_too() {
        let said = Said {
            text: "SSN 123-45-6789, link https://example.test/?token=t".to_owned(),
            args: [(
                "https://example.test/?key=k".to_owned(),
                vec!["eyJh.eyJp.sig".to_owned(), "plain".to_owned()],
            )]
            .into(),
            count: 3,
        };
        let scrubbed = Scrubbed::of(&said).expect("scrubs").into_inner();
        assert_eq!(
            scrubbed,
            Said {
                text: "SSN [redacted], link https://example.test/?token=[redacted]".to_owned(),
                args: [(
                    "https://example.test/?key=[redacted]".to_owned(),
                    vec!["[redacted]".to_owned(), "plain".to_owned()],
                )]
                .into(),
                count: 3,
            }
        );
    }

    /// A field whose own constructor refuses the replacement refuses the value, never stores it.
    #[test]
    fn a_value_that_cannot_hold_its_redaction_is_refused() {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        #[serde(try_from = "String", into = "String")]
        struct Plain(String);
        impl TryFrom<String> for Plain {
            type Error = String;
            fn try_from(text: String) -> Result<Self, String> {
                if text.contains('[') {
                    Err("no brackets".to_owned())
                } else {
                    Ok(Self(text))
                }
            }
        }
        impl From<Plain> for String {
            fn from(plain: Plain) -> String {
                plain.0
            }
        }
        assert!(Scrubbed::of(&Plain("https://example.test/?token=t".to_owned())).is_err());
        assert!(Scrubbed::of(&Plain("nothing here".to_owned())).is_ok());
    }
}
