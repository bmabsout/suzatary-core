//! `suzatary plugin sync` — run each `source` plugin `[[plugins]]` declares, and learn what it
//! reports into the cache tier; then ask each `actor` what it proposes from its store, and file
//! every proposal it may make in the Inbox, as the plugin's, for the owner's tap.
//!
//! A run is monotone and idempotent, and held to it: each run is replayed over what the outside
//! world answered it (`suzatary_plugin::Trial`), and only a run whose items are events of its
//! declared schema, whose replay from the same cursor reports the same, and whose replay from its
//! own cursor teaches nothing more (`suzatary_core::plugin::Law`) is learned, each item only where
//! its store does not already read it as reported. The cursor is saved after the learning, so a run cut short is simply run again. The box runs it on a timer
//! with no lock beyond the store's own; each run prints one journal line and keeps its refusals
//! and the plugin's log lines in the plugin's ledger.
//!
//! An actor reads one store: its own, or the one plugin's its manifest names (`reads`), the
//! owner's grant, and only where that store is at the actor's declared schema. What it proposes
//! is filed by the act, so
//! asking it again on the next tick files nothing it filed before, and nothing it proposes is
//! performed but by a tap.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::{Args as ClapArgs, Subcommand};
use prodrome_core::declared::{Declaration, Declared};
use prodrome_core::fpl::instant_of;
use suzatary_core::plugin::{
    Admitted, Answer, Answers, BundleRoot, Came, Digest, Knowledge, LastRun, Learnt, Ledger,
    Manifest, ProposalRefusal, Refused, Said, World, learn,
};
use suzatary_core::refusal::Refusal;
use suzatary_core::secretary::changes::{self, Changeset};
use suzatary_core::secretary::filed::{Filed, Offered};
use suzatary_core::secretary::inbox::ProposalId;
use suzatary_core::secretary::inbox_event::InboxEvent;
use suzatary_core::secretary::proposer::Proposer;
use suzatary_core::store::Prodrome;
use suzatary_io::clock::SystemClock;
use suzatary_io::durable;
use suzatary_io::http::Ureq;
use suzatary_io::objects::Objects;
use suzatary_io::plugin_store::{Begun, PluginStore};
use suzatary_io::present::Present as _;
use suzatary_plugin::{Budget, Effects, Fetch, Host, Level, Log, Run, Source, Trial};

use crate::instance::{Instance, Plugins};

/// Where the app's reference plugins are, when no `--components` names a directory: the box's
/// timer roots `.#suzatary-plugins` and passes it here.
pub const COMPONENTS: &str = "SUZATARY_PLUGINS";

/// `suzatary plugin …`.
#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Sync each named plugin (every declared one, and every Google account's calendars, when
    /// none is named): a source into the cache tier, then an actor into the Inbox.
    Sync(SyncArgs),
}

/// `suzatary plugin sync`.
#[derive(Debug, ClapArgs)]
pub struct SyncArgs {
    /// The plugins to sync, by name; every plugin `[[plugins]]` declares when none is named.
    names: Vec<String>,
    /// The directory the app's reference plugins are read from: `$SUZATARY_PLUGINS`, else the
    /// data root. A manifest `from = "user"` reads the user repository whatever this says.
    #[arg(long)]
    components: Option<PathBuf>,
    /// Start from no cursor: the plugin reports everything in its window again, and the store
    /// learns only what it does not already hold.
    #[arg(long)]
    fresh: bool,
}

/// Run it.
///
/// # Errors
/// An unreadable instance, a plugin not declared, or any plugin's run that did not end with
/// what it learned stored; every plugin named still runs.
pub fn run(path: &Path, args: &Args) -> anyhow::Result<()> {
    match &args.command {
        Command::Sync(args) => sync_all(path, args),
    }
}

/// `suzatary plugin sync`.
fn sync_all(path: &Path, args: &SyncArgs) -> anyhow::Result<()> {
    let instance = Instance::open(path)?;
    let plugins = &instance.plugins;
    let chosen: Vec<&Manifest> = if args.names.is_empty() {
        // The sources first, so an actor decides from what this tick learned.
        let mut every: Vec<&Manifest> = plugins.manifests.iter().collect();
        every.sort_by_key(|manifest| manifest.world() == World::Actor);
        every
    } else {
        args.names
            .iter()
            .map(|name| {
                plugins.named(name).with_context(|| {
                    format!("no plugin named {name:?} in {}", instance.path.display())
                })
            })
            .collect::<anyhow::Result<_>>()?
    };
    // Each Google account's calendars ride the same timer, when no plugin is named.
    let (google, calendars_stored) = if args.names.is_empty() {
        super::google::sync_calendars(&instance)
    } else {
        (Vec::new(), true)
    };
    for line in &google {
        println!("{line}");
    }
    if chosen.is_empty() {
        if google.is_empty() {
            println!("plugin: none declared in {}", instance.path.display());
        }
        return if calendars_stored {
            Ok(())
        } else {
            bail!("a Google account's calendars were not stored: the lines above say why")
        };
    }
    let components = components(&instance, args.components.clone());
    let host = Host::new(Budget::default()).context("starting the plugin host")?;
    let effects = effects(&instance);
    let conversations = instance.conversation_stores();
    let proposals = instance.inbox_store();
    let render = suzatary_io::render::Typst::new(
        instance.code().to_path_buf(),
        instance.paths.markup_cache.clone(),
        suzatary_io::typeset::Closed::this_binary(),
    );
    let check = |changeset: &Changeset| checked(&instance, &render, changeset);
    let inbox = Inbox {
        conversations: conversations.conversations(),
        proposals: &proposals,
        check: &check,
    };
    let mut unstored = Vec::new();
    if !calendars_stored {
        unstored.push("a Google account's calendars".to_owned());
    }
    for manifest in chosen {
        let (line, ok) = match manifest.world() {
            World::Source => {
                let report = sync(&host, manifest, &components, plugins, &effects, args.fresh);
                let stored = report.stored();
                (report.line, stored)
            }
            World::Actor => {
                let decision = ask(&host, manifest, &components, plugins, &effects, &inbox);
                let ok = decision.filed.is_some();
                (decision.line, ok)
            }
        };
        println!("{line}");
        if !ok {
            unstored.push(manifest.name().to_string());
        }
    }
    if unstored.is_empty() {
        Ok(())
    } else {
        bail!(
            "plugin sync did not store or file {}: the lines above say why",
            unstored.join(", ")
        )
    }
}

/// Where the reference plugins are read from: `named`, else `$SUZATARY_PLUGINS`, else the
/// data root.
#[must_use]
pub fn components(instance: &Instance, named: Option<PathBuf>) -> PathBuf {
    named
        .or_else(|| std::env::var_os(COMPONENTS).map(PathBuf::from))
        .unwrap_or_else(|| instance.data().to_path_buf())
}

/// What a plugin's sync reaches: the transport bounding every plugin (no redirect, nothing
/// resolving off the public internet, so no manifest aims a plugin at this box or the
/// tailnet), the instance's secret store, and the clock.
#[must_use]
pub fn effects(instance: &Instance) -> Effects {
    Effects {
        http: Arc::new(Ureq::public_only()),
        secrets: Arc::new(instance.secrets()),
        clock: Arc::new(SystemClock),
    }
}

/// What one plugin's sync came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The journal's line.
    pub line: String,
    /// What the store learned, or `None` when the run's learning was not stored (a run that
    /// reports nothing stores nothing, and that is stored).
    pub learnt: Option<Learnt>,
}

impl Report {
    /// Whether what the run reported was stored.
    #[must_use]
    pub fn stored(&self) -> bool {
        self.learnt.is_some()
    }
}

/// Sync one plugin with `effects`: load its component from `components`, run it from its
/// cursor, admit each item into its declared schema, learn the admitted into its store, save the
/// cursor, and keep the run in the plugin's ledger.
///
/// Never an error: whatever went wrong is the report's line and the ledger's, and the next
/// run tries again.
#[must_use]
pub fn sync(
    host: &Host,
    manifest: &Manifest,
    components: &Path,
    plugins: &Plugins,
    effects: &Effects,
    fresh: bool,
) -> Report {
    let name = manifest.name();
    let dir = plugins.beside(name);
    let at = effects.clock.now();
    // A ledger that cannot be read is left as it is for the operator, never written over.
    let ledger = match ledger(plugins, name) {
        Ok(ledger) => ledger,
        Err(why) => {
            return Report {
                line: format!("plugin {name}: {why}"),
                learnt: None,
            };
        }
    };
    let failed = |ledger: &Ledger, why: String| {
        let line = format!("plugin {name}: {why}");
        keep(&dir, ledger.clone(), unrun(at, &line), &[]);
        Report { line, learnt: None }
    };
    let (bytes, schema) = match bundle(manifest, components, plugins) {
        Ok(read) => read,
        Err(why) => return failed(&ledger, why),
    };
    let source = match host.load(manifest.clone(), &bytes, &schema) {
        Ok(source) => source,
        Err(refused) => return failed(&ledger, format!("not loaded: {refused}")),
    };
    let ledger = Ledger {
        admitted: Some(Admitted {
            at: instant_of(at),
            component: Digest::of(&bytes).to_string(),
            schema: Digest::of(schema.as_bytes()).to_string(),
        }),
        ..ledger
    };
    let store = match PluginStore::at(plugins.store(name)).begin(source.schema()) {
        // A store begun afresh holds nothing its cursor says was learned, so the run starts
        // from no cursor and reports everything in its window again.
        Ok(Begun::Afresh(store)) => (store, None),
        Ok(Begun::Kept(store)) if fresh => (store, None),
        Ok(Begun::Kept(store)) => match std::fs::read(dir.join("cursor")).present() {
            Ok(cursor) => (store, cursor),
            Err(err) => return failed(&ledger, format!("its cursor could not be read: {err}")),
        },
        // Admitted and not run: the record still names the bytes the host admitted.
        Err(err) => return failed(&ledger, format!("its store could not begin: {err}")),
    };
    let (store, cursor) = store;
    let (trial, restarted) = tried(&source, effects, cursor.as_deref());
    let run = &trial.run;
    let fetches: Vec<(String, Answer)> = run.fetches.iter().filter_map(link).collect();
    let (learnt, what) = match learned(manifest, source.schema(), &store, &dir, &trial) {
        Ok((learnt, what)) => (Some(learnt), what),
        Err(what) => (None, what),
    };
    let links: Vec<String> = fetches
        .iter()
        .map(|(link, answer)| match answer {
            Answer::Status(status) => format!("{link} {status}"),
            Answer::Refused(why) => format!("{link} refused ({why})"),
        })
        .collect();
    let warnings = run
        .log
        .lines
        .iter()
        .filter(|line| line.level == Level::Warn)
        .count();
    let line = format!(
        "plugin {name}: {what}{}; links: {}; {warnings} warning{} in its log{}",
        if restarted {
            " (its cursor was unreadable, so it began again)"
        } else {
            ""
        },
        if links.is_empty() {
            "none fetched".to_owned()
        } else {
            links.join(", ")
        },
        if warnings == 1 { "" } else { "s" },
        if run.log.dropped > 0 {
            format!(", {} more dropped", run.log.dropped)
        } else {
            String::new()
        },
    );
    let came = match learnt {
        Some(_) => Came::Learnt {
            summary: line.clone(),
        },
        None => Came::Failed { why: line.clone() },
    };
    keep(
        &dir,
        ledger,
        ran(at, came, &run.refused, &run.log),
        &fetches,
    );
    Report { line, learnt }
}

/// The two stores an actor's offers are filed in, the Inbox's proposals and the conversation
/// each is drawn in as a card, and the check each offer passes first.
#[derive(Clone, Copy)]
pub struct Inbox<'a> {
    /// The conversations.
    pub conversations: suzatary_core::secretary::conversations::Conversations<'a>,
    /// The Inbox's proposals.
    pub proposals: &'a dyn Prodrome<InboxEvent>,
    /// Whether the change would be applied as it stands: built against its chain, as
    /// `propose_changes` builds the secretary's.
    pub check: &'a dyn Fn(&Changeset) -> Result<(), Refusal>,
}

/// What asking one actor came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The journal's line.
    pub line: String,
    /// The Inbox's proposal for each act it offered, each once, whether this tick filed it or
    /// an earlier one did; `None` when it decided nothing that could be filed.
    pub filed: Option<Vec<ProposalId>>,
    /// Every proposal the host refused, naming the plugin.
    pub refused: Vec<Refused>,
}

/// Ask one actor what it proposes: load its component from `components`, hand it every event of
/// the one store it reads, admit what it proposes, file each offer in the Inbox as the
/// plugin's, and keep the decision in the plugin's ledger.
///
/// Never an error, as [`sync`] is not: whatever went wrong is the decision's line and the
/// ledger's. Nothing here performs an act: an offer waits for the owner's tap.
#[must_use]
pub fn ask(
    host: &Host,
    manifest: &Manifest,
    components: &Path,
    plugins: &Plugins,
    effects: &Effects,
    inbox: &Inbox<'_>,
) -> Decision {
    let name = manifest.name();
    let dir = plugins.beside(name);
    let at = effects.clock.now();
    // A ledger that cannot be read is left as it is for the operator, never written over.
    let ledger = match ledger(plugins, name) {
        Ok(ledger) => ledger,
        Err(why) => {
            return Decision {
                line: format!("plugin {name}: {why}"),
                filed: None,
                refused: Vec::new(),
            };
        }
    };
    let failed = |ledger: &Ledger, why: String, refused: Vec<Refused>| {
        let line = format!("plugin {name}: {why}");
        keep(&dir, ledger.clone(), unrun(at, &line), &[]);
        Decision {
            line,
            filed: None,
            refused,
        }
    };
    let (bytes, schema) = match bundle(manifest, components, plugins) {
        Ok(read) => read,
        Err(why) => return failed(&ledger, why, Vec::new()),
    };
    let actor = match host.load_actor(manifest.clone(), &bytes, &schema) {
        Ok(actor) => actor,
        Err(refused) => return failed(&ledger, format!("not loaded: {refused}"), Vec::new()),
    };
    let ledger = Ledger {
        admitted: Some(Admitted {
            at: instant_of(at),
            component: Digest::of(&bytes).to_string(),
            schema: Digest::of(schema.as_bytes()).to_string(),
        }),
        ..ledger
    };
    let acts = match store_of(plugins, manifest, actor.schema()).and_then(|events| {
        actor
            .acts(events)
            .map_err(|foreign| format!("its store holds {foreign}"))
    }) {
        Ok(acts) => acts,
        Err(why) => return failed(&ledger, why, Vec::new()),
    };
    let decided = actor.decide(Arc::clone(&effects.clock), &acts);
    let offers = match decided.result {
        Ok(offers) => offers,
        Err(stopped) => {
            return failed(
                &ledger,
                format!("decided nothing: {stopped}"),
                decided.refused,
            );
        }
    };
    let Filings {
        objects,
        drawn,
        waiting,
        refused: unbuilt,
        elsewhere,
    } = match filed_all(inbox, at, &offers) {
        Ok(each) => each,
        Err(refusal) => {
            return failed(
                &ledger,
                format!("the Inbox refused its offer: {}", refusal.message()),
                decided.refused,
            );
        }
    };
    let refused: Vec<Refused> = decided.refused.into_iter().chain(unbuilt).collect();
    let line = journal(name, offers.len(), drawn, waiting, &elsewhere, &refused);
    keep(
        &dir,
        ledger,
        ran(
            at,
            Came::Learnt {
                summary: line.clone(),
            },
            &refused,
            &decided.log,
        ),
        &[],
    );
    Decision {
        line,
        filed: Some(objects),
        refused,
    }
}

/// The journal's line for one actor's decision: what it offered, what this tick drew, what
/// waited, who else already asked for what it offered, and every refusal by name.
fn journal(
    name: &suzatary_core::plugin::PluginName,
    offers: usize,
    drawn: usize,
    waiting: usize,
    elsewhere: &[Proposer],
    refused: &[Refused],
) -> String {
    format!(
        "plugin {name}: {offers} offer{}, {drawn} new in the Inbox, {waiting} waiting until the \
         owner answers its others{}, {} refused{}",
        if offers == 1 { "" } else { "s" },
        elsewhere
            .iter()
            .map(|asker| format!(", one already asked by {}", asker.named()))
            .collect::<Vec<_>>()
            .concat(),
        refused.len(),
        refused
            .iter()
            .map(|one| format!("; {one}"))
            .collect::<Vec<_>>()
            .concat(),
    )
}

/// Whether `changeset` would be applied as it stands: built against the chain it names, through
/// the same check `propose_changes` runs, now.
fn checked(
    instance: &Instance,
    render: &suzatary_io::render::Typst,
    changeset: &Changeset,
) -> Result<(), Refusal> {
    let chain = instance
        .chain(Some(changeset.chain()))
        .map_err(|_| Refusal::bad_request(format!("no chain named {}", changeset.chain())))?;
    let categories: Vec<&str> = instance
        .deployment
        .categories
        .keys()
        .map(String::as_str)
        .collect();
    changes::checked(
        changeset,
        &crate::commands::store_of(chain),
        suzatary_core::zone::Moment {
            at: suzatary_core::capability::Clock::now(&SystemClock),
            zone: instance.persona.zone,
        },
        &categories,
        render,
    )
}

/// File every offer in the Inbox.
fn filed_all(
    inbox: &Inbox<'_>,
    at: suzatary_core::capability::Datetime,
    offers: &[suzatary_core::plugin::Offer],
) -> Result<Filings, Refusal> {
    let mut objects = Vec::with_capacity(offers.len());
    let mut drawn = 0usize;
    let mut waiting = 0usize;
    let mut refused = Vec::new();
    let mut elsewhere = Vec::new();
    for offer in offers {
        if let Err(refusal) = (inbox.check)(offer.act()) {
            refused.push(Refused {
                plugin: offer.plugin().clone(),
                refusal: ProposalRefusal::Unbuilt(Said::clip(refusal.message())),
            });
            continue;
        }
        objects.push(
            match suzatary_core::secretary::filed::offer(
                inbox.conversations,
                inbox.proposals,
                at,
                offer,
            )? {
                Offered::Filed(card, object) => {
                    drawn += usize::from(matches!(card, Filed::Drawn(_)));
                    object
                }
                Offered::Elsewhere(object, asker) => {
                    elsewhere.push(asker);
                    object
                }
                Offered::Waiting => {
                    waiting += 1;
                    continue;
                }
            },
        );
    }
    Ok(Filings {
        objects,
        drawn,
        waiting,
        refused,
        elsewhere,
    })
}

/// What filing a decision's offers came to.
struct Filings {
    /// The Inbox's proposal for each offer it holds.
    objects: Vec<ProposalId>,
    /// How many cards this filing drew.
    drawn: usize,
    /// How many new acts waited, the plugin's waiting proposals at their cap.
    waiting: usize,
    /// The offers the check refused.
    refused: Vec<Refused>,
    /// Who first asked for each act the Inbox already held from someone else.
    elsewhere: Vec<Proposer>,
}

/// Every event of the one store `manifest` reads (its own, or the one its manifest grants it),
/// where that store is at `schema`; none where it is at another or holds nothing yet.
fn store_of(
    plugins: &Plugins,
    manifest: &Manifest,
    schema: &Declaration,
) -> Result<Vec<Declared>, String> {
    let reads = manifest.reads();
    if plugins.named(reads.as_str()).is_none() {
        return Err(format!(
            "it reads {reads}'s store, and no plugin is named {reads}"
        ));
    }
    let root = plugins.store(reads);
    PluginStore::at(&root)
        .read_at(schema)
        .and_then(|store| store.map(|store| store.events()).transpose())
        .map(Option::unwrap_or_default)
        .map_err(|err| format!("the store at {} is unreadable: {err}", root.display()))
}

/// The plugin's component and its declared schema, each read once from under its bundle's root
/// (`components` for the app's, the user repository for the user's) and, for the user's, held
/// to the sha256 its manifest pins; the bytes returned are the bytes checked, and the host loads
/// those, so nothing swapped between the check and the load runs.
fn bundle(
    manifest: &Manifest,
    components: &Path,
    plugins: &Plugins,
) -> Result<(Vec<u8>, String), String> {
    let root = match manifest.root() {
        BundleRoot::App => components,
        BundleRoot::User { .. } => &plugins.user,
    };
    let component = inside(root, manifest.component().as_str())?;
    let bytes = std::fs::read(&component)
        .map_err(|err| format!("its component {} is unreadable: {err}", component.display()))?;
    let schema = inside(root, manifest.schema().as_str())?;
    let text = std::fs::read(&schema)
        .map_err(|err| format!("its schema {} is unreadable: {err}", schema.display()))?;
    if let BundleRoot::User {
        component: pinned_component,
        schema: pinned_schema,
    } = manifest.root()
    {
        for (what, pinned, read) in [
            ("component", pinned_component, &bytes),
            ("schema", pinned_schema, &text),
        ] {
            let found = Digest::of(read);
            if &found != pinned {
                return Err(format!(
                    "its {what} hashes to {found}, not the {pinned} its manifest pins, so it is \
                     not the file approved"
                ));
            }
        }
    }
    let text = String::from_utf8(text).map_err(|_| "its schema is not text".to_owned())?;
    Ok((bytes, text))
}

/// `relative` under `root`, once every link in it is followed: a manifest's path is plain
/// segments, and this refuses one that a link inside the root turns into a path outside it.
fn inside(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let base = root
        .canonicalize()
        .map_err(|err| format!("its bundle's root {} is unreadable: {err}", root.display()))?;
    let path = base
        .join(relative)
        .canonicalize()
        .map_err(|err| format!("{relative} is unreadable under {}: {err}", root.display()))?;
    if path.starts_with(&base) {
        Ok(path)
    } else {
        Err(format!(
            "{relative} resolves outside {}, and a bundle is read only from under its root",
            root.display()
        ))
    }
}

/// The trial of a run from `cursor`, and whether it began again from none: the cursor is a
/// cache-tier hint, never history, so one the plugin no longer reads is dropped, and learning
/// from nothing learns only what the store does not hold.
fn tried(source: &Source, effects: &Effects, cursor: Option<&[u8]>) -> (Trial, bool) {
    let trial = source.trial(effects, cursor);
    if cursor.is_some()
        && matches!(
            trial.run.result,
            Err(suzatary_plugin::RunError::BadCursor(_))
        )
    {
        (source.trial(effects, None), true)
    } else {
        (trial, false)
    }
}

/// Hold a trial to its laws, learn the run's events, then save its cursor: what was learnt and
/// the line's opening clause, or, when nothing was stored, only the clause. A run that breaks a
/// law learns nothing and keeps its old cursor, so the next run asks again.
fn learned(
    manifest: &Manifest,
    schema: &Declaration,
    store: &Objects<Declared>,
    dir: &Path,
    trial: &Trial,
) -> Result<(Learnt, String), String> {
    let synced = trial
        .run
        .result
        .as_ref()
        .map_err(|failed| format!("the run failed: {failed}"))?;
    let failed = |run: &Run| run.result.as_ref().err().map(ToString::to_string);
    let (again, onward) = (failed(&trial.again), trial.onward.as_ref().and_then(failed));
    let answers = Answers {
        run: synced,
        again: trial
            .again
            .result
            .as_ref()
            .map_err(|_| again.as_deref().unwrap_or_default()),
        onward: match &trial.onward {
            Some(run) => run
                .result
                .as_ref()
                .map_err(|_| onward.as_deref().unwrap_or_default()),
            None => Err("no cursor to replay from"),
        },
    };
    let known = Knowledge::of(
        &store
            .nodes()
            .map_err(|err| format!("its store is unreadable: {err}"))?,
    );
    let events = answers
        .hold(schema, manifest, &known)
        .map_err(|broken| format!("refused, {broken}"))?;
    let learnt = learn(store, events).map_err(|err| format!("its store refused: {err}"))?;
    let saved = durable::replace(&dir.join("cursor"), &synced.cursor, durable::Mode::Readable);
    let what = format!(
        "{} item{}, {} written, {} known",
        synced.items.len(),
        if synced.items.len() == 1 { "" } else { "s" },
        learnt.written,
        learnt.known,
    );
    match saved {
        Ok(()) => Ok((learnt, what)),
        // Learnt and not saved: the next run reports the same items again and learns nothing.
        Err(err) => Ok((learnt, format!("{what}, its cursor unsaved ({err})"))),
    }
}

/// A fetch as the ledger keeps it, under the declared credential it named; none for a fetch that
/// named none.
fn link(fetch: &Fetch) -> Option<(String, Answer)> {
    let answer = match &fetch.outcome {
        Ok(status) => Answer::Status(*status),
        Err(refusal) => Answer::Refused(refusal.clone()),
    };
    Some((fetch.credential.as_ref()?.to_string(), answer))
}

/// A run of the plugin: what came of it, and what it and the host said on the way.
fn ran(
    at: suzatary_core::capability::Datetime,
    came: Came,
    refused: &[impl ToString],
    log: &Log,
) -> LastRun {
    LastRun {
        at: instant_of(at),
        came,
        refusals: refused.iter().map(ToString::to_string).collect(),
        log: logged(log),
        dropped: log.dropped,
    }
}

/// A run that failed before the plugin ran, so it said nothing.
fn unrun(at: suzatary_core::capability::Datetime, why: &str) -> LastRun {
    LastRun {
        at: instant_of(at),
        came: Came::Failed {
            why: why.to_owned(),
        },
        refusals: Vec::new(),
        log: Vec::new(),
        dropped: 0,
    }
}

/// A plugin's log lines, as the ledger keeps them.
fn logged(log: &Log) -> Vec<String> {
    log.lines
        .iter()
        .map(|line| format!("{:?}: {}", line.level, line.text))
        .collect()
}

/// The ledger after this run, written beside the cursor; a ledger that cannot be written is
/// said on stderr and costs only the Settings page's dates.
fn keep(dir: &Path, ledger: Ledger, run: LastRun, fetches: &[(String, Answer)]) {
    let ledger = ledger.after(run, fetches);
    let written = serde_json::to_vec_pretty(&ledger)
        .map_err(|err| err.to_string())
        .and_then(|bytes| {
            durable::replace(&dir.join("ledger.json"), &bytes, durable::Mode::Readable)
                .map_err(|err| err.to_string())
        });
    if let Err(err) = written {
        eprintln!(
            "plugin: the ledger in {} was not written: {err}",
            dir.display()
        );
    }
}

/// The ledger of `name`, what the Settings page reads: an empty one before the first run.
///
/// # Errors
/// A ledger that is there and cannot be read or parsed, which is not an empty one.
pub fn ledger(
    plugins: &Plugins,
    name: &suzatary_core::plugin::PluginName,
) -> Result<Ledger, String> {
    let path = plugins.beside(name).join("ledger.json");
    let unread = |err: &dyn std::fmt::Display| {
        format!("its ledger {} could not be read: {err}", path.display())
    };
    match std::fs::read(&path).present().map_err(|err| unread(&err))? {
        None => Ok(Ledger::default()),
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|err| unread(&err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plugin never run has an empty ledger; one whose ledger is there and cannot be read (a
    /// directory where the file belongs) or does not parse is an error, never an empty ledger.
    #[test]
    fn only_a_missing_ledger_is_an_empty_one() {
        let dir = tempfile::tempdir().expect("a scratch dir");
        let plugins = Plugins {
            manifests: Vec::new(),
            cache: dir.path().to_path_buf(),
            user: dir.path().to_path_buf(),
        };
        let name = suzatary_core::plugin::PluginName::parse("due").expect("a name");
        assert_eq!(ledger(&plugins, &name), Ok(Ledger::default()));
        let file = plugins.beside(&name).join("ledger.json");
        std::fs::create_dir_all(&file).expect("a directory where the file belongs");
        assert!(ledger(&plugins, &name).is_err());
        std::fs::remove_dir(&file).expect("removed");
        std::fs::write(&file, "{").expect("half a ledger");
        assert!(ledger(&plugins, &name).is_err());
    }

    /// An act someone else already asked for files nothing, and the journal names who asked.
    #[test]
    fn the_journal_names_who_else_asked() {
        let name = suzatary_core::plugin::PluginName::parse("due").expect("a name");
        let line = journal(&name, 2, 1, 0, &[Proposer::Secretary], &[]);
        assert_eq!(
            line,
            "plugin due: 2 offers, 1 new in the Inbox, 0 waiting until the owner answers its \
             others, one already asked by the Suzatary secretary, 0 refused"
        );
    }
}
