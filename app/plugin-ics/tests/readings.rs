//! What the calendar reads of the fixture feeds, pinned in `readings.txt`: whatever carries an
//! occurrence from a feed into the store and back out, every day of the window reads the same.

use std::fmt::Write as _;

use chrono::{NaiveDate, NaiveDateTime};
use prodrome_core::declared::{Declaration, Declared};
use suzatary_core::calendar::{Calendar, When};
use suzatary_core::plugin::{Manifest, admit, learn};
use suzatary_core::store::{InMemory, Prodrome};
use suzatary_plugin_ics::item::SCHEMA;
use suzatary_plugin_ics::sync::{self, Fetched};

const FEEDS: [(&str, &str); 3] = [
    ("PLUGIN_ICS_GOOGLE", include_str!("feeds/google.ics")),
    ("PLUGIN_ICS_MICROSOFT", include_str!("feeds/microsoft.ics")),
    ("PLUGIN_ICS_PROTON", include_str!("feeds/proton.ics")),
];

fn manifest() -> Manifest {
    toml::from_str(
        r#"
        name = "ics"
        component = "ics.wasm"
        schema = "ics.schema"
        world = "source@0.3.0"
        credentials = [
            { name = "PLUGIN_ICS_GOOGLE", into = "url" },
            { name = "PLUGIN_ICS_MICROSOFT", into = "url" },
            { name = "PLUGIN_ICS_PROTON", into = "url" },
        ]
        "#,
    )
    .expect("a manifest")
}

/// Every day of the window, each occurrence on it with all it says, in UTC.
fn printed(read: &Calendar) -> String {
    let first = NaiveDate::from_ymd_opt(2026, 9, 27).expect("a day");
    let mut out = String::new();
    for day in read.days(first, 70, &|utc: NaiveDateTime| utc) {
        if day.entries.is_empty() {
            continue;
        }
        let _ = writeln!(out, "{}", day.date);
        for entry in &day.entries {
            let content = &entry.content;
            let _ = writeln!(
                out,
                "  {} | {} {} {} | {} | {} | {} | {} | {}{}",
                entry.time(),
                entry.id.calendar(),
                entry.id.occurrence(),
                entry.id.uid(),
                content.title(),
                content.place(),
                content.attendees().join("; "),
                content.status().as_str(),
                match content.when() {
                    When::Timed { .. } => "timed",
                    When::AllDay { .. } => "all_day",
                },
                if entry.conflicted {
                    " | conflicted"
                } else {
                    ""
                },
            );
        }
    }
    out
}

#[test]
fn every_reading_of_the_fixture_feeds_is_pinned() {
    let now = NaiveDateTime::parse_from_str("2026-10-05 10:00", "%Y-%m-%d %H:%M").expect("a time");
    let names: Vec<String> = FEEDS.iter().map(|(name, _)| (*name).to_owned()).collect();
    let learned = sync::sync(
        None,
        &names,
        now,
        &mut |name| Fetched::Answered {
            status: 200,
            body: FEEDS
                .iter()
                .find(|(feed, _)| *feed == name)
                .expect("a fixture")
                .1
                .as_bytes()
                .to_vec(),
        },
        &mut |_, _| {},
    )
    .expect("a sync");
    let manifest = manifest();
    let schema = Declaration::admit(include_str!("../ics.schema")).expect("lawful");
    let events = learned
        .items
        .iter()
        .map(|item| admit(&schema, &manifest, SCHEMA, &item.event(now)).expect("admitted"))
        .collect();
    let store = InMemory::<Declared>::at(schema.clone());
    learn(&store, events).expect("learns");
    let read = Calendar::of(&store.nodes().expect("reads"));
    assert_eq!(printed(&read), include_str!("readings.txt"));
}
