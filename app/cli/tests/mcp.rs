//! `suzatary mcp` driven as the agent drives it, a real process over stdio, with the write checked
//! on disk.
//!
//! It runs against a temp chain and a temp instance file, and calls no verb that reaches mail or
//! shells out.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use prodrome_core::event::{Actor, mk_created};
use prodrome_core::fpl;
use prodrome_core::literal::Datetime;
use suzatary_core::capability::Clock;
use suzatary_core::record::{EventStore, mk_authored};
use suzatary_core::store::Prodrome;

/// A scratch root unique to this process and thread.
fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "suzatary-mcp-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("a scratch root");
    root
}

fn at(day: u32) -> Datetime {
    Datetime::new(2026, 9, day, 9, 0, 0, 0).expect("a real instant")
}

/// One trusted todo the agent can talk about, on a chain of its own.
fn a_chain(root: &Path) -> EventStore {
    let untrusted = prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]);
    let store = EventStore::new(root.join("events"), untrusted);
    suzatary_core::store::append_to(
        &store,
        suzatary_core::record::GENESIS,
        mk_created("filething", at(1), "ada", "permit renewal", "").expect("valid"),
    )
    .expect("appends");
    suzatary_core::store::append_to(
        &store,
        suzatary_core::record::GENESIS,
        mk_authored(
            "filething",
            at(2),
            "ada",
            "todo",
            at(2),
            suzatary_core::record::MarkupSource::from_chain(r"File the \*permit\* renewal"),
            fpl::mk_flat(0.35).expect("a term"),
            vec!["Missing it is not recoverable.".to_owned()],
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
    store
}

/// An instance file naming that chain, and nothing this box depends on.
fn an_instance(root: &Path) -> PathBuf {
    let path = root.join("instance.toml");
    std::fs::write(
        &path,
        format!(
            r#"
repo = "{repo}"

[signatures]
root = "836884e1723c7b17f8b87e76a0272f012de6cff08f134eb9dcdb2670695b1818"

# TWO CHAINS (docs/ARCHITECTURE.md §10.2). `life` is the default — what a verb
# with no `chain` writes to — and `suzatary` is the one a verb has to NAME.
[[chains]]
name = "life"
events = "events"
proposals = "events-triage"
untrusted = ["triage"]
actors = ["assistant", "ada", "triage"]
owner = "ada"

[[chains]]
name = "suzatary"
events = "events-suzatary"
proposals = "events-triage-suzatary"
untrusted = ["triage"]
actors = ["assistant", "ada", "triage"]
owner = "ada"

[web]
viewers = ["someone@example.com"]
app_url = "https://example.invalid"

[web.act_as]
"someone@example.com" = "ada"

[web.serving]
host = "127.0.0.1"
port = 8788
tls_dir = "tls"
dist = "dist"
briefings = "briefings"

[categories]
work = 0
home = 3
other = 4

[mail]
root = "mail"
state = "state.json"
notmuch = "notmuch"
mbsyncrc = "mbsyncrc"
heartbeat = "triage-heartbeat"

[[mail.accounts]]
email = "someone@example.com"
subdir = "someone"
sync = "gmail"
"#,
            repo = root.display()
        ),
    )
    .expect("an instance file");
    path
}

/// The server, and the two pipes it speaks over.
struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Session {
    fn start(instance: &Path) -> Session {
        Session::started(crate::binary::keyed().args([
            "--instance",
            &instance.to_string_lossy(),
            "mcp",
        ]))
    }

    /// The agent's process as the triage pass starts it on a box that signs: its own key in the
    /// environment, and no secret store of the machine's.
    fn signing(instance: &Path, agent_seed: &str) -> Session {
        let isolated = instance
            .parent()
            .expect("an instance in a directory")
            .join("home");
        Session::started(
            crate::binary::suzatary()
                .args(["--instance", &instance.to_string_lossy(), "mcp"])
                .env(suzatary_core::signatures::AGENT_KEY, agent_seed)
                .env("HOME", &isolated)
                .env("XDG_CONFIG_HOME", isolated.join("config")),
        )
    }

    fn started(command: &mut Command) -> Session {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the binary runs");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Session {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    /// Send a request and read the answer with the matching id, skipping anything sent unprompted.
    fn ask(&mut self, method: &str, params: &serde_json::Value) -> serde_json::Value {
        self.next_id += 1;
        let id = self.next_id;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        writeln!(self.stdin, "{request}").expect("the pipe takes a request");
        self.stdin.flush().expect("flushes");
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("the pipe answers");
            assert!(
                read > 0,
                "the server closed stdout while answering {method}"
            );
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                panic!("the server wrote a non-JSON line on stdout: {line:?}");
            };
            if message.get("id") == Some(&serde_json::json!(id)) {
                assert!(
                    message.get("error").is_none(),
                    "{method} answered an error: {message}"
                );
                return message["result"].clone();
            }
        }
    }

    /// The handshake, and the server's own claim about itself.
    fn hello(&mut self) -> serde_json::Value {
        let hello = self.ask(
            "initialize",
            &serde_json::json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "the stdio test", "version": "0"},
            }),
        );
        self.notify("notifications/initialized");
        hello
    }

    fn notify(&mut self, method: &str) {
        let notification = serde_json::json!({"jsonrpc": "2.0", "method": method});
        writeln!(self.stdin, "{notification}").expect("the pipe takes a notification");
        self.stdin.flush().expect("flushes");
    }

    /// Call one verb and read its single text block.
    fn call(&mut self, name: &str, arguments: &serde_json::Value) -> String {
        let answered = self.ask(
            "tools/call",
            &serde_json::json!({"name": name, "arguments": arguments}),
        );
        assert_eq!(
            answered["isError"],
            serde_json::json!(false),
            "{name} refused: {answered}"
        );
        answered["content"][0]["text"]
            .as_str()
            .expect("one text block")
            .to_owned()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The batch a run left: one head in the replica beyond its chain, named `head`, resting only on
/// what the chain holds.
fn a_batch(
    root: &Path,
    head: &str,
    target: &EventStore,
) -> suzatary_io::objects::Objects<suzatary_core::record::TodoEvent> {
    let replica = suzatary_io::objects::Objects::chain(EventStore::new(
        root.join("events-triage"),
        prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]),
    ));
    let chain = target.dag().expect("the chain reads");
    // A write's signature rests on it, so a batch is named by the change it signs.
    let beyond: Vec<_> = replica
        .tips()
        .expect("derives")
        .into_iter()
        .filter(|name| chain.get(name).is_none())
        .map(
            |name| match replica.inner().load(&name).expect("an object") {
                prodrome_core::event::Envelope::Signed(signed) => signed.object,
                _ => name,
            },
        )
        .collect();
    assert_eq!(
        beyond.len(),
        1,
        "one write, one head beyond the chain: {beyond:?}"
    );
    assert_eq!(beyond[0].as_str(), head);
    let object = replica
        .inner()
        .load(&beyond[0])
        .expect("the head is an object");
    assert!(
        prodrome_core::event::parents_of(&object)
            .iter()
            .all(|parent| chain.get(parent).is_some()),
        "it rests on what the chain holds"
    );
    replica
}

/// One full conversation: initialize, list the verbs, read the chain, write, then read the disk.
#[test]
fn the_agent_speaks_to_a_real_server_and_its_write_lands_as_triage() {
    let root = scratch("session");
    let store = a_chain(&root);
    let instance = an_instance(&root);
    let mut session = Session::start(&instance);

    let hello = session.hello();
    assert_eq!(hello["serverInfo"]["name"], serde_json::json!("suzatary"));
    assert!(
        hello["capabilities"]["tools"].is_object(),
        "the server offers tools: {hello}"
    );

    let open: serde_json::Value =
        serde_json::from_str(&session.call("list_open_todos", &serde_json::json!({})))
            .expect("the open list is JSON");
    let open = open.as_array().expect("an array");
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0]["id"], serde_json::json!("filething"));
    assert_eq!(open[0]["authored_by"], serde_json::json!("ada"));
    assert_eq!(open[0]["priority"], serde_json::json!("35.00%"));
    // The body is plain text here: this row is read, not rendered.
    assert_eq!(
        open[0]["body"],
        serde_json::json!("File the *permit* renewal")
    );

    let before = store.tips().expect("derives");
    let recorded: serde_json::Value = serde_json::from_str(&session.call(
        "record",
        &serde_json::json!({"kind": "completed", "todo": "filething", "note": "filed it"}),
    ))
    .expect("the receipt is JSON");
    assert_eq!(recorded["kind"], serde_json::json!("completed"));
    assert_eq!(recorded["todo"], serde_json::json!("filething"));
    assert!(
        recorded["reminder"]
            .as_str()
            .expect("a reminder")
            .contains("PROVISIONAL"),
        "the receipt says what the write is worth: {recorded}"
    );
    let name = recorded["hash"].as_str().expect("a hash").to_owned();
    drop(session);

    // The agent's chain is a replica, so the write leaves `events/` alone and is a batch in
    // `events-triage/`.
    let after = suzatary_io::objects::Objects::chain(EventStore::new(
        root.join("events"),
        prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]),
    ));
    assert_eq!(
        after.inner().tips().expect("derives"),
        before,
        "the CHAIN must not have moved: an agent's write is a proposal"
    );
    assert!(
        !root
            .join("events")
            .join("objects")
            .join(format!("{name}.py"))
            .exists(),
        "and the object is not in the chain's store at all"
    );
    let stored = std::fs::read_to_string(
        root.join("events-triage")
            .join("objects")
            .join(format!("{name}.py")),
    )
    .expect("the object is a file named by its own hash, in the PROPOSAL store");
    assert!(
        stored.contains("actor='triage'"),
        "the agent's write is recorded as triage and nothing else: {stored}"
    );
    assert!(stored.contains("Completed("), "{stored}");
    assert!(stored.contains("note='filed it'"), "{stored}");

    let replica = a_batch(&root, &name, &store);

    // Fold at the writer's own clock: `record` stamps `at` with `Clock::now`, so a literal instant
    // would go stale.
    let now = suzatary_io::clock::SystemClock.now();
    let none = prodrome_core::policy::Untrusted::none();
    let believed = |nodes: Vec<suzatary_core::record::Node>| {
        suzatary_core::believed::Believed::of(&nodes, now, &none).expect("folds")
    };
    assert!(
        believed(after.nodes().expect("the chain reads"))
            .env
            .outcomes
            .is_empty(),
        "the chain's fold does not see it under ANY policy, because it is not on the chain"
    );
    assert_eq!(
        believed(replica.nodes().expect("the replica reads"))
            .env
            .outcomes
            .len(),
        1,
        "the batch holds the completion it says it does"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// `tended` is a kind `record` writes, and the todo stays open.
#[test]
fn a_pass_is_recorded_as_a_tending_and_closes_nothing() {
    let root = scratch("tended");
    let _chain = a_chain(&root);
    let instance = an_instance(&root);
    let mut session = Session::start(&instance);
    session.hello();
    let tended: serde_json::Value = serde_json::from_str(&session.call(
        "record",
        &serde_json::json!({"kind": "tended", "todo": "filething", "note": "did it again"}),
    ))
    .expect("the receipt is JSON");
    assert_eq!(tended["kind"], serde_json::json!("tended"), "{tended}");
    drop(session);

    let replica = suzatary_io::objects::Objects::chain(EventStore::new(
        root.join("events-triage"),
        prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]),
    ));
    let env = suzatary_core::believed::Believed::of(
        &replica.nodes().expect("the replica reads"),
        suzatary_io::clock::SystemClock.now(),
        &prodrome_core::policy::Untrusted::none(),
    )
    .expect("folds")
    .env;
    assert!(env.outcomes.is_empty(), "a pass closes nothing: {env:?}");
    let todo = prodrome_core::event::TodoId::new("filething").expect("an id");
    assert_eq!(
        env.tended
            .get(todo.as_str())
            .map(std::collections::BTreeSet::len),
        Some(1),
        "the batch holds the one pass"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// `chain` puts the write in that chain's proposal store and in none of the other three.
#[test]
fn a_chain_argument_writes_to_that_chain_and_no_other() {
    let root = scratch("chains");
    let life = a_chain(&root);
    // The project's chain is begun and holds nothing yet: a proposal writes into a prodrome its
    // chain began, never one of its own.
    let project_genesis = EventStore::new(
        root.join("events-suzatary"),
        prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]),
    )
    .init("todos")
    .expect("begins");
    let instance = an_instance(&root);
    let mut session = Session::start(&instance);
    session.hello();

    let life_before = life.tips().expect("derives");

    // An unserved chain name is a refusal that lists the real ones.
    let refused = session.ask(
        "tools/call",
        &serde_json::json!({
            "name": "list_open_todos",
            "arguments": {"chain": "nowhere"},
        }),
    );
    let said = refused.to_string();
    assert!(said.contains("no chain named"), "{said}");
    assert!(
        said.contains("suzatary"),
        "the refusal lists the chains: {said}"
    );

    let open: serde_json::Value = serde_json::from_str(
        &session.call("list_open_todos", &serde_json::json!({"chain": "suzatary"})),
    )
    .expect("the open list is JSON");
    assert_eq!(open.as_array().expect("an array").len(), 0, "{open}");

    let authored: serde_json::Value = serde_json::from_str(&session.call(
        "todo_add",
        &serde_json::json!({
            "todo_id": "flakyguards",
            "body": "Fix the flaky guards test",
            "rationale": "It fails one run in five and nobody trusts the gate.",
            "category": "other",
            "priority": "40%",
            "chain": "suzatary",
        }),
    ))
    .expect("the receipt is JSON");
    assert_eq!(authored["ok"], serde_json::json!(true), "{authored}");
    drop(session);

    let opened = |dir: &str| {
        suzatary_io::objects::Objects::chain(EventStore::new(
            root.join(dir),
            prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]),
        ))
    };
    let project = opened("events-triage-suzatary");
    let objects: Vec<_> = project
        .objects()
        .expect("the project's replica reads")
        .into_iter()
        .filter(|(name, _)| *name != project_genesis)
        // The agent's key signs each write it makes; the signature is beside the todo.
        .filter(|(_, object)| !matches!(object, prodrome_core::event::Envelope::Signed(_)))
        .collect();
    assert_eq!(objects.len(), 1, "exactly the todo this test authored");
    assert!(
        prodrome_core::event::canonical_envelope(&objects[0].1).contains("flakyguards"),
        "the object is the todo: {:?}",
        objects[0].0
    );
    assert_eq!(
        life.tips().expect("derives"),
        life_before,
        "Ada's own chain must not have moved"
    );
    assert!(
        opened("events-suzatary")
            .objects()
            .expect("the project chain reads")
            .iter()
            .map(|(name, _)| name)
            .eq([&project_genesis]),
        "and neither did the project's own chain — a proposal is not on it"
    );
    assert!(
        opened("events-triage")
            .objects()
            .expect("life's replica reads")
            .iter()
            .all(
                |(_, envelope)| !prodrome_core::event::canonical_envelope(envelope)
                    .contains("flakyguards")
            ),
        "and it is not in the OTHER chain's proposal store either"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// The names a served process actually lists match the versioned surface.
#[test]
fn the_wire_lists_the_verb_table_and_nothing_it_may_never_hold() {
    let root = scratch("surface");
    let _store = a_chain(&root);
    let instance = an_instance(&root);
    let mut session = Session::start(&instance);
    session.hello();

    let listed = session.ask("tools/list", &serde_json::json!({}));
    let tools = listed["tools"].as_array().expect("an array of tools");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(
        names.len(),
        suzatary_core::agent::TriageVerb::ALL.len(),
        "the wire and the sum disagree about how many verbs there are"
    );
    for expected in ["context", "list_open_todos", "todo_add", "record"] {
        assert!(
            names.contains(&expected),
            "{expected} is missing: {names:?}"
        );
    }
    // The names that may never appear, checked where a client would see them.
    for forbidden in ["confirm", "send", "reply"] {
        assert!(
            !names.iter().any(|name| name.contains(forbidden)),
            "{forbidden} reached the wire: {names:?}"
        );
    }

    // An unknown verb is a refusal the agent can read, not a protocol error.
    let answered = session.ask(
        "tools/call",
        &serde_json::json!({"name": "send_message", "arguments": {}}),
    );
    let shown = answered.to_string();
    assert!(
        shown.contains("send_message"),
        "the refusal names what was asked for: {shown}"
    );
    drop(session);
    let _ = std::fs::remove_dir_all(&root);
}

/// A subtodo is one act, the parent's spec gaining `Ref(child)`; an unknown parent is declined.
#[test]
fn a_subtodo_is_the_child_and_its_parents_revision_in_one_act() {
    let root = scratch("subtodo");
    let _chain = a_chain(&root);
    let instance = an_instance(&root);
    let mut session = Session::start(&instance);
    session.hello();

    let ask = |session: &mut Session, id: &str, parent: &str| -> serde_json::Value {
        serde_json::from_str(&session.call(
            "todo_add",
            &serde_json::json!({
                "todo_id": id,
                "body": "Scan the signed forms",
                "rationale": "The filing cannot go without them.",
                "priority": "45%",
                "parent": parent,
            }),
        ))
        .expect("the receipt is JSON")
    };
    let refused = ask(&mut session, "scanghost", "nosuchtodo");
    assert_eq!(refused["ok"], serde_json::json!(false), "{refused}");
    let authored = ask(&mut session, "scanforms", "filething");
    assert_eq!(authored["ok"], serde_json::json!(true), "{authored}");
    assert_eq!(authored["parent"]["todo_id"], "filething", "{authored}");
    drop(session);

    let replica = suzatary_io::objects::Objects::chain(EventStore::new(
        root.join("events-triage"),
        prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]),
    ));
    let prints: Vec<String> = replica
        .objects()
        .expect("the replica reads")
        .iter()
        .filter(|(_, object)| !matches!(object, prodrome_core::event::Envelope::Signed(_)))
        .map(|(_, envelope)| prodrome_core::event::canonical_envelope(envelope))
        .collect();
    assert_eq!(
        prints.len(),
        5,
        "the chain's genesis and two changes, then the act's two"
    );
    let child = prints
        .iter()
        .find(|print| print.contains("todo='scanforms'"))
        .expect("the child");
    assert!(child.contains("category='work'"), "{child}");
    assert!(
        prints
            .iter()
            .any(|print| print.contains("todo='filething'")
                && print.contains("Ref(todo='scanforms')")),
        "the parent's spec references the child: {prints:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A second session re-proposing a todo is told which batch it is already in and to use
/// `todo_update`.
#[test]
fn a_second_run_proposing_the_same_todo_is_refused_by_name_of_the_batch() {
    let root = scratch("standing-id");
    let chain = a_chain(&root);
    let instance = an_instance(&root);

    let ask = |session: &mut Session| -> serde_json::Value {
        serde_json::from_str(&session.call(
            "todo_add",
            &serde_json::json!({
                "todo_id": "permitrenewal",
                "body": "Renew the parking permit",
                "rationale": "An errand with a recoverable cost.",
                "category": "other",
                "priority": "75%",
            }),
        ))
        .expect("the receipt is JSON")
    };

    let mut first = Session::start(&instance);
    first.hello();
    let authored = ask(&mut first);
    assert_eq!(authored["ok"], serde_json::json!(true), "{authored}");
    drop(first);

    let replica = suzatary_io::objects::Objects::chain(EventStore::new(
        root.join("events-triage"),
        prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]),
    ));
    let on_chain = chain.tips().expect("derives");
    // A write's signature rests on it, so the batch is named by the change it signs.
    let heads: Vec<_> = replica
        .tips()
        .expect("derives")
        .into_iter()
        .filter(|head| !on_chain.contains(head))
        .map(
            |head| match replica.inner().load(&head).expect("the head is an object") {
                prodrome_core::event::Envelope::Signed(signed) => signed.object,
                _ => head,
            },
        )
        .collect();
    assert_eq!(heads.len(), 1, "one todo proposed, one batch: {heads:?}");
    let head: String = heads[0].as_str().chars().take(12).collect();
    let standing = replica.tips().expect("derives");

    let mut second = Session::start(&instance);
    second.hello();
    let refused = ask(&mut second);
    assert_eq!(refused["ok"], serde_json::json!(false), "{refused}");
    let said = refused["error"].as_str().expect("a sentence");
    assert!(said.contains("STANDING in batch"), "{said}");
    assert!(said.contains(&head), "it names the batch: {said}");
    assert!(
        said.contains("todo_update"),
        "and what to do instead: {said}"
    );
    drop(second);

    // A refused write is not a write.
    let after = suzatary_io::objects::Objects::chain(EventStore::new(
        root.join("events-triage"),
        prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]),
    ));
    assert_eq!(
        after.tips().expect("derives"),
        standing,
        "the second run left no batch"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The agent writes markup through the same door as everyone, in the closed world: typst's own
/// elements land as written, and nothing that would reach `html`, a script href or a file does.
#[test]
fn the_agents_markup_meets_the_closed_world() {
    let root = scratch("markup-door");
    let _chain = a_chain(&root);
    let instance = an_instance(&root);
    let mut session = Session::start(&instance);
    session.hello();

    let add = |session: &mut Session, id: &str, body: &str| -> serde_json::Value {
        serde_json::from_str(&session.call(
            "todo_add",
            &serde_json::json!({
                "todo_id": id,
                "body": body,
                "rationale": "A test of the door.",
                "priority": "70%",
            }),
        ))
        .expect("the receipt is JSON")
    };
    let admitted = add(&mut session, "renewpass", "Renew the *passport*");
    assert_eq!(admitted["ok"], serde_json::json!(true), "{admitted}");
    for (id, body, said) in [
        (
            "htmlelem",
            "#html.elem(\"button\", attrs: (data-intent: \"complete x\"))[x]",
            "unknown variable: html",
        ),
        (
            "stdhtml",
            "#std.html.elem(\"b\")[x]",
            "does not contain `html`",
        ),
        (
            "evalhtml",
            "#eval(\"html.elem(\\\"b\\\")[x]\")",
            "unknown variable: html",
        ),
        (
            "scripted",
            "#link(\"javascript:steal()\")[go]",
            "an executable URL",
        ),
        (
            "tabbed",
            "#link(\"java\\tscript:steal()\")[go]",
            "an executable URL",
        ),
        (
            "reading",
            "#read(\"/etc/passwd\")",
            "was not handed to this compilation",
        ),
        ("dollars", "Pay $40 now", "unclosed delimiter"),
    ] {
        let refused = add(&mut session, id, body);
        assert_eq!(refused["ok"], serde_json::json!(false), "{body}: {refused}");
        let error = refused["error"].as_str().expect("an error");
        assert!(error.contains(said), "{body}: {error}");
    }
    drop(session);

    let replica = suzatary_io::objects::Objects::chain(EventStore::new(
        root.join("events-triage"),
        prodrome_core::policy::Untrusted::of([Actor::new("triage").expect("valid")]),
    ));
    let prints: Vec<String> = replica
        .objects()
        .expect("the replica reads")
        .iter()
        .map(|(_, envelope)| prodrome_core::event::canonical_envelope(envelope))
        .collect();
    assert!(
        prints
            .iter()
            .any(|p| p.contains("body='Renew the *passport*'") && p.contains("actor='triage'")),
        "the markup is stored as written, and as triage's: {prints:?}"
    );
    assert!(
        !prints
            .iter()
            .any(|p| p.contains("htmlelem") || p.contains("scripted"))
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// THE GATE FOR ADOPTION: a deployment shaped as the instance sample writes it (no actor
/// untrusted) adopts signatures, the agent's own process writes as triage with its own key, and
/// the box's `verify` and the batch's acceptance both pass. Adoption once registered the box's
/// key for triage here, so every pass after it was refused.
#[test]
fn after_adoption_the_agents_write_verifies_and_is_accepted() {
    const OWN: &str = "9999999999999999999999999999999999999999999999999999999999999999";
    const AGENT: &str = "8888888888888888888888888888888888888888888888888888888888888888";
    let seeds = [
        (suzatary_core::signatures::BOX_KEY, OWN.to_owned()),
        (suzatary_core::signatures::AGENT_KEY, AGENT.to_owned()),
        (suzatary_core::mail::dedup_key::DEDUP_KEY, "7".repeat(64)),
    ];
    let root = scratch("adopted-agent");
    let chain = a_chain(&root);
    let instance = an_instance(&root);
    let root_key = prodrome_core::sign::Secret::new(OWN)
        .expect("a seed")
        .public();
    let text = std::fs::read_to_string(&instance).expect("reads");
    std::fs::write(
        &instance,
        text.replace("untrusted = [\"triage\"]", "untrusted = []")
            .replace(
                suzatary_core::cast::keys::root().as_str(),
                root_key.as_str(),
            ),
    )
    .expect("writes");
    let ran = |args: &[&str]| {
        let output = super::todos::signing(&instance, args, &seeds);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    ran(&["device", "init"]);

    let mut session = Session::signing(&instance, AGENT);
    session.hello();
    session.call(
        "record",
        &serde_json::json!({"kind": "tended", "todo": "filething", "note": "chased it"}),
    );
    drop(session);

    ran(&["verify"]);
    let held = chain.dag().expect("the chain reads");
    let replica = suzatary_io::objects::Objects::chain(EventStore::new(
        root.join("events-triage"),
        prodrome_core::policy::Untrusted::none(),
    ));
    let proposed: Vec<String> = replica
        .objects()
        .expect("the replica reads")
        .into_iter()
        .filter(|(name, object)| held.get(name).is_none() && object.event().is_some())
        .map(|(name, _)| name.as_str().to_owned())
        .collect();
    assert_eq!(proposed.len(), 1, "{proposed:?}");
    ran(&["proposals", "accept", &proposed[0]]);
    ran(&["verify"]);
    let _ = std::fs::remove_dir_all(&root);
}
