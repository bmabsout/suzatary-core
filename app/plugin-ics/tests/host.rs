//! Every item the plugin emits is an event the host admits at the plugin's declared schema, and
//! one the calendar reads: the plugin's printer and the Prodrome's held to one spelling. The due
//! actor reads those events as the Prodrome does, and every act it proposes is one the host's
//! door admits.

use chrono::{Datelike, NaiveDateTime, Timelike};
use prodrome_core::declared::Declaration;
use prodrome_core::literal::{Datetime, Value, print_literal, print_str};
use suzatary_core::calendar::Occurrence;
use suzatary_core::plugin::{Manifest, PluginName, admit, propose};
use suzatary_plugin_ics::item::SCHEMA;
use suzatary_plugin_ics::sync::{self, Fetched};
use suzatary_plugin_ics::{due, literal, read};

fn manifest() -> Manifest {
    toml::from_str(
        r#"
        name = "ics"
        component = "ics.wasm"
        schema = "ics.schema"
        world = "source@0.3.0"
        origins = ["https://calendar.example.test"]
        credentials = [{ name = "PLUGIN_ICS_HOME", into = "url" }]
        "#,
    )
    .expect("a manifest")
}

fn schema() -> Declaration {
    Declaration::admit(include_str!("../ics.schema")).unwrap_or_else(|refused| panic!("{refused}"))
}

#[test]
fn the_calendars_schema_is_admitted_in_its_canonical_print_under_its_name() {
    let text = include_str!("../ics.schema");
    let schema = schema();
    assert_eq!(schema.text(), text);
    assert_eq!(schema.key(), "occurrence");
    assert_eq!(schema.name().as_str(), SCHEMA);
    assert!(Occurrence::reads(&schema));
}

#[test]
fn every_item_from_every_fixture_is_admitted_and_reads_as_an_occurrence() {
    let now = NaiveDateTime::parse_from_str("2026-10-05 10:00", "%Y-%m-%d %H:%M").expect("a time");
    let manifest = manifest();
    let schema = schema();
    let mut admitted = 0;
    for feed in [
        include_str!("feeds/google.ics"),
        include_str!("feeds/microsoft.ics"),
        include_str!("feeds/proton.ics"),
    ] {
        let learned = sync::sync(
            None,
            &["PLUGIN_ICS_HOME".to_owned()],
            now,
            &mut |_| Fetched::Answered {
                status: 200,
                body: feed.as_bytes().to_vec(),
            },
            &mut |_, _| {},
        )
        .expect("a sync");
        for item in learned.items {
            let bytes = item.event(now);
            let event = admit(&schema, &manifest, SCHEMA, &bytes)
                .unwrap_or_else(|refused| panic!("{refused}: {}", String::from_utf8_lossy(&bytes)));
            Occurrence::of(&event).unwrap_or_else(|refused| panic!("{refused}"));
            admitted += 1;
        }
    }
    assert!(admitted > 20, "{admitted}");
}

/// Every fixture's events, as the store holds them, and a `Due:` entry among them.
fn events(now: NaiveDateTime) -> Vec<String> {
    let mut events = Vec::new();
    for feed in [
        include_str!("feeds/google.ics"),
        include_str!("feeds/microsoft.ics"),
        include_str!("feeds/proton.ics"),
    ] {
        let learned = sync::sync(
            None,
            &["PLUGIN_ICS_HOME".to_owned()],
            now,
            &mut |_| Fetched::Answered {
                status: 200,
                body: feed.as_bytes().to_vec(),
            },
            &mut |_, _| {},
        )
        .expect("a sync");
        for item in learned.items {
            events.push(String::from_utf8(item.event(now)).expect("text"));
            let mut due = item.clone();
            due.uid = format!("due-{}", item.uid);
            due.title = format!("Due: {} #1 [draft] @home", item.title);
            events.push(String::from_utf8(due.event(now)).expect("text"));
        }
    }
    events
}

#[test]
fn the_due_actor_reads_every_event_as_the_prodrome_does() {
    let now = NaiveDateTime::parse_from_str("2026-10-05 10:00", "%Y-%m-%d %H:%M").expect("a time");
    let schema = schema();
    for text in events(now) {
        let event: prodrome_core::declared::Declared =
            prodrome_core::event::parse_event(&schema, &text).expect("an event");
        let ours = read::strings(&text).expect("read");
        let prodrome = prodrome_core::schema::Schema::to_value(&event);
        let call = prodrome.as_call().expect("a call");
        for (field, value) in &ours {
            assert_eq!(
                call.field(field).and_then(Value::as_str),
                Some(value.as_str()),
                "{field} of {text}"
            );
        }
        assert_eq!(ours.len(), 6, "every string field of {text}");
    }
}

#[test]
fn every_act_the_due_actor_proposes_is_admitted_by_the_hosts_door() {
    let now = NaiveDateTime::parse_from_str("2026-10-05 10:00", "%Y-%m-%d %H:%M").expect("a time");
    let events = events(now);
    let events: Vec<&str> = events.iter().map(String::as_str).collect();
    let proposed = due::decide(&events, now.date()).expect("decided");
    assert!(
        !proposed.is_empty(),
        "the fixtures hold an entry due from today"
    );
    let plugin = PluginName::parse("due").expect("a name");
    for one in proposed {
        propose(&plugin, &one.act, &one.why, &one.sources)
            .unwrap_or_else(|refused| panic!("{refused}: {}", one.act));
    }
}

proptest::proptest! {
    /// The plugin's string printer is the Prodrome's, whatever the text.
    #[test]
    fn a_string_prints_as_the_prodrome_prints_it(text in "\\PC*|.*") {
        proptest::prop_assert_eq!(literal::string(&text), print_str(&text));
    }

    /// Its instants and its tuples too.
    #[test]
    fn an_instant_and_a_tuple_print_as_the_prodrome_prints_them(
        seconds in 0i64..4_000_000_000,
        micros in proptest::prop_oneof![proptest::strategy::Just(0u32), 0u32..1_000_000],
        items in proptest::collection::vec("\\PC{0,8}", 0..4),
    ) {
        let at = chrono::DateTime::from_timestamp(seconds, micros * 1000).expect("an instant").naive_utc();
        let theirs = Datetime::new(
            at.year(), at.month(), at.day(), at.hour(), at.minute(), at.second(), micros,
        ).expect("an instant");
        proptest::prop_assert_eq!(literal::instant(at), print_literal(&Value::Datetime(theirs)));
        proptest::prop_assert_eq!(
            literal::strings(&items),
            print_literal(&Value::Tuple(items.iter().map(Value::str).collect()))
        );
    }
}
