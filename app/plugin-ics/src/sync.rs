//! One sync, as a function: the cursor, the calendars it may read, the time and what each feed
//! answered, to the items learned and the next cursor. The guest (`crate::guest`) only lends it
//! the world's imports, so every rule here is tested natively.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Days, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::event::{self, Window};
use crate::item::{Item, Status};

/// The window, in days before and after today. Seven back, so what a meeting left owed is still
/// in view the week after it; sixty ahead, so a deadline next month is priced against what else
/// that week holds. A day of slack each side, since the box's clock is local and has no zone.
pub const DAYS_BEFORE: u64 = 7;

/// See [`DAYS_BEFORE`].
pub const DAYS_AFTER: u64 = 60;

/// The cursor's version; another is refused, so a cursor this plugin cannot read is said.
const VERSION: u32 = 1;

/// What one feed's request came back as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    /// An answer, whatever its status.
    Answered {
        /// The HTTP status.
        status: u16,
        /// The body.
        body: Vec<u8>,
    },
    /// The host did not send it or did not hand its answer back, as the host said.
    Refused(String),
}

/// How much a line matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Progress.
    Info,
    /// Something the operator should read.
    Warn,
}

/// What a sync learned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Learned {
    /// Every occurrence new or changed since the cursor, and every one gone from its feed while
    /// still in the window, as cancelled.
    pub items: Vec<Item>,
    /// Where the next sync starts.
    pub cursor: Vec<u8>,
}

/// A cursor this plugin did not write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadCursor(pub String);

/// What the last sync held of each calendar: the occurrences in its window, as emitted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u32,
    calendars: BTreeMap<String, Vec<Item>>,
}

/// The window around `now`.
#[must_use]
pub fn window(now: NaiveDateTime) -> Window {
    let today = now.date();
    let midnight = |day: Option<chrono::NaiveDate>| day.unwrap_or(today).and_time(NaiveTime::MIN);
    Window {
        from: midnight(today.checked_sub_days(Days::new(DAYS_BEFORE + 1))),
        to: midnight(today.checked_add_days(Days::new(DAYS_AFTER + 1))),
    }
}

/// Read every calendar in `calendars` from `cursor`.
///
/// A feed that cannot be read (refused, not 200, not iCalendar) leaves its calendar as the
/// cursor had it and is said; the others still sync. An unchanged feed emits nothing. A calendar
/// the cursor held and `calendars` no longer names has its occurrences in the window learned
/// cancelled, once, and leaves the cursor.
///
/// # Errors
/// A cursor that does not parse as this plugin's.
///
/// # Panics
/// Never: a cursor is strings and lists, which JSON always prints.
pub fn sync(
    cursor: Option<&[u8]>,
    calendars: &[String],
    now: NaiveDateTime,
    fetch: &mut dyn FnMut(&str) -> Fetched,
    log: &mut dyn FnMut(Level, String),
) -> Result<Learned, BadCursor> {
    let mut held: Cursor = match cursor {
        None => Cursor {
            version: VERSION,
            calendars: BTreeMap::new(),
        },
        Some(bytes) => {
            serde_json::from_slice(bytes).map_err(|failed| BadCursor(failed.to_string()))?
        }
    };
    if held.version != VERSION {
        return Err(BadCursor(format!(
            "cursor version {}, this plugin writes {VERSION}",
            held.version
        )));
    }
    let window = window(now);
    let named: BTreeSet<&String> = calendars.iter().collect();
    let mut items = Vec::new();
    // A calendar no longer named was removed: what it held in the window is learned cancelled
    // before it goes, so nothing reading the store keeps a removed calendar's events.
    let removed: Vec<String> = held
        .calendars
        .keys()
        .filter(|name| !named.contains(name))
        .cloned()
        .collect();
    for name in removed {
        let gone = held.calendars.remove(&name).unwrap_or_default();
        let (cancelled, _) = changes(&gone, Vec::new(), window);
        log(
            Level::Info,
            format!("{name}: removed, {} occurrences cancelled", cancelled.len()),
        );
        items.extend(cancelled);
    }
    for name in named {
        let before = held.calendars.remove(name).unwrap_or_default();
        let read = read(name, fetch(name), window, &mut |line| {
            log(Level::Warn, format!("{name}: {line}"));
        });
        let now_held = match read {
            None => before,
            Some(current) => {
                let (learned, kept) = changes(&before, current, window);
                log(
                    Level::Info,
                    format!(
                        "{name}: {} occurrences in the window, {} learned",
                        kept.len(),
                        learned.len()
                    ),
                );
                items.extend(learned);
                kept
            }
        };
        held.calendars.insert(name.clone(), now_held);
    }
    Ok(Learned {
        items,
        cursor: serde_json::to_vec(&held).expect("a cursor is plain data"),
    })
}

/// The feed's occurrences in the window, or `None` (said) when it could not be read.
fn read(
    name: &str,
    fetched: Fetched,
    window: Window,
    said: &mut dyn FnMut(String),
) -> Option<Vec<Item>> {
    let body = match fetched {
        Fetched::Refused(why) => {
            said(format!("not fetched: {why}"));
            return None;
        }
        Fetched::Answered { status: 200, body } => body,
        Fetched::Answered { status, .. } => {
            said(format!("the feed answered {status}"));
            return None;
        }
    };
    let Ok(text) = String::from_utf8(body) else {
        said("the feed is not UTF-8".to_owned());
        return None;
    };
    match crate::content::parse(&text) {
        Err(malformed) => {
            said(format!("not iCalendar: {}", malformed.0));
            None
        }
        Ok(calendar) => Some(event::occurrences(&calendar, name, window, said)),
    }
}

/// What changed between the cursor's occurrences and the feed's: the items to emit, and what the
/// cursor now holds.
///
/// New and changed occurrences are emitted. One the cursor held that the feed no longer has is
/// emitted once as CANCELLED while it is still in the window (an event deleted, an instance
/// excluded), and held so, so the next sync emits it again only if it comes back.
fn changes(before: &[Item], current: Vec<Item>, window: Window) -> (Vec<Item>, Vec<Item>) {
    let key = |item: &Item| (item.uid.clone(), item.occurrence.clone());
    let was: BTreeMap<(String, String), &Item> =
        before.iter().map(|item| (key(item), item)).collect();
    let now: BTreeSet<(String, String)> = current.iter().map(key).collect();
    let mut learned: Vec<Item> = current
        .iter()
        .filter(|item| was.get(&key(item)) != Some(item))
        .cloned()
        .collect();
    let mut kept = current;
    for item in before {
        if now.contains(&key(item)) || !item.overlaps(window.from, window.to) {
            continue;
        }
        let gone = item.cancelled();
        if item.status != Status::Cancelled {
            learned.push(gone.clone());
        }
        kept.push(gone);
    }
    kept.sort_by_key(key);
    (learned, kept)
}
