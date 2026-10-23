//! A Configure card over a real server and a scratch instance file: the card shows from and to,
//! the tap writes the setting while the file still reads `from`, the server reads the file again
//! with no restart, and a card whose setting moved since it was filed writes nothing. Only the
//! secret store is faked, since a test must not shell out to the real `secretspec`.

mod common;

use std::sync::Arc;

use common::{DeclaredStore, Fakes, SettingsFixture};
use suzatary_core::capability::Clock as _;
use suzatary_core::models::{Job, Reserve};
use suzatary_core::secretary::acts::Act;
use suzatary_core::secretary::noticed::CONVERSATION;
use suzatary_core::secretary::store::Proposal;
use suzatary_core::settings::configure::Configuration as _;
use suzatary_core::settings::configure::{Configure, Configured};

/// The scratch instance: every required table, and one provider an operator declared by hand.
const INSTANCE: &str = r#"# Ada's box, a scratch copy.
repo = "/tmp/suzatary-configure-tap"
data = "/tmp/suzatary-configure-tap"

[signatures]
root = "836884e1723c7b17f8b87e76a0272f012de6cff08f134eb9dcdb2670695b1818"

[[chains]]
name = "life"
events = "events"
proposals = "events-triage"
untrusted = []
actors = ["ada", "triage"]
owner = "ada"

[web]
viewers = ["ada@example.com"]
app_url = "https://example.test"

[web.act_as]
"ada@example.com" = "ada"

[web.serving]
host = "127.0.0.1"
port = 8787
tls_dir = "/var/lib/suzatary/tls"
dist = "web/dist"
briefings = "briefings"

[categories]
work = 0

[mail]
root = "Mail"
state = "state.json"
notmuch = "notmuch"
mbsyncrc = "mbsyncrc"
last_sync = "/tmp/suzatary-configure-tap-last-sync.json"
heartbeat = ".local/state/triage-heartbeat"
accounts = []

# Declared by hand, before any card.
[[providers]]
name = "phala"
base_url = "https://phala.example.test/v1"
key = "PHALA_API_KEY"
attested = false
"#;

/// The conversation the setup agent's cards are drawn in, here.
const SETUP: &str = "models";

struct Served {
    base: String,
    path: std::path::PathBuf,
    fakes: Arc<Fakes>,
    store: Arc<DeclaredStore>,
}

/// How the server's secret store behaves.
#[derive(Clone, Copy)]
enum Store {
    /// It declares the names it is told of and sets only a declared one, as `secretspec` does.
    Declaring,
    /// It declares the names, and refuses every value.
    Refusing,
    /// It will not take the names it is told of.
    Undeclaring,
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A server writing as `ada` over a scratch instance file, its live settings read from it.
fn serving(tag: &str) -> Served {
    serving_answering(tag, None)
}

/// [`serving`], the secretary answering as `model` scripts it and reading its cards' from out
/// of the same file.
fn serving_answering(tag: &str, model: Option<common::FakeModel>) -> Served {
    serving_storing(tag, model, Store::Declaring)
}

/// [`serving_answering`], its secret store behaving as `behaves`.
fn serving_storing(tag: &str, model: Option<common::FakeModel>, behaves: Store) -> Served {
    let path = std::env::temp_dir().join(format!(
        "suzatary-configure-{tag}-{}-{:?}.toml",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::write(&path, INSTANCE).expect("a scratch instance");
    let edits = suzatary_cli::instance::InstanceEdits::new(path.clone());
    let read = edits.configured().expect("the scratch instance reads");
    let ada = common::login("ada@example.com");
    let whois = common::FakeWhois::new().naming(common::v4(), ada.clone());
    let fakes = Fakes::new(tag).whois(whois).configuration(Arc::new(
        suzatary_cli::instance::InstanceEdits::new(path.clone()),
    ));
    let fakes = Arc::new(match model {
        Some(model) => fakes.model(model),
        None => fakes,
    });
    let store = Arc::new(DeclaredStore::new());
    let (secrets, declares): (
        Arc<dyn suzatary_core::settings::Writes + Send + Sync>,
        Arc<dyn suzatary_core::settings::Declares + Send + Sync>,
    ) = match behaves {
        Store::Declaring => (
            Arc::clone(&store) as _,
            Arc::new(store.declarations.clone()) as _,
        ),
        Store::Refusing => (
            Arc::new(common::SecretsAlwaysFail) as _,
            Arc::new(store.declarations.clone()) as _,
        ),
        Store::Undeclaring => (
            Arc::clone(&store) as _,
            Arc::new(common::DeclaresAlwaysFail) as _,
        ),
    };
    let deployment = common::deployment([ada.clone()], [(ada, common::actor("ada"))]);
    let base = common::Server::new(Arc::clone(&fakes), deployment, Vec::new())
        .settings(SettingsFixture {
            live: suzatary_cli::serve::Live {
                providers: read.providers,
                models: read.models,
                ..SettingsFixture::empty().live
            },
            edits: Arc::new(edits) as _,
            secrets,
            declares,
            ..SettingsFixture::empty()
        })
        .start()
        .base;
    Served {
        base,
        path,
        fakes,
        store,
    }
}

impl Served {
    /// What the file declares now.
    fn configured(&self) -> Configured {
        suzatary_cli::instance::InstanceEdits::new(self.path.clone())
            .configured()
            .expect("the file reads")
    }

    fn file(&self) -> String {
        std::fs::read_to_string(&self.path).expect("the file")
    }

    /// `change` filed as the setup agent will file it.
    fn ask(&self, change: Configure) {
        use suzatary_core::secretary::store::{ConversationId, Entry, Kind, Reasons};
        let id = ConversationId::new(SETUP).expect("a slug");
        // Opened by the first card, as a conversation is; a second start is refused.
        let _ = suzatary_core::secretary::transcript::start(
            self.fakes.conversations.conversations(),
            &id,
            &Entry::Opened {
                at: suzatary_core::capability::Datetime::new(2026, 9, 2, 11, 0, 0, 0)
                    .expect("an instant"),
                title: suzatary_core::secretary::words::Words::kept("Models".to_owned()),
                model: "a fake".to_owned(),
                kind: Kind::Topic,
            },
        );
        suzatary_core::secretary::filed::file(
            self.fakes.conversations.conversations(),
            &self.fakes.inbox,
            self.fakes.clock.now(),
            suzatary_core::secretary::filed::Filing {
                conversation: &id,
                proposer: suzatary_core::secretary::proposer::Proposer::Secretary,
                proposal: Proposal::Act(Act::Configure { change }),
                reasons: Reasons::new(
                    "Ada asked for another model",
                    vec!["briefing:2026-09-02".parse().expect("a reference")],
                )
                .expect("reasons"),
                serves: None,
                revises: None,
            },
        )
        .expect("filed");
    }

    fn tap(&self, conversation: &str, index: usize, body: &serde_json::Value) -> serde_json::Value {
        let mut tapped = common::act(
            &format!(
                "{}/api/secretary/conversations/{conversation}/proposals/{index}/tap",
                self.base
            ),
            body,
        );
        let answer = common::body_json(&mut tapped);
        assert_eq!(common::status(&tapped), 200, "{answer}");
        answer
    }

    /// The box's card asking for `OpenRouter`'s key, tapped with `value`.
    fn tap_the_key(&self, value: &str) -> serde_json::Value {
        let setup = CONVERSATION.as_str();
        let asked = common::reading(&self.base, setup, &[]);
        let index = asked["proposals"]
            .as_array()
            .expect("a list")
            .iter()
            .find(|card| card["act"]["label"] == "the OpenRouter key")
            .and_then(|card| card["index"].as_u64())
            .unwrap_or_else(|| panic!("the box asks for the key: {asked}"));
        self.tap(
            setup,
            usize::try_from(index).expect("an index"),
            &serde_json::json!({ "value": value }),
        )
    }

    /// `OpenRouter` declared by its card and ordered by Review's, so the box owes its key.
    fn openrouter_ordered(&self) {
        self.ask(
            Configure::declare(
                &self.configured(),
                suzatary_core::settings::ProviderSetup::OpenRouter,
            )
            .expect("a card"),
        );
        self.tap(SETUP, 0, &serde_json::json!({}));
        self.ask(
            Configure::order(&self.configured(), Job::Review, &["openrouter/m"]).expect("a card"),
        );
        self.tap(SETUP, 1, &serde_json::json!({}));
    }

    /// `text` said in a new conversation, and that conversation once its round is over.
    fn said(&self, text: &str) -> (String, serde_json::Value) {
        let base = &self.base;
        let mut opened = common::act(
            &format!("{base}/api/secretary/conversations"),
            &serde_json::json!({"title": "Models"}),
        );
        let id = common::body_json(&mut opened)["id"]
            .as_str()
            .expect("an id")
            .to_owned();
        let answer = common::act(
            &format!("{base}/api/secretary/conversations/{id}/say"),
            &serde_json::json!({ "say": text }),
        );
        assert_eq!(common::status(&answer), 202);
        for _ in 0..200 {
            let payload = common::reading(base, &id, &[]);
            if payload["state"] == "idle" {
                return (id, payload);
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("the round never finished");
    }

    /// The Models line keyed `key`, as `GET /api/overview` reads it now.
    fn line(&self, key: &str) -> Option<serde_json::Value> {
        let mut page = common::get(&format!("{}/api/overview", self.base));
        assert_eq!(common::status(&page), 200);
        let page = common::body_json(&mut page);
        page["groups"]
            .as_array()
            .expect("a list")
            .iter()
            .find(|group| group["group"] == "models")
            .and_then(|group| {
                group["lines"]
                    .as_array()?
                    .iter()
                    .find(|line| line["key"] == key)
                    .cloned()
            })
    }
}

/// The card says from and to; its tap writes the order, the Chat line moves with no restart,
/// and the card is drawn on that line while it stands.
#[test]
fn an_order_card_shows_from_and_to_and_its_tap_moves_the_models_line() {
    let served = serving("order");
    let change = Configure::order(
        &served.configured(),
        Job::Secretary,
        &["anthropic/claude-b", "phala/served"],
    )
    .expect("a card");
    served.ask(change);

    let line = served.line("job:secretary").expect("the Chat line");
    assert_eq!(
        line["cards"],
        serde_json::json!([format!("proposal:{SETUP}/0")]),
        "{line}"
    );
    let card = &common::reading(&served.base, SETUP, &[])["proposals"][0];
    assert_eq!(card["act"]["kind"], "configure", "{card}");
    assert_eq!(card["act"]["setting"], "Chat");
    assert_eq!(
        card["act"]["from"],
        "nothing chosen, so the fallback answers"
    );
    assert_eq!(card["act"]["to"], "anthropic/claude-b, then phala/served");
    assert_eq!(card["card"]["answers"][0]["verb"], "change", "{card}");

    let after = served.tap(SETUP, 0, &serde_json::json!({}));
    assert_eq!(after["proposals"][0]["state"], "acted", "{after}");
    assert!(
        served
            .file()
            .contains(r#"secretary = ["anthropic/claude-b", "phala/served"]"#),
        "{}",
        served.file()
    );
    assert!(served.file().starts_with("# Ada's box, a scratch copy."));
    let line = served.line("job:secretary").expect("the Chat line");
    assert_eq!(
        line["detail"],
        "Chat answers with the Anthropic API's claude-b model, then Phala's served model.",
        "{line}"
    );
    assert_eq!(line["cards"], serde_json::json!([]), "{line}");
}

/// Two cards read the same reserve; the first tap moves it, and the second writes nothing and
/// says what the file reads now. The Reserve line moves once.
#[test]
fn a_card_whose_setting_moved_since_it_was_filed_writes_nothing() {
    let served = serving("stale");
    let now = served.configured();
    let share = |share: f64| Reserve::parse(share).expect("a share");
    served.ask(Configure::reserve(&now, share(0.2)).expect("one"));
    served.ask(Configure::reserve(&now, share(0.5)).expect("two"));
    assert_eq!(
        served.line("reserve").expect("the Reserve line")["detail"],
        "Automated jobs keep 30% of the subscription for you."
    );

    let after = served.tap(SETUP, 0, &serde_json::json!({}));
    assert_eq!(after["proposals"][0]["state"], "acted", "{after}");
    let written = served.file();
    assert!(written.contains("reserve = 0.2"), "{written}");

    let after = served.tap(SETUP, 1, &serde_json::json!({}));
    let stale = &after["proposals"][1];
    assert_eq!(
        stale["state"], "withdrawn",
        "the box's, never the owner's no: {after}"
    );
    assert!(
        after.to_string().contains("it now reads keeps 20% for you"),
        "{after}"
    );
    assert_eq!(
        stale["card"]["answers"],
        serde_json::json!([]),
        "a card that can never be performed offers no Try again: {stale}"
    );
    assert_eq!(served.file(), written, "the stale card wrote nothing");
    assert_eq!(
        served.line("reserve").expect("the Reserve line")["cards"],
        serde_json::json!([]),
        "and stands on its line no more"
    );
    assert_eq!(
        served.line("reserve").expect("the Reserve line")["detail"],
        "Automated jobs keep 20% of the subscription for you."
    );
}

/// A provider declared by its card is a Models line at once; once a job orders it, the box asks
/// for its key with no restart, and the key's tap stores it and answers no value.
#[test]
fn a_provider_declared_by_its_card_is_asked_for_its_key_once_a_job_orders_it() {
    let served = serving("provider");
    served.ask(
        Configure::declare(
            &served.configured(),
            suzatary_core::settings::ProviderSetup::OpenRouter,
        )
        .expect("a card"),
    );
    let card = &common::reading(&served.base, SETUP, &[])["proposals"][0];
    assert_eq!(card["act"]["from"], "not added", "{card}");
    assert_eq!(
        card["act"]["to"], "answers at https://openrouter.ai/api/v1, with the OpenRouter key",
        "{card}"
    );
    assert!(!served.store.declares("OPENROUTER_API_KEY"));
    served.tap(SETUP, 0, &serde_json::json!({}));
    assert!(served.file().contains(r#"name = "openrouter""#));
    assert!(
        served.store.declares("OPENROUTER_API_KEY"),
        "the key is declared to the store as the provider is"
    );
    let line = served.line("provider:openrouter").expect("its line");
    assert_eq!(line["standing"]["kind"], "unused", "{line}");

    served.ask(
        Configure::order(&served.configured(), Job::Review, &["openrouter/m"]).expect("a card"),
    );
    served.tap(SETUP, 1, &serde_json::json!({}));
    let line = served.line("provider:openrouter").expect("its line");
    assert_eq!(line["standing"]["kind"], "missing", "{line}");

    let after = served.tap_the_key("do-not-leak-me");
    assert!(!after.to_string().contains("do-not-leak-me"), "{after}");
    assert!(!after.to_string().contains("OPENROUTER_API_KEY"), "{after}");
    assert_eq!(
        served.store.recorded.calls(),
        vec![("OPENROUTER_API_KEY".to_owned(), "do-not-leak-me".to_owned())]
    );
}

/// A store that will not take the new provider's key leaves the live providers as they were,
/// so the overview draws no line for it until the next start reads the file.
#[test]
fn a_provider_whose_key_the_store_will_not_declare_is_not_live() {
    let served = serving_storing("undeclared", None, Store::Undeclaring);
    served.ask(
        Configure::declare(
            &served.configured(),
            suzatary_core::settings::ProviderSetup::OpenRouter,
        )
        .expect("a card"),
    );
    served.tap(SETUP, 0, &serde_json::json!({}));
    assert!(served.file().contains(r#"name = "openrouter""#));
    assert!(
        served.line("provider:openrouter").is_none(),
        "the provider is not live"
    );
}

/// A provider a job orders is not removed: the card's tap writes nothing and names the job; once
/// the job orders something else, the same card's Try again removes it.
#[test]
fn a_provider_a_job_orders_is_removed_only_once_no_job_does() {
    let served = serving("remove");
    served.ask(Configure::order(&served.configured(), Job::Title, &["phala/served"]).expect("one"));
    served.tap(SETUP, 0, &serde_json::json!({}));
    let before = served.file();
    served.ask(Configure::remove(&served.configured(), "phala").expect("a removal"));
    let after = served.tap(SETUP, 1, &serde_json::json!({}));
    assert_eq!(after["proposals"][1]["state"], "failed", "{after}");
    assert!(
        after
            .to_string()
            .contains("Titles still orders Phala: change that first"),
        "{after}"
    );
    assert_eq!(served.file(), before);

    served.ask(Configure::order(&served.configured(), Job::Title, &[]).expect("back"));
    served.tap(SETUP, 2, &serde_json::json!({}));
    let after = served.tap(SETUP, 1, &serde_json::json!({}));
    assert_eq!(after["proposals"][1]["state"], "acted", "{after}");
    assert!(!served.file().contains("phala"), "{}", served.file());
    assert!(served.line("provider:phala").is_none());
}

/// Two taps at once, on two editors of one file, each reading the reserve its card read: the
/// editor's lock lets one write and makes the other read what the first wrote, so exactly one
/// is written and the other is refused as moved. An order and a reserve tapped at once are both
/// written, and the file holds both.
#[test]
fn two_taps_at_once_never_lose_a_change() {
    use std::sync::Barrier;
    use suzatary_core::settings::configure::{Performed, Refused};

    let path = std::env::temp_dir().join(format!(
        "suzatary-configure-race-{}-{:?}.toml",
        std::process::id(),
        std::thread::current().id()
    ));
    let share = |share: f64| Reserve::parse(share).expect("a share");
    let at_once = |cards: [Configure; 2]| {
        let barrier = Arc::new(Barrier::new(2));
        let handles = cards.map(|card| {
            let (barrier, path) = (Arc::clone(&barrier), path.clone());
            std::thread::spawn(move || {
                let edits = suzatary_cli::instance::InstanceEdits::new(path);
                barrier.wait();
                card.perform(&edits)
            })
        });
        handles.map(|handle| handle.join().expect("the thread ends"))
    };
    for round in 0..50 {
        std::fs::write(&path, INSTANCE).expect("a scratch instance");
        let now = suzatary_cli::instance::InstanceEdits::new(path.clone())
            .configured()
            .expect("reads");
        let [first, second] = at_once([
            Configure::reserve(&now, share(0.2)).expect("one"),
            Configure::reserve(&now, share(0.5)).expect("two"),
        ]);
        let written = [&first, &second]
            .iter()
            .filter(|done| matches!(done, Ok(Performed::Written)))
            .count();
        let moved = [&first, &second]
            .iter()
            .filter(|done| matches!(done, Err(Refused::Moved { .. })))
            .count();
        assert_eq!(
            (written, moved),
            (1, 1),
            "round {round}: {first:?} {second:?}"
        );

        std::fs::write(&path, INSTANCE).expect("a scratch instance");
        let now = suzatary_cli::instance::InstanceEdits::new(path.clone())
            .configured()
            .expect("reads");
        let both = at_once([
            Configure::order(&now, Job::Title, &["phala/served"]).expect("an order"),
            Configure::reserve(&now, share(0.2)).expect("a reserve"),
        ]);
        assert_eq!(
            both,
            [Ok(Performed::Written), Ok(Performed::Written)],
            "round {round}"
        );
        let after = suzatary_cli::instance::InstanceEdits::new(path.clone())
            .configured()
            .expect("reads");
        assert_eq!(
            after.models.order(Job::Title).len(),
            1,
            "round {round}: the order was kept"
        );
        assert_eq!(
            after.models.reserve(),
            share(0.2),
            "round {round}: and the reserve"
        );
    }
    let _ = std::fs::remove_file(&path);
}

/// Asked in the app, the secretary files a Configure card whose from is what the file reads
/// now; the owner's tap writes it.
#[test]
fn the_secretary_files_a_configure_card_and_the_tap_writes_it() {
    let served = serving_answering(
        "secretary",
        Some(common::FakeModel::saying([
            r#"{"say": "I will ask.",
                "calls": [{"verb": "propose_act", "args": {
                  "act": "configure",
                  "job": "secretary",
                  "models": ["anthropic/claude-x"],
                  "why": "Ada asked to answer with the API's model.",
                  "sources": "briefing:2026-09-02"
                }}]}"#,
            r#"{"say": "Proposed."}"#,
        ])),
    );
    let (id, payload) = served.said("answer with the API's claude-x");
    let card = &payload["proposals"][0];
    assert_eq!(card["act"]["kind"], "configure", "{payload}");
    assert_eq!(card["act"]["setting"], "Chat");
    assert_eq!(
        card["act"]["from"],
        "nothing chosen, so the fallback answers"
    );
    assert_eq!(card["act"]["to"], "anthropic/claude-x");
    assert!(
        !served.file().contains("claude-x"),
        "the secretary proposes, and nothing is written"
    );

    let after = served.tap(&id, 0, &serde_json::json!({}));
    assert_eq!(after["proposals"][0]["state"], "acted", "{after}");
    assert!(
        served
            .file()
            .contains(r#"secretary = ["anthropic/claude-x"]"#),
        "{}",
        served.file()
    );
}

/// A store that refuses leaves the key's card to try again, nothing stored, and its words carry
/// no value either.
#[test]
fn a_failing_store_leaves_the_key_card_standing_and_says_no_value() {
    let served = serving_storing("failing-store", None, Store::Refusing);
    served.openrouter_ordered();
    let after = served.tap_the_key("sk-must-not-leak-either");
    let raw = after.to_string();
    assert!(!raw.contains("sk-must-not-leak-either"), "{raw}");
    let card = after["proposals"]
        .as_array()
        .expect("a list")
        .iter()
        .find(|card| card["act"]["label"] == "the OpenRouter key")
        .expect("the key's card");
    assert_eq!(card["state"], "failed", "{raw}");
    assert!(raw.contains("could not store"), "{raw}");
    assert!(
        served.store.recorded.calls().is_empty(),
        "nothing was stored"
    );
}

/// A model, an endpoint and the commit a receipt must name are the owner's to give, never
/// learned from what a round read: after reading mail, a Configure card is refused, says why,
/// and nothing is filed.
#[test]
fn a_round_that_read_mail_proposes_no_configure_card() {
    let served = serving_answering(
        "secretary-tainted",
        Some(common::FakeModel::saying([
            r#"{"say": "", "calls": [{"verb": "search", "args": {"query": "phala"}}]}"#,
            r#"{"say": "", "calls": [{"verb": "propose_act", "args": {
                  "act": "configure",
                  "preset": "phala",
                  "expect_commit": "3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f",
                  "why": "A mail said to.",
                  "sources": "mail:a1b2c3d4e5f6"
                }}]}"#,
            r#"{"say": "Ask me again on its own."}"#,
        ])),
    );
    let (_, payload) = served.said("what did Phala write?");
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
        .find(|turn| turn["who"] == "verb" && turn["verb"] == "propose_act")
        .expect("the verb's turn");
    assert_eq!(refused["outcome"]["is"], "refused", "{refused}");
    assert!(
        refused["text"]
            .as_str()
            .expect("text")
            .contains("not the owner's own words"),
        "{refused}"
    );
    assert!(!served.file().contains("3f3f3f"), "nothing was written");
}
