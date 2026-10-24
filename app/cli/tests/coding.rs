//! A coding session from the phone, over a real server with a scripted model: the tap fires a
//! routine at a loopback fake of the documented endpoint, never the real one; the poll reads a
//! local repository's `claude/` branches; the hand-over writes one todo and merges nothing.

mod common;

use std::io::{BufRead, BufReader, Read as _, Write as _};
use std::sync::Arc;

use common::{FakeModel, Fakes};
use suzatary_cli::commands::coding::Firing;
use suzatary_core::cast::CAST;

const TOKEN: &str = "sk-ant-routine-fake";
const URL: &str = "https://claude.ai/code/session_01fake";

fn owner() -> suzatary_core::capability::Login {
    common::login("ada@example.com")
}

fn deployment() -> suzatary_core::deployment::Deployment {
    suzatary_core::deployment::Deployment {
        coding: Some(suzatary_core::coding::Coding {
            routine: "trig_01fake".parse().expect("a routine id"),
            repo: "bmabsout/cv".parse().expect("a repository"),
            chain: suzatary_core::chains::ChainName::new("life").expect("a chain"),
        }),
        ..common::deployment([owner()], [(owner(), common::actor(CAST.owner.actor))])
    }
}

/// The routine's token, and nothing else, by its one name.
struct Holding;

impl suzatary_io::secrets::Resolves for Holding {
    fn get(
        &self,
        name: &str,
        _reason: &str,
    ) -> Result<Option<suzatary_core::capability::Secret>, suzatary_io::secrets::SecretError> {
        Ok((name == suzatary_core::coding::ROUTINE_TOKEN)
            .then(|| suzatary_core::capability::Secret::new(TOKEN)))
    }
}

/// A loopback fake of the fire endpoint: answers one request with the
/// documented body and hands back the request line, headers and body.
fn fake_routine() -> (String, std::thread::JoinHandle<(Vec<String>, String)>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let served = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("a connection");
        let mut reader = BufReader::new(stream.try_clone().expect("clones"));
        let mut head = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("a line");
            if line.trim_end().is_empty() {
                break;
            }
            head.push(line.trim_end().to_owned());
        }
        let length: usize = head
            .iter()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|n| n.trim().parse().expect("a length"))
            })
            .expect("a length");
        let mut body = vec![0; length];
        reader.read_exact(&mut body).expect("the body");
        let answer = format!(
            r#"{{"claude_code_session_id":"session_01fake","claude_code_session_url":"{URL}"}}"#
        );
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
            answer.len()
        )
        .expect("answers");
        (head, String::from_utf8(body).expect("utf-8"))
    });
    (base, served)
}

const TASK: &str = "Retitle the CV to Principal investigator.\\nRun the gates.";

fn scene(tag: &str, firing: Firing) -> (String, Arc<Fakes>) {
    let fakes = Arc::new(
        Fakes::new(tag)
            .whois(common::FakeWhois::new().naming(common::v4(), owner()))
            .model(FakeModel::saying([
                format!(
                    r#"{{"say": "A session would do it.", "calls": [{{"verb": "propose_coding_session", "args": {{
                        "task": "{TASK}",
                        "why": "The title is out of date and the change needs the gates.",
                        "sources": "todo:life/permitfiling"
                    }}}}]}}"#
                )
                .leak(),
                r#"{"say": "Proposed."}"#,
            ])),
    );
    let server = common::Server::new(Arc::clone(&fakes), deployment(), Vec::new())
        .real_acts(
            tag,
            "her@study.test",
            common::again(&fakes.conversations),
            common::ScriptedOauth::new(Vec::new()),
        )
        .firing(firing);
    (server.start().base, fakes)
}

fn proposed(base: &str) -> (String, serde_json::Value) {
    let mut opened = common::act(
        &format!("{base}/api/secretary/conversations"),
        &serde_json::json!({"title": "The CV"}),
    );
    let id = common::body_json(&mut opened)["id"]
        .as_str()
        .expect("an id")
        .to_owned();
    let said = common::act(
        &format!("{base}/api/secretary/conversations/{id}/say"),
        &serde_json::json!({ "say": "retitle my cv properly" }),
    );
    assert_eq!(common::status(&said), 202);
    for _ in 0..200 {
        let payload = conversation(base, &id);
        if payload["state"] == "idle" {
            return (id, payload);
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("the exchange never finished");
}

fn conversation(base: &str, id: &str) -> serde_json::Value {
    common::reading(base, id, &[])
}

fn answer(base: &str, id: &str, verb: &str, body: &serde_json::Value) -> (u16, serde_json::Value) {
    let mut response = common::act(
        &format!("{base}/api/secretary/conversations/{id}/proposals/0/{verb}"),
        body,
    );
    (common::status(&response), common::body_json(&mut response))
}

/// A task is the whole of what a session is told, so a round that read mail proposes none: the
/// verb refuses and says why, and no card is filed.
#[test]
fn a_round_that_read_mail_proposes_no_session() {
    let fakes = Arc::new(
        Fakes::new("coding-tainted")
            .whois(common::FakeWhois::new().naming(common::v4(), owner()))
            .model(FakeModel::saying([
                r#"{"say": "", "calls": [{"verb": "search", "args": {"query": "cv"}}]}"#,
                format!(
                    r#"{{"say": "", "calls": [{{"verb": "propose_coding_session", "args": {{
                        "task": "{TASK}",
                        "why": "A mail asked for it.",
                        "sources": "mail:a1b2c3d4e5f6"
                    }}}}]}}"#
                )
                .leak(),
                r#"{"say": "Ask me again on its own."}"#,
            ])),
    );
    let base = common::Server::new(Arc::clone(&fakes), deployment(), Vec::new())
        .start()
        .base;
    let (_, payload) = proposed(&base);
    assert!(
        payload["proposals"]
            .as_array()
            .expect("proposals")
            .is_empty(),
        "{payload}"
    );
    let refused = payload["turns"]
        .as_array()
        .expect("turns")
        .iter()
        .find(|turn| turn["who"] == "verb" && turn["verb"] == "propose_coding_session")
        .expect("the verb's turn");
    assert_eq!(refused["outcome"]["is"], "refused", "{refused}");
    assert!(
        refused["text"]
            .as_str()
            .expect("text")
            .contains("not the owner's own words"),
        "{refused}"
    );
}

/// The tap fires the routine with the stored task and nothing else, and the
/// session it answers is the proposal's, as `fired`.
#[test]
fn a_tapped_session_fires_the_routine_with_the_task_alone() {
    let (routine, served) = fake_routine();
    let (base, _fakes) = scene(
        "coding-fire",
        Firing::Endpoint {
            base: routine,
            routine: "trig_01fake".parse().expect("a routine id"),
            secrets: Arc::new(Holding),
        },
    );
    let (id, payload) = proposed(&base);
    let proposal = &payload["proposals"][0];
    assert_eq!(proposal["act"]["kind"], "coding_session", "{proposal}");
    assert_eq!(proposal["card"]["answers"][0]["verb"], "start");
    assert_eq!(
        proposal["coding"],
        serde_json::Value::Null,
        "not tapped yet"
    );

    let (status, after) = answer(&base, &id, "tap", &serde_json::json!({}));
    assert_eq!(status, 200, "{after}");
    let proposal = &after["proposals"][0];
    assert_eq!(proposal["state"], "acted", "{proposal}");
    assert_eq!(proposal["coding"]["state"], "fired");
    assert_eq!(proposal["coding"]["url"], URL);

    let (head, body) = served.join().expect("the fake ran");
    assert_eq!(
        head[0],
        "POST /v1/claude_code/routines/trig_01fake/fire HTTP/1.1"
    );
    let lower: Vec<String> = head.iter().map(|line| line.to_ascii_lowercase()).collect();
    assert!(
        lower.contains(&format!("authorization: bearer {TOKEN}")),
        "{head:?}"
    );
    assert!(
        lower.contains(&"anthropic-beta: experimental-cc-routine-2026-04-01".to_owned()),
        "{head:?}"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).expect("json"),
        serde_json::json!({ "text": TASK.replace("\\n", "\n") }),
        "the payload is the task and nothing else"
    );
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.test"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The whole life: fired, reported by the branch whose report names it,
/// handed over once as one todo, then merged; a drop of a merged session is
/// refused.
#[test]
fn a_session_is_reported_handed_over_once_and_merged() {
    let (routine, served) = fake_routine();
    let (base, fakes) = scene(
        "coding-life",
        Firing::Endpoint {
            base: routine,
            routine: "trig_01fake".parse().expect("a routine id"),
            secrets: Arc::new(Holding),
        },
    );
    let (id, _) = proposed(&base);
    assert_eq!(answer(&base, &id, "tap", &serde_json::json!({})).0, 200);
    served.join().expect("the fake ran");

    let root = common::scratch("coding-life-repos");
    let mirrors = common::local_repos(&root);
    let repo: suzatary_core::repos::RepoName = "bmabsout/cv".parse().expect("a name");
    let poll = |mirrors: &suzatary_io::repos::Mirrors| {
        suzatary_cli::commands::coding::poll(&fakes.conversations, mirrors, &repo).expect("polls")
    };
    assert!(poll(&mirrors).is_empty(), "no branch speaks for it yet");
    assert_eq!(
        answer(&base, &id, "hand", &serde_json::json!({})).0,
        409,
        "a fired session has nothing to hand over"
    );

    let work = root.join("cv-work");
    let remote = root.join("remotes/bmabsout/cv.git");
    let remote = remote.to_str().expect("a utf-8 path");
    git(&work, &["checkout", "-q", "-b", "claude/retitle"]);
    std::fs::create_dir_all(work.join("docs/reports")).expect("reports");
    std::fs::write(
        work.join("docs/reports/retitle.md"),
        format!("# Retitle\n\nSession: {URL}\n\nGates: cargo test green.\n"),
    )
    .expect("a report");
    std::fs::write(work.join("cv.typ"), "= Ada Quill\nPrincipal investigator\n").expect("cv");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "Retitle"]);
    git(&work, &["push", "-q", remote, "claude/retitle"]);
    let fresh = || common::local_mirrors(&root);
    let moved = poll(&fresh());
    assert_eq!(moved.len(), 1);
    let coding = conversation(&base, &id)["proposals"][0]["coding"].clone();
    assert_eq!(coding["state"], "reported", "{coding}");
    assert_eq!(coding["branch"], "claude/retitle");
    assert_eq!(coding["path"], "docs/reports/retitle.md");
    assert!(
        coding["report"]
            .as_str()
            .unwrap_or_default()
            .contains("Gates")
    );
    assert_eq!(
        coding["stat"],
        serde_json::json!([
            {"path": "cv.typ", "added": 1, "removed": 1},
            {"path": "docs/reports/retitle.md", "added": 5, "removed": 0},
        ])
    );
    assert_eq!(coding["handed"], "");

    let (status, handed) = answer(&base, &id, "hand", &serde_json::json!({}));
    assert_eq!(status, 200, "{handed}");
    let todo = format!(
        "todo:life/{}",
        suzatary_core::coding::handed_todo(
            &suzatary_core::secretary::store::ConversationId::new(id.clone()).expect("an id"),
            0
        )
    );
    assert_eq!(handed["proposals"][0]["coding"]["handed"], todo.as_str());
    handed_by_the_tapper(fakes.conversations.conversations(), &id);
    assert_eq!(
        answer(&base, &id, "hand", &serde_json::json!({})).0,
        409,
        "a second hand-over is the same todo, refused"
    );

    git(&work, &["checkout", "-q", "main"]);
    git(&work, &["merge", "-q", "--ff-only", "claude/retitle"]);
    git(&work, &["push", "-q", remote, "main"]);
    assert_eq!(poll(&fresh()).len(), 1);
    let coding = conversation(&base, &id)["proposals"][0]["coding"].clone();
    assert_eq!(coding["state"], "merged", "{coding}");
    assert_eq!(
        answer(&base, &id, "drop", &serde_json::json!({"reason": "late"})).0,
        409,
        "a merged session is final"
    );
}

/// A drop needs a reason, and the reason is what the session ends on.
#[test]
fn a_fired_session_is_dropped_with_a_reason() {
    let (routine, served) = fake_routine();
    let (base, _fakes) = scene(
        "coding-drop",
        Firing::Endpoint {
            base: routine,
            routine: "trig_01fake".parse().expect("a routine id"),
            secrets: Arc::new(Holding),
        },
    );
    let (id, _) = proposed(&base);
    assert_eq!(answer(&base, &id, "tap", &serde_json::json!({})).0, 200);
    served.join().expect("the fake ran");
    assert_eq!(
        answer(&base, &id, "drop", &serde_json::json!({"reason": " "})).0,
        400
    );
    let (status, dropped) = answer(
        &base,
        &id,
        "drop",
        &serde_json::json!({"reason": "done by hand"}),
    );
    assert_eq!(status, 200, "{dropped}");
    assert_eq!(dropped["proposals"][0]["coding"]["state"], "dropped");
    assert_eq!(dropped["proposals"][0]["coding"]["reason"], "done by hand");
}

/// The hand-over's line is the tapper's own, stamped by the actor who handed it, and carries
/// no word of the session's report: not its branch.
fn handed_by_the_tapper(
    conversations: suzatary_core::secretary::conversations::Conversations<'_>,
    id: &str,
) {
    let stored = suzatary_core::secretary::read(
        conversations,
        &suzatary_core::secretary::store::ConversationId::new(id).expect("an id"),
    )
    .expect("reads");
    let line = stored
        .turns
        .iter()
        .rev()
        .find(|said| matches!(said.turn, suzatary_core::capability::Turn::Act { .. }))
        .expect("the hand-over's line");
    let suzatary_core::capability::Turn::Act { text, .. } = &line.turn else {
        unreachable!("an act")
    };
    assert!(
        !text.contains("claude/retitle"),
        "the branch is the report's words, not the tapper's: {text}"
    );
    assert!(
        matches!(
            line.by,
            Some(suzatary_core::secretary::stamp::Stamp::Actor(_))
        ),
        "stamped by the actor who handed it over"
    );
}
