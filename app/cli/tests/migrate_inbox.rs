//! `suzatary secretary migrate-inbox` over a real `Instance` (`suzatary init`'s own output),
//! whose inbox holds every old form: a supersession and two withdrawals stored as dismissals, an
//! owner's no, and an asking that served a todo with a spec of its own, answered after. A dry run
//! writes nothing; a run carries each into the form the inbox reads, seals again what rested on
//! it and verifies; a second run writes nothing; a run stopped after writing is finished by the
//! next; and a server of this build does not start before it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use clap::Parser;
use prodrome_core::event::{Actor, Hash};
use prodrome_core::literal::Datetime;
use sha2::{Digest as _, Sha256};
use suzatary_cli::commands::{init, migrate_inbox};
use suzatary_cli::instance::Instance;
use suzatary_core::reference::ProposalRef;
use suzatary_core::secretary::inbox::{self, Asked, Price, ProposalId};
use suzatary_core::secretary::proposer::Proposer;
use suzatary_core::secretary::responded::{Responded, Withdrawal};
use suzatary_core::secretary::store::{ConversationId, Draft, Proposal, Reasons};

#[derive(Debug, Parser)]
struct Wrapper {
    #[command(flatten)]
    args: init::Args,
}

fn fresh_instance(name: &str) -> Instance {
    let dir = std::env::temp_dir().join(format!(
        "suzatary-cli-migrate-inbox-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let instance_path = dir.join("instance.toml");
    let repo = dir.to_str().expect("a utf-8 temp path");
    let args = Wrapper::parse_from([
        "suzatary-migrate-test",
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
    let env = suzatary_cli::paths::Env::under(
        instance_path.with_file_name("home"),
        instance_path.with_file_name("run"),
    );
    init::run_keyed(&instance_path, args, &env, |_| {
        Ok(suzatary_core::signatures::BoxKeys::of(
            &suzatary_core::cast::keys::root(),
            &suzatary_core::capability::Secret::new(suzatary_core::cast::keys::ROOT),
            &suzatary_core::capability::Secret::new(suzatary_core::cast::keys::AGENT),
        )?)
    })
    .expect("a fresh init");
    Instance::read(&instance_path, &env).expect("it reads back")
}

fn at(minute: u32) -> Datetime {
    Datetime::new(2026, 10, 1, 9, minute, 0, 0).expect("an instant")
}

fn actor(name: &str) -> Actor {
    Actor::new(name).expect("an actor")
}

fn asked(subject: &str, index: usize, price: Price) -> Asked {
    Asked {
        proposer: Proposer::Secretary,
        proposal: Proposal::Email(Draft {
            account: "her@example.test".to_owned(),
            to: vec!["mira@example.test".to_owned()],
            subject: subject.to_owned(),
            body: "Thursday works.".to_owned(),
            ..Draft::default()
        }),
        reasons: Reasons::new(
            "she asked for a day",
            vec!["mail:abcdef012345".parse().expect("a reference")],
        )
        .expect("reasons"),
        asked: ProposalRef {
            conversation: ConversationId::new("default").expect("a slug"),
            index,
        },
        price,
    }
}

/// Every object file of the inbox, by name, with its text.
fn files(root: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(root.join("objects"))
        .expect("objects")
        .map(|entry| {
            let path = entry.expect("an entry").path();
            (
                path.file_name()
                    .expect("a name")
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read_to_string(&path).expect("text"),
            )
        })
        .collect()
}

/// Write `text` as an object of the store at `root`, named by its bytes, as an older build wrote
/// it, past the reader this build asks.
fn put(root: &Path, text: &str) -> Hash {
    let name = Hash::new(hex::encode(Sha256::digest(text.as_bytes()))).expect("a name");
    std::fs::write(root.join("objects").join(format!("{name}.py")), text).expect("written");
    name
}

/// The ids of the five cards the old inbox holds, after it is written.
struct Old {
    superseded: ProposalId,
    replacing: ProposalId,
    withdrawn: ProposalId,
    owners: ProposalId,
    priced: ProposalId,
}

/// An inbox holding every old form.
fn old_inbox(instance: &Instance) -> (PathBuf, Old) {
    let store = instance.inbox_store();
    let root = store.inner().root().to_path_buf();
    let secretary = actor("assistant");
    let ask = |subject, index, minute| {
        inbox::ask(
            &store,
            asked(subject, index, Price::unpriced()),
            &secretary,
            at(minute),
        )
        .expect("asked")
    };
    let superseded = ask("first draft", 0, 0);
    let replacing = ask("second draft", 1, 1);
    let withdrawn = ask("a reminder", 2, 2);
    let owners = ask("a follow-up", 3, 3);
    // The dismissals an older build wrote for a withdrawal, which this build still reads.
    inbox::dismiss(
        &store,
        &superseded,
        &secretary,
        at(4),
        "superseded by proposal 1",
    )
    .expect("dismissed");
    inbox::dismiss(
        &store,
        &withdrawn,
        &secretary,
        at(5),
        "the box withdrew it: nothing was written: the file reads another provider",
    )
    .expect("dismissed");
    inbox::dismiss(&store, &owners, &actor("ada"), at(6), "not this week").expect("dismissed");
    // An asking that served a todo with a spec of its own, and the owner's no resting on it.
    let serving = asked(
        "the deed",
        4,
        Price::Serves("todo:life/reply".parse().expect("a todo")),
    );
    let priced = ProposalId::of(&serving, at(7));
    let genesis = std::fs::read_dir(root.join("objects"))
        .expect("objects")
        .find_map(|entry| {
            let path = entry.expect("an entry").path();
            std::fs::read_to_string(&path)
                .expect("text")
                .starts_with("Genesis(")
                .then(|| {
                    path.file_stem()
                        .expect("a stem")
                        .to_string_lossy()
                        .into_owned()
                })
        })
        .expect("the inbox began");
    let event = prodrome_core::literal::print_literal(&prodrome_core::schema::Schema::to_value(
        &suzatary_core::secretary::inbox_event::InboxEvent {
            proposal: priced.clone(),
            at: at(7),
            actor: secretary.clone(),
            said: suzatary_core::secretary::inbox_event::Said::Asked(Box::new(serving)),
        },
    ));
    let old_asking = event.replace("spec=None", "spec=Flat(value=0.4)");
    assert_ne!(old_asking, event);
    let asking = put(
        &root,
        &format!("Change(genesis='{genesis}', deps=(), event={old_asking})"),
    );
    put(
        &root,
        &format!(
            "Change(genesis='{genesis}', deps=('{asking}',), event=Dismissed(proposal='{priced}', \
             at=datetime(2026, 10, 1, 9, 8, 0), actor='ada', reason='not now'))"
        ),
    );
    (
        root,
        Old {
            superseded,
            replacing,
            withdrawn,
            owners,
            priced,
        },
    )
}

fn shown(fold: &inbox::Fold, id: &ProposalId) -> Option<Responded> {
    fold.get(id)?
        .reading
        .shown()
        .map(|answer| match &answer.response {
            Responded::Withdrawn(withdrawal) => Responded::Withdrawn(withdrawal.clone()),
            Responded::Dismissed { reason } => Responded::Dismissed {
                reason: reason.clone(),
            },
            other => panic!("{other}"),
        })
}

#[test]
fn migrate_inbox_carries_every_old_form_once() {
    let instance = fresh_instance("carries");
    let (root, old) = old_inbox(&instance);
    let unread = inbox::fold(&instance.inbox_store()).expect("the inbox reads");
    assert!(
        unread.get(&old.priced).is_none(),
        "this build's reader leaves the old asking out, and what rests on it"
    );
    assert!(!inbox::findings(&instance.inbox_store()).is_empty());
    let before = files(&root);

    migrate_inbox::run(&instance, true).expect("a dry run");
    assert_eq!(files(&root), before, "a dry run writes nothing");

    migrate_inbox::run(&instance, false).expect("a run");
    let store = instance.inbox_store();
    assert!(
        inbox::findings(&store).is_empty(),
        "{:?}",
        inbox::findings(&store)
    );
    let fold = inbox::fold(&store).expect("the inbox reads");
    assert_eq!(
        shown(&fold, &old.superseded),
        Some(Responded::Withdrawn(Withdrawal::Superseded))
    );
    assert_eq!(
        shown(&fold, &old.withdrawn),
        Some(Responded::Withdrawn(Withdrawal::NoLongerNeeded {
            why: "nothing was written: the file reads another provider".to_owned()
        }))
    );
    assert_eq!(
        shown(&fold, &old.owners),
        Some(Responded::Dismissed {
            reason: "not this week".to_owned()
        }),
        "the owner's no is kept"
    );
    assert_eq!(shown(&fold, &old.replacing), None, "it still stands");
    let priced = fold.get(&old.priced).expect("asked");
    assert_eq!(
        priced.asked.price,
        Price::Serves("todo:life/reply".parse().expect("a todo"))
    );
    assert_eq!(
        shown(&fold, &old.priced),
        Some(Responded::Dismissed {
            reason: "not now".to_owned()
        }),
        "what rested on the old asking rests on its image"
    );

    let carried = files(&root);
    migrate_inbox::run(&instance, false).expect("a second run");
    assert_eq!(files(&root), carried, "a second run writes nothing");
}

#[test]
fn a_run_stopped_after_writing_is_finished_by_the_next() {
    let instance = fresh_instance("stopped");
    let (root, old) = old_inbox(&instance);
    let history: Vec<(Hash, String)> = files(&root)
        .into_iter()
        .map(|(file, text)| {
            (
                Hash::new(file.trim_end_matches(".py")).expect("a name"),
                text,
            )
        })
        .collect();
    let plan = migrate_inbox::plan(&history, "assistant").expect("a plan");
    assert_eq!(plan.carried.len(), 3, "{:?}", plan.carried);
    for (name, text) in &plan.objects {
        std::fs::write(root.join("objects").join(format!("{name}.py")), text).expect("written");
    }
    migrate_inbox::run(&instance, false).expect("the next run");
    let store = instance.inbox_store();
    assert!(
        inbox::findings(&store).is_empty(),
        "{:?}",
        inbox::findings(&store)
    );
    let fold = inbox::fold(&store).expect("the inbox reads");
    assert_eq!(
        shown(&fold, &old.superseded),
        Some(Responded::Withdrawn(Withdrawal::Superseded))
    );
}

#[derive(Debug, Parser)]
struct Serve {
    #[command(flatten)]
    args: suzatary_cli::commands::serve::Args,
}

/// A server of this build over an inbox still in the old forms does not start, and says which
/// verb to run; once the run is done, it starts.
#[test]
fn a_server_over_an_uncarried_inbox_does_not_start() {
    let instance = fresh_instance("served");
    let (root, _) = old_inbox(&instance);
    let before = files(&root);
    let refused =
        suzatary_cli::commands::serve::serve(&instance, Serve::parse_from(["serve"]).args)
            .expect_err("it does not start");
    assert!(
        refused
            .to_string()
            .contains("run `suzatary secretary migrate-inbox` first"),
        "{refused}"
    );
    assert_eq!(files(&root), before, "the refusal writes nothing");
    migrate_inbox::run(&instance, false).expect("a run");
    migrate_inbox::uncarried(&instance).expect("nothing is left to carry");
    assert_starts(&instance);
}

/// The binary's `serve` over `instance` listens: it is still running and accepts a connection
/// within a minute. The server is stopped after.
fn assert_starts(instance: &Instance) {
    let home = instance.path.with_file_name("home");
    let runtime = instance.path.with_file_name("run");
    std::fs::create_dir_all(&runtime).expect("a runtime directory");
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|free| free.local_addr())
        .expect("a free loopback port")
        .port();
    // The gate answers only at the app's own port, so the scratch app lives at this one.
    let file = std::fs::read_to_string(&instance.path).expect("the instance file");
    let moved = file.replace(
        "app_url = \"https://ada-box.tailnet.ts.net\"",
        &format!("app_url = \"http://127.0.0.1:{port}\""),
    );
    assert_ne!(moved, file, "init names the box's app_url");
    std::fs::write(&instance.path, moved).expect("the instance file");
    let log = instance.path.with_file_name("serve.log");
    let mut server = crate::binary::keyed()
        .env("HOME", &home)
        .env("XDG_RUNTIME_DIR", &runtime)
        .args(["--instance", &instance.path.to_string_lossy()])
        .args(["serve", "--host", "127.0.0.1", "--port", &port.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::fs::File::create(&log).expect("a log file"))
        .spawn()
        .expect("the binary runs");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let listening = loop {
        if let Some(status) = server.try_wait().expect("asked") {
            break Err(format!("it exited {status}"));
        }
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break Ok(());
        }
        if std::time::Instant::now() > deadline {
            break Err("it never listened".to_owned());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let _ = server.kill();
    server.wait().expect("the server is reaped");
    if let Err(why) = listening {
        panic!(
            "a server over a carried inbox starts, but {why}: {}",
            std::fs::read_to_string(&log).unwrap_or_default()
        );
    }
}

/// A card the box withdrew on one side and the owner dismissed on the other, written apart, read
/// as one closed card before the run, and still does after it: the owner's no, with his words.
#[test]
fn a_card_closed_twice_apart_settles_as_the_owners_no() {
    let instance = fresh_instance("twice");
    let (root, old) = old_inbox(&instance);
    let (asking, genesis) =
        files(&root)
            .into_iter()
            .fold((None, None), |(asking, genesis), (file, text)| {
                let name = file.trim_end_matches(".py").to_owned();
                let asks = text.contains(&format!("event=Asked(proposal='{}'", old.withdrawn));
                let begins = text.starts_with("Genesis(");
                (
                    asking.or(asks.then(|| name.clone())),
                    genesis.or(begins.then_some(name)),
                )
            });
    let (asking, genesis) = (asking.expect("the asking"), genesis.expect("the genesis"));
    put(
        &root,
        &format!(
            "Change(genesis='{genesis}', deps=('{asking}',), event=Dismissed(proposal='{}', \
             at=datetime(2026, 10, 1, 9, 9, 0), actor='ada', reason='I wrote to her myself'))",
            old.withdrawn
        ),
    );
    migrate_inbox::run(&instance, false).expect("a run");
    let store = instance.inbox_store();
    assert!(
        inbox::findings(&store).is_empty(),
        "{:?}",
        inbox::findings(&store)
    );
    let fold = inbox::fold(&store).expect("the inbox reads");
    assert_eq!(
        shown(&fold, &old.withdrawn),
        Some(Responded::Dismissed {
            reason: "I wrote to her myself".to_owned()
        })
    );
    assert!(
        fold.one(&old.withdrawn).is_ok(),
        "no conflict is left to settle"
    );
}
