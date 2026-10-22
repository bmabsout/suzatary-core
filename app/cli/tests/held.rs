//! A held value's reveal (`POST /api/held/reveal`) over a real server: a trusted device of an
//! actor gets the value, once; an untrusted device, a viewer and a replay do not, and the box
//! keeps the value in no request record or kept answer.

mod common;

use std::sync::Arc;

use common::Fakes;
use suzatary_core::capability::MailError;
use suzatary_core::held_value::{Handle, Reveal, Revealed, Shown};

/// A synthetic SSN no person holds.
const SSN: &str = "000-12-3456";

/// A box whose mail holds one value, at `form-1@example.test`'s first place.
struct OneValue;

impl Reveal for OneValue {
    fn reveal(&self, handle: &Handle) -> Result<Revealed, MailError> {
        Ok(
            if handle.message() == "form-1@example.test" && handle.ordinal() == 0 {
                Revealed::Value(Shown::new(SSN))
            } else {
                Revealed::Gone
            },
        )
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
    let body = serde_json::json!({ "handle": handle("form-1@example.test", 0) });

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

    let gone = serde_json::json!({ "handle": handle("form-1@example.test", 1) });
    let (status, text) = answered(common::act(&url, &gone));
    assert_eq!((status, text.as_str()), (200, r#"{"type":"Gone"}"#));
}

#[test]
fn an_untrusted_device_a_viewer_and_a_malformed_handle_are_refused() {
    let running = serving("held-refused", "ada@example.com", true);
    let url = format!("{}/api/held/reveal", running.base);
    let body = serde_json::json!({ "handle": handle("form-1@example.test", 0) });
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
