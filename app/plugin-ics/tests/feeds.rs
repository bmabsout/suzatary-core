//! The plugin against feeds shaped like each provider's: recurrence, time zones, overrides,
//! cancellations, and the sync's own law that an unchanged feed emits nothing.

use std::fmt::Write as _;

use chrono::NaiveDateTime;
use proptest::prelude::*;
use suzatary_plugin_ics::item::{Item, Status, When};
use suzatary_plugin_ics::sync::{self, Fetched, Learned};

const GOOGLE: &str = include_str!("feeds/google.ics");
const MICROSOFT: &str = include_str!("feeds/microsoft.ics");
const PROTON: &str = include_str!("feeds/proton.ics");

fn at(text: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").expect("a time")
}

/// A Monday morning.
fn monday() -> NaiveDateTime {
    at("2026-10-05 10:00")
}

/// One sync of one calendar whose feed is `feed`, from `cursor`, at `now`.
fn sync_one(cursor: Option<&[u8]>, feed: &str, now: NaiveDateTime) -> Learned {
    let mut lines = Vec::new();
    sync::sync(
        cursor,
        &["PLUGIN_ICS_HOME".to_owned()],
        now,
        &mut |_| Fetched::Answered {
            status: 200,
            body: feed.as_bytes().to_vec(),
        },
        &mut |_, line| lines.push(line),
    )
    .expect("a sync")
}

fn of<'a>(items: &'a [Item], uid: &str) -> Vec<&'a Item> {
    items.iter().filter(|item| item.uid == uid).collect()
}

fn timed(item: &Item) -> (String, String) {
    match &item.when {
        When::Timed { start, end } => (start.clone(), end.clone()),
        When::AllDay { .. } => panic!("{} is all day", item.uid),
    }
}

#[test]
fn google_a_weekly_meeting_with_an_exclusion_a_move_and_a_cancellation() {
    let learned = sync_one(None, GOOGLE, monday());
    let standup = of(&learned.items, "standup-7f3a@google.example");
    let keys: Vec<&str> = standup
        .iter()
        .map(|item| item.occurrence.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            // 21 September ends before the window opens; 12 October is an EXDATE.
            "20260928T130000Z",
            "20261005T130000Z",
            "20261019T130000Z",
            "20261026T130000Z",
            // New York leaves daylight time on 1 November: nine o'clock is 14:00 UTC after it.
            "20261102T140000Z",
            "20261109T140000Z",
            "20261116T140000Z",
            "20261123T140000Z",
            "20261130T140000Z",
        ]
    );
    let first = standup[0];
    assert_eq!(
        timed(first),
        (
            "2026-09-28T13:00:00Z".to_owned(),
            "2026-09-28T13:30:00Z".to_owned()
        )
    );
    assert_eq!(first.title, "Weekly standup");
    assert_eq!(first.place, "Room 4, east wing");
    assert_eq!(
        first.attendees,
        ["Ada", "Solberg, Mira"],
        "names only, and only where there is one"
    );
    assert_eq!(first.status, Status::Confirmed);
    let moved = standup[2];
    assert_eq!(moved.title, "Weekly standup (moved to Tuesday)");
    assert_eq!(
        timed(moved).0,
        "2026-10-20T18:00:00Z",
        "the override's own time, under the instance it replaces"
    );
    assert_eq!(standup[3].status, Status::Cancelled);
    let retreat = of(&learned.items, "retreat-91c@google.example");
    assert_eq!(retreat.len(), 1);
    assert_eq!(
        retreat[0].when,
        When::AllDay {
            start: "2026-10-09".to_owned(),
            end: "2026-10-11".to_owned()
        }
    );
    assert_eq!(retreat[0].occurrence, "20261009");
    let dentist = of(&learned.items, "dentist-2b@google.example");
    assert_eq!(dentist[0].status, Status::Tentative);
    assert_eq!(timed(dentist[0]).0, "2026-10-07T16:30:00Z");
    assert!(
        learned
            .items
            .iter()
            .all(|item| item.calendar == "PLUGIN_ICS_HOME")
    );
}

#[test]
fn microsoft_a_windows_zone_name_read_through_its_own_definition() {
    let learned = sync_one(None, MICROSOFT, monday());
    let uid = "040000008200E00074C5B7101A82E00800000000A0F4C2B1D0C1DC01000000000000000010000000AAAAAAAAAAAAAAAAAAAAAAAAA";
    let lab = of(&learned.items, uid);
    assert_eq!(
        lab.len(),
        2,
        "the second Tuesdays of October and November: {lab:?}"
    );
    assert_eq!(
        timed(lab[0]),
        (
            "2026-10-13T17:00:00Z".to_owned(),
            "2026-10-13T18:30:00Z".to_owned()
        )
    );
    assert_eq!(
        timed(lab[1]).0,
        "2026-11-10T18:00:00Z",
        "Pacific standard time from 1 November"
    );
    assert_eq!(
        lab[0].title,
        "Lab meeting: quarterly review of the instruments"
    );
    assert_eq!(lab[0].place, "Building 7");
    let holiday = of(&learned.items, "040000008200E00074C5B7101A82E00800000000B1");
    assert_eq!(holiday[0].occurrence, "20261012");
}

#[test]
fn proton_a_count_that_crosses_a_change_of_offset() {
    let learned = sync_one(None, PROTON, monday());
    let yoga = of(&learned.items, "yoga-c1e0@proton.example");
    assert_eq!(yoga.len(), 10);
    assert_eq!(
        timed(yoga[0]),
        (
            "2026-10-20T05:30:00Z".to_owned(),
            "2026-10-20T06:30:00Z".to_owned()
        )
    );
    // Zurich leaves summer time on 25 October: half past seven is 06:30 UTC from then on.
    assert_eq!(timed(yoga[4]).0, "2026-10-24T05:30:00Z");
    assert_eq!(timed(yoga[5]).0, "2026-10-25T06:30:00Z");
    assert_eq!(timed(yoga[9]).0, "2026-10-29T06:30:00Z");
    let call = of(&learned.items, "call-88d@proton.example");
    assert_eq!(
        call[0].attendees,
        ["Ada"],
        "an organizer is not an attendee"
    );
}

#[test]
fn an_unchanged_feed_emits_nothing_the_second_time() {
    for feed in [GOOGLE, MICROSOFT, PROTON] {
        let first = sync_one(None, feed, monday());
        assert!(!first.items.is_empty());
        let second = sync_one(Some(&first.cursor), feed, monday());
        assert!(second.items.is_empty(), "{:?}", second.items);
        assert_eq!(second.cursor, first.cursor);
    }
}

#[test]
fn a_new_week_emits_only_what_entered_the_window() {
    let first = sync_one(None, GOOGLE, monday());
    let later = sync_one(Some(&first.cursor), GOOGLE, at("2026-10-12 10:00"));
    let keys: Vec<&str> = later
        .items
        .iter()
        .map(|item| item.occurrence.as_str())
        .collect();
    assert_eq!(
        keys,
        ["20261207T140000Z"],
        "28 September left the window silently"
    );
}

#[test]
fn an_event_gone_from_its_feed_is_learned_cancelled_once() {
    let first = sync_one(None, GOOGLE, monday());
    let without = GOOGLE.replace(
        "UID:dentist-2b@google.example",
        "UID:dentist-2b@google.example-renamed",
    );
    let second = sync_one(Some(&first.cursor), &without, monday());
    let gone: Vec<&Item> = of(&second.items, "dentist-2b@google.example");
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].status, Status::Cancelled);
    assert_eq!(
        gone[0].title, "Dentist",
        "the cancellation carries what it cancels"
    );
    assert_eq!(
        of(&second.items, "dentist-2b@google.example-renamed").len(),
        1
    );
    let third = sync_one(Some(&second.cursor), &without, monday());
    assert!(third.items.is_empty(), "{:?}", third.items);
    let back = sync_one(Some(&third.cursor), GOOGLE, monday());
    assert_eq!(
        of(&back.items, "dentist-2b@google.example")[0].status,
        Status::Tentative,
        "and it may come back"
    );
}

#[test]
fn an_excluded_instance_is_learned_cancelled() {
    let first = sync_one(None, GOOGLE, monday());
    let excluded = GOOGLE.replace(
        "EXDATE;TZID=America/New_York:20261012T090000",
        "EXDATE;TZID=America/New_York:20261012T090000,20261109T090000",
    );
    let second = sync_one(Some(&first.cursor), &excluded, monday());
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].occurrence, "20261109T140000Z");
    assert_eq!(second.items[0].status, Status::Cancelled);
}

#[test]
fn a_changed_title_is_learned_once() {
    let first = sync_one(None, PROTON, monday());
    let renamed = PROTON.replace(
        "SUMMARY:Call with the bank",
        "SUMMARY:Call with the bank (rescheduled)",
    );
    let second = sync_one(Some(&first.cursor), &renamed, monday());
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].title, "Call with the bank (rescheduled)");
}

#[test]
fn a_feed_that_cannot_be_read_leaves_its_calendar_as_it_was() {
    let first = sync_one(None, PROTON, monday());
    for fetched in [
        Fetched::Refused("CredentialUnset(\"PLUGIN_ICS_HOME\")".to_owned()),
        Fetched::Answered {
            status: 404,
            body: Vec::new(),
        },
        Fetched::Answered {
            status: 200,
            body: b"<html>sign in</html>".to_vec(),
        },
    ] {
        let mut said = Vec::new();
        let learned = sync::sync(
            Some(&first.cursor),
            &["PLUGIN_ICS_HOME".to_owned()],
            monday(),
            &mut |_| fetched.clone(),
            &mut |_, line| said.push(line),
        )
        .expect("a sync");
        assert!(
            learned.items.is_empty(),
            "nothing is withdrawn on a failed read"
        );
        assert_eq!(learned.cursor, first.cursor);
        assert!(
            said.iter()
                .any(|line| line.starts_with("PLUGIN_ICS_HOME: ")),
            "{said:?}"
        );
    }
}

/// A calendar removed has every occurrence of it in the window learned cancelled, once, so
/// nothing reading the store keeps its events; the calendars still named are untouched.
#[test]
fn a_removed_calendar_s_occurrences_are_learned_cancelled_once() {
    let both = ["PLUGIN_ICS_WORK".to_owned(), "PLUGIN_ICS_HOME".to_owned()];
    let feed = |name: &str| Fetched::Answered {
        status: 200,
        body: if name == "PLUGIN_ICS_WORK" {
            MICROSOFT
        } else {
            PROTON
        }
        .as_bytes()
        .to_vec(),
    };
    let first = sync::sync(
        None,
        &both,
        monday(),
        &mut |name| feed(name),
        &mut |_, _| {},
    )
    .expect("a sync");
    let held = first
        .items
        .iter()
        .filter(|item| item.calendar == "PLUGIN_ICS_WORK" && item.status != Status::Cancelled)
        .count();
    assert!(held > 0, "the work calendar held something");
    let home = ["PLUGIN_ICS_HOME".to_owned()];
    let second = sync::sync(
        Some(&first.cursor),
        &home,
        monday(),
        &mut |name| feed(name),
        &mut |_, _| {},
    )
    .expect("a sync");
    assert_eq!(second.items.len(), held, "{:?}", second.items);
    assert!(
        second
            .items
            .iter()
            .all(|item| item.calendar == "PLUGIN_ICS_WORK" && item.status == Status::Cancelled)
    );
    let third = sync::sync(
        Some(&second.cursor),
        &home,
        monday(),
        &mut |name| feed(name),
        &mut |_, _| {},
    )
    .expect("a sync");
    assert!(third.items.is_empty(), "{:?}", third.items);
}

#[test]
fn each_calendar_is_read_through_its_own_credential() {
    let mut asked = Vec::new();
    let learned = sync::sync(
        None,
        &["PLUGIN_ICS_WORK".to_owned(), "PLUGIN_ICS_HOME".to_owned()],
        monday(),
        &mut |name| {
            asked.push(name.to_owned());
            Fetched::Answered {
                status: 200,
                body: if name == "PLUGIN_ICS_WORK" {
                    MICROSOFT
                } else {
                    PROTON
                }
                .as_bytes()
                .to_vec(),
            }
        },
        &mut |_, _| {},
    )
    .expect("a sync");
    assert_eq!(asked, ["PLUGIN_ICS_HOME", "PLUGIN_ICS_WORK"]);
    assert!(
        learned
            .items
            .iter()
            .any(|item| item.calendar == "PLUGIN_ICS_WORK")
    );
    assert!(
        learned
            .items
            .iter()
            .any(|item| item.calendar == "PLUGIN_ICS_HOME")
    );
}

#[test]
fn a_cursor_this_plugin_did_not_write_is_refused() {
    let refused = sync::sync(
        Some(b"fence"),
        &[],
        monday(),
        &mut |_| unreachable!(),
        &mut |_, _| {},
    );
    assert!(refused.is_err());
}

/// A feed of `events` simple events, each daily or weekly from a day near `now`, some counted.
fn feed(events: &[(u8, u8, bool, u8)]) -> String {
    let mut out = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n");
    for (n, (day, hour, weekly, count)) in events.iter().enumerate() {
        let freq = if *weekly { "WEEKLY" } else { "DAILY" };
        let count = if *count == 0 {
            String::new()
        } else {
            format!(";COUNT={count}")
        };
        write!(
            out,
            "BEGIN:VEVENT\r\nUID:e{n}\r\nDTSTART;TZID=Europe/Paris:202609{:02}T{:02}0000\r\nDURATION:PT45M\r\nRRULE:FREQ={freq}{count}\r\nSUMMARY:Event {n}\r\nEND:VEVENT\r\n",
            1 + day % 28,
            hour % 24
        )
        .expect("a String takes any write");
    }
    out.push_str("END:VCALENDAR\r\n");
    out
}

proptest! {
    /// The law the timer rests on: a sync is a function of its cursor, the feed and the time,
    /// and a second sync of an unchanged feed learns nothing.
    #[test]
    fn syncing_twice_is_syncing_once(events in prop::collection::vec((0u8..28, 0u8..24, any::<bool>(), 0u8..30), 0..8), days in 0i64..90) {
        let feed = feed(&events);
        let now = monday() + chrono::Duration::days(days);
        let first = sync_one(None, &feed, now);
        prop_assert_eq!(&sync_one(None, &feed, now), &first);
        let second = sync_one(Some(&first.cursor), &feed, now);
        prop_assert!(second.items.is_empty());
        prop_assert_eq!(second.cursor, first.cursor);
    }
}
