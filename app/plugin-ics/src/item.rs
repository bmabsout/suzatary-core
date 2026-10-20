//! What the plugin emits: one item per event occurrence, an EVENT of its declared schema
//! (`ics.schema`), printed in the objects' own literal grammar.
//!
//! The host parses these bytes at that declaration and refuses an item that does not print back
//! to the same bytes, so the field order and spellings here are the declaration's;
//! `tests/host.rs` holds the two to it. [`Item`] itself is also what the cursor keeps, as JSON
//! private to this plugin.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::literal;

/// The schema every item is in, by its content name: the hash of `ics.schema`'s text, which
/// `tests/host.rs` pins.
pub const SCHEMA: &str = "c97b894305a4977cde486b7d2db899c97a36e078e48de408373cb8a188a9a143";

/// The constructor every item is.
const LEARNED: &str = "Learned";

/// The most characters of a title, a place or a UID kept.
pub const MAX_TEXT: usize = 500;

/// The most characters of one attendee's name kept.
pub const MAX_NAME: usize = 200;

/// The most attendees kept.
pub const MAX_ATTENDEES: usize = 50;

/// One occurrence of one event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// The credential the feed was read through, which names the calendar, never its URL.
    pub calendar: String,
    /// The event's `UID`.
    pub uid: String,
    /// The occurrence's identity within the event: its original start, `YYYYMMDDTHHMMSSZ` or
    /// `YYYYMMDD` (what a `RECURRENCE-ID` names), so a moved instance is still itself.
    pub occurrence: String,
    /// `SUMMARY`, or `""`.
    pub title: String,
    /// When it is.
    pub when: When,
    /// `LOCATION`, or `""`.
    pub place: String,
    /// Each attendee's `CN`, in order; an attendee with no name is left out, never its address.
    pub attendees: Vec<String>,
    /// `STATUS`.
    pub status: Status,
}

/// When an occurrence is: a span of UTC instants, or of whole days.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum When {
    /// From `start` until `end`, both `YYYY-MM-DDTHH:MM:SSZ`.
    Timed {
        /// The start.
        start: String,
        /// The end, at or after the start.
        end: String,
    },
    /// From the day `start` until the day before `end`, both `YYYY-MM-DD`.
    AllDay {
        /// The first day.
        start: String,
        /// The day after the last.
        end: String,
    },
}

/// `STATUS`, `CONFIRMED` when the feed says nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// It is happening.
    Confirmed,
    /// It may happen.
    Tentative,
    /// It is not happening: cancelled in the feed, or gone from it.
    Cancelled,
}

impl When {
    /// A timed span.
    #[must_use]
    pub fn timed(start: NaiveDateTime, end: NaiveDateTime) -> Self {
        When::Timed {
            start: instant(start),
            end: instant(end.max(start)),
        }
    }

    /// A span of whole days, at least one.
    #[must_use]
    pub fn all_day(start: NaiveDate, end: NaiveDate) -> Self {
        let end = if end > start {
            end
        } else {
            start.succ_opt().unwrap_or(start)
        };
        When::AllDay {
            start: start.format("%Y-%m-%d").to_string(),
            end: end.format("%Y-%m-%d").to_string(),
        }
    }

    /// Its start and end as UTC instants, a day read as its midnight.
    #[must_use]
    pub fn span(&self) -> Option<(NaiveDateTime, NaiveDateTime)> {
        match self {
            When::Timed { start, end } => Some((
                NaiveDateTime::parse_from_str(start, "%Y-%m-%dT%H:%M:%SZ").ok()?,
                NaiveDateTime::parse_from_str(end, "%Y-%m-%dT%H:%M:%SZ").ok()?,
            )),
            When::AllDay { start, end } => Some((
                NaiveDate::parse_from_str(start, "%Y-%m-%d")
                    .ok()?
                    .and_time(NaiveTime::MIN),
                NaiveDate::parse_from_str(end, "%Y-%m-%d")
                    .ok()?
                    .and_time(NaiveTime::MIN),
            )),
        }
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn instant(at: NaiveDateTime) -> String {
    at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// The first `max` characters of `text`.
#[must_use]
pub fn clipped(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

impl When {
    /// Its one spelling in the store, an ISO 8601 interval: `start/end`.
    #[must_use]
    pub fn interval(&self) -> String {
        match self {
            When::Timed { start, end } | When::AllDay { start, end } => format!("{start}/{end}"),
        }
    }
}

impl Status {
    /// Its spelling, one of the declaration's alternatives.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Confirmed => "confirmed",
            Status::Tentative => "tentative",
            Status::Cancelled => "cancelled",
        }
    }
}

impl Item {
    /// The plugin that read it, by its name: the credential's namespace, `ics` of
    /// `PLUGIN_ICS_HOME`. The host lends only names in its own namespace, so this is the name
    /// the host knows it by, and the actor every item it emits says wrote it.
    #[must_use]
    pub fn writer(&self) -> String {
        self.calendar
            .strip_prefix("PLUGIN_")
            .and_then(|rest| rest.split_once('_'))
            .map_or_else(String::new, |(plugin, _)| plugin.to_ascii_lowercase())
    }

    /// The event the store learns it by, learned at `at`: its bytes, as the host reads them.
    #[must_use]
    pub fn event(&self, at: NaiveDateTime) -> Vec<u8> {
        literal::call(
            LEARNED,
            &[
                (
                    "occurrence",
                    literal::string(&format!(
                        "{} {} {}",
                        self.calendar, self.occurrence, self.uid
                    )),
                ),
                ("at", literal::instant(at)),
                ("actor", literal::string(&self.writer())),
                ("title", literal::string(&self.title)),
                ("when", literal::string(&self.when.interval())),
                ("place", literal::string(&self.place)),
                ("attendees", literal::strings(&self.attendees)),
                ("status", literal::string(self.status.as_str())),
            ],
        )
        .into_bytes()
    }

    /// The same occurrence, cancelled.
    #[must_use]
    pub fn cancelled(&self) -> Self {
        Self {
            status: Status::Cancelled,
            ..self.clone()
        }
    }

    /// Whether any of it falls in `[from, to)`.
    #[must_use]
    pub fn overlaps(&self, from: NaiveDateTime, to: NaiveDateTime) -> bool {
        self.when.span().is_some_and(|(start, end)| {
            start < to && (end > from || (end == start && start >= from))
        })
    }
}
