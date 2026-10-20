//! The calendar's reference actor: an occurrence titled `Due: <thing>`, today or later and never
//! cancelled, asks for a todo for the thing, due that day.
//!
//! Meetings are never todos, and this proposes none: a `Due:` entry is the owner writing a
//! deadline on the calendar, and what is owed by it is the todo. It reads the store of
//! `ics.schema`, every calendar's occurrences as one, and returns its proposals as values; the
//! owner's tap is what adds the todo, through the Inbox's own act, and dismissing the card is the
//! answer no.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use chrono::NaiveDate;
use sha2::{Digest as _, Sha256};

use crate::literal;
use crate::read;

/// The title's prefix that marks a deadline, matched without case.
pub const PREFIX: &str = "due:";

/// The chain the todo is proposed on: the first, the owner's own.
pub const CHAIN: &str = "life";

/// One proposal, as the actor world returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposed {
    /// The Inbox's act, printed as it stores it.
    pub act: String,
    /// Why, as text.
    pub why: String,
    /// What it rests on.
    pub sources: Vec<String>,
}

/// What one occurrence's events say, joined: its title, its first day, and whether any said it
/// is cancelled.
#[derive(Debug, Default)]
struct Occurrence {
    titles: BTreeSet<String>,
    days: BTreeSet<NaiveDate>,
    cancelled: bool,
}

/// The proposals of `events`, the store's events as printed, on `today`: a function of the set
/// and of the day, so asking again on the same day asks for the same acts.
///
/// # Errors
/// An event that is not a call of the calendar's schema.
pub fn decide(events: &[&str], today: NaiveDate) -> Result<Vec<Proposed>, String> {
    let mut occurrences: BTreeMap<String, Occurrence> = BTreeMap::new();
    for event in events {
        let fields = read::strings(event)?;
        let field = |name: &str| {
            fields
                .get(name)
                .ok_or_else(|| format!("an event with no {name}"))
        };
        let seen = occurrences.entry(field("occurrence")?.clone()).or_default();
        seen.titles.insert(field("title")?.clone());
        seen.cancelled |= field("status")? == "cancelled";
        if let Some(day) = field("when")?
            .get(..10)
            .and_then(|day| NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        {
            seen.days.insert(day);
        }
    }
    Ok(occurrences
        .iter()
        .filter(|(_, seen)| !seen.cancelled)
        .filter_map(|(key, seen)| {
            // One title and one day, or the feed changed it and the owner reads it again.
            let (title, day) = (only(&seen.titles)?, *only(&seen.days)?);
            let thing = due(title)?;
            (day >= today).then(|| proposed(key, thing, day, today))
        })
        .collect())
}

/// The one member of `set`, or `None` for none or several.
fn only<T>(set: &BTreeSet<T>) -> Option<&T> {
    let mut members = set.iter();
    members.next().filter(|_| members.next().is_none())
}

/// The thing a `Due:` title names, or `None` for any other title.
fn due(title: &str) -> Option<&str> {
    let head = title.get(..PREFIX.len())?;
    let thing = title[PREFIX.len()..].trim();
    (head.eq_ignore_ascii_case(PREFIX) && !thing.is_empty()).then_some(thing)
}

fn proposed(occurrence: &str, thing: &str, day: NaiveDate, today: NaiveDate) -> Proposed {
    let todo = format!("due{}", &hex(&Sha256::digest(occurrence.as_bytes()))[..12]);
    let date = day.format("%Y-%m-%d").to_string();
    let add = literal::call(
        "Add",
        &[
            ("todo", literal::string(&todo)),
            ("body", literal::string(&markup(thing))),
            (
                "rationale",
                literal::string(
                    "A deadline the owner wrote on the calendar: work that matters by that day.",
                ),
            ),
            ("kind", literal::string("todo")),
            ("category", literal::string("")),
            (
                "priority",
                literal::call(
                    "DeadlinePrice",
                    &[
                        ("end_date", literal::string(&date)),
                        ("start_percent", "75".to_owned()),
                        ("end_percent", "45".to_owned()),
                        ("lead_up_days", "7".to_owned()),
                        ("start_date", "None".to_owned()),
                    ],
                ),
            ),
            ("detail", literal::string("")),
            ("waiting_on", literal::string("")),
        ],
    );
    Proposed {
        act: literal::call(
            "Changes",
            &[
                ("chain", literal::string(CHAIN)),
                ("acts", format!("({add},)")),
            ],
        ),
        why: format!("The calendar says \"Due: {thing}\" on {date}."),
        sources: vec![format!("briefing:{}", today.format("%Y-%m-%d"))],
    }
}

/// Text as typst markup that reads as the text: the Inbox admits a todo's body as markup, and a
/// title is data.
fn markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if ['\\', '[', ']', '#', '$', '@', '*', '_', '<', '>', '`'].contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn learned(occurrence: &str, title: &str, when: &str, status: &str) -> String {
        literal::call(
            "Learned",
            &[
                ("occurrence", literal::string(occurrence)),
                ("at", "datetime(2026, 10, 5, 9, 0, 0)".to_owned()),
                ("actor", literal::string("ics")),
                ("title", literal::string(title)),
                ("when", literal::string(when)),
                ("place", literal::string("")),
                ("attendees", "()".to_owned()),
                ("status", literal::string(status)),
            ],
        )
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 5).expect("a day")
    }

    fn decided(events: &[String]) -> Vec<Proposed> {
        let events: Vec<&str> = events.iter().map(String::as_str).collect();
        decide(&events, today()).expect("decided")
    }

    #[test]
    fn a_due_entry_today_or_later_asks_for_its_todo_due_that_day() {
        let [one] = decided(&[
            learned(
                "PLUGIN_ICS_HOME 20261020 a@x",
                "DUE: renew #passport",
                "2026-10-20/2026-10-21",
                "confirmed",
            ),
            learned(
                "PLUGIN_ICS_HOME 20261006T090000Z b@x",
                "Standup",
                "2026-10-06T09:00:00Z/2026-10-06T09:30:00Z",
                "confirmed",
            ),
        ])
        .try_into()
        .expect("one proposal");
        assert!(
            one.act
                .starts_with("Changes(chain='life', acts=(Add(todo='due")
                && one.act.contains("body='renew \\\\#passport'")
                && one.act.contains("end_date='2026-10-20'"),
            "{}",
            one.act
        );
        assert_eq!(
            one.why,
            "The calendar says \"Due: renew #passport\" on 2026-10-20."
        );
        assert_eq!(one.sources, ["briefing:2026-10-05"]);
    }

    #[test]
    fn a_past_cancelled_moved_or_bare_entry_asks_for_nothing() {
        let entry = |occurrence: &str, title: &str, day: &str, status: &str| {
            learned(occurrence, title, &format!("{day}/{day}"), status)
        };
        assert!(
            decided(&[
                entry("past", "Due: taxes", "2026-10-04", "confirmed"),
                entry("bare", "Due:  ", "2026-10-20", "confirmed"),
                entry("gone", "Due: rent", "2026-10-20", "confirmed"),
                entry("gone", "Due: rent", "2026-10-20", "cancelled"),
                entry("moved", "Due: visa", "2026-10-20", "confirmed"),
                entry("moved", "Due: visa", "2026-10-22", "confirmed"),
                entry("other", "Dinner: due soon", "2026-10-20", "confirmed"),
            ])
            .is_empty()
        );
    }

    #[test]
    fn its_proposals_are_a_function_of_the_set() {
        let events = [
            learned("a", "Due: one", "2026-10-07/2026-10-08", "confirmed"),
            learned("b", "Due: two", "2026-10-09/2026-10-10", "tentative"),
        ];
        let mut reversed = events.clone();
        reversed.reverse();
        let twice = [events.as_slice(), events.as_slice()].concat();
        assert_eq!(decided(&events), decided(&reversed));
        assert_eq!(decided(&events), decided(&twice));
        assert_eq!(decided(&events).len(), 2);
    }

    #[test]
    fn a_title_is_escaped_as_the_box_escapes_data() {
        let text = "a\\b[c]#d$e@f*g_h<i>j`k plain";
        assert_eq!(
            markup(text),
            suzatary_core::escape::escape_markup(text).as_str()
        );
    }
}
