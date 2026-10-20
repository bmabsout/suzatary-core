//! A recorded decision over a real server: the secretary proposes one, nothing is recorded
//! until the owner's tap, and the tap records it as his, which `decisions` then reads.

mod common;

use std::sync::Arc;

use common::{FakeModel, Fakes};
use suzatary_core::decisions::{DecisionId, Decisions};

fn ada() -> suzatary_core::capability::Login {
    common::login("ada@example.com")
}

fn settled(base: &str, id: &str) -> serde_json::Value {
    for _ in 0..200 {
        let payload = common::reading(base, id, &[]);
        if payload["state"] == "idle" {
            return payload;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("the round never finished");
}

#[test]
fn a_proposed_decision_is_recorded_by_his_tap_alone() {
    let whois = common::FakeWhois::new().naming(common::v4(), ada());
    let fakes = Arc::new(
        Fakes::new("decision-tap")
            .whois(whois)
            .model(FakeModel::saying([
                r#"{"say": "", "calls": [{"verb": "propose_decision", "args": {
                "id": "plugins", "title": "Plugins hold only their imports",
                "body": "A plugin is a *component*.", "rationale": "Authority is its imports.",
                "why": "We settled it today.", "sources": "mail:a1b2c3d4e5f6"}}]}"#,
                r#"{"say": "I asked to record it."}"#,
            ])),
    );
    let deployment = common::deployment([ada()], [(ada(), common::actor("ada"))]);
    let base = common::Server::new(Arc::clone(&fakes), deployment, Vec::new())
        .start()
        .base;
    let decisions = Decisions {
        store: &fakes.decisions,
    };
    let plugins = DecisionId::new("plugins").expect("an id");

    let mut opened = common::act(
        &format!("{base}/api/secretary/conversations"),
        &serde_json::json!({"title": "Decisions"}),
    );
    let id = common::body_json(&mut opened)["id"]
        .as_str()
        .expect("an id")
        .to_owned();
    let say = common::act(
        &format!("{base}/api/secretary/conversations/{id}/say"),
        &serde_json::json!({"say": "record the plugins decision"}),
    );
    assert_eq!(common::status(&say), 202);
    let payload = settled(&base, &id);
    let card = &payload["proposals"][0];
    assert_eq!(card["act"]["kind"], "decision", "{payload}");
    assert_eq!(card["act"]["body"], "A plugin is a *component*.");
    assert_eq!(card["state"], "standing");
    assert!(
        decisions.ledger().expect("reads").is_empty(),
        "nothing is recorded before the tap"
    );

    let tapped = common::act(
        &format!("{base}/api/secretary/conversations/{id}/proposals/0/tap"),
        &serde_json::json!({}),
    );
    assert_eq!(common::status(&tapped), 200);
    let after = settled(&base, &id);
    assert_eq!(after["proposals"][0]["state"], "acted", "{after}");
    let ledger = decisions.ledger().expect("reads");
    let recorded = ledger.get(&plugins).expect("recorded");
    assert_eq!(
        recorded.statement().title(),
        "Plugins hold only their imports"
    );
    assert!(ledger.in_force(&plugins));
}
