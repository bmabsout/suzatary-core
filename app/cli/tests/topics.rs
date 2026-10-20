//! Topics, their names and the search, over a real server with a scripted model: a topic
//! opened blank is named by the title job after its first answer, a rename is an append that
//! wins, the list carries each conversation's kind and last line, and the search finds a turn
//! in any of them.

mod common;

use std::sync::Arc;

use common::{FakeModel, Fakes};

fn ada() -> suzatary_core::capability::Login {
    common::login("ada@example.com")
}

fn listed(base: &str) -> Vec<serde_json::Value> {
    common::listing(base)["conversations"]
        .as_array()
        .expect("conversations")
        .clone()
}

fn row(base: &str, id: &str) -> serde_json::Value {
    listed(base)
        .into_iter()
        .find(|row| row["id"] == id)
        .expect("the conversation is listed")
}

/// Poll until `done` holds of the conversation's row — the title job runs
/// after the round, behind the 202 like the round itself.
fn until(base: &str, id: &str, done: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    for _ in 0..400 {
        let row = row(base, id);
        if done(&row) {
            return row;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!(
        "{id} never reached the state asked for: {:?}",
        row(base, id)
    );
}

#[test]
fn a_blank_topic_is_named_after_its_first_answer_and_renamed_by_an_append() {
    let whois = common::FakeWhois::new().naming(common::v4(), ada());
    let fakes = Arc::new(Fakes::new("topics").whois(whois).model(FakeModel::saying([
        r#"{"say": "The permit is due on the 3rd of October."}"#,
        "\"Permit deadline.\"",
    ])));
    let deployment = common::deployment([ada()], [(ada(), common::actor("ada"))]);
    let base = common::Server::new(Arc::clone(&fakes), deployment, Vec::new())
        .start()
        .base;

    let mut opened = common::act(
        &format!("{base}/api/secretary/conversations"),
        &serde_json::json!({"title": ""}),
    );
    assert_eq!(common::status(&opened), 200);
    let opened = common::body_json(&mut opened);
    let id = opened["id"].as_str().expect("an id").to_owned();
    assert_eq!(opened["title"], "New topic");
    let fresh = row(&base, &id);
    assert_eq!(fresh["kind"], "topic");
    assert_eq!(fresh["last"], "");
    assert_eq!(fresh["last_at"], fresh["opened"]);

    let said = common::act(
        &format!("{base}/api/secretary/conversations/{id}/say"),
        &serde_json::json!({"say": "When is the permit due?"}),
    );
    assert_eq!(common::status(&said), 202);
    let named = until(&base, &id, |row| row["title"] != "New topic");
    assert_eq!(named["title"], "Permit deadline");
    assert_eq!(named["last"], "The permit is due on the 3rd of October.");

    // The job was asked one question, with no verb in it.
    let asked = fakes.model.asked();
    let titling = asked.last().expect("the title job asked");
    assert_eq!(titling.turns.len(), 1);
    let suzatary_core::capability::Turn::Person { text, .. } = &titling.turns[0] else {
        panic!("the job's question is one person turn");
    };
    assert!(text.starts_with(suzatary_core::secretary::title::ASK));
    assert!(text.contains("When is the permit due?"));

    let mut renamed = common::act(
        &format!("{base}/api/secretary/conversations/{id}/title"),
        &serde_json::json!({"title": "  The October permit "}),
    );
    assert_eq!(common::status(&renamed), 200);
    assert_eq!(
        common::body_json(&mut renamed)["title"],
        "The October permit"
    );
    assert_eq!(row(&base, &id)["title"], "The October permit");

    let refused = common::act(
        &format!("{base}/api/secretary/conversations/{id}/title"),
        &serde_json::json!({"title": "two\nlines"}),
    );
    assert_eq!(common::status(&refused), 400);
    let absent = common::act(
        &format!("{base}/api/secretary/conversations/2026-01-01-9/title"),
        &serde_json::json!({"title": "Nothing"}),
    );
    assert_eq!(common::status(&absent), 404);

    // The search finds both sides of the exchange, newest first.
    let found = common::searching(&base, " permit ").expect("a search");
    assert_eq!(found["query"], "permit");
    let hits = found["hits"].as_array().expect("hits");
    assert_eq!(
        hits.iter()
            .map(|hit| (hit["turn"].as_u64(), hit["side"].as_str()))
            .collect::<Vec<_>>(),
        vec![(Some(1), Some("answer")), (Some(0), Some("person"))]
    );
    assert!(hits.iter().all(|hit| hit["conversation"] == id.as_str()));
    assert_eq!(hits[0]["title"], "The October permit");
    assert_eq!(hits[0]["kind"], "topic");
    assert_eq!(
        common::searching(&base, " ")
            .expect_err("a blank search is refused")
            .status(),
        suzatary_core::refusal::Status::BadRequest
    );
}

/// A topic opened with a name keeps it: the job is never asked, so a model
/// with one reply is enough.
#[test]
fn a_named_topic_is_never_titled_by_the_job() {
    let whois = common::FakeWhois::new().naming(common::v4(), ada());
    let fakes = Arc::new(
        Fakes::new("topics-named")
            .whois(whois)
            .model(FakeModel::saying([r#"{"say": "Done."}"#])),
    );
    let deployment = common::deployment([ada()], [(ada(), common::actor("ada"))]);
    let base = common::Server::new(Arc::clone(&fakes), deployment, Vec::new())
        .start()
        .base;
    let mut opened = common::act(
        &format!("{base}/api/secretary/conversations"),
        &serde_json::json!({"title": "Taxes"}),
    );
    let id = common::body_json(&mut opened)["id"]
        .as_str()
        .expect("an id")
        .to_owned();
    let said = common::act(
        &format!("{base}/api/secretary/conversations/{id}/say"),
        &serde_json::json!({"say": "File them."}),
    );
    assert_eq!(common::status(&said), 202);
    until(&base, &id, |row| row["last"] == "Done.");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(fakes.model.asked().len(), 1, "one round, and no title job");
    assert_eq!(row(&base, &id)["title"], "Taxes");
    let too_long = common::act(
        &format!("{base}/api/secretary/conversations"),
        &serde_json::json!({"title": "x".repeat(81)}),
    );
    assert_eq!(common::status(&too_long), 400);
}
