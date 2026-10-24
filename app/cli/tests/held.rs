//! A held value over a real server. Its reveal (`POST /api/held/reveal`) gives a trusted device of
//! an actor the value, once; an untrusted device, a viewer and a replay get nothing, and the box
//! keeps the value in no request record or kept answer. A proposed email names it by its
//! placeholder, and only the owner's tap writes the value into the message on the wire.

mod common;

use std::sync::Arc;

use common::{FakeCorpus, FakeMessage, FakeModel, Fakes};
use suzatary_core::capability::MailError;
use suzatary_core::held_value::{Handle, Reveal, Revealed, Shown};
use suzatary_core::redact::Kind;

/// A synthetic SSN no person holds.
const SSN: &str = "000-12-3456";

/// The message that holds the value, by the hash `show` takes and the notmuch id the fake corpus
/// answers it with.
const HASH: &str = "f0f0f0f0f0f0";
const MESSAGE: &str = "id-f0f0f0f0f0f0";

/// A box whose mail holds one value, at `MESSAGE`'s first place.
struct OneValue;

impl Reveal for OneValue {
    fn reveal(&self, handle: &Handle) -> Result<Revealed, MailError> {
        Ok(if handle.message() == MESSAGE && handle.ordinal() == 0 {
            Revealed::Value(Shown::new(Kind::Ssn, SSN))
        } else {
            Revealed::Gone
        })
    }
}

fn handle(message: &str, ordinal: u16) -> String {
    Handle::new(message, ordinal).expect("a handle").print()
}

/// A box over one value, its peer `login` mapped to the actor ada when `acts`.
fn serving(tag: &str, login: &str, acts: bool) -> common::Running {
    let login = common::login(login);
    let fakes = Arc::new(
        Fakes::new(tag).whois(common::FakeWhois::new().naming(common::v4(), login.clone())),
    );
    let actors = if acts {
        vec![(login.clone(), common::actor("ada"))]
    } else {
        Vec::new()
    };
    let deployment = common::deployment([login], actors);
    common::Server::new(fakes, deployment, Vec::new())
        .reveal(Arc::new(OneValue))
        .start()
}

fn answered(response: ureq::http::Response<ureq::Body>) -> (u16, String) {
    let status = common::status(&response);
    let mut response = response;
    let text = response.body_mut().read_to_string().expect("a body");
    (status, text)
}

/// Every file under `dir`, read, whether any holds `text`.
fn holds(dir: &std::path::Path, text: &str) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            let path = entry.path();
            if path.is_dir() {
                holds(&path, text)
            } else {
                std::fs::read(&path)
                    .is_ok_and(|bytes| bytes.windows(text.len()).any(|w| w == text.as_bytes()))
            }
        })
}

#[test]
fn a_trusted_device_reveals_the_value_once_and_the_box_keeps_none_of_it() {
    let running = serving("held-trusted", "ada@example.com", true);
    let url = format!("{}/api/held/reveal", running.base);
    let body = serde_json::json!({ "handle": handle(MESSAGE, 0) });

    let (status, text) = answered(common::act_signed(&url, &body, &common::phone()));
    assert_eq!(status, 200, "{text}");
    let shown: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(shown, serde_json::json!({ "type": "Shown", "value": SSN }));

    // The same request again is a replay: the box answers what it kept, which is not the value.
    let (status, text) = answered(common::act_signed(&url, &body, &common::phone()));
    assert_eq!((status, text.as_str()), (200, r#"{"type":"NotKept"}"#));

    let port = running.port();
    for dir in [
        common::requests_dir(port),
        std::env::temp_dir().join(format!("suzatary-test-answers-{port}")),
    ] {
        assert!(!holds(&dir, SSN), "{} holds the value", dir.display());
        assert!(!holds(&dir, "3456"), "{} holds its digits", dir.display());
    }

    let gone = serde_json::json!({ "handle": handle(MESSAGE, 1) });
    let (status, text) = answered(common::act(&url, &gone));
    assert_eq!((status, text.as_str()), (200, r#"{"type":"Gone"}"#));
}

#[test]
fn an_untrusted_device_a_viewer_and_a_malformed_handle_are_refused() {
    let running = serving("held-refused", "ada@example.com", true);
    let url = format!("{}/api/held/reveal", running.base);
    let body = serde_json::json!({ "handle": handle(MESSAGE, 0) });
    let stranger = prodrome_core::sign::Secret::new(&"7".repeat(64)).expect("a seed");
    let (status, text) = answered(common::act_signed(&url, &body, &stranger));
    assert_eq!(status, 403, "{text}");
    assert!(!text.contains(SSN), "{text}");
    let (status, text) = answered(common::act_unsigned(&url, &body));
    assert_ne!(status, 200, "{text}");
    assert!(!text.contains(SSN), "{text}");
    let (status, _) = answered(common::act(
        &url,
        &serde_json::json!({ "handle": "held:not base64.0" }),
    ));
    assert_eq!(status, 400);

    let viewing = serving("held-viewer", "bo@example.com", false);
    let url = format!("{}/api/held/reveal", viewing.base);
    let (status, text) = answered(common::act(&url, &body));
    assert_eq!(status, 403, "{text}");
    assert!(!text.contains(SSN), "{text}");
}

/// A model that reads the message, then proposes three emails naming a held value in the body (the
/// one the box holds, one at a place the message does not hold, and the held one under a forged
/// kind) and four the verb refuses (a withheld value in the body, a held value in the subject, a
/// code, and a value from a message the conversation did not read).
fn proposing() -> FakeModel {
    let held = |kind: &str, ordinal| {
        format!(
            "[{kind} ending 3456 · tap to show · {}]",
            handle(MESSAGE, ordinal)
        )
    };
    let email = |subject: &str, body: String| {
        serde_json::json!({"verb": "propose_email", "args": {
            "account": "her@example.invalid",
            "to": "counsel@example.test",
            "subject": subject,
            "body": body,
            "why": "Counsel asked for the number for the filing.",
            "sources": "mail:a1b2c3d4e5f6",
        }})
    };
    FakeModel::saying_owned([
        serde_json::json!({"say": "Here are the drafts.", "calls": [
            {"verb": "show", "args": {"id": HASH}},
            email("The filing", format!("My SSN is {}, for the form.", held("SSN", 0))),
            email("The filing again", format!("It is {}.", held("SSN", 1))),
            email("The filing, forged", format!("It is {}.", held("ITIN", 0))),
            email("Withheld", "It is [SSN ending 3456 · withheld].".to_owned()),
            email(&held("SSN", 0), "In the subject.".to_owned()),
            email("A code", format!(
                "It is [verification code · tap to show · {}].",
                handle(MESSAGE, 0)
            )),
            email("Unread", format!(
                "It is [SSN ending 3456 · tap to show · {}].",
                handle("other@example.test", 0)
            )),
        ]})
        .to_string(),
        r#"{"say": "Proposed."}"#.to_owned(),
    ])
}

/// The conversation once nobody is answering.
fn settled(base: &str, id: &str) -> serde_json::Value {
    for _ in 0..200 {
        let payload = common::reading(base, id, &[]);
        if payload["state"] == "idle" {
            return payload;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("the conversation never settled");
}

#[test]
fn a_held_value_reaches_only_the_wire_and_only_on_the_owners_tap() {
    let login = common::login("ada@example.com");
    let fakes = Arc::new(
        Fakes::new("held-send")
            .whois(common::FakeWhois::new().naming(common::v4(), login.clone()))
            .model(proposing())
            .corpus(FakeCorpus::new().with(FakeMessage {
                hash: HASH.to_owned(),
                thread: "t1".to_owned(),
                sender: "irs@example.test".to_owned(),
                subject: "Your form".to_owned(),
                body: "The form you sent.".to_owned(),
                timestamp: "2026-10-01T09:00:00".to_owned(),
            })),
    );
    let deployment = common::deployment([login.clone()], [(login, common::actor("ada"))]);
    let running = common::Server::new(Arc::clone(&fakes), deployment, Vec::new())
        .reveal(Arc::new(OneValue))
        .start();
    let base = &running.base;

    let mut opened = common::act(
        &format!("{base}/api/secretary/conversations"),
        &serde_json::json!({"title": "The filing"}),
    );
    let id = common::body_json(&mut opened)["id"]
        .as_str()
        .expect("an id")
        .to_owned();
    let said = common::act(
        &format!("{base}/api/secretary/conversations/{id}/say"),
        &serde_json::json!({"say": "draft the filing reply"}),
    );
    assert_eq!(common::status(&said), 202);
    let proposed = settled(base, &id);
    let proposals = proposed["proposals"].as_array().expect("proposals");
    assert_eq!(
        proposals.len(),
        3,
        "the withheld, subject, code and unread drafts are refused: {proposals:?}"
    );
    assert!(running.sent.sent().is_empty(), "proposing sends nothing");
    let sentence = proposals[0]["card"]["sentence"]
        .as_str()
        .expect("a sentence");
    assert!(
        sentence.starts_with("Includes your SSN ending 3456, from ")
            && sentence.contains("Everyone on To and Cc receives it: counsel@example.test."),
        "{sentence}"
    );

    let tap = |index: usize| {
        let mut tapped = common::act_signed(
            &format!("{base}/api/secretary/conversations/{id}/proposals/{index}/tap"),
            &serde_json::json!({}),
            &common::phone(),
        );
        assert_eq!(common::status(&tapped), 200);
        common::body_json(&mut tapped)
    };
    let after = tap(0);
    assert_eq!(after["proposals"][0]["state"], "sent");
    let sent = running.sent.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].body, format!("My SSN is {SSN}, for the form."));

    for (index, why) in [
        (1, "the SSN ending 3456 is no longer held"),
        (
            2,
            "the message now holds a different value where the ITIN ending 3456 was",
        ),
    ] {
        let after = tap(index);
        let row = &after["proposals"][index];
        assert_eq!(row["state"], "failed", "{row}");
        let outcome = row["outcome"].as_str().expect("an outcome");
        assert!(outcome.contains(why), "{outcome}");
    }
    assert_eq!(running.sent.sent().len(), 1, "a refused tap sends nothing");

    // The value is in no prompt, stored line, kept answer or request record.
    let asked = format!("{:?}{:?}", fakes.model.asked(), fakes.model.resumed_with());
    assert!(!asked.contains(SSN), "a prompt holds the value");
    let port = running.port();
    for dir in [
        fakes.root.clone(),
        common::requests_dir(port),
        std::env::temp_dir().join(format!("suzatary-test-answers-{port}")),
    ] {
        assert!(!holds(&dir, SSN), "{} holds the value", dir.display());
    }
    assert!(!settled(base, &id).to_string().contains(SSN));
}
