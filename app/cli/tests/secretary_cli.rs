//! The operator's `suzatary secretary` verbs over a real `Instance` (`suzatary init`'s own
//! output), answering proposals made by a round of the scripted model over the served app.

mod common;

use std::sync::Arc;

use clap::Parser;
use suzatary_cli::commands::init;
use suzatary_cli::instance::Instance;
use suzatary_core::secretary::responded::Responded;
use suzatary_core::secretary::store::ConversationId;

/// A thin wrapper so a test can build [`init::Args`] through `clap`'s own
/// parser, exactly `tests/init.rs`'s own `Wrapper`/`parse`.
#[derive(Debug, Parser)]
struct Wrapper {
    #[command(flatten)]
    args: init::Args,
}

fn parse(argv: &[&str]) -> init::Args {
    let mut full = vec!["suzatary-secretary-cli-test"];
    full.extend_from_slice(argv);
    Wrapper::parse_from(full).args
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "suzatary-cli-secretary-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp dir");
    dir
}

/// A fresh `Instance` over a temp directory: `ada` and the agents on every roster.
fn fresh_instance(name: &str) -> Instance {
    let dir = temp_dir(name);
    let instance_path = dir.join("instance.toml");
    let repo = dir.to_str().expect("a utf-8 temp path");
    let args = parse(&[
        "--name",
        "Ada",
        "--city",
        "London",
        "--zone",
        "Europe/London",
        "--box",
        "ada-box.tailnet.ts.net",
        "--login",
        "her@example.test",
        "--repo",
        repo,
    ]);
    init::run_keyed(&instance_path, args, &env_beside(&instance_path), |_| {
        Ok(suzatary_core::signatures::BoxKeys::of(
            &suzatary_core::cast::keys::root(),
            &suzatary_core::capability::Secret::new(suzatary_core::cast::keys::ROOT),
            &suzatary_core::capability::Secret::new(suzatary_core::cast::keys::AGENT),
        )?)
    })
    .expect("a fresh init");
    Instance::read(&instance_path, &env_beside(&instance_path)).expect("it reads back")
}

/// A round of the scripted model over the served app, writing into
/// `instance`'s own stores: one proposed email, in a conversation this
/// answers the id of.
fn proposed(instance: &Instance) -> ConversationId {
    let first = instance.chains.first();
    let chain = suzatary_io::objects::Objects::chain(suzatary_core::record::EventStore::new(
        first.events.clone(),
        first.policy.untrusted.clone(),
    ));
    let ada = common::login("ada@example.test");
    let fakes = common::Fakes::over(instance.data().to_path_buf(), chain)
        .whois(common::FakeWhois::new().naming(common::v4(), ada.clone()))
        .model(common::FakeModel::saying([
            r#"{"say": "Here is the reply.",
                "calls": [{"verb": "propose_email", "args": {
                  "account": "her@example.test", "to": "counsel@example.test",
                  "subject": "The reply", "body": "Here it is.",
                  "why": "Counsel asked for it and the last scan is done.",
                  "sources": "mail:a1b2c3d4e5f6"}}]}"#,
            r#"{"say": "Proposed."}"#,
        ]));
    let deployment = common::deployment([ada.clone()], [(ada, common::actor("ada"))]);
    let base = common::Server::new(Arc::new(fakes), deployment, Vec::new())
        .start()
        .base;
    let mut opened = common::act(
        &format!("{base}/api/secretary/conversations"),
        &serde_json::json!({"title": "Morning"}),
    );
    let id = common::body_json(&mut opened)["id"]
        .as_str()
        .expect("an id")
        .to_owned();
    let said = common::act(
        &format!("{base}/api/secretary/conversations/{id}/say"),
        &serde_json::json!({"say": "draft the reply to counsel"}),
    );
    assert_eq!(common::status(&said), 202);
    for _ in 0..200 {
        let payload = common::reading(&base, &id, &[]);
        if payload["state"] == "idle"
            && payload["proposals"]
                .as_array()
                .is_some_and(|p| !p.is_empty())
        {
            return ConversationId::new(id).expect("the id the box minted");
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("the round never proposed");
}

/// The CLI's own `dismiss` answers the proposal in the Inbox, as the route
/// does: a `Cancelled` with the owner's reason, and one act turn; a second
/// dismissal answers the same and writes nothing.
#[test]
fn the_cli_dismiss_cancels_the_proposal_the_route_would() {
    let instance = fresh_instance("dismiss");
    let id = proposed(&instance);
    let conversations = instance.conversation_stores();
    let inbox = suzatary_io::objects::Objects::inbox(instance.secretary.inbox.clone());
    let dismiss = || {
        suzatary_cli::commands::secretary::dismiss(
            &instance,
            id.as_str(),
            0,
            "wrong person, this goes to Mira",
            None,
        )
        .expect("the dismissal runs");
    };

    dismiss();
    let stored =
        suzatary_core::secretary::read(conversations.conversations(), &id).expect("it reads");
    let fold = suzatary_core::secretary::inbox::fold(&inbox).expect("the inbox reads");
    let standing = stored.standing(0, &fold).expect("proposal 0");
    assert!(
        matches!(
            standing.object.reading.shown().map(|answer| &answer.response),
            Some(Responded::Dismissed { reason }) if reason == "wrong person, this goes to Mira"
        ),
        "{standing:?}"
    );
    let turns = stored.turns.len();

    dismiss();
    let again =
        suzatary_core::secretary::read(conversations.conversations(), &id).expect("it reads");
    assert_eq!(
        again.turns.len(),
        turns,
        "a second dismissal writes nothing"
    );
    let fold = suzatary_core::secretary::inbox::fold(&inbox).expect("the inbox reads");
    assert_eq!(
        again
            .standing(0, &fold)
            .expect("proposal 0")
            .object
            .responses
            .len(),
        1
    );
}

/// `suzatary verify`, as the binary prints it.
fn verify(instance: &Instance) -> std::process::Output {
    crate::binary::keyed()
        .args([
            "--instance",
            &instance.data().join("instance.toml").to_string_lossy(),
            "verify",
        ])
        .output()
        .expect("the binary runs")
}

/// `suzatary verify` walks every store: one OK line per chain, per proposals
/// replica, for the inbox, the conversations, the memory and the decisions, and a damaged
/// conversation object is a finding and a failure, never silence.
#[test]
fn verify_walks_every_chain_replica_the_inbox_the_conversations_and_the_memory() {
    let instance = fresh_instance("verify");
    proposed(&instance);

    let clean = verify(&instance);
    let said = String::from_utf8_lossy(&clean.stdout);
    assert!(clean.status.success(), "{said}");
    let mut wanted: Vec<String> = instance
        .chains
        .iter()
        .flat_map(|chain| {
            [
                format!(
                    "{} chain OK — {}",
                    chain.policy.name,
                    chain.events.display()
                ),
                format!(
                    "{} proposals OK — {}",
                    chain.policy.name,
                    chain.proposals.display()
                ),
            ]
        })
        .collect();
    wanted.push(format!("inbox OK — {}", instance.secretary.inbox.display()));
    wanted.push(format!(
        "conversations OK — {}",
        instance.secretary.line_store().display()
    ));
    wanted.push(format!(
        "memory OK — {}",
        instance.secretary.memory.display()
    ));
    wanted.push(format!(
        "digests OK — {}",
        instance.secretary.digests.display()
    ));
    wanted.push(format!("decisions OK — {}", instance.decisions().display()));
    wanted.push(format!("requests OK — {}", instance.requests().display()));
    assert_eq!(said.lines().collect::<Vec<_>>(), wanted);

    let object = std::fs::read_dir(instance.secretary.line_store().join("objects"))
        .expect("the conversations hold objects")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|ext| ext == "py"))
        .expect("an object");
    let mut bytes = std::fs::read(&object).expect("an object reads");
    bytes.extend_from_slice(b"# tampered\n");
    std::fs::write(&object, bytes).expect("an object writes");

    let damaged = verify(&instance);
    assert!(
        !damaged.status.success(),
        "a damaged conversation fails verify"
    );
    let said = String::from_utf8_lossy(&damaged.stdout);
    assert!(!said.contains("conversations OK"), "{said}");
    let complained = String::from_utf8_lossy(&damaged.stderr);
    assert!(
        complained
            .lines()
            .any(|line| line.starts_with("conversations: ")),
        "{complained}"
    );
}

/// A proposal line whose inbox record this box does not hold is named everywhere it is read:
/// `secretary show` prints it in its place with why, and `verify` reports it and fails.
#[test]
fn show_and_verify_name_a_proposal_line_the_inbox_does_not_hold() {
    let instance = fresh_instance("unplaced");
    let id = proposed(&instance);
    // The inbox as a replica would hold it before the proposal's record arrived: empty.
    std::fs::remove_dir_all(&instance.secretary.inbox).expect("the inbox removes");
    std::fs::create_dir_all(&instance.secretary.inbox).expect("an empty inbox");

    let shown = crate::binary::keyed()
        .args([
            "--instance",
            &instance.data().join("instance.toml").to_string_lossy(),
            "secretary",
            "show",
            id.as_str(),
        ])
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&shown.stdout);
    assert!(
        shown.status.success(),
        "{said}{}",
        String::from_utf8_lossy(&shown.stderr)
    );
    assert!(
        said.contains("PROPOSAL 0 [UNPLACED — the inbox holds no record of it]"),
        "{said}"
    );

    let verified = verify(&instance);
    assert!(!verified.status.success(), "an unplaced line fails verify");
    let complained = String::from_utf8_lossy(&verified.stderr);
    assert!(
        complained.lines().any(|line| line
            == format!(
                "conversations: proposal:{id}/0 has no card: the inbox holds no record of it"
            )),
        "{complained}"
    );
}

/// Every directory the data push may stage is a store `suzatary verify` reads: the push rests
/// on `verify` alone, so a store it stages and nothing checks is one a damaged object could
/// reach the remote through. The directories are read from `safe-push.sh`'s own `ADD_PATHS`.
#[test]
fn every_directory_the_push_stages_is_a_store_verify_reads() {
    let instance = fresh_instance("verify-staged");
    let out = verify(&instance);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{said}");
    let read: Vec<std::path::PathBuf> = said
        .lines()
        .filter_map(|line| line.split_once(" OK — "))
        .map(|(_, root)| std::path::PathBuf::from(root))
        .collect();
    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/safe-push.sh"),
    )
    .expect("reads safe-push.sh");
    let staged = script
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("ADD_PATHS='"))
        .next_back()
        .and_then(|rest| rest.split_once('\''))
        .expect("ADD_PATHS is assigned in safe-push.sh")
        .0
        .to_owned();
    let directories: Vec<&str> = staged
        .split_whitespace()
        .filter_map(|path| path.strip_suffix('/'))
        .collect();
    assert!(directories.len() >= 10, "{directories:?}");
    for directory in directories {
        let root = instance.data().join(directory);
        assert!(
            read.contains(&root),
            "safe-push.sh stages {directory}/, which verify does not read: {said}"
        );
    }
}

/// An environment beside the instance file at `path`, so a test reads none of this process's.
fn env_beside(path: &std::path::Path) -> suzatary_cli::paths::Env {
    suzatary_cli::paths::Env::under(path.with_file_name("home"), path.with_file_name("run"))
}
