//! `suzatary plugin sync` over a loopback feed: the reference calendar component and its actor,
//! the host, the store, the Inbox and the ledger, everything but the transport and the secret
//! store the box's own.
//!
//! Run by name with `SUZATARY_PLUGINS` naming `nix build .#suzatary-plugins`'s output, as
//! `scripts/merge-gate.sh` does; the production transport refuses loopback by design, so the
//! test lends the verb's body a transport that reaches the server it starts, by struct update.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use suzatary_cli::commands::plugin;
use suzatary_cli::instance::Plugins;
use suzatary_core::calendar::{Calendar, Status};
use suzatary_core::capability::Secret;
use suzatary_core::plugin::{Came, Digest, Manifest, PluginName, ProposalRefusal, Said};
use suzatary_core::refusal::Refusal;
use suzatary_core::secretary::changes::Changeset;
use suzatary_core::secretary::inbox;
use suzatary_core::secretary::inbox_reading::Reading;
use suzatary_core::secretary::proposer::Proposer;
use suzatary_core::secretary::store::Proposal;
use suzatary_core::store::Prodrome;
use suzatary_io::clock::SystemClock;
use suzatary_io::http::Ureq;
use suzatary_io::objects::Objects;
use suzatary_io::plugin_store::PluginStore;
use suzatary_io::secrets::Resolves;
use suzatary_plugin::{Budget, Effects, Host};

/// A feed whose body the test changes between syncs.
struct Feed {
    origin: String,
    body: Arc<Mutex<String>>,
}

impl Feed {
    fn serve(body: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let origin = format!(
            "http://127.0.0.1:{}",
            listener.local_addr().expect("an address").port()
        );
        let body = Arc::new(Mutex::new(body.to_owned()));
        let served = Arc::clone(&body);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|read| read > 0) && line.trim() != "" {
                    line.clear();
                }
                let body = served
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                let _ = write!(
                    &stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/calendar\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Self { origin, body }
    }

    fn set(&self, body: &str) {
        body.clone_into(&mut self.body.lock().unwrap_or_else(PoisonError::into_inner));
    }
}

/// The calendar's link, set; nothing else is.
struct Links(BTreeMap<String, String>);

impl Resolves for Links {
    fn get(
        &self,
        name: &str,
        _reason: &str,
    ) -> Result<Option<Secret>, suzatary_io::secrets::SecretError> {
        Ok(self.0.get(name).map(Secret::new))
    }
}

/// Every day at nine since 2020, and a meeting today that the second feed cancels.
fn calendar(standup: &str) -> String {
    let today = chrono::Utc::now().date_naive().format("%Y%m%d");
    format!(
        "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:daily@example.test\r\nDTSTART:20200101T090000Z\r\n\
         DURATION:PT30M\r\nRRULE:FREQ=DAILY\r\nSUMMARY:Daily\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:review@example.test\r\nDTSTART:{today}T150000Z\r\nDTEND:{today}T160000Z\r\n\
         SUMMARY:Review\r\nSTATUS:{standup}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    )
}

fn components() -> PathBuf {
    std::env::var_os("SUZATARY_PLUGINS").map_or_else(
        || panic!("SUZATARY_PLUGINS is not set: it names `nix build .#suzatary-plugins`'s output"),
        PathBuf::from,
    )
}

fn manifest(origin: &str) -> Manifest {
    toml::from_str(&format!(
        r#"
        name = "ics"
        component = "ics.wasm"
        schema = "ics.schema"
        world = "source@0.3.0"
        origins = ["{origin}"]
        credentials = [
            {{ name = "PLUGIN_ICS_HOME", into = "url" }},
            {{ name = "PLUGIN_ICS_WORK", into = "url" }},
        ]
        "#
    ))
    .expect("the calendar's manifest")
}

fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .map(|entry| {
                    entry
                        .expect("an entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
#[allow(clippy::too_many_lines)]
fn a_sync_learns_once_keeps_its_ledger_and_reads_a_cancellation() {
    let feed = Feed::serve(&calendar("CONFIRMED"));
    let scratch = tempfile::tempdir().expect("a scratch dir");
    let manifest = manifest(&feed.origin);
    let plugins = Plugins {
        manifests: vec![manifest.clone()],
        cache: scratch.path().join("cache"),
        user: scratch.path().join("user"),
    };
    let effects = Effects {
        http: Arc::new(Ureq::new()),
        secrets: Arc::new(Links(BTreeMap::from([(
            "PLUGIN_ICS_HOME".to_owned(),
            format!("{}/calendar/private-s3cret/basic.ics", feed.origin),
        )]))),
        clock: Arc::new(SystemClock),
    };
    let host = Host::new(Budget::default()).expect("the host");
    let sync = |fresh| plugin::sync(&host, &manifest, &components(), &plugins, &effects, fresh);

    let learnt = |report: &plugin::Report| report.learnt.expect("stored");
    let first = sync(false);
    assert!(first.stored(), "{}", first.line);
    assert!(learnt(&first).written >= 60, "{}", first.line);
    assert!(first.line.starts_with("plugin ics: "), "{}", first.line);
    assert!(first.line.contains("PLUGIN_ICS_HOME 200"), "{}", first.line);
    assert!(
        first
            .line
            .contains("PLUGIN_ICS_WORK refused (PLUGIN_ICS_WORK is declared and not set)"),
        "{}",
        first.line
    );
    assert!(!first.line.contains("s3cret"), "the link is never said");
    assert_eq!(
        files(&plugins.cache.join("plugins/ics/store")),
        [".gitattributes", ".lock", "objects", "schema", "store.lock"]
    );
    assert_eq!(
        files(&plugins.cache.join("plugins/ics")),
        ["cursor", "ledger.json", "store"]
    );
    let ledger = plugin::ledger(&plugins, manifest.name()).expect("the ledger reads");
    let synced = |link: &str| {
        ledger
            .links
            .health(link)
            .and_then(suzatary_core::mail::health::Health::succeeded)
    };
    assert!(synced("PLUGIN_ICS_HOME").is_some());
    assert_eq!(synced("PLUGIN_ICS_WORK"), None);
    assert!(
        !std::fs::read_to_string(plugins.cache.join("plugins/ics/ledger.json"))
            .expect("a ledger")
            .contains("s3cret")
    );

    let again = sync(false);
    assert!(again.stored());
    assert_eq!(
        (learnt(&again).written, learnt(&again).known),
        (0, 0),
        "an unchanged feed reports nothing: {}",
        again.line
    );

    let fresh = sync(true);
    assert_eq!(
        learnt(&fresh).written,
        0,
        "learning from nothing learns only what is new: {}",
        fresh.line
    );
    assert!(learnt(&fresh).known >= 60);

    feed.set(&calendar("CANCELLED"));
    let cancelled = sync(false);
    assert_eq!(learnt(&cancelled).written, 1, "{}", cancelled.line);
    let store = PluginStore::at(plugins.store(manifest.name()))
        .read()
        .expect("reads")
        .expect("a store");
    let read = Calendar::of(&store.nodes().expect("reads"));
    let today = chrono::Utc::now().date_naive();
    let days = read.days(today, 1, &|utc| utc);
    let review = days[0]
        .entries
        .iter()
        .find(|entry| entry.content.title() == "Review")
        .expect("today's review");
    assert_eq!(review.content.status(), Status::Cancelled);

    std::fs::write(plugins.cache.join("plugins/ics/cursor"), b"not a cursor").expect("writes");
    let restarted = sync(false);
    assert!(restarted.stored());
    assert!(restarted.line.contains("began again"), "{}", restarted.line);
    assert_eq!(learnt(&restarted).written, 0);

    // A cursor that is there and cannot be read fails the run, and the ledger says so, rather
    // than reading as none and reporting the whole window again.
    let cursor = plugins.cache.join("plugins/ics/cursor");
    std::fs::remove_file(&cursor).expect("removed");
    std::fs::create_dir(&cursor).expect("a directory where the cursor belongs");
    let unread = sync(false);
    assert!(!unread.stored(), "{}", unread.line);
    assert!(
        unread.line.contains("its cursor could not be read"),
        "{}",
        unread.line
    );
    let last = plugin::ledger(&plugins, manifest.name())
        .expect("the ledger reads")
        .last
        .expect("the run is recorded");
    assert!(
        matches!(&last.came, Came::Failed { why } if why.contains("its cursor could not be read")),
        "{last:?}"
    );
}

/// A bundle in the user repository loads and is admitted by the same laws as the app's, and a
/// path a link turns outside that repository is refused before anything of it is read.
#[test]
#[allow(clippy::too_many_lines)]
fn a_plugin_from_the_user_repository_loads_and_one_reaching_outside_it_is_refused() {
    let feed = Feed::serve(&calendar("CONFIRMED"));
    let scratch = tempfile::tempdir().expect("a scratch dir");
    let user = scratch.path().join("user");
    std::fs::create_dir_all(user.join("plugins/mine")).expect("the user's plugins");
    for file in ["ics.wasm", "ics.schema"] {
        std::fs::copy(
            components().join(file),
            user.join("plugins/mine").join(file),
        )
        .expect("a bundle in the user repository");
    }
    let pinned = |file: &str| {
        Digest::of(&std::fs::read(components().join(file)).expect("a reference file")).to_string()
    };
    let manifest = |component: &str| -> Manifest {
        toml::from_str(&format!(
            r#"
            name = "ics"
            from = "user"
            component = "{component}"
            component_sha256 = "{}"
            schema = "plugins/mine/ics.schema"
            schema_sha256 = "{}"
            world = "source@0.3.0"
            origins = ["{}"]
            credentials = [{{ name = "PLUGIN_ICS_HOME", into = "url" }}]
            "#,
            pinned("ics.wasm"),
            pinned("ics.schema"),
            feed.origin
        ))
        .expect("a manifest from the user repository")
    };
    let mine = manifest("plugins/mine/ics.wasm");
    let plugins = Plugins {
        manifests: vec![mine.clone()],
        cache: scratch.path().join("cache"),
        user: user.clone(),
    };
    let effects = Effects {
        http: Arc::new(Ureq::new()),
        secrets: Arc::new(Links(BTreeMap::from([(
            "PLUGIN_ICS_HOME".to_owned(),
            format!("{}/calendar/private-s3cret/basic.ics", feed.origin),
        )]))),
        clock: Arc::new(SystemClock),
    };
    let host = Host::new(Budget::default()).expect("the host");
    // The app's directory holds nothing here: the bundle is read from the user's.
    let nowhere = scratch.path().join("no-app-plugins");
    let synced = plugin::sync(&host, &mine, &nowhere, &plugins, &effects, false);
    assert!(synced.stored(), "{}", synced.line);
    assert!(
        synced.learnt.expect("stored").written >= 60,
        "{}",
        synced.line
    );
    let admitted = plugin::ledger(&plugins, mine.name())
        .expect("the ledger reads")
        .admitted
        .expect("the admission is recorded");
    assert_eq!(
        (admitted.component, admitted.schema),
        (pinned("ics.wasm"), pinned("ics.schema")),
        "the very bytes loaded"
    );

    // A component swapped in the repository is not the file approved, and is not run. The
    // copy may be read-only (from the store), so the swap replaces the file rather than
    // writing through it.
    let component = user.join("plugins/mine/ics.wasm");
    std::fs::remove_file(&component).expect("the approved component goes");
    std::fs::write(&component, b"\0asm\x0d\0\x01\0").expect("a swapped component");
    let swapped = plugin::sync(&host, &mine, &nowhere, &plugins, &effects, false);
    assert!(!swapped.stored());
    assert!(
        swapped.line.contains("not the file approved"),
        "{}",
        swapped.line
    );
    std::fs::copy(
        components().join("ics.wasm"),
        user.join("plugins/mine/ics.wasm"),
    )
    .expect("the approved component again");
    // So is a swapped schema, though the component is the one approved.
    let schema = user.join("plugins/mine/ics.schema");
    let approved = std::fs::read(&schema).expect("the approved schema");
    std::fs::remove_file(&schema).expect("the approved schema goes");
    std::fs::write(&schema, [approved.as_slice(), b"\n"].concat()).expect("a swapped schema");
    let swapped = plugin::sync(&host, &mine, &nowhere, &plugins, &effects, false);
    assert!(!swapped.stored());
    assert!(
        swapped.line.contains("its schema hashes to"),
        "{}",
        swapped.line
    );
    std::fs::write(&schema, approved).expect("the approved schema again");

    // Admitted, and then its store cannot begin (a file where its directory goes): the
    // admission is recorded all the same, naming the bytes the host admitted.
    let unbegun = Plugins {
        cache: scratch.path().join("other-cache"),
        ..plugins.clone()
    };
    let store = unbegun.store(mine.name());
    std::fs::create_dir_all(store.parent().expect("a parent")).expect("the plugin's directory");
    std::fs::write(&store, b"a file, not a store").expect("a store that cannot begin");
    let stuck = plugin::sync(&host, &mine, &nowhere, &unbegun, &effects, false);
    assert!(stuck.line.contains("could not begin"), "{}", stuck.line);
    let admitted = plugin::ledger(&unbegun, mine.name())
        .expect("the ledger reads")
        .admitted
        .expect("the admission is recorded though the store did not begin");
    assert_eq!(admitted.component, pinned("ics.wasm"));

    let outside = scratch.path().join("elsewhere.wasm");
    std::fs::copy(components().join("ics.wasm"), &outside).expect("a component outside");
    std::os::unix::fs::symlink(&outside, user.join("plugins/escape.wasm")).expect("a link");
    let escaping = manifest("plugins/escape.wasm");
    let refused = plugin::sync(&host, &escaping, &nowhere, &plugins, &effects, false);
    assert!(!refused.stored());
    assert!(
        refused.line.contains("resolves outside"),
        "{}",
        refused.line
    );
    let climbing = r#"
        name = "ics"
        from = "user"
        component = "../ics.wasm"
        schema = "ics.schema"
        world = "source@0.3.0"
    "#;
    assert!(
        toml::from_str::<Manifest>(climbing).is_err(),
        "a path out of the repository is refused when the manifest is read"
    );
}

/// The calendar's feed with a `Due:` entry and a meeting tomorrow, synced, and the due actor
/// granted the calendar's store: what each actor test starts from.
struct Due {
    _feed: Feed,
    _scratch: tempfile::TempDir,
    actor: Manifest,
    plugins: Plugins,
    effects: Effects,
    host: Host,
    conversations: suzatary_io::conversations::ConversationStores,
    proposals: Objects<suzatary_core::secretary::inbox_event::InboxEvent>,
}

/// The due actor's manifest, granted `reads` where one is named.
fn due_actor(reads: Option<&str>) -> Manifest {
    toml::from_str(&format!(
        "name = \"due\"\ncomponent = \"due.wasm\"\nschema = \"ics.schema\"\n\
         world = \"actor@0.3.0\"\n{}",
        reads.map_or_else(String::new, |name| format!("reads = \"{name}\"\n"))
    ))
    .expect("the due actor's manifest")
}

impl Due {
    fn synced() -> Self {
        let tomorrow = (chrono::Utc::now() + chrono::Duration::days(1))
            .date_naive()
            .format("%Y%m%d");
        let feed = Feed::serve(&format!(
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:visa@example.test\r\n\
             DTSTART;VALUE=DATE:{tomorrow}\r\nSUMMARY:Due: renew the visa\r\nEND:VEVENT\r\n\
             BEGIN:VEVENT\r\nUID:standup@example.test\r\nDTSTART:{tomorrow}T090000Z\r\n\
             DTEND:{tomorrow}T093000Z\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        ));
        let scratch = tempfile::tempdir().expect("a scratch dir");
        let source = manifest(&feed.origin);
        let actor = due_actor(Some("ics"));
        let plugins = Plugins {
            manifests: vec![source.clone(), actor.clone()],
            cache: scratch.path().join("cache"),
            user: scratch.path().join("user"),
        };
        let effects = Effects {
            http: Arc::new(Ureq::new()),
            secrets: Arc::new(Links(BTreeMap::from([(
                "PLUGIN_ICS_HOME".to_owned(),
                format!("{}/calendar/private-s3cret/basic.ics", feed.origin),
            )]))),
            clock: Arc::new(SystemClock),
        };
        let host = Host::new(Budget::default()).expect("the host");
        let synced = plugin::sync(&host, &source, &components(), &plugins, &effects, false);
        assert!(synced.stored(), "{}", synced.line);
        Self {
            conversations: suzatary_io::conversations::ConversationStores::open(
                scratch.path().join("lines"),
                scratch.path().join("lines-keys"),
                Arc::new(suzatary_core::secretary::conversations::Forgot::default()),
            ),
            proposals: Objects::open(scratch.path().join("inbox")),
            _feed: feed,
            _scratch: scratch,
            actor,
            plugins,
            effects,
            host,
        }
    }

    /// Ask `actor`, every change checked by `check`.
    fn ask(
        &self,
        actor: &Manifest,
        check: &dyn Fn(&Changeset) -> Result<(), Refusal>,
    ) -> plugin::Decision {
        plugin::ask(
            &self.host,
            actor,
            &components(),
            &self.plugins,
            &self.effects,
            &plugin::Inbox {
                conversations: self.conversations.conversations(),
                proposals: &self.proposals,
                check,
            },
        )
    }

    fn written(&self) -> (usize, usize) {
        (
            self.conversations.lines.objects().expect("reads").len(),
            self.proposals.objects().expect("reads").len(),
        )
    }
}

/// An actor reads only its own store unless its manifest grants another, and a change its chain
/// refuses is refused before it is filed, naming the plugin.
#[test]
fn an_actor_reads_what_it_is_granted_and_files_only_what_builds() {
    let due = Due::synced();
    let own = due.ask(&due_actor(None), &|_| Ok(()));
    assert_eq!(own.filed, Some(Vec::new()), "{}", own.line);

    let unbuilt = due.ask(&due.actor, &|_| {
        Err(Refusal::conflict("a todo duevisa is on life already"))
    });
    assert_eq!(unbuilt.filed, Some(Vec::new()), "{}", unbuilt.line);
    assert_eq!(
        unbuilt
            .refused
            .iter()
            .map(|refused| (refused.plugin.as_str(), &refused.refusal))
            .collect::<Vec<_>>(),
        [(
            "due",
            &ProposalRefusal::Unbuilt(Said::clip("a todo duevisa is on life already"))
        )],
    );
    assert!(due.proposals.objects().expect("reads").is_empty());
}

/// An actor is asked after the sources, from the one store its manifest grants it: each act it
/// offers is filed in the Inbox once, as the plugin's, drawn in the plugin's own conversation,
/// and asking again on the next tick files nothing. Nothing is performed: the card waits.
#[test]
fn an_actor_offers_from_the_calendars_store_and_each_act_is_filed_once() {
    let due = Due::synced();
    let first = due.ask(&due.actor, &|_| Ok(()));
    let filed = first.filed.clone().expect("filed");
    assert_eq!(filed.len(), 1, "{}", first.line);
    assert!(
        first
            .line
            .starts_with("plugin due: 1 offer, 1 new in the Inbox, 0 waiting"),
        "{}",
        first.line
    );
    let fold = inbox::fold(&due.proposals).expect("reads");
    let object = fold.get(&filed[0]).expect("asked");
    assert_eq!(
        object.asked.proposer,
        Proposer::Plugin(PluginName::parse("due").expect("a name"))
    );
    assert_eq!(object.asked.asked.to_string(), "proposal:plugin-due/0");
    assert!(
        matches!(object.reading, Reading::Standing),
        "it waits for a tap"
    );
    let Proposal::Changes(changes) = &object.asked.proposal else {
        panic!("a change to a chain");
    };
    assert_eq!(changes.chain(), "life");
    assert!(
        plugin::ledger(&due.plugins, due.actor.name())
            .expect("the ledger reads")
            .last
            .is_some_and(|run| run.stored())
    );

    let written = due.written();
    let again = due.ask(&due.actor, &|_| Ok(()));
    assert_eq!(again.filed, Some(filed), "{}", again.line);
    assert!(again.line.contains("0 new in the Inbox"), "{}", again.line);
    assert_eq!(due.written(), written, "nothing written");
}
