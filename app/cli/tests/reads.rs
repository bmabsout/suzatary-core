//! What a reader sees: every test is a GET as this box's own node over the shared six-todo chain
//! unless its own header says otherwise; who may act is `writes.rs`'s and `objects.rs`'s.

mod common;

use common::Fakes;
use prodrome_core::event::{mk_completed, mk_created, seal_hash};
use suzatary_core::record::mk_authored;

fn base() -> String {
    let fakes = Fakes::new("reads");
    let deployment = common::deployment([], []);
    common::start(
        std::sync::Arc::new(fakes),
        deployment,
        common::loopback_is_this_box(),
    )
}

fn chain(base: &str) -> serde_json::Value {
    let mut response = common::get(&format!("{base}/api/chain"));
    assert_eq!(common::status(&response), 200);
    common::body_json(&mut response)
}

/// Every literal rehashes to its own name, so the wire carries the bytes the store holds, as a
/// browser checks before it believes anything about an object.
#[test]
fn every_literal_rehashes_to_its_own_name() {
    let body = chain(&base());
    let links = body["links"].as_array().expect("an array");
    assert!(!links.is_empty());
    for link in links {
        let literal = link["literal"].as_str().expect("a literal");
        let envelope: suzatary_core::record::Envelope = prodrome_core::event::parse_envelope(
            &suzatary_core::record::TodoVocabulary::default(),
            literal,
        )
        .expect("the literal parses back");
        assert_eq!(
            seal_hash(&envelope).as_str(),
            link["hash"].as_str().expect("a hash"),
        );
    }
}

/// Every object's parents come before it, in the linearisation's order, and `prev` is the first
/// of them.
#[test]
fn parents_come_before_the_object() {
    let body = chain(&base());
    let links = body["links"].as_array().expect("an array");
    let mut seen = std::collections::BTreeSet::new();
    for link in links {
        let parents = link["parents"].as_array().expect("parents");
        assert_eq!(
            link["prev"],
            parents.first().cloned().unwrap_or_else(|| "".into())
        );
        for parent in parents {
            assert!(seen.contains(parent.as_str().expect("a name")), "{link}");
        }
        seen.insert(link["hash"].as_str().expect("a hash").to_owned());
    }
}

/// The payload carries this deployment's `untrusted`, sorted, so the browser folds the same
/// objects under the same policy offline.
#[test]
fn the_payload_carries_the_deployments_trust_policy() {
    let body = chain(&base());
    assert_eq!(body["untrusted"], serde_json::json!(["triage"]));
}

/// `heads` is every head, sorted, and `verify` is empty on a healthy chain.
#[test]
fn heads_and_verify() {
    let body = chain(&base());
    assert_eq!(body["verify"], serde_json::json!([]));
    let heads = body["heads"].as_array().expect("a set");
    assert!(!heads.is_empty());
    assert!(
        heads
            .windows(2)
            .all(|pair| pair[0].as_str() < pair[1].as_str())
    );
}

fn fold(base: &str, params: &str) -> serde_json::Value {
    let mut response = common::get(&format!("{base}/api/fold{params}"));
    assert_eq!(common::status(&response), 200, "{params}");
    common::body_json(&mut response)
}

/// Entries carry state and price.
#[test]
fn entries_carry_state_and_price() {
    let base = base();
    let body = fold(&base, "");
    let entries = body["entries"].as_array().expect("entries");
    let by_todo = |id: &str| -> &serde_json::Value {
        entries
            .iter()
            .find(|e| e["todo"] == id)
            .unwrap_or_else(|| panic!("{id} is in the fold"))
    };
    assert_eq!(by_todo("renew-card")["state"], "completed");
    assert_eq!(by_todo("file-permit")["authored"]["category"], "work");
    assert!(by_todo("file-permit")["value"].is_number());
}

/// `t` defaults to now.
#[test]
fn t_defaults_to_now() {
    let base = base();
    let body = fold(&base, "");
    assert!(body["at"].as_str().is_some());
}

/// A malformed `t` is a 400 naming what an ISO datetime looks like.
#[test]
fn a_malformed_t_is_400() {
    let base = base();
    let mut response = common::get(&format!("{base}/api/fold?t=yesterday"));
    assert_eq!(common::status(&response), 400);
    assert!(
        common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("ISO datetime")
    );
}

/// `limit` answers the lowest fulfillments, ascending, the query the Android widget asks: rows
/// are ordered by value and not by id, a resolved todo is never among them, and the whole fold
/// is unchanged for a caller that names no limit.
#[test]
fn a_limited_fold_is_the_lowest_fulfillments_ascending_and_open() {
    let base = base();
    let whole = fold(&base, "");
    let all = whole["entries"].as_array().expect("entries");
    assert!(all.len() > 3, "the shared chain has six todos");
    let ids: Vec<&str> = all
        .iter()
        .filter_map(|entry| entry["todo"].as_str())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "an unlimited fold is still sorted by id");

    let body = fold(&base, "?limit=3");
    let entries = body["entries"].as_array().expect("entries");
    assert_eq!(entries.len(), 3);
    let values: Vec<f64> = entries
        .iter()
        .map(|entry| entry["value"].as_f64().expect("a priced entry"))
        .collect();
    assert!(
        values.windows(2).all(|pair| pair[0] <= pair[1]),
        "ascending in fulfillment — low means urgent: {values:?}"
    );
    for entry in entries {
        assert_eq!(entry["state"], "open", "what is owed, not what is settled");
    }
    // `renew-card` is completed on the shared chain, so no limit reaches it.
    let asked = fold(&base, &format!("?limit={}", all.len() + 10));
    for entry in asked["entries"].as_array().expect("entries") {
        assert_ne!(entry["todo"], "renew-card");
    }
}

/// A malformed `limit` is a 400 with the wording the mail's cap already uses.
#[test]
fn a_malformed_limit_is_400() {
    let base = base();
    let mut response = common::get(&format!("{base}/api/fold?limit=none"));
    assert_eq!(common::status(&response), 400);
    assert_eq!(
        common::body_json(&mut response)["error"],
        "limit must be a positive integer, got 'none'"
    );
}

/// An untrusted completion is claimed but not bound, on the wire: `ada` authors, and `triage`
/// (untrusted) claims a completion the trusted fold refuses to bind.
#[test]
fn an_untrusted_completion_is_claimed_but_not_bound() {
    let root = common::scratch("reads-untrusted-completion");
    let events = suzatary_core::record::EventStore::new(
        root.join("events"),
        prodrome_core::policy::Untrusted::of([common::actor("triage")]),
    );
    let born = common::shift(common::now(), -10);
    let made = common::shift(common::now(), -5);
    suzatary_core::store::append_to(
        &events,
        suzatary_core::record::GENESIS,
        mk_created("call-harrow", born, "ada", "", "").expect("valid"),
    )
    .expect("appends");
    suzatary_core::store::append_to(
        &events,
        suzatary_core::record::GENESIS,
        mk_authored(
            "call-harrow",
            made,
            "ada",
            "todo",
            made,
            suzatary_core::record::MarkupSource::escaped("Email Harrow"),
            prodrome_core::fpl::mk_flat(0.5).expect("valid"),
            Vec::new(),
            "",
            "",
            suzatary_core::record::MarkupSource::empty(),
            None,
            Vec::new(),
            Vec::new(),
            "",
        )
        .expect("valid"),
    )
    .expect("appends");
    suzatary_core::store::append_to(
        &events,
        suzatary_core::record::GENESIS,
        mk_completed("call-harrow", common::shift(made, 1), "triage", "").expect("valid"),
    )
    .expect("appends");
    let fakes = Fakes::with_store("reads-untrusted-completion", events);
    let deployment = common::deployment([], []);
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment,
        common::loopback_is_this_box(),
    );
    let body = fold(&base, "");
    let entry = body["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["todo"] == "call-harrow")
        .expect("call-harrow is in the fold");
    assert_eq!(entry["state"], "open");
    assert_eq!(entry["claimed"], "completed");
}

/// The stream is chronological, and every `Authored` carries its diff, over a chain with two
/// `Authored` records.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "builds a dedicated two-Authored chain and walks every field of \
              the resulting stream and diff; splitting it would scatter one \
              fixture across several functions for no reader's benefit"
)]
fn the_stream_is_chronological_and_every_authored_carries_its_diff() {
    let root = common::scratch("reads-diffs");
    let events = suzatary_core::record::EventStore::new(
        root.join("events"),
        prodrome_core::policy::Untrusted::of([common::actor("triage")]),
    );
    let born = common::shift(common::now(), -10);
    let first = common::shift(common::now(), -5);
    let second = common::shift(common::now(), -4);
    suzatary_core::store::append_to(
        &events,
        suzatary_core::record::GENESIS,
        mk_created("file-permit", born, "ada", "", "").expect("valid"),
    )
    .expect("appends");
    suzatary_core::store::append_to(
        &events,
        suzatary_core::record::GENESIS,
        mk_authored(
            "file-permit",
            first,
            "ada",
            "todo",
            first,
            suzatary_core::record::MarkupSource::escaped("File the deed"),
            prodrome_core::fpl::mk_flat(0.5).expect("valid"),
            Vec::new(),
            "",
            "",
            suzatary_core::record::MarkupSource::empty(),
            None,
            Vec::new(),
            Vec::new(),
            "",
        )
        .expect("valid"),
    )
    .expect("appends");
    suzatary_core::store::append_to(
        &events,
        suzatary_core::record::GENESIS,
        mk_authored(
            "file-permit",
            second,
            "ada",
            "todo",
            first,
            suzatary_core::record::MarkupSource::escaped("File the permit renewal"),
            prodrome_core::fpl::mk_flat(0.5).expect("valid"),
            Vec::new(),
            "work",
            "",
            suzatary_core::record::MarkupSource::empty(),
            None,
            Vec::new(),
            Vec::new(),
            "",
        )
        .expect("valid"),
    )
    .expect("appends");
    let fakes = Fakes::with_store("reads-diffs", events);
    let deployment = common::deployment([], []);
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment,
        common::loopback_is_this_box(),
    );
    let mut response = common::get(&format!("{base}/api/todo/file-permit"));
    assert_eq!(common::status(&response), 200);
    let body = common::body_json(&mut response);

    // The reading part arrives as the render of one page, "why" included.
    let reading = body["reading"].as_str().expect("the reading, rendered");
    assert!(reading.contains("\"todohead\""), "{reading}");
    assert!(reading.contains("\"whytree\""), "{reading}");

    let stream = body["stream"].as_array().expect("a stream");
    assert_eq!(
        stream
            .iter()
            .map(|l| l["kind"].as_str())
            .collect::<Vec<_>>(),
        vec![Some("Created"), Some("Authored"), Some("Authored")]
    );
    let ats: Vec<&str> = stream
        .iter()
        .map(|l| l["at"].as_str().unwrap_or_default())
        .collect();
    let mut sorted = ats.clone();
    sorted.sort_unstable();
    assert_eq!(ats, sorted);

    let authored_hashes: Vec<&str> = stream
        .iter()
        .filter(|l| l["kind"] == "Authored")
        .map(|l| l["hash"].as_str().unwrap_or_default())
        .collect();
    let diffs = body["diffs"].as_object().expect("diffs");
    assert_eq!(
        diffs
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        authored_hashes
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
    );
    let first_diff = diffs[authored_hashes[0]].as_array().expect("a diff");
    let fields: std::collections::BTreeSet<&str> = first_diff
        .iter()
        .map(|c| c["field"].as_str().unwrap_or_default())
        .collect();
    assert!(fields.contains("body"));
    assert!(!fields.contains("category"));
    let second_diff = diffs[authored_hashes[1]].as_array().expect("a diff");
    let second_fields: std::collections::BTreeSet<&str> = second_diff
        .iter()
        .map(|c| c["field"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(second_fields, ["body", "category"].into_iter().collect());
}

/// An unknown todo id is a 404, and a malformed one a 400.
#[test]
fn an_unknown_id_is_404_and_a_malformed_one_400() {
    let base = base();
    assert_eq!(
        common::status(&common::get(&format!("{base}/api/todo/no-such-todo"))),
        404
    );
    assert_eq!(
        common::status(&common::get(&format!("{base}/api/todo/Not%20An%20Id"))),
        400
    );
}

/// A large answer is gzipped when accepted, and an empty one is left alone.
///
/// The empty probe is axum's bodiless 405, since almost every real answer is over
/// `CompressionLayer`'s 32-byte gate; a path under `/api/act/` would answer a sentence, and be
/// compressed.
#[test]
fn a_large_answer_is_gzipped_and_an_empty_one_is_not() {
    let base = base();
    let big = common::agent()
        .get(format!("{base}/api/chain"))
        .header("accept-encoding", "gzip")
        .call()
        .expect("the request completes");
    assert_eq!(common::status(&big), 200);
    assert_eq!(
        big.headers()
            .get("content-encoding")
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );

    let empty = common::agent()
        .put(format!("{base}/api/objects"))
        .header("accept-encoding", "gzip")
        .send_empty()
        .expect("the request completes");
    assert_eq!(common::status(&empty), 405);
    assert!(empty.headers().get("content-encoding").is_none());
}
