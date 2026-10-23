//! The small routes: chain segments, `/api/tip`, a todo's links, the tree, device tokens
//! and usage.

mod common;

use common::{Fakes, actor, deployment, login, v4};

fn writer() -> (String, Fakes) {
    let fakes =
        Fakes::new("routes").whois(common::FakeWhois::new().naming(v4(), login("her@x.test")));
    (String::new(), fakes)
}

/// A bare path means the first chain, answering byte for byte as its named path does.
#[test]
fn a_bare_path_and_the_first_chains_path_are_one_answer() {
    let fakes = Fakes::new("routes-chain-bare");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    // Clock-free routes only, since a folded answer carries the instant it was read at.
    for (bare, named) in [
        ("/api/chain", "/api/life/chain"),
        ("/api/tip", "/api/life/tip"),
    ] {
        let mut one = common::get(&format!("{base}{bare}"));
        let mut two = common::get(&format!("{base}{named}"));
        assert_eq!(common::status(&one), 200, "{bare}");
        assert_eq!(common::status(&two), 200, "{named}");
        assert_eq!(
            common::body_json(&mut one),
            common::body_json(&mut two),
            "{bare} and {named} must be one answer about one store"
        );
    }
    // A fold carries the instant it was read at, so its two answers are compared by their todos.
    let todos = |path: &str| -> Vec<String> {
        let mut fold = common::get(&format!("{base}{path}"));
        assert_eq!(common::status(&fold), 200, "{path}");
        let mut todos: Vec<String> = common::body_json(&mut fold)["entries"]
            .as_array()
            .expect("a list")
            .iter()
            .map(|entry| entry["todo"].as_str().unwrap_or_default().to_owned())
            .collect();
        todos.sort();
        todos
    };
    assert_eq!(todos("/api/fold"), todos("/api/life/fold"));
}

/// An unserved chain is a 404 that names every chain this deployment declares.
#[test]
fn a_chain_nothing_is_bound_to_is_refused_with_the_list() {
    let fakes = Fakes::new("routes-chain-unknown");
    let project = suzatary_core::chains::ChainPolicy {
        name: suzatary_core::chains::ChainName::new("suzatary").expect("a slug"),
        ..common::chain()
    };
    let deploying = suzatary_core::deployment::Deployment {
        chains: suzatary_core::chains::Chains::of(vec![common::chain(), project])
            .expect("two chains"),
        ..deployment([], [])
    };
    let base = common::start(
        std::sync::Arc::new(fakes),
        deploying,
        common::loopback_is_this_box(),
    );
    for path in [
        "/api/nowhere/fold",
        "/api/nowhere/chain",
        "/api/nowhere/tip",
        "/api/nowhere/me",
        "/api/nowhere/objects",
    ] {
        let mut response = common::get(&format!("{base}{path}"));
        assert_eq!(common::status(&response), 404, "{path}");
        let said = common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert!(said.contains("no chain named"), "{path}: {said}");
        assert!(said.contains("life"), "{path}: {said}");
        assert!(said.contains("suzatary"), "{path}: {said}");
    }
}

#[test]
fn me_names_every_chain_this_deployment_serves() {
    let fakes = Fakes::new("routes-chain-me");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    for path in ["/api/me", "/api/life/me"] {
        let mut response = common::get(&format!("{base}{path}"));
        assert_eq!(common::status(&response), 200, "{path}");
        assert_eq!(
            common::body_json(&mut response)["chains"],
            serde_json::json!(["life"]),
            "{path}"
        );
    }
}

/// `GET /api/about` names the release installed beside the running binary
/// (`stamp::release`), and the build it was stamped with: a test harness has no release file
/// beside it, so it is `dev` and completed nothing.
#[test]
fn about_names_the_version_this_binary_was_built_as() {
    let fakes = Fakes::new("routes-about");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    let mut response = common::get(&format!("{base}/api/about"));
    assert_eq!(common::status(&response), 200);
    let body = common::body_json(&mut response);
    assert_eq!(
        suzatary_cli::stamp::release().expect("the release reads"),
        &suzatary_cli::stamp::Release::Dev
    );
    assert_eq!(body["version"], serde_json::json!("dev"));
    assert_eq!(
        body["build"],
        serde_json::json!(suzatary_cli::stamp::build())
    );
    assert_eq!(
        body["completes"].as_array().map(Vec::len),
        Some(0),
        "what this version completed: {body}"
    );
}

/// `GET /api/releases` on a deployment that serves no project chain answers no release.
#[test]
fn releases_without_the_project_chain_are_none() {
    let fakes = Fakes::new("routes-releases");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    let mut response = common::get(&format!("{base}/api/releases"));
    assert_eq!(common::status(&response), 200);
    let body = common::body_json(&mut response);
    assert_eq!(body["chain"], serde_json::json!("suzatary"));
    assert_eq!(body["releases"], serde_json::json!([]));
}

/// Asks the router, in both directions, rather than trusting the reserved list.
#[test]
fn the_reserved_names_are_exactly_the_ones_a_static_route_shadows() {
    let fakes = Fakes::new("routes-chain-reserved");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    let reaches_the_chain_router = |name: &str| -> bool {
        let mut response = common::get(&format!("{base}/api/{name}/fold"));
        common::body_json(&mut response)["error"]
            .as_str()
            .unwrap_or_default()
            .contains("no chain named")
    };
    for name in suzatary_cli::instance::RESERVED {
        assert!(
            !reaches_the_chain_router(name),
            "{name:?} reaches the chain router, so it need not be reserved"
        );
    }
    // `mail` is usable because matchit backtracks past a static prefix that has no match.
    for usable in [
        "mail", "chain", "fold", "me", "tip", "tree", "objects", "author", "content", "stores",
    ] {
        assert!(
            reaches_the_chain_router(usable),
            "{usable:?} is a usable chain name and must not be reserved"
        );
        assert!(
            !suzatary_cli::instance::RESERVED.contains(&usable),
            "{usable:?} is reserved and need not be"
        );
    }
}

/// A phone polls the tip, so a cached answer would tell it nothing moved when something did.
#[test]
fn the_tip_route_is_tiny_and_true_and_never_cached() {
    let fakes = Fakes::new("routes-tip");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    let mut response = common::get(&format!("{base}/api/tip"));
    assert_eq!(common::status(&response), 200);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store"),
        "a polled tip must never come from a cache"
    );
    let payload = common::body_json(&mut response);
    let heads = payload["heads"].as_array().expect("the heads");
    assert!(!heads.is_empty(), "a chain has a head: {payload}");
    assert!(
        heads
            .iter()
            .all(|head| head.as_str().map(str::len) == Some(64)),
        "a head is an object hash"
    );
    assert!(
        payload.to_string().len() < 20 + 67 * heads.len(),
        "the tip payload is its heads and nothing else: {payload}"
    );
    assert_eq!(payload.as_object().expect("an object").len(), 1);
}

#[test]
fn a_todos_links_are_derived_from_content_in_both_directions() {
    let fakes = Fakes::new("routes-links");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    let links = |id: &str| {
        let mut response = common::get(&format!("{base}/api/todo/{id}"));
        assert_eq!(common::status(&response), 200, "{id}");
        common::body_json(&mut response)["links"].clone()
    };
    let ids = |side: &serde_json::Value| -> Vec<String> {
        side.as_array()
            .expect("a side")
            .iter()
            .map(|n| n["todo"].as_str().expect("an id").to_owned())
            .collect()
    };

    let packing = links("pack-bags");
    assert_eq!(ids(&packing["out"]), vec!["book-flight"]);
    assert!(ids(&packing["in"]).is_empty());
    let flight = links("book-flight");
    assert_eq!(ids(&flight["in"]), vec!["pack-bags"]);
    assert!(ids(&flight["out"]).is_empty());
    let neighbour = &packing["out"][0];
    assert_eq!(neighbour["state"], "open");
    assert!(neighbour["value"].is_number());
    assert!(neighbour["unconfirmed"].is_boolean());
}

/// Every todo under `node`, checking on the way that each is `Ref(todo)` and
/// that every level ascends.
fn todos(node: &serde_json::Value, out: &mut Vec<String>) {
    if node["kind"] == "todo" {
        assert_eq!(
            node["term"],
            format!("Ref(todo='{}')", node["todo"].as_str().expect("an id"))
        );
        out.push(node["todo"].as_str().expect("an id").to_owned());
    }
    let values: Vec<f64> = node["children"]
        .as_array()
        .expect("children")
        .iter()
        .map(|child| child["worth"]["value"].as_f64().expect("a value"))
        .collect();
    assert!(values.is_sorted(), "children ascend: {values:?}");
    for child in node["children"].as_array().expect("children") {
        todos(child, out);
    }
}

/// The tree by category, by urgency and flat; an unknown `by` is a 400.
#[test]
fn the_tree_groups_the_open_todos_and_every_node_carries_its_term() {
    let fakes = Fakes::new("routes-tree");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    let tree = |query: &str| {
        let mut response = common::get(&format!("{base}/api/tree{query}"));
        assert_eq!(common::status(&response), 200, "{query}");
        common::body_json(&mut response)
    };
    let open = [
        "book-flight",
        "email-harrow",
        "file-permit",
        "pack-bags",
        "water-plants",
    ];

    let by_category = tree("");
    assert_eq!(by_category["by"], "category");
    assert_eq!(by_category["root"]["kind"], "group");
    assert_eq!(by_category["root"]["by"], "all");
    assert!(
        by_category["root"]["term"]
            .as_str()
            .expect("a print")
            .starts_with("Conj(")
    );
    for group in by_category["root"]["children"].as_array().expect("groups") {
        assert_eq!(group["by"], "category", "{group}");
    }
    let mut placed = Vec::new();
    todos(&by_category["root"], &mut placed);
    placed.sort();
    assert_eq!(placed, open);

    let by_urgency = tree("?by=urgency");
    for section in by_urgency["root"]["children"].as_array().expect("sections") {
        assert_eq!(section["by"], "urgency");
        assert!(
            ["Problem", "Watch", "Fine"].contains(&section["label"].as_str().expect("a label"))
        );
    }
    let flat = tree("?by=flat");
    let rows = flat["root"]["children"].as_array().expect("rows");
    assert_eq!(rows.len(), open.len());
    assert!(rows.iter().all(|row| row["kind"] == "todo"));
    let flat_term = flat["root"]["term"].as_str().expect("a print");
    assert!(
        open.iter()
            .all(|todo| flat_term.contains(&format!("Ref(todo='{todo}')"))),
        "the flat list is the Conj of every open todo: {flat_term}"
    );

    let refused = common::get(&format!("{base}/api/tree?by=colour"));
    assert_eq!(common::status(&refused), 400);
}

/// A push token appends nothing, so no fold would notice this gate failing.
#[test]
fn a_named_peer_registers_a_token_and_loopback_may_not() {
    let (_, fakes) = writer();
    let heads_before = {
        use suzatary_core::store::Prodrome;
        fakes.chain.tips().expect("derives")
    };
    let base = common::Server::new(
        std::sync::Arc::new(fakes),
        deployment([login("her@x.test")], [(login("her@x.test"), actor("ada"))]),
        Vec::new(),
    )
    .start()
    .base;
    let token = serde_json::json!({"platform": "android", "token": "a-token-from-the-phone"});
    let mut response = common::act(&format!("{base}/api/act/device"), &token);
    assert_eq!(common::status(&response), 200);
    let payload = common::body_json(&mut response);
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["devices"], 1);

    let mut tip = common::get(&format!("{base}/api/tip"));
    assert_eq!(
        common::body_json(&mut tip)["heads"],
        serde_json::json!(
            heads_before
                .iter()
                .map(prodrome_core::event::Hash::as_str)
                .collect::<Vec<_>>()
        ),
        "registering a device must not append an object"
    );
}

#[test]
fn a_device_registration_from_loopback_is_refused() {
    let fakes = Fakes::new("routes-device-loopback");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([login("her@x.test")], [(login("her@x.test"), actor("ada"))]),
        common::loopback_is_this_box(),
    );
    let mut response = common::act(
        &format!("{base}/api/act/device"),
        &serde_json::json!({"platform": "android", "token": "t"}),
    );
    assert_eq!(common::status(&response), 403);
    assert_eq!(
        common::body_json(&mut response)["error"],
        "a write must come from a named tailnet peer; loopback and the local shell use the CLI"
    );
}

/// The Android app's report, as `Push.java` builds it (`platform`, `token`, `nonce`), is taken,
/// and its retry with the same nonce answers the same: one phone, whatever the count of reports.
#[test]
fn the_android_apps_report_and_its_retry_register_one_phone() {
    let (_, fakes) = writer();
    let base = common::Server::new(
        std::sync::Arc::new(fakes),
        deployment([login("her@x.test")], [(login("her@x.test"), actor("ada"))]),
        Vec::new(),
    )
    .start()
    .base;
    let report = serde_json::json!({
        "platform": "android",
        "token": "a-token-from-the-phone",
        "nonce": "2b1f0c9e-7a8d-4e55-9c1a-3f6d2e8b4a10",
    });
    for _ in 0..2 {
        let mut response = common::act(&format!("{base}/api/act/device"), &report);
        assert_eq!(common::status(&response), 200);
        assert_eq!(
            common::body_json(&mut response),
            serde_json::json!({"ok": true, "devices": 1})
        );
    }
}

/// A token is one line and a platform one of two, since the file's format is lines.
#[test]
fn a_malformed_platform_or_token_is_refused_before_it_is_written() {
    let (_, fakes) = writer();
    let base = common::Server::new(
        std::sync::Arc::new(fakes),
        deployment([login("her@x.test")], [(login("her@x.test"), actor("ada"))]),
        Vec::new(),
    )
    .start()
    .base;
    for (body, why) in [
        (
            serde_json::json!({"platform": "windows", "token": "t"}),
            "unknown variant `windows`, expected `android` or `ios`",
        ),
        (
            serde_json::json!({"platform": "android", "token": "t", "app": "x"}),
            "unknown field `app`, expected one of `platform`, `token`, `nonce`",
        ),
        (
            serde_json::json!({"platform": "android", "token": ""}),
            "token must be one non-empty line",
        ),
        (
            serde_json::json!({"platform": "android", "token": "two\tfields"}),
            "token must be one non-empty line",
        ),
    ] {
        let mut response = common::act(&format!("{base}/api/act/device"), &body);
        assert_eq!(common::status(&response), 400, "{body}");
        assert_eq!(common::body_json(&mut response)["error"], why);
    }
}

/// `GET /api/usage` answers two windows with the ceiling absent rather than invented.
#[test]
fn usage_answers_two_windows_and_never_a_price() {
    let fakes = Fakes::new("routes-usage");
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    let mut response = common::get(&format!("{base}/api/usage"));
    assert_eq!(common::status(&response), 200);
    let body = common::body_json(&mut response);
    assert_eq!(body["five_hour"]["window"], "five_hour");
    assert_eq!(body["seven_day"]["window"], "seven_day");
    assert_eq!(body["seen_requests"], 0);
    assert_eq!(
        body["seen_since"],
        serde_json::Value::Null,
        "the fake read everything"
    );
    // Assert the `kind` tag, since a missing key indexes as `Null` and would pass silently.
    assert_eq!(
        body["five_hour"]["allowance"]["kind"], "unknown",
        "the fake never reads the endpoint, so this window has no allowance opinion"
    );
    assert_eq!(
        body["five_hour"]["most_seen_before_refusal"],
        serde_json::Value::Null,
        "nothing was ever refused"
    );
    assert_eq!(
        body["five_hour"]["on_credits"],
        serde_json::Value::Null,
        "no refusal, no credits"
    );
    let text = body.to_string();
    for forbidden in ["$", "usd", "USD", "price", "cost"] {
        assert!(
            !text.contains(forbidden),
            "a price-shaped word on the wire: {forbidden:?}"
        );
    }
}

/// A read's tag is a digest of its bytes: the same answer asked with its tag is a 304 with no
/// body, and an answer the chain moved under carries a new tag and its new bytes.
#[test]
fn a_read_answers_304_to_its_own_tag_and_a_new_tag_once_the_chain_moves() {
    let root = common::scratch("routes-etag");
    let events = common::six_todo_chain(&root);
    let base = common::start(
        std::sync::Arc::new(Fakes::with_store("routes-etag", events)),
        deployment([], []),
        common::loopback_is_this_box(),
    );
    let asked = |held: Option<&str>| {
        let request = common::agent().get(&format!("{base}/api/tip"));
        let request = match held {
            Some(tag) => request.header("if-none-match", tag),
            None => request,
        };
        request.call().expect("the request completes")
    };
    let tag_of = |response: &ureq::http::Response<ureq::Body>| {
        response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .expect("a read carries a tag")
            .to_owned()
    };

    let mut first = asked(None);
    assert_eq!(common::status(&first), 200);
    let tag = tag_of(&first);
    assert!(
        tag.starts_with("W/\""),
        "weak, so it holds under every coding: {tag}"
    );
    let was = common::body_json(&mut first);

    let mut same = asked(Some(&tag));
    assert_eq!(common::status(&same), 304);
    assert_eq!(tag_of(&same), tag);
    assert_eq!(
        same.headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store"),
        "a 304 says what the 200 would"
    );
    assert!(
        same.body_mut()
            .read_to_vec()
            .expect("an empty body")
            .is_empty()
    );

    let events = suzatary_core::record::EventStore::new(
        root.join("events"),
        prodrome_core::policy::Untrusted::of([actor("triage")]),
    );
    suzatary_core::store::append_to(
        &events,
        suzatary_core::record::GENESIS,
        prodrome_core::event::mk_created("a-new-todo", common::now(), "ada", "A new todo", "")
            .expect("valid"),
    )
    .expect("appends");
    let mut after = asked(Some(&tag));
    assert_eq!(
        common::status(&after),
        200,
        "the bytes moved, so the tag did"
    );
    assert_ne!(tag_of(&after), tag);
    assert_ne!(common::body_json(&mut after), was);
}
