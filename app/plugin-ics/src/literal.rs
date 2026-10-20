//! The objects' literal grammar (the Prodrome's SPEC §2), as far as an item needs it: a string as
//! `CPython`'s `repr` prints it, an instant, a tuple of strings and a call. The host parses an item
//! by its declared schema and refuses one that does not print back to the same bytes, so this
//! printer must be the Prodrome's to the byte; `tests/host.rs` holds it to it over every fixture
//! and over generated text.
//!
//! The guest cannot link the Prodrome's own printer: that crate takes chrono's clock, which on
//! wasm32-unknown-unknown imports what the host refuses.

use std::fmt::Write as _;

use chrono::{Datelike, NaiveDateTime, Timelike};
use unicode_general_category::{GeneralCategory, get_general_category};

/// `CPython`'s `str.isprintable`: every category but Cc, Cf, Cs, Co, Cn, Zl, Zp and Zs, with SPACE
/// printable despite being Zs.
fn printable(c: char) -> bool {
    c == ' '
        || !matches!(
            get_general_category(c),
            GeneralCategory::Control
                | GeneralCategory::Format
                | GeneralCategory::Surrogate
                | GeneralCategory::PrivateUse
                | GeneralCategory::Unassigned
                | GeneralCategory::LineSeparator
                | GeneralCategory::ParagraphSeparator
                | GeneralCategory::SpaceSeparator
        )
}

/// `text` as `CPython`'s `repr(str)` prints it: single quotes unless it holds a `'` and no `"`.
#[must_use]
pub fn string(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            _ if c == quote => {
                out.push('\\');
                out.push(c);
            }
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            _ if (c as u32) < 0x7f || printable(c) => out.push(c),
            _ => {
                let point = c as u32;
                if point < 0x100 {
                    let _ = write!(out, "\\x{point:02x}");
                } else if point < 0x10000 {
                    let _ = write!(out, "\\u{point:04x}");
                } else {
                    let _ = write!(out, "\\U{point:08x}");
                }
            }
        }
    }
    out.push(quote);
    out
}

/// `at` as a `datetime(...)`, its microseconds only when there are some.
#[must_use]
pub fn instant(at: NaiveDateTime) -> String {
    let mut out = format!(
        "datetime({}, {}, {}, {}, {}, {}",
        at.year(),
        at.month(),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    );
    let micros = at.and_utc().timestamp_subsec_micros();
    if micros != 0 {
        let _ = write!(out, ", {micros}");
    }
    out.push(')');
    out
}

/// Strings as a tuple, a one-tuple with its comma.
#[must_use]
pub fn strings(items: &[String]) -> String {
    let printed: Vec<String> = items.iter().map(|item| string(item)).collect();
    match printed.as_slice() {
        [one] => format!("({one},)"),
        _ => format!("({})", printed.join(", ")),
    }
}

/// A call: `name(field=value, …)`, each value already printed, in the declared order.
#[must_use]
pub fn call(name: &str, fields: &[(&str, String)]) -> String {
    let fields: Vec<String> = fields
        .iter()
        .map(|(field, value)| format!("{field}={value}"))
        .collect();
    format!("{name}({})", fields.join(", "))
}
