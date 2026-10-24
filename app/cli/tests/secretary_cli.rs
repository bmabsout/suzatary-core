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

/// One conversation of `kind` opened under `title`, its turns said in order, a minute apart.
fn conversation(
    instance: &Instance,
    id: &str,
    title: &str,
    kind: suzatary_core::secretary::store::Kind,
    turns: Vec<suzatary_core::capability::Turn>,
) {
    use suzatary_core::secretary::store::Entry;
    let stores = instance.conversation_stores();
    let id = ConversationId::new(id).expect("a slug");
    let at = |minute: u32| {
        prodrome_core::literal::Datetime::new(2026, 10, 20, 9, minute, 0, 0).expect("an instant")
    };
    let opened = Entry::Opened {
        at: at(0),
        title: suzatary_core::secretary::words::Words::kept(title.to_owned()),
        model: "scripted".to_owned(),
        kind,
    };
    suzatary_core::secretary::transcript::start(stores.conversations(), &id, &opened)
        .expect("opened");
    for (minute, turn) in (1..).zip(turns) {
        let said = Entry::Said {
            at: at(minute),
            turn,
            by: None,
        };
        suzatary_core::secretary::transcript::push(stores.conversations(), &id, &said)
            .expect("said");
    }
}

fn asked(text: &str) -> suzatary_core::capability::Turn {
    suzatary_core::capability::Turn::Person {
        text: text.to_owned(),
        references: Vec::new(),
        attachments: Vec::new(),
    }
}

fn answered(text: &str) -> suzatary_core::capability::Turn {
    suzatary_core::capability::Turn::Model {
        text: text.to_owned(),
        calls: Vec::new(),
    }
}

/// A `draft_note` call as the store holds it: the model's call, the note, the verb's answer.
fn drafted() -> [suzatary_core::capability::Turn; 3] {
    use suzatary_core::capability::{ToolCall, Turn};
    use suzatary_core::secretary::SecretaryVerb;
    let tool = |answer: &str| Turn::Tool {
        verb: SecretaryVerb::DraftNote.into(),
        answer: answer.to_owned(),
        outcome: suzatary_core::secretary::outcome::Outcome::Answered,
        files: Vec::new(),
    };
    [
        Turn::Model {
            text: String::new(),
            calls: vec![ToolCall {
                verb: SecretaryVerb::DraftNote.into(),
                args: std::collections::BTreeMap::new(),
            }],
        },
        tool("Packing\n\nPassport, charger."),
        tool(r#"{"drafted": "Packing"}"#),
    ]
}

/// Four conversations on a fresh instance: a topic with a note, a topic, a daily sweep and the
/// default.
fn carried(name: &str) -> Instance {
    use suzatary_core::secretary::store::Kind;
    let instance = fresh_instance(name);
    let [call, note, answer] = drafted();
    conversation(
        &instance,
        "2026-10-20-1",
        "Trip to the coast!",
        Kind::Topic,
        vec![
            asked("what should I pack?"),
            call,
            note,
            answer,
            answered("Noted."),
            asked("and the hotel?"),
            answered("Booked."),
        ],
    );
    conversation(
        &instance,
        "2026-10-20-2",
        "Hello",
        Kind::Topic,
        vec![asked("hello"), answered("Hello.")],
    );
    conversation(
        &instance,
        "2026-10-20-sweep",
        "Sweep",
        Kind::Review,
        vec![asked("sweep the open list"), answered("Nothing to close.")],
    );
    conversation(
        &instance,
        "default",
        "Secretary",
        Kind::Default,
        vec![asked("hi"), answered("Hi."), asked("bye"), answered("Bye.")],
    );
    instance
}

/// A carry map written under the test's scratch directory.
fn carry_map(name: &str, text: &str) -> std::path::PathBuf {
    let path = temp_dir(name).join("map.toml");
    std::fs::write(&path, text).expect("the map is written");
    path
}

/// The carry's count and its lines, sorted.
fn carrying(
    instance: &Instance,
    run: &suzatary_cli::commands::secretary::CarryRun,
) -> anyhow::Result<(usize, Vec<String>)> {
    let mut said = Vec::new();
    let count = suzatary_cli::commands::secretary::carry(instance, run, &mut said)?;
    let mut lines: Vec<String> = String::from_utf8(said)
        .expect("utf-8")
        .lines()
        .map(str::to_owned)
        .collect();
    lines.sort();
    Ok((count, lines))
}

/// How many objects the membership store holds.
fn held(instance: &Instance) -> usize {
    use suzatary_core::store::Prodrome as _;
    instance
        .secretary
        .topics
        .open()
        .memberships
        .objects()
        .expect("reads")
        .len()
}

/// The listing names each conversation with its date, rounds and title, and a map with any
/// problem is refused whole, listing every problem, and writes nothing.
#[test]
fn the_carry_lists_the_conversations_and_refuses_a_wrong_map_whole() {
    use suzatary_cli::commands::secretary::CarryRun;
    let instance = carried("carry-refused");
    let begun = held(&instance);
    let (_, listed) = carrying(&instance, &CarryRun::List).expect("the listing runs");
    assert_eq!(
        listed,
        vec![
            "2026-10-20-1  2026-10-20  2 round(s)  Trip to the coast!",
            "2026-10-20-2  2026-10-20  1 round(s)  Hello",
            "2026-10-20-sweep  2026-10-20  1 round(s)  Sweep",
            "default  2026-10-20  2 round(s)  Secretary",
        ]
    );

    let wrong = carry_map(
        "carry-wrong",
        "[\"2026-10-20-1\"]\ntopics = [\"trip\"]\n\
         [\"2026-10-21-9\"]\ntopics = [\"nowhere\"]\n\
         [\"2026-10-20-2\"]\ntopics = [\"a-much-too-long-topic\", \"greetings\", \"greetings\"]\n\
         [default]\ntopics = [\"a\", \"b\", \"c\", \"d\"]\n",
    );
    for refused in [CarryRun::Show(wrong.clone()), CarryRun::Write(wrong)] {
        let refusal = format!(
            "{:#}",
            carrying(&instance, &refused).expect_err("the map is refused")
        );
        for named in [
            "nothing was written",
            "2026-10-21-9 is not a conversation the store holds",
            "a topic is at most 3 words",
            "2026-10-20-2 names greetings twice",
            "default names 4 topic(s); name 1 to 3",
        ] {
            assert!(refusal.contains(named), "{named}: {refusal}");
        }
    }
    assert_eq!(held(&instance), begun, "a refused map writes nothing");
}

/// A dry run with a map says what each conversation gets and writes nothing. The run gives each
/// named conversation's rounds and note its topics and the others none, and a second run writes
/// nothing.
#[test]
fn the_carry_gives_the_conversations_a_map_names_their_topics_once() {
    use suzatary_cli::commands::secretary::CarryRun;
    use suzatary_core::secretary::topics::target::Target;
    use suzatary_core::secretary::topics::topic::Topic;
    let instance = carried("carry");
    let begun = held(&instance);
    let good = carry_map(
        "carry-good",
        "[\"2026-10-20-1\"]\ntopics = [\"trip\", \"coast\"]\n[default]\ntopics = [\"daily\"]\n",
    );
    let (count, said) =
        carrying(&instance, &CarryRun::Show(good.clone())).expect("the dry run runs");
    assert_eq!(count, 8);
    assert_eq!(
        said,
        vec![
            "2026-10-20-1  coast, trip  2 round(s) and 1 note(s), 6 membership(s) to write",
            "2026-10-20-2  (no topic)",
            "2026-10-20-sweep  (no topic)",
            "default  daily  2 round(s) and 0 note(s), 2 membership(s) to write",
            "would write 8 membership(s) as assistant; wrote nothing",
        ]
    );
    assert_eq!(held(&instance), begun, "a dry run writes nothing");

    assert_eq!(
        carrying(&instance, &CarryRun::Write(good.clone()))
            .expect("the carry runs")
            .0,
        8
    );
    let reading = instance
        .secretary
        .topics
        .open()
        .stores()
        .reading()
        .expect("reads");
    let target = |text: &str| text.parse::<Target>().expect("a target");
    let topic = |names: &[&str]| {
        names
            .iter()
            .map(|name| Topic::new(*name).expect("a topic"))
            .collect::<std::collections::BTreeSet<_>>()
    };
    for (on, filed) in [
        ("round:2026-10-20-1/0", &["trip", "coast"][..]),
        ("note:2026-10-20-1/2", &["trip", "coast"]),
        ("round:2026-10-20-1/5", &["trip", "coast"]),
        ("round:default/0", &["daily"]),
        ("round:default/2", &["daily"]),
    ] {
        assert_eq!(reading.topics(&target(on)), topic(filed), "{on}");
    }
    assert_eq!(
        reading.memberships().len(),
        5,
        "a conversation the map does not name gets no topic"
    );
    let written = held(&instance);

    let (again, said) = carrying(&instance, &CarryRun::Write(good)).expect("the carry runs again");
    assert_eq!(again, 0, "a second run writes nothing: {said:?}");
    assert_eq!(held(&instance), written);
}
