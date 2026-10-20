//! The phone as a replica: `POST /api/objects` driven as a phone drives it, over a real server.
//!
//! `v4` is the writer `ada@example.com` and `v6` a viewer; unresolved peers are `gate.rs`'s.

mod common;

use std::sync::Arc;

use common::Fakes;
use prodrome_core::event::{
    Hash, canonical_envelope, mk_completed, mk_created, mk_reopened, mk_sealed, seal_hash,
};
use suzatary_core::record::Envelope;
use suzatary_core::store::Prodrome as _;

fn ada() -> suzatary_core::capability::Login {
    common::login("ada@example.com")
}

fn guest() -> suzatary_core::capability::Login {
    common::login("guest@example.com")
}

/// A server where `v4` writes as `ada` and `guest@example.com` (via `v6`) may only read.
fn writer(tag: &str) -> (String, Arc<Fakes>) {
    let whois = common::FakeWhois::new()
        .naming(common::v4(), ada())
        .naming(common::v6(), guest());
    let fakes = Fakes::new(tag).whois(whois);
    let caps = Arc::new(fakes);
    let deployment = common::deployment([ada(), guest()], [(ada(), common::actor("ada"))]);
    let base = common::start(Arc::clone(&caps), deployment, Vec::new());
    (base, caps)
}

fn events(caps: &Fakes) -> Vec<suzatary_core::record::TodoEvent> {
    caps.chain
        .objects()
        .expect("reads")
        .into_iter()
        .filter_map(|(_, envelope)| envelope.event().cloned())
        .collect()
}

/// An instant the fixture's clock already believes, so a claim is not future-dated.
fn at(minute: u32) -> prodrome_core::literal::Datetime {
    prodrome_core::literal::Datetime::new(2026, 9, 2, 11, minute, 0, 0).expect("a real instant")
}

/// What a phone does: seal an event onto a head it holds.
fn sealed(prev: Option<&Hash>, event: suzatary_core::record::TodoEvent) -> Envelope {
    mk_sealed(prev.cloned(), event)
}

/// The phone's signature of `object`, which it posts beside it.
fn signature(object: &Envelope) -> Envelope {
    Envelope::Signed(common::phone().sign(&seal_hash(object)))
}

/// What a post of `objects` lands, each object then its signature, by name.
fn landed(objects: &[&Envelope]) -> serde_json::Value {
    objects
        .iter()
        .flat_map(|object| [seal_hash(object), seal_hash(&signature(object))])
        .map(|name| name.as_str().to_owned())
        .collect()
}

/// Hand objects over the way the app does: each signed by the phone that sealed it.
fn post(base: &str, objects: &[&Envelope]) -> ureq::http::Response<ureq::Body> {
    let literals: Vec<String> = objects
        .iter()
        .flat_map(|o| [canonical_envelope(o), canonical_envelope(&signature(o))])
        .collect();
    common::act(
        &format!("{base}/api/objects"),
        &serde_json::json!({ "objects": literals }),
    )
}

// --- The gate ----------------------------------------------------------------

/// A POST from loopback is refused.
#[test]
fn a_post_from_loopback_is_refused() {
    let whois = common::FakeWhois::new().naming(common::v4(), ada());
    let fakes = Fakes::new("sync-loopback").whois(whois);
    let caps = Arc::new(fakes);
    let deployment = common::deployment([ada()], [(ada(), common::actor("ada"))]);
    let before = events(&caps).len();
    let base = common::start(
        Arc::clone(&caps),
        deployment,
        common::loopback_is_this_box(),
    );
    let object = common::written(
        &caps.chain,
        mk_completed("file-permit", at(8), "ada", "").expect("valid"),
    );
    let mut response = post(&base, &[&object]);
    assert_eq!(common::status(&response), 403);
    assert!(
        common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("named tailnet peer")
    );
    assert_eq!(events(&caps).len(), before);
}

/// A viewer who is not an actor may read the objects and not post any.
#[test]
fn a_viewer_who_is_not_an_actor_may_read_but_not_post() {
    let whois = common::FakeWhois::new()
        .naming(common::v4(), ada())
        .naming(common::v6(), guest());
    let fakes = Fakes::new("sync-guest").whois(whois);
    let caps = Arc::new(fakes);
    let deployment = common::deployment([ada(), guest()], [(ada(), common::actor("ada"))]);
    let base6 = common::Server::new(Arc::clone(&caps), deployment, Vec::new())
        .v6()
        .start()
        .base;
    assert_eq!(
        common::status(&common::get(&format!("{base6}/api/objects"))),
        200
    );
    let object = common::written(
        &caps.chain,
        mk_completed("file-permit", at(8), "ada", "").expect("valid"),
    );
    let mut refused = post(&base6, &[&object]);
    assert_eq!(common::status(&refused), 403);
    assert!(
        common::body_json(&mut refused)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("not write")
    );
}

/// The CSRF triple guards the sync route.
#[test]
fn the_csrf_triple_guards_the_sync_route() {
    let (base, caps) = writer("sync-csrf");
    let object = common::written(
        &caps.chain,
        mk_completed("file-permit", at(8), "ada", "").expect("valid"),
    );
    let body = serde_json::json!({ "objects": [canonical_envelope(&object)] });

    // No app header.
    let mut no_header = common::agent()
        .post(format!("{base}/api/objects"))
        .send_json(body.clone())
        .expect("the request completes");
    assert_eq!(common::status(&no_header), 403);
    assert!(
        common::body_json(&mut no_header)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("x-suzatary-act")
    );

    // Labelled cross-site by the browser that sent it.
    for site in ["cross-site", "same-site", "none"] {
        let mut response = common::agent()
            .post(format!("{base}/api/objects"))
            .header("x-suzatary-act", "1")
            .header("sec-fetch-site", site)
            .send_json(body.clone())
            .expect("the request completes");
        assert_eq!(common::status(&response), 403, "{site}");
        assert!(
            common::body_json(&mut response)["error"]
                .as_str()
                .unwrap_or_default()
                .contains("same-origin"),
            "{site}"
        );
    }

    // A page from before the box's write contract: told to load the new one, before anything is
    // written.
    let mut stale = common::agent()
        .post(format!("{base}/api/objects"))
        .header("x-suzatary-act", "1")
        .header("sec-fetch-site", "same-origin")
        .send_json(body.clone())
        .expect("the request completes");
    assert_eq!(common::status(&stale), 426);
    assert!(
        common::body_json(&mut stale)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("was not sent")
    );

    // Not JSON.
    let form = common::agent()
        .post(format!("{base}/api/objects"))
        .header("x-suzatary-act", "1")
        .header("content-type", "application/x-www-form-urlencoded")
        .send(&b"objects=x"[..])
        .expect("the request completes");
    assert_eq!(common::status(&form), 403);
}

// --- What the box takes ------------------------------------------------------

/// A sealed act from a phone lands on the chain, and the answer names it.
#[test]
fn a_sealed_act_from_a_phone_lands_on_the_chain() {
    let (base, caps) = writer("sync-lands");
    let before = events(&caps).len();
    let object = common::written(
        &caps.chain,
        mk_completed("file-permit", at(8), "ada", "").expect("valid"),
    );
    let mut response = post(&base, &[&object]);
    assert_eq!(
        common::status(&response),
        200,
        "{:?}",
        common::body_json(&mut response)
    );
    let body = common::body_json(&mut response);
    assert_eq!(body["adopted"], landed(&[&object]));
    assert_eq!(body["refused"], serde_json::json!([]));
    assert!(
        body["heads"]
            .as_array()
            .expect("a set")
            .contains(&serde_json::json!(seal_hash(&signature(&object)).as_str())),
        "{body}"
    );
    assert_eq!(events(&caps).len(), before + 1);
    assert_eq!(caps.chain.verify(), Vec::<String>::new());
    let mut fold = common::get(&format!("{base}/api/fold"));
    let rows = common::body_json(&mut fold);
    let row = rows["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|row| row["todo"] == "file-permit")
        .expect("the todo is a row");
    assert_eq!(row["state"], "completed");
}

/// What a tab does since it writes changes: hold the box's objects in the Prodrome's replica,
/// append there, and post the one object that answers.
#[test]
fn a_change_a_phone_appended_lands_through_adoption() {
    use prodrome_core::store::{MemoryStore, Replica as _};
    let (base, caps) = writer("sync-change");
    let held = caps.chain.objects().expect("reads");
    let prints: std::collections::BTreeMap<Hash, String> = held
        .iter()
        .map(|(name, envelope)| (name.clone(), canonical_envelope(envelope)))
        .collect();
    let tab: MemoryStore<suzatary_core::record::TodoEvent> = MemoryStore::default();
    tab.receive(
        caps.chain.tips().expect("tips").into_iter().collect(),
        &|name| prints.get(name).map(|print| print.clone().into_bytes()),
    )
    .expect("the box's objects verify");
    let name = tab
        .append(mk_completed("file-permit", at(8), "ada", "").expect("valid"))
        .expect("the replica appends");
    let literal = String::from_utf8(tab.print(&name).expect("held")).expect("utf8");
    assert!(literal.starts_with("Change(genesis="), "{literal}");

    let signed = Envelope::Signed(common::phone().sign(&name));
    let mut response = common::act(
        &format!("{base}/api/objects"),
        &serde_json::json!({ "objects": [literal, canonical_envelope(&signed)] }),
    );
    let body = common::body_json(&mut response);
    assert_eq!(
        body["adopted"],
        serde_json::json!([name.as_str(), seal_hash(&signed).as_str()]),
        "{body}"
    );
    assert_eq!(body["refused"], serde_json::json!([]));
    assert!(
        caps.chain
            .tips()
            .expect("tips")
            .contains(&seal_hash(&signed)),
        "the change's signature is a head, and nothing joins it"
    );
    assert_eq!(caps.chain.verify(), Vec::<String>::new());
    let mut fold = common::get(&format!("{base}/api/fold"));
    let rows = common::body_json(&mut fold);
    let row = rows["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|row| row["todo"] == "file-permit")
        .expect("the todo is a row");
    assert_eq!(row["state"], "completed");
}

/// Posting the same object twice is the same hash twice; the hash is the identity.
#[test]
fn a_re_post_appends_nothing_and_says_it_was_already_held() {
    let (base, caps) = writer("sync-idempotent");
    let object = common::written(
        &caps.chain,
        mk_completed("file-permit", at(8), "ada", "").expect("valid"),
    );
    let _ = post(&base, &[&object]);
    let after = events(&caps).len();
    let mut again = post(&base, &[&object]);
    let body = common::body_json(&mut again);
    assert_eq!(body["adopted"], serde_json::json!([]));
    assert_eq!(body["already"], landed(&[&object]));
    assert_eq!(events(&caps).len(), after);
}

/// The name is the hash of the print, never the client's claim.
#[test]
fn bytes_that_are_not_an_object_are_refused_by_name() {
    let (base, caps) = writer("sync-rehash");
    let before = events(&caps).len();
    let mut response = common::act(
        &format!("{base}/api/objects"),
        &serde_json::json!({"objects": ["Sealed(prev='', event=Nonsense())", "not a literal"]}),
    );
    assert_eq!(common::status(&response), 200);
    let body = common::body_json(&mut response);
    let refused = body["refused"].as_array().expect("refusals");
    assert_eq!(refused.len(), 2);
    for one in refused {
        assert_eq!(one["hash"], "");
        assert!(
            one["reason"]
                .as_str()
                .unwrap_or_default()
                .contains("not one canonical object"),
            "{one}"
        );
    }
    assert_eq!(events(&caps).len(), before);
}

/// An event naming an actor other than the poster's mapping is refused.
#[test]
fn an_object_naming_another_actor_is_refused() {
    let (base, caps) = writer("sync-foreign-actor");
    let before = events(&caps).len();
    let forged = common::written(
        &caps.chain,
        mk_completed("file-permit", at(8), "triage", "").expect("valid"),
    );
    let mut response = post(&base, &[&forged]);
    let body = common::body_json(&mut response);
    assert_eq!(body["adopted"], serde_json::json!([]));
    let refused = &body["refused"][0];
    assert_eq!(refused["hash"], seal_hash(&forged).as_str());
    assert!(
        refused["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("triage"),
        "{refused}"
    );
    assert_eq!(events(&caps).len(), before);
}

/// A parent nobody holds is refused, with everything under it.
#[test]
fn an_object_resting_on_nothing_the_box_holds_is_refused() {
    let (base, caps) = writer("sync-orphan");
    let before = events(&caps).len();
    let unknown = Hash::new("f".repeat(64)).expect("64 hex");
    let orphan = sealed(
        Some(&unknown),
        mk_completed("file-permit", at(8), "ada", "").expect("valid"),
    );
    let child = sealed(
        Some(&seal_hash(&orphan)),
        mk_completed("water-plants", at(9), "ada", "").expect("valid"),
    );
    let mut response = post(&base, &[&orphan, &child]);
    let body = common::body_json(&mut response);
    assert_eq!(body["adopted"], serde_json::json!([]));
    assert_eq!(
        body["refused"].as_array().expect("refusals").len(),
        4,
        "the two objects and the signatures resting on them"
    );
    assert_eq!(events(&caps).len(), before);
    assert_eq!(caps.chain.verify(), Vec::<String>::new());
}

/// Objects posted in any order are placed by parent.
#[test]
fn a_post_in_reverse_order_still_lands_parents_first() {
    let (base, caps) = writer("sync-order");
    // One todo twice, so the second change rests on the first.
    let tab = common::tab(&caps.chain);
    let first = common::acted(
        &tab,
        mk_completed("file-permit", at(8), "ada", "").expect("valid"),
    );
    let second = common::acted(
        &tab,
        mk_reopened("file-permit", at(9), "ada", "").expect("valid"),
    );
    let mut response = post(&base, &[&second, &first]);
    let body = common::body_json(&mut response);
    assert_eq!(
        body["adopted"],
        landed(&[&first, &second]),
        "parents first, whatever order they arrived in"
    );
    let heads = body["heads"].as_array().expect("a set");
    assert!(heads.contains(&serde_json::json!(seal_hash(&signature(&second)).as_str())));
    assert!(!heads.contains(&serde_json::json!(seal_hash(&first).as_str())));
    assert_eq!(caps.chain.verify(), Vec::<String>::new());
}

/// Two diverged replicas leave the box both heads: nothing joins them, and the fold reads both
/// branches' facts.
#[test]
fn divergence_leaves_both_heads_and_the_box_writes_nothing() {
    let (base, caps) = writer("sync-merge");
    // Two tabs of the same box, neither seeing the other's act.
    let (one, other) = (common::tab(&caps.chain), common::tab(&caps.chain));
    let before = caps.chain.objects().expect("reads").len();
    let mine = common::acted(
        &one,
        mk_completed("file-permit", at(8), "ada", "").expect("valid"),
    );
    let theirs = common::acted(
        &other,
        mk_completed("water-plants", at(8), "ada", "").expect("valid"),
    );
    assert_eq!(common::status(&post(&base, &[&mine])), 200);
    let mut response = post(&base, &[&theirs]);
    let body = common::body_json(&mut response);
    let heads = body["heads"].as_array().expect("a set");
    // Each act's head is its phone's signature of it.
    for name in [&mine, &theirs].map(|object| seal_hash(&signature(object))) {
        assert!(heads.contains(&serde_json::json!(name.as_str())), "{body}");
    }
    assert_eq!(
        caps.chain.objects().expect("reads").len(),
        before + 4,
        "the two posts and their signatures, and nothing the box wrote"
    );
    assert_eq!(caps.chain.verify(), Vec::<String>::new());

    let mut fold = common::get(&format!("{base}/api/fold"));
    let rows = common::body_json(&mut fold);
    for todo in ["file-permit", "water-plants"] {
        let row = rows["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .find(|row| row["todo"] == todo)
            .unwrap_or_else(|| panic!("{todo} is a row"));
        assert_eq!(row["state"], "completed", "{todo}");
    }
}

/// Nothing is clamped: a future-dated `at` is stored as written.
#[test]
fn the_phones_at_is_stored_as_the_phone_claimed_it() {
    let (base, caps) = writer("sync-at");
    let claimed = prodrome_core::literal::Datetime::new(2031, 1, 2, 3, 4, 5, 0).expect("real");
    let object = common::written(
        &caps.chain,
        mk_completed("file-permit", claimed, "ada", "").expect("valid"),
    );
    assert_eq!(common::status(&post(&base, &[&object])), 200);
    assert!(
        events(&caps)
            .iter()
            .any(|event| event.todo().as_str() == "file-permit" && event.at() == claimed),
        "the claimed instant is stored as written"
    );
}

// --- The read side ------------------------------------------------------------

/// `since` names the heads the caller holds; one the box does not hold holds back nothing.
#[test]
fn objects_since_answers_only_what_the_caller_lacks() {
    let (base, caps) = writer("sync-since");
    let heads: Vec<String> = caps
        .chain
        .tips()
        .expect("derives")
        .iter()
        .map(|head| head.as_str().to_owned())
        .collect();
    let since = heads.join(",");

    let mut all = common::get(&format!("{base}/api/objects"));
    let everything = common::body_json(&mut all);
    let held = everything["objects"].as_array().expect("objects").len();
    assert!(held > 0);
    assert_eq!(everything["heads"], serde_json::json!(heads));
    assert!(everything["untrusted"].as_array().is_some());

    let mut caught_up = common::get(&format!("{base}/api/objects?since={since}"));
    assert_eq!(
        common::body_json(&mut caught_up)["objects"],
        serde_json::json!([])
    );

    let object = common::written(
        &caps.chain,
        mk_completed("file-permit", at(8), "ada", "").expect("valid"),
    );
    assert_eq!(common::status(&post(&base, &[&object])), 200);
    let mut behind = common::get(&format!("{base}/api/objects?since={since}"));
    let missing = common::body_json(&mut behind);
    assert_eq!(
        missing["objects"].as_array().expect("objects").len(),
        2,
        "the object and its signature"
    );
    assert_eq!(missing["objects"][0]["hash"], seal_hash(&object).as_str());
    assert_eq!(
        missing["objects"][0]["literal"],
        canonical_envelope(&object)
    );

    let stranger = "e".repeat(64);
    let mut unknown = common::get(&format!("{base}/api/objects?since={stranger}"));
    assert_eq!(
        common::body_json(&mut unknown)["objects"]
            .as_array()
            .expect("objects")
            .len(),
        held + 2
    );
    // An act not yet posted beside a head the box holds: the held one answers.
    let mut beside = common::get(&format!("{base}/api/objects?since={stranger},{since}"));
    assert_eq!(
        common::body_json(&mut beside)["objects"]
            .as_array()
            .expect("objects")
            .len(),
        2
    );

    // A `since` that is not a name is a refusal, not a guess.
    assert_eq!(
        common::status(&common::get(&format!("{base}/api/objects?since=beef"))),
        400
    );
}

/// `GET /api/me` is the actor a replica must seal as, or `null` for a read-only login.
#[test]
fn me_names_the_actor_or_says_this_login_cannot_seal() {
    let whois = common::FakeWhois::new()
        .naming(common::v4(), ada())
        .naming(common::v6(), guest());
    let fakes = Fakes::new("sync-me").whois(whois);
    let caps = Arc::new(fakes);
    let deployment = common::deployment([ada(), guest()], [(ada(), common::actor("ada"))]);
    let base = common::start(Arc::clone(&caps), deployment.clone(), Vec::new());
    let base6 = common::Server::new(Arc::clone(&caps), deployment, Vec::new())
        .v6()
        .start()
        .base;

    let mut mine = common::get(&format!("{base}/api/me"));
    let body = common::body_json(&mut mine);
    assert_eq!(body["login"], "ada@example.com");
    assert_eq!(body["actor"], "ada");

    let mut theirs = common::get(&format!("{base6}/api/me"));
    let viewer = common::body_json(&mut theirs);
    assert_eq!(viewer["login"], "guest@example.com");
    assert!(viewer["actor"].is_null(), "a viewer may not seal: {viewer}");
}

// --- The authoring a replica cannot do for itself -----------------------------

/// `POST /api/author` appends nothing and answers the event's canonical print.
#[test]
fn author_hands_back_an_event_and_writes_nothing() {
    let (base, caps) = writer("sync-author");
    let before = events(&caps).len();
    let heads = caps.chain.tips().expect("derives");
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"add": {
            "body": "Renew the lease",
            "category": "home",
            "priority": {"how": "flat", "percent": 35},
            "rationale": "The landlord wants notice.",
        }}),
    );
    assert_eq!(
        common::status(&response),
        200,
        "{:?}",
        common::body_json(&mut response)
    );
    let body = common::body_json(&mut response);
    assert_eq!(body["todo"], "renewthelease");
    let print = body["event"].as_str().expect("a print").to_owned();
    assert_eq!(events(&caps).len(), before);
    assert_eq!(caps.chain.tips().expect("derives"), heads);

    let event: suzatary_core::record::TodoEvent = prodrome_core::event::parse_event(
        &suzatary_core::record::TodoVocabulary::default(),
        &print,
    )
    .expect("the core printed it");
    assert_eq!(event.actor().as_str(), "ada");
    // Prose is escaped, which is why the box authors rather than the browser.
    let object = common::written(&caps.chain, event);
    assert_eq!(common::status(&post(&base, &[&object])), 200);
    assert_eq!(events(&caps).len(), before + 1);

    let mut fold = common::get(&format!("{base}/api/fold"));
    let rows = common::body_json(&mut fold);
    assert!(
        rows["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .any(|row| row["todo"] == "renewthelease"),
        "the todo the box authored and the phone sealed is on the chain"
    );
}

/// A recurring price from the web builds the same cycle as the CLI's flags.
#[test]
fn author_adds_a_recurring_todo_from_the_webs_own_cycle_spelling() {
    let (base, caps) = writer("sync-author-recur");
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"add": {
            "body": common::CAST.todos.high.body,
            "category": common::CAST.todos.high.category,
            "priority": {"how": "recur", "days": 60, "from_percent": 100, "to_percent": 30},
            "rationale": "They droop after two months.",
        }}),
    );
    assert_eq!(
        common::status(&response),
        200,
        "{:?}",
        common::body_json(&mut response)
    );
    let body = common::body_json(&mut response);
    let print = body["event"].as_str().expect("a print").to_owned();
    assert!(print.contains("Recur"), "{print}");

    let event: suzatary_core::record::TodoEvent = prodrome_core::event::parse_event(
        &suzatary_core::record::TodoVocabulary::default(),
        &print,
    )
    .expect("the core printed it");
    let object = common::written(&caps.chain, event);
    assert_eq!(common::status(&post(&base, &[&object])), 200);

    let mut fold = common::get(&format!("{base}/api/fold"));
    let rows = common::body_json(&mut fold);
    let row = rows["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|row| row["todo"] == body["todo"])
        .expect("the recurring todo is on the chain");
    assert_eq!(
        row["recurring"], true,
        "a Recur price restarts from its own passes: {row}"
    );
}

/// `add` with a `parent` hands back the child's event and the parent's revision, unappended.
#[test]
fn author_adds_a_subtodo_as_the_child_and_its_parents_revision() {
    let (base, caps) = writer("sync-author-subtodo");
    let before = events(&caps).len();
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"add": {
            "body": "Print the boarding pass",
            "category": "home",
            "priority": {"how": "flat", "percent": 60},
            "rationale": "",
            "parent": "book-flight",
        }}),
    );
    assert_eq!(
        common::status(&response),
        200,
        "{:?}",
        common::body_json(&mut response)
    );
    let body = common::body_json(&mut response);
    assert_eq!(body["todo"], "printtheboardingpass");
    assert_eq!(events(&caps).len(), before, "nothing was appended");
    let parse = |key: &str| {
        prodrome_core::event::parse_event(
            &suzatary_core::record::TodoVocabulary::default(),
            body[key].as_str().expect("a print"),
        )
        .expect("the core printed it")
    };
    let tab = common::tab(&caps.chain);
    let child = common::acted(&tab, parse("event"));
    let revision = common::acted(&tab, parse("revision"));
    assert_eq!(common::status(&post(&base, &[&child, &revision])), 200);

    let mut tree = common::get(&format!("{base}/api/tree?by=flat"));
    let tree = common::body_json(&mut tree);
    let parent = tree["root"]["children"]
        .as_array()
        .expect("children")
        .iter()
        .find(|node| node["todo"] == "book-flight")
        .expect("the parent is a root");
    assert_eq!(
        parent["children"][0]["todo"], "printtheboardingpass",
        "{tree}"
    );

    let mut plain = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"add": {
            "body": "Nothing above it",
            "category": "home",
            "priority": {"how": "flat", "percent": 60},
            "rationale": "",
        }}),
    );
    assert!(common::body_json(&mut plain)["revision"].is_null());

    let orphan = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"add": {
            "body": "An orphan",
            "category": "home",
            "priority": {"how": "flat", "percent": 60},
            "rationale": "",
            "parent": "no-such-todo",
        }}),
    );
    assert_eq!(common::status(&orphan), 404);
}

/// A body naming two arms or none, an arm of the wrong shape, or a field no arm reads is
/// refused at the parse.
#[test]
fn author_takes_exactly_one_of_add_or_confirm() {
    let (base, _caps) = writer("sync-author-shape");
    for body in [
        serde_json::json!({}),
        serde_json::json!({"add": {}, "confirm": {"todo": "file-permit"}}),
        serde_json::json!({"add": "a string"}),
        serde_json::json!({"confirm": 7}),
        serde_json::json!({"confirm": "file-permit"}),
        serde_json::json!({"confirm": {"todo": "file-permit"}, "note": "beside the arm"}),
        serde_json::json!({"confirm": {"todo": "file-permit", "colour": "red"}}),
        serde_json::json!({"add": {
            "body": "x", "category": "home", "rationale": "r",
            "priority": {"how": "flat", "percent": 75}, "colour": "red",
        }}),
    ] {
        let response = common::act(&format!("{base}/api/author"), &body);
        assert_eq!(common::status(&response), 400, "{body}");
    }
}

/// `confirm` enforces both of `authoring::confirm_todo`'s refusals in the box.
#[test]
fn confirming_content_a_trusted_actor_wrote_confirms_nothing() {
    let (base, _caps) = writer("sync-confirm");
    // `file-permit` is a trusted actor's own record in the fixture chain.
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"confirm": {"todo": "file-permit"}}),
    );
    assert_eq!(common::status(&response), 409);
    assert!(
        common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("nothing to confirm")
    );

    // A todo with no content on the chain is a 404.
    let missing = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"confirm": {"todo": "no-such-todo"}}),
    );
    assert_eq!(common::status(&missing), 404);
}

/// Undo and redo also append nothing; undoing the completed `renew-card` is a `Reopened`.
#[test]
fn undo_hands_back_a_reopened_naming_the_completion_it_undid() {
    let (base, caps) = writer("sync-undo");
    let before = events(&caps).len();
    let heads = caps.chain.tips().expect("derives");
    let done_hash = caps
        .chain
        .objects()
        .expect("reads")
        .into_iter()
        .find(|(_, envelope)| {
            matches!(
                envelope.event(),
                Some(suzatary_core::record::TodoEvent::Completed(e))
                    if e.todo.as_str() == "renew-card"
            )
        })
        .map(|(hash, _)| hash)
        .expect("the fixture's resolved todo has a Completed event");

    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"undo": "renew-card"}),
    );
    assert_eq!(
        common::status(&response),
        200,
        "{:?}",
        common::body_json(&mut response)
    );
    let body = common::body_json(&mut response);
    assert_eq!(body["todo"], "renew-card");
    assert_eq!(events(&caps).len(), before);
    assert_eq!(caps.chain.tips().expect("derives"), heads);

    let print = body["event"].as_str().expect("a print").to_owned();
    let event: suzatary_core::record::TodoEvent = prodrome_core::event::parse_event(
        &suzatary_core::record::TodoVocabulary::default(),
        &print,
    )
    .expect("the core printed it");
    let suzatary_core::record::TodoEvent::Reopened(lifecycle) = &event else {
        panic!("expected a Reopened, got {}", event.kind_name())
    };
    assert_eq!(lifecycle.actor.as_str(), "ada");
    assert_eq!(lifecycle.note, format!("undo of {}", done_hash.as_str()));
}

/// `undo` and `redo` each name a todo, checked at the parse.
#[test]
fn an_undo_or_a_redo_names_its_todo() {
    let (base, _caps) = writer("sync-act-shape");
    for body in [
        serde_json::json!({"undo": 7}),
        serde_json::json!({"undo": null}),
        serde_json::json!({"sideways": "renew-card"}),
        serde_json::json!({"act": "undo", "todo": "renew-card"}),
        serde_json::json!({"add": {}, "undo": "renew-card"}),
    ] {
        let response = common::act(&format!("{base}/api/author"), &body);
        assert_eq!(common::status(&response), 400, "{body}");
    }
}

/// Redo refuses in the box's words when the last act was not an undo.
#[test]
fn redo_refuses_when_the_last_act_was_not_an_undo() {
    let (base, _caps) = writer("sync-redo-refusal");
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"redo": "renew-card"}),
    );
    assert_eq!(common::status(&response), 409);
    assert!(
        common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("is not an undo")
    );
}

// --- Editing a todo already on the chain -------------------------------------

/// `update` appends nothing, and fields the request leaves out carry over.
#[test]
fn update_hands_back_an_event_and_writes_nothing() {
    let (base, caps) = writer("sync-update");
    let before = events(&caps).len();
    let heads = caps.chain.tips().expect("derives");
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"update": {
            "todo": "book-flight",
            "body": "Book the flight to Oslo, direct",
        }}),
    );
    assert_eq!(
        common::status(&response),
        200,
        "{:?}",
        common::body_json(&mut response)
    );
    let body = common::body_json(&mut response);
    assert_eq!(body["todo"], "book-flight");
    assert_eq!(events(&caps).len(), before);
    assert_eq!(caps.chain.tips().expect("derives"), heads);

    let print = body["event"].as_str().expect("a print").to_owned();
    let event: suzatary_core::record::TodoEvent = prodrome_core::event::parse_event(
        &suzatary_core::record::TodoVocabulary::default(),
        &print,
    )
    .expect("the core printed it");
    let suzatary_core::record::TodoEvent::Authored(record) = &event else {
        panic!("expected an Authored, got {}", event.kind_name())
    };
    assert_eq!(record.actor.as_str(), "ada");
    assert_eq!(
        record.payload.body.as_str(),
        "Book the flight to Oslo, direct"
    );
    // `category` was never named in the request.
    assert_eq!(
        record
            .payload
            .category
            .as_ref()
            .map(prodrome_core::event::Name::as_str),
        Some("home")
    );

    let object = common::written(&caps.chain, event);
    assert_eq!(common::status(&post(&base, &[&object])), 200);
    assert_eq!(events(&caps).len(), before + 1);
}

/// An edit that names nothing is refused in the box's own words.
#[test]
fn an_empty_update_is_refused_in_the_boxs_own_words() {
    let (base, _caps) = writer("sync-update-empty");
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"update": {"todo": "book-flight"}}),
    );
    assert_eq!(common::status(&response), 400);
    assert!(
        common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("must change something")
    );
}

/// Ticking a subtodo sends the whole checklist back with one line flipped.
#[test]
fn ticking_a_subtodo_sends_the_whole_checklist_back() {
    let (base, _caps) = writer("sync-update-subtodo");
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"update": {
            "todo": "pack-bags",
            "subtodos": [
                {"body": "[tickets]", "done": true},
                {"body": "[chargers]", "done": true},
            ],
        }}),
    );
    assert_eq!(
        common::status(&response),
        200,
        "{:?}",
        common::body_json(&mut response)
    );
    let body = common::body_json(&mut response);
    let print = body["event"].as_str().expect("a print").to_owned();
    let event: suzatary_core::record::TodoEvent = prodrome_core::event::parse_event(
        &suzatary_core::record::TodoVocabulary::default(),
        &print,
    )
    .expect("the core printed it");
    let suzatary_core::record::TodoEvent::Authored(record) = &event else {
        panic!("expected an Authored, got {}", event.kind_name())
    };
    assert_eq!(record.payload.subtodos.len(), 2);
    assert!(
        record.payload.subtodos.iter().all(|sub| sub.done),
        "both ticked"
    );
    assert_eq!(record.payload.body.as_str(), "Pack for the trip");
}

/// A todo id the chain has never mentioned is a 404.
#[test]
fn updating_an_unknown_todo_is_a_404() {
    let (base, _caps) = writer("sync-update-missing");
    let response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"update": {"todo": "no-such-todo", "body": "x"}}),
    );
    assert_eq!(common::status(&response), 404);
}

// --- Defer, as a curve -------------------------------------------------------

/// Deferring a flat term writes a `Piecewise` whose rationale is generated from the date.
#[test]
fn deferring_wraps_the_current_term_and_supplies_its_own_rationale() {
    let (base, caps) = writer("sync-defer-flat");
    let before = events(&caps).len();
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"update": {
            "todo": "book-flight",
            "defer_to": "2026-09-20",
        }}),
    );
    assert_eq!(
        common::status(&response),
        200,
        "{:?}",
        common::body_json(&mut response)
    );
    let body = common::body_json(&mut response);
    assert_eq!(events(&caps).len(), before);
    let print = body["event"].as_str().expect("a print").to_owned();
    let event: suzatary_core::record::TodoEvent = prodrome_core::event::parse_event(
        &suzatary_core::record::TodoVocabulary::default(),
        &print,
    )
    .expect("the core printed it");
    let suzatary_core::record::TodoEvent::Authored(record) = &event else {
        panic!("expected an Authored, got {}", event.kind_name())
    };
    let spec = &record.payload.spec;
    let expected = suzatary_core::authoring::defer(
        &prodrome_core::fpl::mk_flat(0.55).expect("valid"),
        chrono::NaiveDate::from_ymd_opt(2026, 9, 20).expect("a real day"),
        suzatary_core::zone::Moment {
            at: suzatary_core::capability::Datetime::new(2026, 9, 2, 12, 0, 0, 0).expect("noon"),
            zone: suzatary_core::zone::Zone::UTC,
        },
    )
    .expect("defers");
    assert_eq!(spec, &expected);
    assert_eq!(
        record.payload.rationale,
        vec!["deferred to 2026-09-20".to_owned()]
    );
}

/// A deferral to a date not after today is refused.
#[test]
fn a_deferral_not_after_today_is_refused_over_the_route() {
    let (base, _caps) = writer("sync-defer-past");
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"update": {
            "todo": "book-flight",
            "defer_to": "2026-09-02",
        }}),
    );
    assert_eq!(common::status(&response), 400);
    assert!(
        common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("defer to a date after today")
    );
}

/// A deferral past a `Decay`'s deadline is a reprice, not a deferral.
#[test]
fn a_deferral_past_the_deadline_is_refused_as_a_reprice_over_the_route() {
    let (base, _caps) = writer("sync-defer-deadline");
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"update": {
            "todo": "file-permit",
            "defer_to": "2026-09-10",
        }}),
    );
    assert_eq!(common::status(&response), 400);
    assert!(
        common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("a reprice, not a deferral")
    );
}

/// `defer_to` and `priority` are exclusive.
#[test]
fn defer_to_and_priority_together_is_refused() {
    let (base, _caps) = writer("sync-defer-exclusive");
    let mut response = common::act(
        &format!("{base}/api/author"),
        &serde_json::json!({"update": {
            "todo": "book-flight",
            "defer_to": "2026-09-20",
            "priority": {"how": "flat", "percent": 40},
        }}),
    );
    assert_eq!(common::status(&response), 400);
    assert!(
        common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("at most one of priority or defer_to")
    );
}

/// A body carrying `update` alongside `add`, `confirm` or `act` is refused.
#[test]
fn update_and_another_arm_together_is_refused_at_the_parse() {
    let (base, _caps) = writer("sync-update-shape");
    for body in [
        serde_json::json!({"update": {}, "add": {}}),
        serde_json::json!({"update": {}, "confirm": {"todo": "file-permit"}}),
        serde_json::json!({"update": {"todo": "book-flight", "body": "x", "colour": "red"}}),
        serde_json::json!({"update": "a string"}),
    ] {
        let response = common::act(&format!("{base}/api/author"), &body);
        assert_eq!(common::status(&response), 400, "{body}");
    }
}

/// A `Created` for an unknown todo is taken; only what `suzatary_core::sync` names is refused.
#[test]
fn the_sync_route_has_no_opinions_beyond_the_four_refusals() {
    let (base, caps) = writer("sync-no-opinions");
    let object = common::written(
        &caps.chain,
        mk_created("brandnew", at(8), "ada", "", "").expect("valid"),
    );
    let mut response = post(&base, &[&object]);
    let body = common::body_json(&mut response);
    assert_eq!(body["refused"], serde_json::json!([]));
    assert_eq!(body["adopted"], landed(&[&object]));
    assert_eq!(caps.chain.verify(), Vec::<String>::new());
}
