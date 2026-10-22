//! A held value, stage 1 of `docs/HELDVALUES-PLAN-2026-10-22.md`: an identity number in mail,
//! which the box keeps where it is and a model reads only as a [`Placeholder`].
//!
//! Every text a model reads goes through [`mask`], with the message it came from, or through
//! [`withheld`] when no message is known. A placeholder names the number's kind and its last
//! digits, never the number. A [`Handle`] in a placeholder names the message and the number's
//! place in it; the owner's trusted device asks the box for that one value on a tap.

use base64::Engine as _;
use serde_json::Value;

use crate::redact::{Kind, identity_numbers};

/// The text form's prefix.
pub const PREFIX: &str = "held:";

/// How many of a number's last digits a placeholder shows.
pub const HINT_DIGITS: usize = 4;

/// The longest Message-ID a handle names: RFC 5322's line limit.
pub const MAX_MESSAGE: usize = 998;

/// One held value's name: the message it is in, and its place among that message's values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle {
    message: String,
    ordinal: u16,
}

/// A text that is no [`Handle`]'s text form.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a held value's handle: {0}")]
pub struct NotAHandle(String);

impl Handle {
    /// The handle of the `ordinal`th value of `message`, a notmuch id.
    ///
    /// # Errors
    /// [`NotAHandle`] for an empty or overlong id.
    pub fn new(message: &str, ordinal: u16) -> Result<Self, NotAHandle> {
        if message.is_empty() || message.len() > MAX_MESSAGE {
            return Err(NotAHandle(format!(
                "a message id of {} bytes",
                message.len()
            )));
        }
        Ok(Self {
            message: message.to_owned(),
            ordinal,
        })
    }

    /// Read the text form [`Handle::print`] writes: `held:<message id in base64url>.<ordinal>`.
    ///
    /// # Errors
    /// [`NotAHandle`] for any other text.
    pub fn parse(text: &str) -> Result<Self, NotAHandle> {
        let refused = || NotAHandle(text.chars().take(64).collect());
        let rest = text.strip_prefix(PREFIX).ok_or_else(refused)?;
        let (message, ordinal) = rest.rsplit_once('.').ok_or_else(refused)?;
        let ordinal = ordinal.parse::<u16>().map_err(|_| refused())?;
        if ordinal.to_string() != rest[message.len() + 1..] {
            return Err(refused());
        }
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(message)
            .map_err(|_| refused())?;
        let message = String::from_utf8(bytes).map_err(|_| refused())?;
        Self::new(&message, ordinal).map_err(|_| refused())
    }

    /// The text form.
    #[must_use]
    pub fn print(&self) -> String {
        format!(
            "{PREFIX}{}.{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&self.message),
            self.ordinal
        )
    }

    /// The notmuch id of the message the value is in.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Its place among the message's [`Values`].
    #[must_use]
    pub fn ordinal(&self) -> u16 {
        self.ordinal
    }
}

/// Whether a placeholder can be revealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// On a tap, by this handle.
    Revealable(Handle),
    /// Never: the text came from nothing the box can read again (Junk read from the server, a
    /// calendar, a page on the web), or its message no longer reads as holding it.
    Withheld,
}

/// What a model reads in a number's place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder {
    kind: Kind,
    hint: String,
    reach: Reach,
}

impl Placeholder {
    /// The placeholder for `value`, a number of `kind`.
    #[must_use]
    pub fn of(kind: Kind, value: &str, reach: Reach) -> Self {
        let digits: Vec<char> = value.chars().filter(char::is_ascii_digit).collect();
        let hint = digits[digits.len().saturating_sub(HINT_DIGITS)..]
            .iter()
            .collect();
        Self { kind, hint, reach }
    }

    /// The text form: `[SSN ending 3456 · tap to show · held:…]`, or `[SSN ending 3456 ·
    /// withheld]`.
    #[must_use]
    pub fn print(&self) -> String {
        match &self.reach {
            Reach::Revealable(handle) => format!(
                "[{} ending {} · tap to show · {}]",
                self.kind.name(),
                self.hint,
                handle.print()
            ),
            Reach::Withheld => format!("[{} ending {} · withheld]", self.kind.name(), self.hint),
        }
    }
}

/// One message's identity numbers as written, each once, in the order they first appear.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Values(Vec<String>);

impl Values {
    /// The numbers in `texts`, read in order: one message's every decoded text.
    pub fn of<'a>(texts: impl IntoIterator<Item = &'a str>) -> Self {
        let mut values: Vec<String> = Vec::new();
        for text in texts {
            for found in identity_numbers(text) {
                let value = &text[found.start..found.end];
                if !values.iter().any(|held| same(held, value)) {
                    values.push(value.to_owned());
                }
            }
        }
        Self(values)
    }

    /// The value at `ordinal`.
    #[must_use]
    pub fn get(&self, ordinal: u16) -> Option<&str> {
        self.0.get(usize::from(ordinal)).map(String::as_str)
    }

    /// Where `value` is among them, by its digits.
    #[must_use]
    pub fn ordinal_of(&self, value: &str) -> Option<u16> {
        self.0
            .iter()
            .position(|held| same(held, value))
            .and_then(|at| u16::try_from(at).ok())
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Two spellings of one number: `000-12-3456` and `000 12 3456` are the same value.
fn same(a: &str, b: &str) -> bool {
    a.chars()
        .filter(char::is_ascii_digit)
        .eq(b.chars().filter(char::is_ascii_digit))
}

/// `text` with each identity number replaced by its placeholder, revealable when `values`, the
/// numbers of `message`, hold it.
#[must_use]
pub fn mask(text: &str, message: &str, values: &Values) -> String {
    replaced(text, |kind, value| {
        let reach = values
            .ordinal_of(value)
            .and_then(|ordinal| Handle::new(message, ordinal).ok())
            .map_or(Reach::Withheld, Reach::Revealable);
        Placeholder::of(kind, value, reach)
    })
}

/// `text` with each identity number replaced by a placeholder no tap reveals.
#[must_use]
pub fn withheld(text: &str) -> String {
    replaced(text, |kind, value| {
        Placeholder::of(kind, value, Reach::Withheld)
    })
}

fn replaced(text: &str, placeholder: impl Fn(Kind, &str) -> Placeholder) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for found in identity_numbers(text) {
        out.push_str(&text[at..found.start]);
        out.push_str(&placeholder(found.kind, &text[found.start..found.end]).print());
        at = found.end;
    }
    out.push_str(&text[at..]);
    out
}

/// Every string of `value`, object keys included, through [`withheld`]: the last pass over a
/// verb's answer before a model reads it, whatever the verb read.
#[must_use]
pub fn withheld_value(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(withheld(&text)),
        Value::Array(items) => Value::Array(items.into_iter().map(withheld_value).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (withheld(&key), withheld_value(value)))
                .collect(),
        ),
        other @ (Value::Null | Value::Bool(_) | Value::Number(_)) => other,
    }
}

/// A held value read back for the owner, which never prints in a log line or a `{:?}`.
#[derive(Clone, PartialEq, Eq)]
pub struct Shown(String);

impl Shown {
    /// The value, as the message writes it.
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    /// The value, for the one answer that carries it.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Debug for Shown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Shown(a held value)")
    }
}

/// What reading one handle found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revealed {
    /// The value.
    Value(Shown),
    /// The message is gone, or holds no value at that place any more.
    Gone,
}

/// Reading one held value back from the message that holds it, for the owner's tap alone. The
/// server's state holds it, and no record does, so no verb of the secretary or the agent can.
pub trait Reveal {
    /// The value `handle` names.
    ///
    /// # Errors
    /// The mail index's own failure.
    fn reveal(&self, handle: &Handle) -> Result<Revealed, crate::capability::MailError>;
}

/// A server that reads no mail: every handle is gone.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unread;

impl Reveal for Unread {
    fn reveal(&self, _: &Handle) -> Result<Revealed, crate::capability::MailError> {
        Ok(Revealed::Gone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Synthetic numbers no person holds: SSN area 000, ITIN area 912, an EIN of zeros.
    const SSN: &str = "000-12-3456";
    const ITIN: &str = "912-70-1234";

    #[test]
    fn a_placeholder_names_the_kind_and_last_digits_and_never_the_number() {
        let values = Values::of([format!("SSN {SSN}, ITIN {ITIN}").as_str()]);
        let masked = mask(
            &format!("mine is {SSN}; also 000 12 3456 and {ITIN}"),
            "form-1@example.test",
            &values,
        );
        let handle = |ordinal| {
            Handle::new("form-1@example.test", ordinal)
                .expect("a handle")
                .print()
        };
        assert_eq!(
            masked,
            format!(
                "mine is [SSN ending 3456 · tap to show · {}]; also [SSN ending 3456 · tap to \
                 show · {}] and [ITIN ending 1234 · tap to show · {}]",
                handle(0),
                handle(0),
                handle(1)
            )
        );
        assert!(!masked.contains(SSN) && !masked.contains(ITIN));
    }

    #[test]
    fn a_number_the_message_does_not_hold_is_withheld() {
        let masked = mask(&format!("SSN {SSN}"), "x@example.test", &Values::default());
        assert_eq!(masked, "SSN [SSN ending 3456 · withheld]");
        assert_eq!(
            withheld(&format!("ITIN {ITIN}")),
            "ITIN [ITIN ending 1234 · withheld]"
        );
    }

    #[test]
    fn text_with_no_number_is_unchanged() {
        let text = "Order 123456789 ships 2026-10-21; call 555 0100.";
        assert_eq!(mask(text, "x@example.test", &Values::of([text])), text);
        assert_eq!(withheld(text), text);
    }

    /// A placeholder reads as no number, so masking twice changes nothing.
    #[test]
    fn a_masked_text_masks_to_itself() {
        let values = Values::of([SSN]);
        let once = mask(&format!("SSN {SSN}"), "x@example.test", &values);
        assert_eq!(mask(&once, "x@example.test", &values), once);
        assert_eq!(withheld(&once), once);
    }

    #[test]
    fn every_string_of_an_answer_is_withheld() {
        let answer = serde_json::json!({
            "subject": format!("SSN {SSN}"),
            "rows": [{"body": format!("ITIN {ITIN}"), "n": 3}],
            format!("SSN {SSN}"): true,
        });
        let text = withheld_value(answer).to_string();
        assert!(!text.contains(SSN) && !text.contains(ITIN), "{text}");
        assert!(text.contains("[ITIN ending 1234 · withheld]"), "{text}");
    }

    #[test]
    fn a_handle_refuses_what_it_does_not_print() {
        for text in [
            "",
            "held:",
            "held:.0",
            "held:YQ",
            "held:YQ.x",
            "held:YQ.01",
            "held:YQ.70000",
            "mail:YQ.0",
            "held:!!.0",
        ] {
            assert!(Handle::parse(text).is_err(), "{text}");
        }
        assert!(Handle::new("", 0).is_err());
        assert!(Handle::new(&"a".repeat(MAX_MESSAGE + 1), 0).is_err());
    }

    proptest! {
        /// The text form prints and parses back to the same handle.
        #[test]
        fn a_handle_round_trips(message in "[ -~]{1,200}", ordinal in any::<u16>()) {
            let handle = Handle::new(&message, ordinal).expect("a handle");
            prop_assert_eq!(Handle::parse(&handle.print()), Ok(handle));
        }
    }
}
