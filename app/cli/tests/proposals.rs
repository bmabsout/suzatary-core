//! `suzatary proposals carry`, the migration that carries the triage agent's open batches into
//! the Inbox as its cards, run by the binary over a fresh `suzatary init` instance.

mod common;

use clap::Parser;
use prodrome_core::event::{Hash, mk_completed};
use suzatary_cli::commands::init;
use suzatary_cli::instance::Instance;
use suzatary_core::record::TodoEvent;
use suzatary_core::secretary::changes::Act;
use suzatary_core::secretary::proposer::Proposer;
use suzatary_core::secretary::store::Proposal;
use suzatary_core::store::Prodrome as _;

#[derive(Debug, Parser)]
struct Wrapper {
    #[command(flatten)]
    args: init::Args,
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "suzatary-cli-carry-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp dir");
    dir
}

/// A fresh instance, as `suzatary init` writes one.
fn fresh_instance(name: &str) -> Instance {
    let dir = temp_dir(name);
    let path = dir.join("instance.toml");
    let repo = dir.to_str().expect("a utf-8 temp path");
    let args = Wrapper::parse_from([
        "carry-test",
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
    ])
    .args;
    let env = suzatary_cli::paths::Env::under(dir.join("home"), dir.join("run"));
    init::run_keyed(&path, args, &env, |_| {
        Ok(suzatary_core::signatures::BoxKeys::of(
            &suzatary_core::cast::keys::root(),
            &suzatary_core::capability::Secret::new(suzatary_core::cast::keys::ROOT),
            &suzatary_core::capability::Secret::new(suzatary_core::cast::keys::AGENT),
        )?)
    })
    .expect("a fresh init");
    Instance::read(&path, &env).expect("it reads back")
}

/// `todo` as the triage agent authored it, at `percent`.
fn authored(todo: &str, body: &str, percent: u32) -> TodoEvent {
    suzatary_core::authoring::build_todo(
        &suzatary_core::authoring::NewTodo {
            todo: todo.to_owned(),
            fields: suzatary_core::todo::TodoFields {
                body: body.to_owned(),
                rationale: "an errand with a recoverable cost".to_owned(),
                kind: "todo".to_owned(),
                category: "home".to_owned(),
                price: suzatary_core::todo::Price::Flat { percent },
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

/// `suzatary proposals carry` with `extra`, its stdout, and whether it exited zero.
fn carry(instance: &Instance, extra: &[&str]) -> (String, bool) {
    let out = crate::binary::keyed()
        .args([
            "--instance",
            &instance.data().join("instance.toml").to_string_lossy(),
            "proposals",
            "carry",
        ])
        .args(extra)
        .output()
        .expect("the binary runs");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.success(),
    )
}

/// Every triage card the inbox holds: its acts' words.
fn cards(instance: &Instance) -> Vec<Vec<&'static str>> {
    suzatary_core::secretary::inbox::fold(&instance.inbox_store())
        .expect("the inbox reads")
        .objects()
        .filter(|object| object.asked.proposer == Proposer::Triage)
        .map(|object| match &object.asked.proposal {
            Proposal::Changes(changeset) => changeset.acts().iter().map(Act::word).collect(),
            other => panic!("a triage card is a changeset, not {other:?}"),
        })
        .collect()
}

/// The open decision a review wrote for a held batch, naming its head.
fn held(chain: &suzatary_core::chains::ChainName, head: &Hash) -> TodoEvent {
    suzatary_core::authoring::build_decision(
        &suzatary_core::authoring::NewDecision {
            body: "Settle the triage batch".to_owned(),
            why: "it is yours to say".to_owned(),
            performs: suzatary_core::reference::Reference::Batch {
                chain: chain.clone(),
                head: suzatary_core::reference::BatchHead::new(head.as_str()).expect("a head"),
            },
            detail: "it is yours to say".to_owned(),
            priority: None,
            category: String::new(),
            source: String::new(),
        },
        suzatary_core::zone::Moment {
            at: common::now(),
            zone: suzatary_core::zone::Zone::UTC,
        },
        "assistant",
        &[],
        &std::collections::BTreeSet::new(),
    )
    .expect("a decision")
}

/// A MIGRATION, PROVEN ON A COPY: a dry run writes nothing; `--apply` files one triage card per
/// open batch with the acts the batch's events write, completes the decision that held it, and
/// drops its objects from the replica; a second `--apply` finds nothing and writes nothing.
#[test]
fn open_batches_become_triage_cards_once() {
    let instance = fresh_instance("once");
    let chain = instance.chains.first();
    let target = suzatary_io::objects::Objects::chain(suzatary_core::record::EventStore::new(
        chain.events.clone(),
        chain.policy.untrusted.clone(),
    ));
    target
        .append(authored("deedcopy", "Ask for the deed copy", 70))
        .expect("the owner's todo");
    let replica = suzatary_io::proposals::Proposing::open(
        suzatary_io::proposals::Replica::new(
            chain.proposals.clone(),
            chain.policy.untrusted.clone(),
        ),
        suzatary_core::record::EventStore::new(
            chain.events.clone(),
            chain.policy.untrusted.clone(),
        ),
    )
    .expect("the replica adopts the chain");
    let added: Hash = replica
        .append(authored("parkingpermit", "Renew the parking permit", 75))
        .expect("the agent proposes");
    replica
        .append(
            mk_completed("parkingpermit", common::now(), "triage", "renewed").expect("an event"),
        )
        .expect("the agent proposes");
    replica
        .append(authored(
            "deedcopy",
            "Ask the registry for the deed copy",
            70,
        ))
        .expect("the agent rewords");
    // The decision a review wrote for the first batch.
    let decision = held(&chain.policy.name, &added);
    let decided = decision.todo().as_str().to_owned();
    target.append(decision).expect("the decision");
    let replica_before = replica.objects().expect("reads").len();

    let (said, ok) = carry(&instance, &[]);
    assert!(ok, "{said}");
    assert!(
        said.contains("parkingpermit: add, complete as one card"),
        "{said}"
    );
    assert!(said.contains("deedcopy: update as one card"), "{said}");
    assert!(cards(&instance).is_empty(), "a dry run files nothing");
    assert_eq!(replica.objects().expect("reads").len(), replica_before);

    let (said, ok) = carry(&instance, &["--apply"]);
    assert!(ok, "{said}");
    let mut filed = cards(&instance);
    filed.sort();
    assert_eq!(filed, vec![vec!["add", "complete"], vec!["update"]]);
    let chain_names: std::collections::BTreeSet<Hash> = target
        .objects()
        .expect("reads")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(
        replica
            .objects()
            .expect("reads")
            .iter()
            .all(|(name, _)| chain_names.contains(name)),
        "the replica holds nothing the chain does not"
    );
    let closed = suzatary_core::view::events_of(&target.nodes().expect("reads"))
        .into_iter()
        .any(|event| {
            matches!(&event, TodoEvent::Completed(life) if event.todo().as_str() == decided && life.note.starts_with("carried to the Inbox as proposal:triage/"))
        });
    assert!(
        closed,
        "the decision that held the batch is completed: {said}"
    );

    let objects = instance.inbox_store().objects().expect("reads").len();
    let (said, ok) = carry(&instance, &["--apply"]);
    assert!(ok, "{said}");
    assert!(said.contains("no open batch"), "{said}");
    assert_eq!(
        instance.inbox_store().objects().expect("reads").len(),
        objects,
        "a second run writes nothing"
    );
}
