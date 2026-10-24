//! The proposal routes over a real server, chain and replica: a run lands in the replica, the
//! Inbox renders it, accept and reject move it, and the gate stands in front.

mod common;

use std::sync::Arc;

use common::Fakes;
use prodrome_core::event::mk_completed;
use suzatary_core::store::Prodrome as _;

fn ada() -> suzatary_core::capability::Login {
    common::login("ada@example.com")
}

fn guest() -> suzatary_core::capability::Login {
    common::login("guest@example.com")
}

/// A server where `v4` writes as `ada` and `guest@example.com` may only read.
fn served(tag: &str) -> (String, Arc<Fakes>) {
    let whois = common::FakeWhois::new()
        .naming(common::v4(), ada())
        .naming(common::v6(), guest());
    let caps = Arc::new(Fakes::new(tag).whois(whois));
    let deployment = common::deployment([ada(), guest()], [(ada(), common::actor("ada"))]);
    let base = common::start(Arc::clone(&caps), deployment, Vec::new());
    (base, caps)
}

/// One triage run through `Proposing`, the interpreter a run actually uses: a todo, and its
/// completion, which rests on it.
fn a_run(caps: &Fakes) -> Vec<prodrome_core::event::Hash> {
    let run = caps.agent_run();
    let first = run.append(a_new_todo()).expect("appends");
    let second = run
        .append(mk_completed("newthing", common::now(), "triage", "renewed").expect("valid"))
        .expect("appends");
    vec![first, second]
}

/// The changes of `picked` that `names` lists, in its order, after checking it lists one more
/// object per change: the agent's signature beside it.
fn with_signatures(names: &serde_json::Value, picked: &[String]) -> Vec<String> {
    let names: Vec<&str> = names
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert_eq!(names.len(), 2 * picked.len(), "{names:?}");
    names
        .into_iter()
        .filter(|name| picked.iter().any(|one| one == name))
        .map(str::to_owned)
        .collect()
}

/// The `Authored` a `todo_add` builds, through `authoring::build_todo`.
fn a_new_todo() -> suzatary_core::record::TodoEvent {
    suzatary_core::authoring::build_todo(
        &suzatary_core::authoring::NewTodo {
            todo: "newthing".to_owned(),
            fields: suzatary_core::todo::TodoFields {
                body: "Renew the parking permit".to_owned(),
                rationale: "an errand with a recoverable cost".to_owned(),
                kind: "todo".to_owned(),
                category: "home".to_owned(),
                price: suzatary_core::todo::Price::Flat { percent: 75 },
                detail: String::new(),
                waiting_on: String::new(),
            },
            source: None,
            subtodos: Vec::new(),
        },
        suzatary_core::zone::Moment {
            at: common::now(),
            zone: suzatary_core::zone::Zone::UTC,
        },
        "triage",
        &["work", "home", "other", "study"],
        &suzatary_io::typeset::InProcess,
    )
    .expect("a valid todo")
}

fn batches(base: &str) -> serde_json::Value {
    let mut response = common::get(&format!("{base}/api/proposals"));
    assert_eq!(common::status(&response), 200);
    common::body_json(&mut response)
}

/// A run writes to the replica and the chain does not move.
#[test]
fn a_triage_run_leaves_a_batch_and_moves_no_chain() {
    let (base, caps) = served("proposals-batch");
    let before = caps.chain.tips().expect("derives");
    let names = a_run(&caps);

    assert_eq!(
        caps.chain.tips().expect("derives"),
        before,
        "the chain must not have moved"
    );
    let listed = batches(&base);
    assert_eq!(listed["chain"], "life");
    let batch = &listed["batches"][0];
    assert_eq!(batch["head"], names[1].as_str());
    assert_eq!(batch["actor"], "triage");
    let objects = batch["objects"].as_array().expect("two objects");
    assert_eq!(objects.len(), 2);
    assert!(listed["batches"][1].is_null(), "one run, one batch");
}

/// Accepting adopts the objects onto the chain and into the fold, and the batch leaves.
#[test]
fn accepting_a_batch_moves_it_onto_the_chain_and_empties_the_inbox() {
    let (base, caps) = served("proposals-accept");
    let names = a_run(&caps);
    let picked: Vec<String> = names.iter().map(|name| name.as_str().to_owned()).collect();

    let mut response = common::act(
        &format!("{base}/api/proposals/accept"),
        &serde_json::json!({ "objects": picked }),
    );
    assert_eq!(common::status(&response), 200);
    let receipt = common::body_json(&mut response);
    assert_eq!(receipt["chain"], "life");
    assert_eq!(
        with_signatures(&receipt["adopted"], &picked),
        picked,
        "the batch, each change with the agent's signature beside it"
    );
    let held: Vec<_> = caps
        .chain
        .objects()
        .expect("reads")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(
        names.iter().all(|name| held.contains(name)),
        "the chain holds the batch now"
    );
    assert!(
        batches(&base)["batches"]
            .as_array()
            .expect("a list")
            .is_empty(),
        "an accepted batch is not standing any more"
    );
    let mut fold = common::get(&format!("{base}/api/fold"));
    let entries = common::body_json(&mut fold)["entries"].clone();
    let entries = entries.as_array().expect("a list");
    let added = entries
        .iter()
        .find(|entry| entry["todo"] == "newthing")
        .expect("the todo the batch authored");
    assert_eq!(added["authored"]["body"], "Renew the parking permit");

    // The fixture still names `triage` untrusted, so an accepted completion stays a claim.
    assert_eq!(added["claimed"], "completed");
    assert_eq!(added["state"], "open");
}

/// A decision naming `object`, authored as a real review would; answers its todo id.
fn hold(caps: &Fakes, object: &str) -> String {
    let known: std::collections::BTreeSet<String> =
        suzatary_core::view::events_of(&caps.chain.nodes().expect("reads"))
            .iter()
            .map(|event| event.todo().as_str().to_owned())
            .collect();
    let new = suzatary_core::authoring::NewDecision {
        body: format!("Settle the triage batch {object} on life"),
        why: "This one completes a todo you wrote yourself.".to_owned(),
        performs: suzatary_core::reference::Reference::Batch {
            chain: suzatary_core::chains::ChainName::new("life").expect("a chain name"),
            head: suzatary_core::reference::BatchHead::new(object.to_owned())
                .expect("a batch head"),
        },
        detail: "This one completes a todo you wrote yourself.".to_owned(),
        priority: None,
        category: String::new(),
        source: String::new(),
    };
    let event = suzatary_core::authoring::build_decision(
        &new,
        suzatary_core::zone::Moment {
            at: common::now(),
            zone: suzatary_core::zone::Zone::UTC,
        },
        "assistant",
        &[],
        &known,
    )
    .expect("a valid decision");
    let id = event.todo().as_str().to_owned();
    caps.chain.append(event).expect("authors the decision");
    id
}

/// What the fold says of one todo.
fn state_of(base: &str, todo: &str) -> serde_json::Value {
    let mut fold = common::get(&format!("{base}/api/fold"));
    let entries = common::body_json(&mut fold)["entries"].clone();
    entries
        .as_array()
        .expect("a list")
        .iter()
        .find(|entry| entry["todo"] == todo)
        .expect("the decision")["state"]
        .clone()
}

/// Accepting a held batch closes its decision record as the gate's actor.
#[test]
fn accepting_a_held_batch_through_the_route_settles_its_own_decision() {
    let (base, caps) = served("proposals-accept-held");
    let names = a_run(&caps);
    let decision = hold(&caps, names[1].as_str());

    let picked: Vec<String> = names.iter().map(|name| name.as_str().to_owned()).collect();
    let response = common::act(
        &format!("{base}/api/proposals/accept"),
        &serde_json::json!({ "objects": picked }),
    );
    assert_eq!(common::status(&response), 200);
    assert_eq!(
        state_of(&base, &decision),
        "completed",
        "accepting through the route settles the decision it adopted, not just the objects"
    );
}

/// A decision settles when the object it names leaves, even after the batch is renamed.
#[test]
fn a_decision_settles_when_the_object_it_names_leaves_not_with_the_batch_head() {
    let (base, caps) = served("proposals-accept-grown");
    let names = a_run(&caps);
    let first = hold(&caps, names[0].as_str());
    let second = hold(&caps, names[1].as_str());

    let response = common::act(
        &format!("{base}/api/proposals/accept"),
        &serde_json::json!({ "objects": [names[0].as_str()] }),
    );
    assert_eq!(common::status(&response), 200);
    assert_eq!(state_of(&base, &first), "completed");
    assert_eq!(
        state_of(&base, &second),
        "open",
        "its object still waits, so the decision on it does too"
    );

    let response = common::act(
        &format!("{base}/api/proposals/accept"),
        &serde_json::json!({ "objects": [names[1].as_str()] }),
    );
    assert_eq!(common::status(&response), 200);
    assert_eq!(state_of(&base, &second), "completed");
}

/// Accepting twice adopts nothing the second time.
#[test]
fn accepting_the_same_batch_twice_takes_nothing_the_second_time() {
    let (base, caps) = served("proposals-twice");
    let names = a_run(&caps);
    let picked: Vec<String> = names.iter().map(|name| name.as_str().to_owned()).collect();
    let body = serde_json::json!({ "objects": picked });

    let _ = common::act(&format!("{base}/api/proposals/accept"), &body);
    let tip = caps.chain.tips().expect("derives");
    let mut response = common::act(&format!("{base}/api/proposals/accept"), &body);
    assert_eq!(common::status(&response), 200);
    let receipt = common::body_json(&mut response);
    assert!(receipt["adopted"].as_array().expect("a list").is_empty());
    assert_eq!(receipt["already"], serde_json::json!(picked));
    assert_eq!(
        caps.chain.tips().expect("derives"),
        tip,
        "the hash is the name"
    );
}

/// An ancestor-open pick is refused and nothing moves.
#[test]
fn an_ancestor_open_pick_is_refused_and_the_chain_stands_still() {
    let (base, caps) = served("proposals-open");
    let names = a_run(&caps);
    let before = caps.chain.tips().expect("derives");

    let mut response = common::act(
        &format!("{base}/api/proposals/accept"),
        &serde_json::json!({ "objects": [names[1].as_str()] }),
    );
    assert_eq!(common::status(&response), 400);
    let said = common::body_json(&mut response)["error"]
        .as_str()
        .expect("a sentence")
        .to_owned();
    assert!(
        said.contains("Picking a completion means taking the todo it completes"),
        "{said}"
    );
    assert!(said.contains(&names[0].as_str()[..12]), "{said}");
    assert_eq!(caps.chain.tips().expect("derives"), before);
}

/// Rejecting removes from the replica and leaves the chain alone; the same event over the same
/// heads is the same object, so only a replay of the very write would bring it back, and every
/// write a run makes carries the clock's instant.
#[test]
fn rejecting_removes_the_batch_and_touches_no_chain() {
    let (base, caps) = served("proposals-reject");
    let names = a_run(&caps);
    let before = caps.chain.tips().expect("derives");
    let picked: Vec<String> = names.iter().map(|name| name.as_str().to_owned()).collect();

    let mut response = common::act(
        &format!("{base}/api/proposals/reject"),
        &serde_json::json!({ "objects": picked }),
    );
    assert_eq!(common::status(&response), 200);
    let removed = common::body_json(&mut response);
    assert_eq!(removed["chain"], "life");
    assert_eq!(with_signatures(&removed["removed"], &picked), picked);

    assert_eq!(
        caps.chain.tips().expect("derives"),
        before,
        "rejecting touches no chain"
    );
    assert!(
        batches(&base)["batches"]
            .as_array()
            .expect("a list")
            .is_empty()
    );
    let replayed = caps.agent_run().append(a_new_todo()).expect("appends");
    assert_eq!(replayed, names[0], "the same bytes are the same object");
}

/// A descendant-open rejection is refused.
#[test]
fn rejecting_a_todo_without_what_rests_on_it_is_refused() {
    let (base, caps) = served("proposals-orphan");
    let names = a_run(&caps);
    let mut response = common::act(
        &format!("{base}/api/proposals/reject"),
        &serde_json::json!({ "objects": [names[0].as_str()] }),
    );
    assert_eq!(common::status(&response), 400);
    let said = common::body_json(&mut response)["error"]
        .as_str()
        .expect("a sentence")
        .to_owned();
    assert!(
        said.contains("still rests on what this would remove"),
        "{said}"
    );
    assert_eq!(
        batches(&base)["batches"][0]["objects"]
            .as_array()
            .expect("a list")
            .len(),
        2,
        "nothing was removed"
    );
}

/// A read-only login sees the Inbox and cannot settle a batch.
#[test]
fn a_viewer_reads_the_inbox_and_settles_nothing() {
    let (writer_base, caps) = served("proposals-viewer");
    // The same capabilities served over v6, a viewer and never an actor.
    let deployment = common::deployment([ada(), guest()], [(ada(), common::actor("ada"))]);
    let base = common::Server::new(Arc::clone(&caps), deployment, Vec::new())
        .v6()
        .start()
        .base;
    let names = a_run(&caps);

    let mut listed = common::get(&format!("{base}/api/proposals"));
    assert_eq!(common::status(&listed), 200, "a viewer may read the inbox");
    assert_eq!(
        common::body_json(&mut listed)["batches"]
            .as_array()
            .expect("a list")
            .len(),
        1
    );

    for verb in ["accept", "reject"] {
        let mut response = common::act(
            &format!("{base}/api/proposals/{verb}"),
            &serde_json::json!({ "objects": [names[0].as_str()] }),
        );
        assert_eq!(common::status(&response), 403, "{verb}");
        assert!(
            common::body_json(&mut response)["error"]
                .as_str()
                .expect("a sentence")
                .contains("may read this app but not write to it")
        );
    }
    assert_eq!(
        batches(&writer_base)["batches"][0]["objects"]
            .as_array()
            .expect("a list")
            .len(),
        2,
        "nothing moved either way"
    );
}
