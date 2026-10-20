//! The inbox and the conversations as replicas a tab pulls: what a replica lacks, by heads, and
//! the decorations only the box can give, by content.

mod common;

use std::sync::Arc;

use common::{FakeModel, Fakes};

fn ada() -> suzatary_core::capability::Login {
    common::login("ada@example.com")
}

/// A server where `v4` writes as `ada`, whose model proposes one email and then answers.
fn writing(tag: &str) -> String {
    let whois = common::FakeWhois::new().naming(common::v4(), ada());
    let model = FakeModel::saying([
        r#"{"say": "= Drafted\n\nHere is the reply.",
            "calls": [{"verb": "propose_email", "args": {
              "account": "her@example.invalid",
              "to": "counsel@example.test",
              "subject": "The deed",
              "body": "Sending the deed today.",
              "why": "Counsel asked for it.",
              "sources": "mail:a1b2c3d4e5f6"
            }}]}"#,
        r#"{"say": "Proposed."}"#,
    ]);
    let fakes = Fakes::new(tag).whois(whois).model(model);
    let deployment = common::deployment([ada()], [(ada(), common::actor("ada"))]);
    common::Server::new(Arc::new(fakes), deployment, Vec::new())
        .start()
        .base
}

/// A conversation with one round answered and one proposal standing.
fn answered(base: &str) -> String {
    let mut opened = common::act(
        &format!("{base}/api/secretary/conversations"),
        &serde_json::json!({"title": "The deed"}),
    );
    let id = common::body_json(&mut opened)["id"]
        .as_str()
        .expect("an id")
        .to_owned();
    let said = common::act(
        &format!("{base}/api/secretary/conversations/{id}/say"),
        &serde_json::json!({"say": "write to counsel"}),
    );
    assert_eq!(common::status(&said), 202);
    for _ in 0..200 {
        if common::reading(base, &id, &[])["state"] == "idle" {
            return id;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("the round never finished");
}

fn objects(base: &str, store: &str, since: &str) -> serde_json::Value {
    let mut answer = common::get(&format!("{base}/api/stores/{store}/objects?since={since}"));
    assert_eq!(common::status(&answer), 200);
    common::body_json(&mut answer)
}

/// A replica holding a store's heads lacks nothing, one holding nothing lacks everything, and the
/// objects come parents first.
#[test]
fn a_replica_pulls_what_it_lacks_by_heads() {
    let base = writing("stores-pull");
    let _ = answered(&base);
    for store in ["inbox", "lines"] {
        let all = objects(&base, store, "");
        let heads: Vec<&str> = all["heads"]
            .as_array()
            .expect("heads")
            .iter()
            .map(|head| head.as_str().expect("a name"))
            .collect();
        assert!(!heads.is_empty(), "{store} has heads");
        assert_eq!(all["untrusted"], serde_json::json!([]));
        let names: Vec<&str> = all["objects"]
            .as_array()
            .expect("objects")
            .iter()
            .map(|object| object["hash"].as_str().expect("a name"))
            .collect();
        assert!(heads.iter().all(|head| names.contains(head)), "{store}");
        let none = objects(&base, store, &heads.join(","));
        assert_eq!(
            none["objects"],
            serde_json::json!([]),
            "{store} at its heads"
        );
        assert_eq!(none["heads"], all["heads"]);
        assert_eq!(all["rest"], 0, "{store} fits one piece");
        assert_eq!(none["rest"], 0);
    }
}

/// The Inbox badge is counted on the box, so a tab shows it without pulling either store: nothing
/// waits before the secretary proposes, and its one proposal waits after.
#[test]
fn the_badge_is_counted_without_the_conversations() {
    let base = writing("stores-waiting");
    let count = || {
        let mut answer = common::get(&format!("{base}/api/waiting"));
        assert_eq!(common::status(&answer), 200);
        common::body_json(&mut answer)["count"].clone()
    };
    assert_eq!(count(), 0);
    let _ = answered(&base);
    assert_eq!(count(), 1);
}

/// A store a tab does not hold is no store.
#[test]
fn only_the_two_stores_are_pulled() {
    let base = writing("stores-unknown");
    assert_eq!(
        common::status(&common::get(&format!("{base}/api/stores/memory/objects"))),
        404
    );
}

/// A page from before the lines asks for the store they replaced, and is told to load the new
/// version rather than answered that nothing is there.
#[test]
fn a_page_asking_for_the_retired_store_is_told_to_reload() {
    let base = writing("stores-retired");
    assert_eq!(
        common::status(&common::get(&format!(
            "{base}/api/stores/conversations/objects"
        ))),
        426
    );
}

/// What only the box can give is asked by content: a stored answer's block by its sha, and a
/// proposal's drawing by its index.
#[test]
fn the_decorations_are_asked_by_content() {
    let base = writing("stores-dress");
    let id = answered(&base);
    let reading = common::reading(&base, &id, &[]);
    let block = reading["turns"]
        .as_array()
        .expect("turns")
        .iter()
        .filter(|turn| turn["who"] == "model")
        .flat_map(|turn| turn["blocks"].as_array().expect("a model's blocks").clone())
        .next()
        .expect("the answer has a block");
    let sha = suzatary_core::secretary::answer::sha(block["source"].as_str().expect("a source"));
    let mut rendered = common::get(&format!(
        "{base}/api/secretary/conversations/{id}/blocks/{sha}"
    ));
    assert_eq!(common::status(&rendered), 200);
    let html = common::body_json(&mut rendered)["html"].clone();
    assert!(html.is_string());
    assert_eq!(
        block["html"], html,
        "the tab dresses the block with the box's rendering"
    );

    let mut drawn = common::get(&format!(
        "{base}/api/secretary/conversations/{id}/drawn?indices=0,0"
    ));
    assert_eq!(common::status(&drawn), 200);
    assert_eq!(
        common::body_json(&mut drawn)["proposals"],
        serde_json::json!([reading["proposals"][0], reading["proposals"][0]]),
        "an email draws nothing against a chain, so its row is the replica's, once per index asked"
    );
    assert_eq!(
        common::status(&common::get(&format!(
            "{base}/api/secretary/conversations/{id}/drawn?indices=0,9"
        ))),
        404,
        "one index there is no proposal of refuses the read"
    );
    assert_eq!(
        common::status(&common::get(&format!(
            "{base}/api/secretary/conversations/{id}/drawn?indices=x"
        ))),
        400
    );
    for asked in ["", "?indices=", "?indices=0,,1"] {
        assert_eq!(
            common::status(&common::get(&format!(
                "{base}/api/secretary/conversations/{id}/drawn{asked}"
            ))),
            400,
            "{asked:?} names no proposal, or one that is not an index"
        );
    }
}
