//! The plugin host's fences, each tried by the fence plugin (`app/plugin-fence`) and refused by
//! its typed error, the host carrying on after every one.
//!
//! The components are built by the flake, not by this test:
//! `SUZATARY_FENCE_PLUGINS=$(nix build .#suzatary-plugin-fence --no-link --print-out-paths)`
//! names the directory holding `fence.wasm`, `escapee.wasm`, `actor.wasm` and
//! `actor-escapee.wasm`, and `SUZATARY_PLUGINS`
//! (`.#suzatary-plugins`) the app's reference plugins, which are held to the same fences.
//! `scripts/merge-gate.sh` and the flake's `suzatary-plugin-fence` check set both and run this
//! target by name.
//!
//! Nothing here reaches outward: `fetch` goes to a server on 127.0.0.1 this test starts, the
//! secret store is a fake that records every name it is asked for, and no path is read but the
//! two components.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use prodrome_core::declared::{Declaration, Declared};
use prodrome_core::event::parse_event;
use suzatary_core::capability::Secret;
use suzatary_core::plugin::{
    Item, MAX_OFFERS, MAX_SAID, Manifest, Offer, PluginName, ProposalRefusal, Refusal, Said, World,
};
use suzatary_core::reference::Reference;
use suzatary_core::secretary::proposer::Proposer;
use suzatary_io::clock::SystemClock;
use suzatary_io::http::Ureq;
use suzatary_io::secrets::Resolves;
use suzatary_plugin::{Actor, Budget, Decided, Effects, Host, LoadError, Run, RunError, Source};
use wasmtime::component::Component;
use wasmtime::component::types::ComponentItem;

/// The value of the declared header credential; no answer the plugin sees may hold it.
const KEY: &str = "k3y-never-handed-back";

/// One request the loopback server received.
#[derive(Debug, Clone)]
struct Seen {
    path: String,
    headers: Vec<(String, String)>,
}

/// An HTTP server on 127.0.0.1 that answers three paths and keeps every request.
struct Server {
    origin: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let origin = format!(
            "http://127.0.0.1:{}",
            listener.local_addr().expect("its address").port()
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&seen);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let path = line.split(' ').nth(1).unwrap_or_default().to_owned();
                let mut headers = Vec::new();
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                        break;
                    }
                    if let Some((name, value)) = header.trim_end().split_once(':') {
                        headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
                    }
                }
                let body = match path.as_str() {
                    "/items" => "alpha\nbeta\ngamma".to_owned(),
                    "/echo" => headers
                        .iter()
                        .map(|(name, value)| [name.as_str(), ": ", value, "\n"].concat())
                        .collect(),
                    "/secret/s3cret" => "from the secret feed".to_owned(),
                    // Every day at nine since 2020, so today's window always holds some.
                    "/calendar/s3cret" => {
                        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:daily@example.test\r\n\
                         DTSTART:20200101T090000Z\r\nDURATION:PT30M\r\nRRULE:FREQ=DAILY\r\n\
                         SUMMARY:Daily\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
                            .to_owned()
                    }
                    _ => String::new(),
                };
                kept.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(Seen { path, headers });
                let _ = write!(
                    &stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Self { origin, seen }
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// A secret store holding the plugin's credentials and one of the app's own, which records every
/// name it is asked for.
struct Secrets {
    values: BTreeMap<String, String>,
    asked: Mutex<Vec<String>>,
}

impl Resolves for Secrets {
    fn get(
        &self,
        name: &str,
        _reason: &str,
    ) -> Result<Option<Secret>, suzatary_io::secrets::SecretError> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(name.to_owned());
        Ok(self.values.get(name).map(Secret::new))
    }
}

/// One host, one server and the fence plugin loaded against a manifest allowing that server.
struct Fixture {
    server: Server,
    secrets: Arc<Secrets>,
    source: Source,
}

fn components() -> PathBuf {
    std::env::var_os("SUZATARY_FENCE_PLUGINS").map_or_else(
        || {
            panic!(
                "SUZATARY_FENCE_PLUGINS is not set: it names `nix build .#suzatary-plugin-fence`'s \
                 output, which scripts/merge-gate.sh passes in"
            )
        },
        PathBuf::from,
    )
}

fn component(name: &str) -> Vec<u8> {
    let path = components().join(name);
    std::fs::read(&path).unwrap_or_else(|failed| panic!("{}: {failed}", path.display()))
}

/// One of the app's reference plugins, from `SUZATARY_PLUGINS`.
fn reference(name: &str) -> Vec<u8> {
    let directory = std::env::var_os("SUZATARY_PLUGINS").unwrap_or_else(|| {
        panic!(
            "SUZATARY_PLUGINS is not set: it names `nix build .#suzatary-plugins`'s output, which \
             scripts/merge-gate.sh passes in"
        )
    });
    let path = PathBuf::from(directory).join(name);
    std::fs::read(&path).unwrap_or_else(|failed| panic!("{}: {failed}", path.display()))
}

/// Every function a component imports, by instance.
fn import_surface(bytes: &[u8]) -> BTreeMap<String, Vec<String>> {
    let engine = wasmtime::Engine::default();
    let component = Component::from_binary(&engine, bytes).expect("a component");
    component
        .component_type()
        .imports(&engine)
        .map(|(name, import)| {
            let ComponentItem::ComponentInstance(instance) = import.ty else {
                panic!("{name} is not an instance");
            };
            let functions = instance
                .exports(&engine)
                .filter(|(_, item)| matches!(item.ty, ComponentItem::ComponentFunc(_)))
                .map(|(function, _)| function.to_owned())
                .collect();
            (name.to_owned(), functions)
        })
        .collect()
}

/// The fence plugin's schema: a line of a body, said by someone at some time. Its items are
/// never stored, so only admission reads it.
const FENCE_SCHEMA: &str = "Schema(key='line', events=(Event(name='Said', fields=(Field(name='line', \
    type=Text()), Field(name='at', type=Instant()), Field(name='actor', type=Text()))),), \
    registers=(Register(name='line', order=Discrete(), inflationary=False, \
    writes=(Write(event='Said', field='line'),)),))";

fn manifest(origin: &str) -> Manifest {
    toml::from_str(&format!(
        r#"
        name = "fence"
        component = "fence.wasm"
        schema = "fence.schema"
        world = "source@0.3.0"
        origins = ["{origin}"]
        credentials = [
            {{ name = "PLUGIN_FENCE_KEY", into = "header", header = "x-key", origin = "{origin}" }},
            {{ name = "PLUGIN_FENCE_FEED", into = "url" }},
            {{ name = "PLUGIN_FENCE_UNSET", into = "bearer", origin = "{origin}" }},
        ]
        "#
    ))
    .expect("the fence manifest")
}

fn host() -> Host {
    Host::new(Budget {
        time: Duration::from_secs(2),
        memory: 32 * 1024 * 1024,
    })
    .expect("the host starts")
}

impl Fixture {
    fn new() -> Self {
        let server = Server::start();
        let secrets = Arc::new(Secrets {
            values: BTreeMap::from([
                ("PLUGIN_FENCE_KEY".to_owned(), KEY.to_owned()),
                (
                    "PLUGIN_FENCE_FEED".to_owned(),
                    format!("{}/secret/s3cret", server.origin),
                ),
                (
                    "ANTHROPIC_API_KEY".to_owned(),
                    "the-app-s-own-key".to_owned(),
                ),
            ]),
            asked: Mutex::new(Vec::new()),
        });
        let source = host()
            .load(
                manifest(&server.origin),
                &component("fence.wasm"),
                FENCE_SCHEMA,
            )
            .expect("the fence plugin targets the source world");
        Self {
            server,
            secrets,
            source,
        }
    }

    fn sync(&self, cursor: &str) -> Run {
        let effects = Effects {
            http: Arc::new(Ureq::new()),
            secrets: Arc::clone(&self.secrets) as Arc<dyn Resolves + Send + Sync>,
            clock: Arc::new(SystemClock),
        };
        self.source.sync(effects, Some(cursor.as_bytes()))
    }

    fn asked(&self) -> Vec<String> {
        self.secrets
            .asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The honest path still works: what a stopped or refused call left behind did not.
    fn still_answers(&self) {
        let run = self.sync(&format!("items {}", self.server.origin));
        assert_eq!(
            run.result.map(|synced| synced.items.len()),
            Ok(3),
            "the host answers the next call"
        );
    }
}

fn lines(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .map(|item| {
            assert_eq!(item.schema, "fence.line");
            String::from_utf8(item.bytes.clone()).expect("text")
        })
        .collect()
}

/// The refusal the host recorded, and the plugin's report of the same one.
fn refused_once(run: &Run, refusal: &Refusal) {
    assert_eq!(
        run.refused,
        std::slice::from_ref(refusal),
        "the host's own record"
    );
    assert_eq!(run.result, Err(RunError::Refused(refusal.clone())));
}

#[test]
fn an_allowed_origin_answers_and_a_call_is_a_function_of_its_cursor() {
    let fixture = Fixture::new();
    let cursor = format!("items {}", fixture.server.origin);
    let first = fixture.sync(&cursor);
    let synced = first.result.clone().expect("the plugin synced");
    assert_eq!(lines(&synced.items), ["alpha", "beta", "gamma"]);
    assert_eq!(synced.cursor, b"done");
    assert!(first.refused.is_empty());
    assert_eq!(first.log.lines.len(), 1, "{:?}", first.log);
    // A fresh instance per call: the same cursor over the same answers is the same result.
    assert_eq!(fixture.sync(&cursor).result, first.result);
    let seen = fixture.server.seen();
    assert_eq!(seen.len(), 2);
    assert!(seen.iter().all(|request| request.path == "/items"));
    assert!(
        seen.iter()
            .all(|request| !request.headers.iter().any(|(name, _)| name == "x-key")),
        "no credential was named, so none was sent"
    );
    assert!(fixture.asked().is_empty());
}

#[test]
fn an_origin_the_manifest_does_not_allow_is_refused_before_anything_is_sent() {
    let fixture = Fixture::new();
    let port = fixture.server.origin.rsplit(':').next().expect("a port");
    for (url, origin) in [
        ("https://example.com/", "https://example.com"),
        (
            &*format!("http://localhost:{port}/items"),
            &*format!("http://localhost:{port}"),
        ),
    ] {
        let run = fixture.sync(&format!("host {url}"));
        refused_once(&run, &Refusal::OriginNotAllowed(Said::clip(origin)));
    }
    let disguised = format!("{}@example.com/", fixture.server.origin);
    let run = fixture.sync(&format!("host {disguised}"));
    refused_once(&run, &Refusal::BadUrl(Said::clip(disguised)));
    assert!(fixture.server.seen().is_empty());
    fixture.still_answers();
}

#[test]
fn a_credential_not_declared_is_refused_and_never_resolved() {
    let fixture = Fixture::new();
    for (name, declared) in [
        ("ANTHROPIC_API_KEY", false),
        ("PLUGIN_FENCE_NOPE", false),
        // Declared, as a URL: not for a header.
        ("PLUGIN_FENCE_FEED", true),
    ] {
        let run = fixture.sync(&format!("credential {} {name}", fixture.server.origin));
        refused_once(&run, &Refusal::CredentialNotDeclared(Said::clip(name)));
        assert_eq!(
            run.fetches[0].credential.as_ref().map(ToString::to_string),
            declared.then(|| name.to_owned()),
            "a link is kept under a declared name only"
        );
    }
    for name in ["ANTHROPIC_API_KEY", "PLUGIN_FENCE_KEY"] {
        let run = fixture.sync(&format!("secret-url {name}"));
        refused_once(&run, &Refusal::CredentialNotDeclared(Said::clip(name)));
    }
    assert!(
        fixture.asked().is_empty(),
        "the secret store was asked for {:?}",
        fixture.asked()
    );
    assert!(fixture.server.seen().is_empty());
    fixture.still_answers();
}

#[test]
fn a_declared_but_unset_credential_is_refused_by_name() {
    let fixture = Fixture::new();
    let run = fixture.sync(&format!(
        "credential {} PLUGIN_FENCE_UNSET",
        fixture.server.origin
    ));
    refused_once(
        &run,
        &Refusal::CredentialUnset(Said::clip("PLUGIN_FENCE_UNSET")),
    );
    assert_eq!(fixture.asked(), ["PLUGIN_FENCE_UNSET"]);
    assert!(fixture.server.seen().is_empty());
}

/// The value goes out in the request, and no door brings it back: the answer that echoes it is
/// withheld, and the world has no import that returns a credential at all.
#[test]
fn a_credential_value_reaches_the_server_and_never_the_plugin() {
    let fixture = Fixture::new();
    let run = fixture.sync(&format!("echo {}", fixture.server.origin));
    refused_once(&run, &Refusal::Echoed(Said::clip("PLUGIN_FENCE_KEY")));
    let seen = fixture.server.seen();
    assert_eq!(seen.len(), 1);
    assert!(
        seen[0]
            .headers
            .contains(&("x-key".to_owned(), KEY.to_owned())),
        "the host put the declared credential in its header: {:?}",
        seen[0].headers
    );
    assert_eq!(fixture.asked(), ["PLUGIN_FENCE_KEY"]);
    assert!(
        !format!("{run:?}").contains(KEY),
        "the run never holds the value"
    );

    let surface = import_surface(&component("fence.wasm"));
    assert_eq!(
        surface,
        BTreeMap::from([
            (
                "suzatary:plugin/clock@0.3.0".to_owned(),
                vec!["now".to_owned()]
            ),
            (
                "suzatary:plugin/fetch@0.3.0".to_owned(),
                vec!["fetch".to_owned()]
            ),
            (
                "suzatary:plugin/log@0.3.0".to_owned(),
                vec!["log".to_owned()]
            ),
        ]),
        "every function a plugin can call; only fetch answers with outside bytes"
    );
    assert_eq!(surface.keys().map(String::as_str).collect::<Vec<_>>(), {
        let mut world = World::Source.imports().to_vec();
        world.sort_unstable();
        world
    });
}

#[test]
fn a_secret_url_is_fetched_without_the_plugin_ever_holding_it() {
    let fixture = Fixture::new();
    let run = fixture.sync("secret-url PLUGIN_FENCE_FEED");
    let synced = run.result.expect("the feed answered");
    assert_eq!(lines(&synced.items), ["from the secret feed"]);
    assert_eq!(fixture.asked(), ["PLUGIN_FENCE_FEED"]);
    assert_eq!(
        fixture
            .server
            .seen()
            .iter()
            .map(|request| request.path.clone())
            .collect::<Vec<_>>(),
        ["/secret/s3cret"]
    );
}

#[test]
fn a_plugin_is_lent_the_names_it_declares_and_no_value() {
    let fixture = Fixture::new();
    let run = fixture.sync("lent");
    let synced = run.result.expect("it answers");
    let mut lent = lines(&synced.items);
    lent.sort();
    assert_eq!(
        lent,
        [
            "PLUGIN_FENCE_FEED",
            "PLUGIN_FENCE_KEY",
            "PLUGIN_FENCE_UNSET"
        ]
    );
    assert!(
        fixture.asked().is_empty(),
        "lending a name asks the secret store nothing"
    );
}

#[test]
fn an_endless_loop_is_stopped_at_its_deadline() {
    let fixture = Fixture::new();
    let started = Instant::now();
    let run = fixture.sync("loop");
    assert_eq!(run.result, Err(RunError::Deadline));
    let took = started.elapsed();
    assert!(
        took >= Duration::from_secs(2) && took < Duration::from_secs(10),
        "stopped after {took:?}"
    );
    fixture.still_answers();
}

#[test]
fn unbounded_memory_is_stopped_at_its_cap() {
    let fixture = Fixture::new();
    let run = fixture.sync("grow");
    assert_eq!(run.result, Err(RunError::Memory));
    fixture.still_answers();
}

#[test]
fn a_panic_is_a_trap_and_the_host_carries_on() {
    let fixture = Fixture::new();
    let run = fixture.sync("panic");
    assert!(
        matches!(run.result, Err(RunError::Trapped(_))),
        "{:?}",
        run.result
    );
    fixture.still_answers();
}

/// Whatever a plugin says, at whatever door, the call the host keeps holds a bound's worth of it:
/// an error, a refusal of its own making, a log line and a credential's name, each a megabyte.
#[test]
fn a_megabyte_said_at_every_door_is_kept_bounded() {
    let fixture = Fixture::new();
    for kind in ["malformed", "cursor", "refused"] {
        let run = fixture.sync(&format!("megabyte {kind}"));
        let said = match &run.result {
            Err(
                RunError::Malformed(said)
                | RunError::BadCursor(said)
                | RunError::Refused(Refusal::BadUrl(said)),
            ) => said.clone(),
            other => panic!("{kind}: {other:?}"),
        };
        assert!(said.as_str().len() <= MAX_SAID, "{kind}");
        assert!(said.as_str().ends_with(" bytes cut)"), "{kind}: {said}");
        assert!(
            run.log
                .lines
                .iter()
                .all(|line| line.text.as_str().len() <= MAX_SAID)
        );
        assert_eq!(run.fetches.len(), 1, "{kind}");
        assert_eq!(
            run.fetches[0].credential, None,
            "an undeclared name is no link"
        );
        assert!(
            run.refused
                .iter()
                .all(|refusal| refusal.to_string().len() <= 2 * MAX_SAID)
        );
        assert!(
            format!("{run:?}").len() < 8 * MAX_SAID,
            "{kind}: the whole call as the host keeps it"
        );
    }
    fixture.still_answers();
}

#[test]
fn a_component_importing_a_filesystem_or_a_network_is_never_loaded() {
    let refused = host()
        .load(
            manifest("http://127.0.0.1:9"),
            &component("escapee.wasm"),
            FENCE_SCHEMA,
        )
        .map(|_| ())
        .expect_err("the escapee imports what the source world does not give");
    assert_eq!(
        refused,
        LoadError::ForeignImports(vec![
            "wasi:filesystem/preopens@0.2.0".to_owned(),
            "wasi:sockets/instance-network@0.2.0".to_owned(),
        ])
    );
}

/// The calendar is a source like any other: loaded against its world, its links read through
/// the host by name, an unset one refused and recorded, every item it emits one the store admits,
/// and a second sync of the same feed learning nothing.
#[test]
#[allow(clippy::too_many_lines)]
fn the_calendar_plugin_holds_to_the_same_fences() {
    let server = Server::start();
    let manifest: Manifest = toml::from_str(&format!(
        r#"
        name = "ics"
        component = "ics.wasm"
        schema = "ics.schema"
        world = "source@0.3.0"
        origins = ["{}"]
        credentials = [
            {{ name = "PLUGIN_ICS_HOME", into = "url" }},
            {{ name = "PLUGIN_ICS_UNSET", into = "url" }},
        ]
        "#,
        server.origin
    ))
    .expect("the calendar's manifest");
    let secrets = Arc::new(Secrets {
        values: BTreeMap::from([
            (
                "PLUGIN_ICS_HOME".to_owned(),
                format!("{}/calendar/s3cret", server.origin),
            ),
            (
                "ANTHROPIC_API_KEY".to_owned(),
                "the-app-s-own-key".to_owned(),
            ),
        ]),
        asked: Mutex::new(Vec::new()),
    });
    let bytes = reference("ics.wasm");
    assert_eq!(import_surface(&bytes).into_keys().collect::<Vec<_>>(), {
        let mut world: Vec<String> = World::Source
            .imports()
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        world.sort_unstable();
        world
    });
    let source = host()
        .load(
            manifest.clone(),
            &bytes,
            &String::from_utf8(reference("ics.schema")).expect("a schema is text"),
        )
        .expect("the calendar targets the source world");
    let effects = || Effects {
        http: Arc::new(Ureq::new()),
        secrets: Arc::clone(&secrets) as Arc<dyn Resolves + Send + Sync>,
        clock: Arc::new(SystemClock),
    };
    let run = source.sync(effects(), None);
    assert!(
        !format!("{run:?}").contains("s3cret"),
        "the link is in no run"
    );
    let synced = run.result.expect("the calendar answers");
    assert_eq!(
        run.refused,
        [Refusal::CredentialUnset(Said::clip("PLUGIN_ICS_UNSET"))]
    );
    assert_eq!(
        run.fetches
            .iter()
            .map(|fetch| (
                fetch.credential.as_ref().map(ToString::to_string),
                fetch.outcome.clone()
            ))
            .collect::<Vec<_>>(),
        [
            (Some("PLUGIN_ICS_HOME".to_owned()), Ok(200)),
            (
                Some("PLUGIN_ICS_UNSET".to_owned()),
                Err(Refusal::CredentialUnset(Said::clip("PLUGIN_ICS_UNSET")))
            ),
        ],
        "the host's own record of each link"
    );
    assert!(
        synced.items.len() >= 60,
        "a daily event over the window: {}",
        synced.items.len()
    );
    for item in &synced.items {
        let event =
            suzatary_core::plugin::admit(source.schema(), &manifest, &item.schema, &item.bytes)
                .unwrap_or_else(|refused| panic!("{refused}"));
        suzatary_core::calendar::Occurrence::of(&event)
            .unwrap_or_else(|refused| panic!("{refused}"));
    }
    let mut asked = secrets
        .asked
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    asked.sort();
    assert_eq!(
        asked,
        ["PLUGIN_ICS_HOME", "PLUGIN_ICS_UNSET"],
        "never the app's own key"
    );
    assert_eq!(
        server
            .seen()
            .iter()
            .map(|seen| seen.path.clone())
            .collect::<Vec<_>>(),
        ["/calendar/s3cret"]
    );
    let again = source.sync(effects(), Some(&synced.cursor));
    assert_eq!(
        again.result.map(|synced| synced.items.len()),
        Ok(0),
        "an unchanged feed"
    );

    let seen = server.seen().len();
    let trial = source.trial(&effects(), None);
    assert_eq!(
        trial.again, trial.run,
        "the same cursor over the same answers"
    );
    assert_eq!(
        trial
            .onward
            .map(|onward| onward.result.map(|synced| synced.items.len())),
        Some(Ok(0)),
        "its own cursor over the same answers reports nothing"
    );
    assert_eq!(
        server.seen().len(),
        seen + 1,
        "a trial sends its one fetch once; the replays send nothing"
    );
}

/// The fence's actor, loaded against the actor world under the fence's schema.
fn actor() -> Actor {
    let manifest: Manifest = toml::from_str(
        r#"
        name = "fence"
        component = "actor.wasm"
        schema = "fence.schema"
        world = "actor@0.3.0"
        "#,
    )
    .expect("the fence actor's manifest");
    host()
        .load_actor(manifest, &component("actor.wasm"), FENCE_SCHEMA)
        .expect("the fence actor targets the actor world")
}

/// The store an actor is handed: one `Said` per line, written as the fence. The first line is
/// the mode the fence actor plays.
fn said(actor: &Actor, lines: &[&str]) -> Vec<Declared> {
    lines
        .iter()
        .map(|line| {
            parse_event(
                actor.schema(),
                &format!("Said(line='{line}', at=datetime(2026, 10, 5, 9, 0, 0), actor='fence')"),
            )
            .expect("an event of the fence's schema")
        })
        .collect()
}

fn decided(actor: &Actor, lines: &[&str]) -> Decided {
    let acts = actor
        .acts(said(actor, lines))
        .expect("events of its own schema");
    actor.decide(Arc::new(SystemClock), &acts)
}

/// What an actor proposes is a set of values, each an act of the Inbox's closed vocabulary with
/// a reason and a ground; anything else is refused naming the plugin, and nothing is performed.
#[test]
fn an_actor_proposes_values_and_the_host_admits_only_the_inboxs_acts() {
    let actor = actor();
    let echoed = decided(&actor, &["echo", "second"]);
    let offers = echoed.result.expect("the actor decided");
    assert_eq!(
        offers.iter().map(Offer::key).collect::<Vec<_>>(),
        ["echo", "second"]
            .map(|line| format!("Changes(chain='life', acts=(Complete(todo='{line}', note=''),))")),
    );
    for offer in &offers {
        assert_eq!(offer.proposer(), Proposer::Plugin(fence_name()));
        assert_eq!(offer.reasons().why(), "echoed");
        assert_eq!(
            offer.reasons().sources(),
            ["briefing:2026-10-05"
                .parse::<Reference>()
                .expect("a reference")]
        );
    }
    assert!(echoed.refused.is_empty());
    assert_eq!(
        decided(&actor, &["echo", "second", "echo"]).result,
        decided(&actor, &["echo", "second"]).result,
        "an actor is handed a set: a repeated act is handed once"
    );
    assert_eq!(echoed.log.lines.len(), 1, "{:?}", echoed.log);

    let twice = decided(&actor, &["twice"]).result.expect("decided");
    assert_eq!(
        twice
            .iter()
            .map(|offer| offer.reasons().why())
            .collect::<Vec<_>>(),
        ["first"],
        "one act asked twice is one offer, whichever order it was asked in"
    );
    for (mode, refused) in [
        ("forge", "something outside the Inbox's acts"),
        ("elsewhere", "something outside the Inbox's acts"),
        ("recur", "an act of Auth, which a plugin may not ask for"),
        (
            "sprawl",
            "an offer of 1900071 bytes, longer than this host reads",
        ),
        ("mute", "an act without its reasons: why must say"),
        (
            "groundless",
            "an act without its reasons: sources must name",
        ),
    ] {
        let run = decided(&actor, &[mode]);
        assert_eq!(run.result, Ok(Vec::new()), "{mode}: nothing admitted");
        let [refusal] = run.refused.as_slice() else {
            panic!("{mode}: one refusal, got {:?}", run.refused);
        };
        assert_eq!(refusal.plugin, fence_name(), "{mode}: the plugin is named");
        assert!(
            refusal
                .to_string()
                .starts_with(&format!("the fence plugin proposed {refused}")),
            "{mode}: {refusal}"
        );
    }
    assert_eq!(
        decided(&actor, &[]).result,
        Ok(Vec::new()),
        "an empty store proposes nothing"
    );
}

/// One decision is read only to its cap: a flood is cut there and the rest refused by count,
/// naming the plugin; an actor that mints a new act from the clock asks for a new one each call,
/// which the Inbox bounds by what the plugin has waiting (`secretary::filed`).
#[test]
fn a_flood_is_cut_at_the_cap_and_a_clock_mints_a_new_act_each_call() {
    let actor = actor();
    let flood = decided(&actor, &["flood"]);
    assert_eq!(
        flood.result.map(|offers| offers.len()),
        Ok(MAX_OFFERS),
        "read only to the cap"
    );
    let [beyond] = flood.refused.as_slice() else {
        panic!("one refusal, got {:?}", flood.refused);
    };
    assert_eq!(beyond.plugin, fence_name());
    assert_eq!(beyond.refusal, ProposalRefusal::Beyond(10_000 - MAX_OFFERS));

    let minted = |n| {
        decided(&actor, &["clock"])
            .result
            .unwrap_or_else(|stopped| panic!("call {n}: {stopped}"))
            .iter()
            .map(Offer::key)
            .collect::<Vec<_>>()
    };
    let (first, second) = (minted(1), minted(2));
    assert_eq!((first.len(), second.len()), (1, 1));
    assert_ne!(first, second, "a new act each call");
}

fn fence_name() -> PluginName {
    PluginName::parse("fence").expect("a name")
}

/// The calendar's actor is an actor like any other: it imports nothing the actor world does not
/// give, reads the
/// calendar's store by its schema, and what it proposes is admitted as the Inbox's act, a todo
/// for each `Due:` entry not cancelled; a meeting proposes nothing.
#[test]
fn the_due_actor_holds_to_the_actor_fences() {
    let bytes = reference("due.wasm");
    let surface = import_surface(&bytes);
    assert!(
        surface
            .keys()
            .all(|name| World::Actor.imports().contains(&name.as_str())),
        "{surface:?}"
    );
    let manifest: Manifest = toml::from_str(
        r#"
        name = "due"
        component = "due.wasm"
        schema = "ics.schema"
        world = "actor@0.3.0"
        "#,
    )
    .expect("the due actor's manifest");
    let schema = String::from_utf8(reference("ics.schema")).expect("a schema is text");
    let actor = host()
        .load_actor(manifest, &bytes, &schema)
        .expect("the due actor targets the actor world");
    let learned = |occurrence: &str, title: &str, status: &str| {
        parse_event(
            actor.schema(),
            &format!(
                "Learned(occurrence='{occurrence}', at=datetime(2026, 10, 5, 9, 0, 0), \
                 actor='ics', title='{title}', when='2099-01-01/2099-01-02', place='', \
                 attendees=(), status='{status}')"
            ),
        )
        .expect("an event of the calendar's schema")
    };
    let acts = actor
        .acts([
            learned(
                "PLUGIN_ICS_HOME 20990101 a@x",
                "Due: renew the visa",
                "confirmed",
            ),
            learned("PLUGIN_ICS_HOME 20990101 b@x", "Due: the rent", "confirmed"),
            learned("PLUGIN_ICS_HOME 20990101 b@x", "Due: the rent", "cancelled"),
            learned("PLUGIN_ICS_HOME 20990101 c@x", "Standup", "confirmed"),
        ])
        .expect("its own schema's events");
    let decided = actor.decide(Arc::new(SystemClock), &acts);
    assert!(decided.refused.is_empty(), "{:?}", decided.refused);
    let offers = decided.result.expect("the actor decided");
    let [offer] = offers.as_slice() else {
        panic!("one offer, got {offers:?}");
    };
    assert_eq!(
        offer.proposer(),
        Proposer::Plugin(PluginName::parse("due").expect("a name"))
    );
    assert!(
        offer
            .key()
            .starts_with("Changes(chain='life', acts=(Add(todo='due")
            && offer.key().contains("body='renew the visa'")
            && offer.key().contains("end_date='2099-01-01'"),
        "{}",
        offer.key()
    );
    assert!(matches!(
        offer.reasons().sources(),
        [Reference::Briefing { .. }]
    ));
    assert_eq!(
        actor.decide(Arc::new(SystemClock), &acts).result,
        Ok(offers),
        "asked again, it asks for the same act"
    );
}

/// An actor is handed its own store's events and nothing else: an event of another schema is
/// refused before anything runs, and one actor's acts are never another's.
#[test]
fn an_actor_is_handed_only_its_own_store() {
    let actor = actor();
    // The same events under another register's name: another schema, so another name.
    let other =
        Declaration::admit(&FENCE_SCHEMA.replace("Register(name='line'", "Register(name='other'"))
            .expect("another lawful schema");
    let foreign = parse_event(
        &other,
        "Said(line='echo', at=datetime(2026, 10, 5, 9, 0, 0), actor='fence')",
    )
    .expect("an event of the other schema");
    let refused = actor.acts([foreign]).expect_err("not its own schema");
    assert_eq!(refused.plugin, "fence");
    assert_eq!(refused.found, other.name().as_str());

    let second: Manifest = toml::from_str(
        r#"
        name = "second"
        component = "actor.wasm"
        schema = "fence.schema"
        world = "actor@0.3.0"
        "#,
    )
    .expect("a second actor's manifest");
    let second = host()
        .load_actor(second, &component("actor.wasm"), FENCE_SCHEMA)
        .expect("the same component as another plugin");
    let theirs = second.acts(said(&second, &["echo"])).expect("its own");
    assert!(matches!(
        actor.decide(Arc::new(SystemClock), &theirs).result,
        Err(RunError::Malformed(_))
    ));
}

/// An actor's megabyte error and log line are kept a bound's worth each, as a source's are.
#[test]
fn an_actors_megabyte_is_kept_bounded() {
    let actor = actor();
    let decided = decided(&actor, &["megabyte"]);
    let Err(RunError::Malformed(said)) = &decided.result else {
        panic!("{:?}", decided.result);
    };
    assert!(said.as_str().len() <= MAX_SAID);
    assert!(said.as_str().ends_with(" bytes cut)"), "{said}");
    assert!(
        decided
            .log
            .lines
            .iter()
            .all(|line| line.text.as_str().len() <= MAX_SAID)
    );
    assert!(format!("{decided:?}").len() < 8 * MAX_SAID);
}

#[test]
fn an_actor_is_stopped_at_its_deadline_and_its_cap_and_the_host_carries_on() {
    let actor = actor();
    assert_eq!(decided(&actor, &["loop"]).result, Err(RunError::Deadline));
    assert_eq!(decided(&actor, &["grow"]).result, Err(RunError::Memory));
    assert!(decided(&actor, &["echo"]).result.is_ok());
}

/// An actor's whole authority is its imports, and none reaches outward: one importing `fetch`
/// is never loaded, and a component is loaded only as the world its manifest names.
#[test]
fn an_actor_importing_fetch_is_never_loaded() {
    let bytes = component("actor.wasm");
    let surface = import_surface(&bytes);
    assert!(
        surface
            .keys()
            .all(|name| World::Actor.imports().contains(&name.as_str())),
        "{surface:?}"
    );
    let manifest = |world: &str| -> Manifest {
        toml::from_str(&format!(
            r#"
            name = "fence"
            component = "actor.wasm"
            schema = "fence.schema"
            world = "{world}"
            "#
        ))
        .expect("a manifest")
    };
    let refused = host()
        .load_actor(
            manifest("actor@0.3.0"),
            &component("actor-escapee.wasm"),
            FENCE_SCHEMA,
        )
        .map(|_| ())
        .expect_err("an actor that fetches is refused");
    assert_eq!(
        refused,
        LoadError::ForeignImports(vec!["suzatary:plugin/fetch@0.3.0".to_owned()])
    );
    assert_eq!(
        host()
            .load_actor(
                manifest("actor@0.3.0"),
                &component("fence.wasm"),
                FENCE_SCHEMA
            )
            .map(|_| ())
            .expect_err("a source is not an actor"),
        LoadError::ForeignImports(vec!["suzatary:plugin/fetch@0.3.0".to_owned()])
    );
    assert_eq!(
        host()
            .load(manifest("actor@0.3.0"), &bytes, FENCE_SCHEMA)
            .map(|_| ())
            .expect_err("an actor's manifest loads no source"),
        LoadError::OtherWorld {
            named: World::Actor,
            loading: World::Source,
        }
    );
    assert_eq!(
        host()
            .load(manifest("source@0.3.0"), &bytes, FENCE_SCHEMA)
            .map(|_| ())
            .expect_err("an actor exports no sync"),
        LoadError::WrongWorld("suzatary:plugin/sync@0.3.0")
    );
}
