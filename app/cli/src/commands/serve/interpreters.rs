//! The one place that chooses an interpreter for every capability.
//!
//! `suzatary-core` is generic over a record and `suzatary-io` implements the traits;
//! this names which implementation each gets, so a fixture is a substitution the
//! routes cannot tell apart.

use std::net::IpAddr;
use std::sync::Arc;

use suzatary_core::capability::{
    Compact, Conversation, Model, ModelError, Pick, Reply, SessionModel, Sessions, Usage, Weather,
    WeatherError, Whois, WhoisError,
};
use suzatary_core::drive::WritePort as _;
use suzatary_io::secrets::Secretspec;
use suzatary_io::whois::Tailscaled;

use suzatary_core::models::{Backend, Job, ModelEntry, Models, Reserve};
use suzatary_core::settings::DeclaredProvider;

use crate::instance::Instance;

/// Who names a peer.
///
/// The fixture arm is test-only and compiled out of the shipped build; it exists because
/// a headless browser here always arrives from loopback, which no daemon names.
pub enum Peers {
    /// The local daemon, over its unix socket.
    Daemon(Tailscaled),
    /// Every address is this login; test only.
    #[cfg(feature = "fixture")]
    Fixture(suzatary_core::capability::Login),
}

impl Whois for Peers {
    fn peer(&self, address: IpAddr) -> Result<suzatary_core::capability::Peer, WhoisError> {
        match self {
            Peers::Daemon(daemon) => daemon.peer(address),
            // Every peer is the fixture's writer, so none is this box.
            #[cfg(feature = "fixture")]
            Peers::Fixture(login) => Ok(suzatary_core::capability::Peer::Known(login.clone())),
        }
    }
}

/// Which model answers a conversation.
///
/// The scripted arm is test-only and compiled out of the shipped build, because no
/// browser test can drive a paid, networked, nondeterministic `claude -p`. Verbs are
/// dispatched on this side of the trait, so any arm can only answer text.
pub enum Brain {
    /// The `claude` CLI as a subprocess.
    Cli(suzatary_io::model::ClaudeCli),
    /// The Anthropic Messages API, usable when its key resolves.
    Anthropic(suzatary_io::anthropic::AnthropicMessages),
    /// A declared `[[providers]]` entry, over the OpenAI-compatible interpreter.
    OpenAi(suzatary_io::openai::OpenAiCompatible),
    /// A job's model, chosen per turn (see `SpillingBrain`).
    Spilling(Box<SpillingBrain>),
    /// A file of scripted replies; test only.
    #[cfg(feature = "fixture")]
    Scripted(crate::commands::fixture::Scripted),
}

impl Model for Brain {
    fn name(&self) -> String {
        match self {
            Brain::Cli(cli) => cli.name(),
            Brain::Anthropic(api) => api.name(),
            Brain::OpenAi(api) => api.name(),
            Brain::Spilling(spilling) => spilling.name(),
            #[cfg(feature = "fixture")]
            Brain::Scripted(scripted) => scripted.name(),
        }
    }

    fn turn(
        &self,
        conversation: &Conversation,
        tools: &serde_json::Value,
    ) -> Result<Reply, ModelError> {
        match self {
            Brain::Cli(cli) => cli.turn(conversation, tools),
            // Logged here because `suzatary-io` never calls `tracing`, and the Usage
            // page does not count an API turn.
            Brain::Anthropic(api) => {
                let (reply, usage) = api.turn_with_usage(conversation, tools)?;
                if let Some(usage) = usage {
                    tracing::info!(
                        model = %api.name(),
                        input_tokens = usage.input_tokens,
                        output_tokens = usage.output_tokens,
                        "serve: secretary turn answered over the Anthropic Messages API \
                         (not yet counted on the Usage page)"
                    );
                } else {
                    tracing::info!(
                        model = %api.name(),
                        "serve: secretary turn answered over the Anthropic Messages API"
                    );
                }
                Ok(reply)
            }
            // Logged here because `suzatary-io` never calls `tracing`.
            Brain::OpenAi(api) => {
                let (reply, receipt) = api.turn_with_receipt(conversation, tools)?;
                if let Some(receipt) = &receipt {
                    tracing::info!(
                        model = %api.name(),
                        receipt = %receipt.identifier,
                        "serve: turn answered by a declared provider (attested)"
                    );
                } else {
                    tracing::info!(
                        model = %api.name(),
                        "serve: turn answered by a declared provider"
                    );
                }
                Ok(reply.with_receipt(receipt.map(|r| r.identifier)))
            }
            // One level deep: `brain_for` never returns `Spilling`.
            Brain::Spilling(spilling) => spilling.answer(conversation, tools),
            #[cfg(feature = "fixture")]
            Brain::Scripted(scripted) => scripted.turn(conversation, tools),
        }
    }
}

/// Only the `claude` CLI subprocess holds a session; the API interpreters are stateless.
impl Sessions for Brain {
    fn pick(&self) -> Pick<'_> {
        match self {
            Brain::Cli(cli) => Pick::Held(cli),
            Brain::Spilling(spilling) => spilling.pick(),
            Brain::Anthropic(_) | Brain::OpenAi(_) => Pick::Turn(Box::new(self)),
            #[cfg(feature = "fixture")]
            Brain::Scripted(_) => Pick::Turn(Box::new(self)),
        }
    }
}

/// The day's forecast for `[persona]`'s city.
pub struct Forecast(pub suzatary_core::persona::Persona);

impl Weather for Forecast {
    fn today(&self) -> Result<String, WeatherError> {
        // Wall-clock UTC, because `Clock::now()` is naive and cannot feed a zone conversion.
        suzatary_io::weather::line(chrono::Utc::now(), &self.0)
    }
}

/// Whether each declared secret resolves and which mail grants are held, never a value.
pub struct Declared {
    /// The secret store.
    pub secrets: Secretspec,
    /// This box's OAuth slots.
    pub tokens: suzatary_io::oauth::TokenStore,
    /// The reason the store's audit records for each question.
    pub asking: &'static str,
}

impl suzatary_core::capability::Secrets for Declared {
    // The value is discarded in the expression that reads it: only presence crosses.
    fn secret_state(
        &self,
        settable: &suzatary_core::settings::Settable,
    ) -> suzatary_core::capability::SecretState {
        suzatary_io::secrets::state(&self.secrets.get(settable.name(), self.asking))
    }

    fn mail_grants(
        &self,
        account: &suzatary_core::mail::account::MailAccount,
    ) -> suzatary_core::capability::MailGrants {
        use suzatary_core::mail::account::Sync;
        match account.sync {
            // A Google account's reading grant is its sign-in with Google, in the secret store.
            Sync::Google => suzatary_core::capability::MailGrants {
                imap: suzatary_core::mail::address::EmailAddress::new(account.email.as_str())
                    .is_ok_and(|email| {
                        matches!(
                            self.google_grant(&email),
                            suzatary_core::google::Held::Signed(_)
                        )
                    }),
                smtp: false,
            },
            Sync::MsImap(_) | Sync::Gmail | Sync::Proton(_) => {
                let held = self.tokens.granted(account);
                suzatary_core::capability::MailGrants {
                    imap: held.contains(&"imap"),
                    smtp: held.contains(&"smtp"),
                }
            }
        }
    }

    // The grant is read and parsed here, and only its parts leave.
    fn google_grant(
        &self,
        account: &suzatary_core::mail::address::EmailAddress,
    ) -> suzatary_core::google::Held {
        match self
            .secrets
            .get(&suzatary_core::google::grant_name(account), self.asking)
        {
            Ok(Some(kept)) => suzatary_core::google::KeptGrant::parse(kept.expose())
                .map_or(suzatary_core::google::Held::NotSignedIn, |kept| {
                    suzatary_core::google::Held::Signed(kept.parts)
                }),
            Ok(None) | Err(_) => suzatary_core::google::Held::NotSignedIn,
        }
    }
}

/// One test call before a model's credential is kept: to the first entry a job orders (or the
/// fallback) that reads it, with the value given and never the stored one, its result recorded
/// in the model health, so a model that answered is known to answer.
pub enum Verifying {
    /// For real, over the orders as they stand when asked.
    Real {
        /// The deployment, for each backend's interpreter.
        instance: Arc<Instance>,
        /// The providers and each job's order, read when asked.
        live: super::live::Shared,
        /// Where each call's result is joined.
        answered: suzatary_io::model_health::Answered,
    },
    /// No call leaves: the fixture's model is scripted and answers whatever it is given.
    #[cfg(feature = "fixture")]
    Fixture {
        /// The deployment, for which entry a name serves.
        instance: Arc<Instance>,
        /// The providers and each job's order.
        live: super::live::Shared,
        /// Where the answer is joined.
        answered: suzatary_io::model_health::Answered,
    },
}

/// The secret `backend` reads: the subscription's token, the Messages API's key, or a declared
/// provider's.
fn reads<'a>(providers: &'a [DeclaredProvider], backend: &Backend) -> Option<&'a str> {
    match backend {
        Backend::ClaudeCli => Some(suzatary_io::model::TOKEN),
        Backend::AnthropicApi => Some(suzatary_io::anthropic::KEY),
        Backend::Declared(name) => providers
            .iter()
            .find(|provider| &provider.name == name)
            .map(|provider| provider.key.as_str()),
    }
}

/// The first entry a job orders, then the fallback, that reads the secret `name`.
fn served(instance: &Instance, live: &super::Live, name: &str) -> Option<ModelEntry> {
    let fallback = instance.fallback();
    Job::ALL
        .iter()
        .flat_map(|job| live.models.order(*job))
        .chain([&fallback])
        .find(|entry| reads(&live.providers, &entry.backend) == Some(name))
        .cloned()
}

/// The Messages API at the fallback's model, for a key no job orders yet: the bootstrap's other
/// way in is still tested before it is kept.
fn unordered(instance: &Instance, name: &str) -> Option<ModelEntry> {
    (name == suzatary_io::anthropic::KEY).then(|| ModelEntry {
        backend: Backend::AnthropicApi,
        model: instance.fallback().model,
    })
}

/// One call of `entry` with `key`, asking for a word.
fn probe(
    instance: &Instance,
    providers: &[DeclaredProvider],
    entry: &ModelEntry,
    key: &suzatary_core::capability::Secret,
) -> Result<(), ModelError> {
    let model = entry.model.as_str().to_owned();
    let brain = match &entry.backend {
        Backend::ClaudeCli => Brain::Cli(
            cli_of(instance, &model, &instance.secrets())
                .with_token(suzatary_io::model::TokenSource::fixed(key.clone())),
        ),
        Backend::AnthropicApi => Brain::Anthropic(suzatary_io::anthropic::AnthropicMessages::new(
            model,
            key.clone(),
        )),
        Backend::Declared(name) => {
            let provider = providers
                .iter()
                .find(|provider| &provider.name == name)
                .ok_or_else(|| ModelError::Unreachable(format!("no provider named {name}")))?;
            Brain::OpenAi(suzatary_io::openai::OpenAiCompatible::new(
                provider.base_url.clone(),
                model,
                key.clone(),
                (&provider.attestation).into(),
            ))
        }
    };
    let asked = Conversation {
        system: "This is a test that the model answers. Answer with the one word: ok.".to_owned(),
        turns: vec![suzatary_core::capability::Turn::Person {
            text: "ok?".to_owned(),
            references: Vec::new(),
            attachments: Vec::new(),
        }],
    };
    brain.turn(&asked, &serde_json::json!([])).map(|_| ())
}

/// Join one call of `entry` into `answered`, now, `failed` saying why it did not answer. Only a
/// refused credential is joined as a failure: a rate limit, a timeout or an outage says nothing
/// of whether the model answers this box, so it is joined as nothing. A register that cannot be
/// written costs the call nothing, since the next call writes it again.
pub fn answered_by(
    answered: &suzatary_io::model_health::Answered,
    entry: &ModelEntry,
    failed: Option<&ModelError>,
) {
    use suzatary_core::capability::Clock as _;
    if failed.is_some_and(|failed| !failed.refused_credential()) {
        return;
    }
    let at = prodrome_core::fpl::instant_of(suzatary_io::clock::SystemClock.now());
    let words = failed.map(ToString::to_string);
    if let Err(failed) = answered.record(&entry.to_string(), at, words.as_deref()) {
        tracing::warn!("model health: {failed}");
    }
}

impl suzatary_core::settings::Verify for Verifying {
    fn verify(
        &self,
        name: &suzatary_core::settings::Settable,
        value: &suzatary_core::capability::Secret,
    ) -> Result<(), String> {
        match self {
            Verifying::Real {
                instance,
                live,
                answered,
            } => {
                let live = super::locks::read(live).clone();
                let (entry, ordered) = match served(instance, &live, name.name()) {
                    Some(entry) => (entry, true),
                    None => match unordered(instance, name.name()) {
                        Some(entry) => (entry, false),
                        None => return Ok(()),
                    },
                };
                let called = probe(instance, &live.providers, &entry, value);
                // An entry no job orders answers nobody, so its call is no reading of the box's.
                if ordered {
                    answered_by(answered, &entry, called.as_ref().err());
                }
                called.map_err(|failed| format!("{entry}: {failed}"))
            }
            #[cfg(feature = "fixture")]
            Verifying::Fixture {
                instance,
                live,
                answered,
            } => {
                let live = super::locks::read(live).clone();
                if let Some(entry) = served(instance, &live, name.name()) {
                    answered_by(answered, &entry, None);
                }
                Ok(())
            }
        }
    }
}

/// Where the usage page reads its transcripts from.
///
/// The fixture reads nowhere, because a cold walk of the box's own transcripts
/// exceeds a browser scene's time budget.
#[derive(Debug, Clone)]
pub enum Ledger {
    /// The box's own transcripts.
    Box,
    /// An empty root the fixture owns; the usage page answers "nothing yet".
    Nowhere(std::path::PathBuf),
}

impl Ledger {
    /// A fresh empty directory under the system temp dir, this process's own.
    #[must_use]
    pub fn nowhere() -> Self {
        let root = std::env::temp_dir().join(format!(
            "suzatary-fixture-transcripts-{}",
            std::process::id()
        ));
        // A directory that cannot be made reads as empty too, so failure is ignored.
        let _ = std::fs::create_dir_all(&root);
        Self::Nowhere(root)
    }
}

/// The first usable entry of `order` with what `usable` made of it, or `None`; pure, with the
/// impure half in `brain_for`.
///
/// `claude-cli` is offered to `usable` while the subscription has room above the reserve (an
/// unreadable meter counts as room); every other entry always, and `usable` answers for a
/// keyed backend exactly when its key resolves, so the key it read is the one the brain holds.
pub fn pick_entry<'a, T>(
    order: &'a [ModelEntry],
    allowance: &suzatary_core::usage::Allowance,
    reserve: Reserve,
    mut usable: impl FnMut(&'a ModelEntry) -> Option<T>,
) -> Option<(&'a ModelEntry, T)> {
    order.iter().find_map(|entry| {
        let offered = match &entry.backend {
            Backend::ClaudeCli => subscription_has_room(allowance, reserve),
            Backend::AnthropicApi | Backend::Declared(_) => true,
        };
        offered
            .then(|| usable(entry))
            .flatten()
            .map(|made| (entry, made))
    })
}

/// Whether the subscription has room above the reserve.
///
/// `RateLimited` and `Refused` are the allowance endpoint refusing to answer, not the
/// subscription being full, so the first is judged by its remembered reading and either,
/// with none, counts as room.
fn subscription_has_room(allowance: &suzatary_core::usage::Allowance, reserve: Reserve) -> bool {
    let reserve = reserve.get();
    match allowance {
        suzatary_core::usage::Allowance::Reported(reported) => {
            reported.utilization <= 1.0 - reserve
        }
        suzatary_core::usage::Allowance::RateLimited {
            last_reported: Some(reported),
            ..
        } => reported.utilization <= 1.0 - reserve,
        suzatary_core::usage::Allowance::RateLimited {
            last_reported: None,
            ..
        }
        | suzatary_core::usage::Allowance::Refused { .. }
        | suzatary_core::usage::Allowance::Unknown => true,
    }
}

/// The five-hour window's allowance, read by a fresh reader.
///
/// Fresh rather than the [`super::World`]'s because its callers run before or without
/// one, and only once per process, never per request.
#[must_use]
pub fn current_allowance(instance: &Instance) -> suzatary_core::usage::Allowance {
    let reader = suzatary_io::usage::Allowance::new(
        transcripts(instance),
        instance.secrets(),
        "choosing which model answers a job",
        &instance.paths.usage_cache,
    );
    reader
        .allowance()
        .into_iter()
        .find_map(|(window, sum)| (window == suzatary_core::usage::Window::FiveHour).then_some(sum))
        .unwrap_or(suzatary_core::usage::Allowance::Unknown)
}

/// The deny-all every call to the model's CLI is fenced by, as the instance names it.
#[must_use]
pub fn fence(instance: &Instance) -> suzatary_io::model::Fence {
    suzatary_io::model::Fence::at(instance.secretary.settings.clone())
}

/// The `claude` CLI subprocess at `model`, with the settings fence, working
/// directory and token; the one spawner every caller shares so none can drift.
#[must_use]
pub fn cli_of(
    instance: &Instance,
    model: &str,
    secrets: &suzatary_io::secrets::Secretspec,
) -> suzatary_io::model::ClaudeCli {
    suzatary_io::model::ClaudeCli::new(model.to_owned(), fence(instance))
        .in_directory(instance.code().to_path_buf())
        .with_token(suzatary_io::model::TokenSource::from_store(secrets.clone()))
        .with_uploads(upload_store(instance))
        .with_files(file_store(instance))
}

/// The web as the secretary may read it.
///
/// Allow rules are reread from the instance file on every ask so a new rule counts
/// without a restart; a file that no longer reads allows nothing.
#[derive(Debug, Clone)]
pub struct WebAccess {
    fetcher: suzatary_io::web::Fetcher,
    instance: std::path::PathBuf,
}

impl WebAccess {
    /// Over this deployment's instance file, ten seconds a request.
    #[must_use]
    pub fn of(instance: &Instance) -> Self {
        Self {
            fetcher: suzatary_io::web::Fetcher::new(std::time::Duration::from_secs(10)),
            instance: instance.path.clone(),
        }
    }
}

impl suzatary_core::capability::Web for WebAccess {
    fn allow_rules(&self) -> Vec<suzatary_core::web::AllowRule> {
        crate::instance::File::read(&self.instance)
            .map(|file| file.secretary.web_allow)
            .unwrap_or_default()
    }

    fn perform(
        &self,
        request: &suzatary_core::web::WebRequest,
    ) -> Result<suzatary_core::capability::WebAnswer, suzatary_core::capability::WebFailure> {
        self.fetcher.perform(request)
    }
}

/// The owner's Drive as the secretary may read it.
///
/// `[secretary] drive_allow` is reread on every ask so a new folder counts at once; a
/// file that no longer reads allows nothing.
#[derive(Debug, Clone)]
pub struct DriveAccess {
    drive: suzatary_io::drive::ReadOnly,
    instance: std::path::PathBuf,
}

impl DriveAccess {
    /// Over this deployment's instance file and Drive CLI.
    #[must_use]
    pub fn of(instance: &Instance) -> Self {
        Self {
            drive: proton_drive(instance).read_only(),
            instance: instance.path.clone(),
        }
    }
}

/// Claude Code's transcripts on this box.
pub(crate) fn transcripts(instance: &Instance) -> suzatary_io::usage::Transcripts {
    suzatary_io::usage::Transcripts::new(
        instance.paths.transcripts.clone(),
        &instance.paths.home,
        instance.code(),
    )
}

/// The Drive CLI this instance names, downloading into the user's runtime
/// directory (memory, private to the user) when there is one.
#[must_use]
pub fn proton_drive(instance: &Instance) -> suzatary_io::drive::ProtonDrive {
    suzatary_io::drive::ProtonDrive::new(
        instance.paths.drive_cli.clone(),
        instance.paths.drive_scratch.clone(),
        instance.code().to_path_buf(),
    )
}

impl suzatary_core::capability::Drive for DriveAccess {
    fn allowed_folders(&self) -> Vec<suzatary_core::drive::DrivePath> {
        crate::instance::File::read(&self.instance)
            .map(|file| file.secretary.drive_allow)
            .unwrap_or_default()
    }

    fn port(&self) -> &dyn suzatary_core::drive::Port {
        &self.drive
    }

    fn read(
        &self,
        file: &suzatary_core::drive::DrivePath,
    ) -> Result<suzatary_core::file_read::FileRead, suzatary_core::capability::DriveFailure> {
        self.drive.read(file)
    }
}

/// The owner's Drive as the vault a private upload goes to.
pub struct DriveVault(suzatary_io::drive::ProtonDrive);

impl suzatary_core::vault::Vault for DriveVault {
    fn keep(
        &self,
        name: &str,
        bytes: &[u8],
    ) -> Result<suzatary_core::drive::DrivePath, suzatary_core::capability::DriveFailure> {
        self.0.keep(&suzatary_core::vault::folder(), name, bytes)
    }
}

/// This instance's Drive, as the vault.
#[must_use]
pub fn drive_vault(instance: &Instance) -> DriveVault {
    DriveVault(proton_drive(instance))
}

/// The Junk reader the server and the CLI secretary hold.
pub type Junk = suzatary_io::junk::OutlookJunk<suzatary_io::junk::SignedIn>;

/// The roster's Microsoft accounts' Junk, with `[mail.junk]`'s churn.
///
/// `SUZATARY_JUNK_SEED` names a seed the reader rereads per digest, and the token is
/// minted silently from the box's grant store or not at all.
#[must_use]
pub fn junk(instance: &Instance) -> Junk {
    suzatary_io::junk::OutlookJunk::new(
        &instance.mail.accounts,
        instance.mail.junk.clone(),
        std::env::var_os("SUZATARY_JUNK_SEED").map(std::path::PathBuf::from),
        suzatary_io::junk::SignedIn::new(crate::commands::mail::tokens(instance)),
    )
}

/// Every account's mail server, searched for mail older than the routine sync's window.
pub type Older = suzatary_io::older::Servers<
    suzatary_io::junk::SignedIn,
    suzatary_io::http::Ureq,
    suzatary_io::older::Notmuched,
>;

/// The mail servers an older search reaches: each Microsoft account's IMAP session signed in
/// silently, Gmail's API on lieer's grant or a Google account's own, its leaves under the mail root and its states beside
/// the sync's, indexed as a sync indexes.
#[must_use]
pub fn older(instance: &Instance) -> Older {
    suzatary_io::older::Servers::new(
        instance.mail.accounts.clone(),
        (
            instance.mail.root.clone(),
            instance.paths.imap_state.clone(),
        ),
        suzatary_io::junk::SignedIn::new(crate::commands::mail::tokens(instance)),
        suzatary_io::http::Ureq::new(),
        suzatary_io::older::Notmuched::new(
            crate::commands::mail::syncer(instance),
            crate::commands::mail::notmuch(instance),
        ),
        std::sync::Arc::new(crate::commands::mail::google_grants(instance)),
    )
}

/// The timer's own mail sync, whose report is the state directory's `last-sync.json`.
#[must_use]
pub fn mail_sync(instance: &Instance) -> suzatary_io::mail_sync::Timer {
    suzatary_io::mail_sync::Timer::new(instance.paths.last_sync.clone(), &instance.mail.heartbeat)
}

/// The owner's uploads: `attachments/` inside the conversations directory, so it
/// is box-local and git-ignored with them, and skipped by their listing.
#[must_use]
pub fn upload_store(instance: &Instance) -> suzatary_io::attachments::AttachmentStore {
    suzatary_io::attachments::AttachmentStore::new(
        instance.secretary.conversations.join("attachments"),
        suzatary_io::pdftotext::Pdftotext::new(instance.code()),
    )
}

/// The upload index: `upload-index/` inside the conversations directory, beside the uploads it
/// is read from, so it is box-local and git-ignored with them and never pushed.
#[must_use]
pub fn upload_index(instance: &Instance) -> suzatary_io::upload_index::UploadIndexFiles {
    suzatary_io::upload_index::UploadIndexFiles::new(
        instance.secretary.conversations.join("upload-index"),
        upload_store(instance),
    )
}

/// The files verbs hand the model: `files/` inside the conversations directory, beside the
/// uploads and git-ignored with them.
#[must_use]
pub fn file_store(instance: &Instance) -> suzatary_io::blobs::Blobs {
    suzatary_io::blobs::Blobs::open(instance.secretary.conversations.join("files"))
}

/// The entry [`pick_entry`] picks from `job`'s order, and its interpreter.
///
/// An order with nothing usable answers with [`Instance::fallback`] and one log line,
/// never a refusal.
fn brain_for(
    instance: &Instance,
    providers: &[DeclaredProvider],
    models: &Models,
    job: Job,
    allowance: &suzatary_core::usage::Allowance,
    secrets: &suzatary_io::secrets::Secretspec,
) -> (ModelEntry, Brain) {
    let order = models.order(job);
    let picked = pick_entry(order, allowance, models.reserve(), |entry| {
        built(instance, providers, entry, secrets)
    });
    let Some((entry, brain)) = picked else {
        let fallback = instance.fallback();
        if order.is_empty() {
            tracing::info!(
                job = job.as_str(),
                "serve: {} names no order — {fallback}",
                job.as_str()
            );
        } else {
            tracing::warn!(
                job = job.as_str(),
                "serve: nothing in {}'s order is usable right now — {fallback}",
                job.as_str()
            );
        }
        let brain = Brain::Cli(cli_of(instance, fallback.model.as_str(), secrets));
        return (fallback, brain);
    };
    tracing::info!(
        job = job.as_str(),
        "serve: {}'s order picks {entry}",
        job.as_str()
    );
    (entry.clone(), brain)
}

/// `entry`'s interpreter: the subscription's always, a keyed backend's only with the key the
/// store answered now, read once and held by the interpreter it makes.
fn built(
    instance: &Instance,
    providers: &[DeclaredProvider],
    entry: &ModelEntry,
    secrets: &suzatary_io::secrets::Secretspec,
) -> Option<Brain> {
    let model = entry.model.as_str().to_owned();
    let key = || {
        key_of(providers, &entry.backend, secrets).unwrap_or_else(|failed| {
            tracing::warn!("serve: {} is not usable: {failed}", entry.backend.as_str());
            None
        })
    };
    Some(match &entry.backend {
        Backend::ClaudeCli => Brain::Cli(cli_of(instance, &model, secrets)),
        Backend::AnthropicApi => Brain::Anthropic(
            suzatary_io::anthropic::AnthropicMessages::new(model, key()?).with_stores(
                suzatary_io::blocks::Stores {
                    uploads: Some(upload_store(instance)),
                    files: Some(file_store(instance)),
                },
            ),
        ),
        Backend::Declared(name) => {
            let provider = providers.iter().find(|provider| &provider.name == name)?;
            Brain::OpenAi(suzatary_io::openai::OpenAiCompatible::new(
                provider.base_url.clone(),
                model,
                key()?,
                (&provider.attestation).into(),
            ))
        }
    })
}

/// A keyed backend's key as the store answered; `None` for the subscription, whose CLI
/// reads its own token.
///
/// # Errors
/// The store could not say.
pub fn key_of(
    providers: &[DeclaredProvider],
    backend: &Backend,
    secrets: &suzatary_io::secrets::Secretspec,
) -> Result<Option<suzatary_core::capability::Secret>, suzatary_io::secrets::SecretError> {
    match backend {
        Backend::ClaudeCli => Ok(None),
        Backend::AnthropicApi => {
            secrets.get(suzatary_io::anthropic::KEY, suzatary_io::anthropic::REASON)
        }
        Backend::Declared(name) => providers
            .iter()
            .find(|provider| &provider.name == name)
            .map_or(Ok(None), |provider| {
                secrets.get(
                    &provider.key,
                    "modelrouter: a declared provider answers a job",
                )
            }),
    }
}

/// Which interpreter answers a job, chosen once over the instance file's `[models]`.
///
/// For one-shot CLI runs only; a server answers for hours while its orders change, so
/// it uses [`brain_for_serving`].
#[must_use]
pub fn choose_brain(
    instance: &Instance,
    job: Job,
    allowance: &suzatary_core::usage::Allowance,
) -> Brain {
    let secrets = instance.secrets();
    brain_for(
        instance,
        &instance.providers,
        &instance.models,
        job,
        allowance,
        &secrets,
    )
    .1
}

/// A job's model, chosen per turn rather than once at boot.
///
/// The window fills and the settings page rewrites the order while the server runs,
/// so every turn re-runs `brain_for`; the allowance is cached hourly. The held `cli`
/// keeps a conversation's session whenever the pick is that subprocess.
pub struct SpillingBrain {
    job: Job,
    instance: Arc<Instance>,
    live: super::live::Shared,
    allowance: Arc<suzatary_io::usage::Allowance>,
    cli: Recorded,
    /// Held across turns, so an attested gateway's report is fetched once per keyset.
    identities: Arc<suzatary_io::attestation::Identities>,
    /// Where each turn's answer, or failure to answer, is joined, by the entry that took it.
    answered: suzatary_io::model_health::Answered,
}

/// One round's pick of a stateless entry: the interpreter it made, its answers joined into the
/// model health under that entry.
struct Picked<'a> {
    entry: ModelEntry,
    brain: Brain,
    answered: &'a suzatary_io::model_health::Answered,
}

impl Model for Picked<'_> {
    fn name(&self) -> String {
        self.brain.name()
    }

    fn turn(
        &self,
        conversation: &Conversation,
        tools: &serde_json::Value,
    ) -> Result<Reply, ModelError> {
        let result = self.brain.turn(conversation, tools);
        noted(self.answered, &self.entry, &result);
        result
    }
}

/// The fallback's `claude` subprocess, each of its calls joined into the model health.
pub struct Recorded {
    cli: suzatary_io::model::ClaudeCli,
    entry: ModelEntry,
    answered: suzatary_io::model_health::Answered,
}

impl Recorded {
    /// Join `result` unless it says nothing of whether the model answers: a halt is a person's,
    /// and a lost session is the CLI's.
    fn noted<T>(&self, result: Result<T, ModelError>) -> Result<T, ModelError> {
        noted(&self.answered, &self.entry, &result);
        result
    }
}

/// Join `result` for `entry`, unless it says nothing of whether the model answers.
fn noted<T>(
    answered: &suzatary_io::model_health::Answered,
    entry: &ModelEntry,
    result: &Result<T, ModelError>,
) {
    match result {
        Err(ModelError::Halted | ModelError::Lost(_)) => {}
        Ok(_) => answered_by(answered, entry, None),
        Err(failed) => answered_by(answered, entry, Some(failed)),
    }
}

impl Model for Recorded {
    fn name(&self) -> String {
        self.cli.name()
    }

    fn turn(
        &self,
        conversation: &Conversation,
        tools: &serde_json::Value,
    ) -> Result<Reply, ModelError> {
        self.noted(self.cli.turn(conversation, tools))
    }
}

impl SessionModel for Recorded {
    fn open(
        &self,
        id: &suzatary_core::capability::SessionId,
        conversation: &Conversation,
        tools: &serde_json::Value,
        halt: &suzatary_core::capability::Halt,
    ) -> Result<suzatary_core::capability::Sessioned, ModelError> {
        self.noted(self.cli.open(id, conversation, tools, halt))
    }

    fn resume(
        &self,
        session: &suzatary_core::capability::Session,
        delta: &suzatary_core::capability::Delta,
        tools: &serde_json::Value,
        on_delta: &dyn Fn(&str),
        halt: &suzatary_core::capability::Halt,
    ) -> Result<suzatary_core::capability::Sessioned, ModelError> {
        self.noted(self.cli.resume(session, delta, tools, on_delta, halt))
    }
}

impl SpillingBrain {
    /// Build one over `instance`'s own roots, reading the providers and the orders from `live`
    /// per turn and joining each call into `answered`.
    #[must_use]
    pub fn new(
        instance: &Instance,
        live: super::live::Shared,
        job: Job,
        answered: suzatary_io::model_health::Answered,
    ) -> Self {
        let secrets = instance.secrets();
        let allowance = suzatary_io::usage::Allowance::new(
            transcripts(instance),
            instance.secrets(),
            "modelrouter: choosing which model answers a job, per turn",
            &instance.paths.usage_cache,
        );
        if !fence(instance).stands() {
            tracing::warn!(
                "serve: the model's tool fence {} is not there — every call to the model's CLI \
                 is refused until it is",
                instance.secretary.settings.display()
            );
        }
        Self {
            job,
            cli: Recorded {
                cli: cli_of(instance, instance.fallback().model.as_str(), &secrets),
                entry: instance.fallback(),
                answered: answered.clone(),
            },
            instance: Arc::new(instance.clone()),
            live,
            allowance: Arc::new(allowance),
            identities: Arc::default(),
            answered,
        }
    }

    /// This turn's answer from the entry this instant's order picks, joined into the model
    /// health under that entry.
    fn answer(
        &self,
        conversation: &Conversation,
        tools: &serde_json::Value,
    ) -> Result<Reply, ModelError> {
        let (entry, brain) = self.choose_now();
        let result = brain.turn(conversation, tools);
        noted(&self.answered, &entry, &result);
        result
    }

    fn secrets(&self) -> suzatary_io::secrets::Secretspec {
        self.instance.secrets()
    }

    fn five_hour(&self) -> suzatary_core::usage::Allowance {
        five_hour(&self.allowance)
    }

    fn live(&self) -> super::Live {
        super::locks::read(&self.live).clone()
    }

    /// [`brain_for`] over this instant's order and allowance: the entry it picked, the
    /// fallback when none is usable, and its interpreter.
    fn choose_now(&self) -> (ModelEntry, Brain) {
        let live = self.live();
        let (entry, brain) = brain_for(
            &self.instance,
            &live.providers,
            &live.models,
            self.job,
            &self.five_hour(),
            &self.secrets(),
        );
        let brain = match brain {
            Brain::OpenAi(api) => Brain::OpenAi(api.with_identities(Arc::clone(&self.identities))),
            other => other,
        };
        (entry, brain)
    }

    /// The job's order, which reads nothing: the model that answers is named by the pick that
    /// answered.
    fn name(&self) -> String {
        format!("the {} order", self.job.as_str())
    }

    /// This round's model, read once now: the held subprocess when the order picks it, else
    /// the interpreter picked, owned by the round.
    fn pick(&self) -> Pick<'_> {
        let (entry, brain) = self.choose_now();
        if entry == self.instance.fallback() {
            return Pick::Held(&self.cli);
        }
        Pick::Turn(Box::new(Picked {
            entry,
            brain,
            answered: &self.answered,
        }))
    }
}

/// The five-hour window's allowance as `allowance` reads it now.
fn five_hour(allowance: &suzatary_io::usage::Allowance) -> suzatary_core::usage::Allowance {
    allowance
        .allowance()
        .into_iter()
        .find_map(|(window, sum)| (window == suzatary_core::usage::Window::FiveHour).then_some(sum))
        .unwrap_or(suzatary_core::usage::Allowance::Unknown)
}

/// The compactor's model: `[secretary] compactor` through the subscription's `claude` CLI, on
/// its plain path (the prompt as the whole system prompt, the ask's text as it stands, no
/// tools), from an empty directory under an empty configuration, and only while the subscription
/// has room above the reserve automated jobs never take.
pub struct CompactorModel {
    asked: suzatary_io::model::ClaudeCli,
    name: suzatary_core::models::ModelName,
    allowance: suzatary_io::usage::Allowance,
    live: super::live::Shared,
}

impl CompactorModel {
    /// `name` over `instance`'s subscription, under the reserve `live` holds.
    #[must_use]
    pub fn new(
        instance: &Instance,
        live: &super::live::Shared,
        name: suzatary_core::models::ModelName,
    ) -> Self {
        let (empty, config) = (&instance.paths.compactor, &instance.paths.compactor_config);
        for directory in [empty, config] {
            if let Err(failed) = std::fs::create_dir_all(directory) {
                tracing::warn!(
                    "compactor: {} could not be made: {failed}",
                    directory.display()
                );
            }
        }
        let secrets = instance.secrets();
        Self {
            asked: suzatary_io::model::ClaudeCli::new(name.as_str(), fence(instance))
                .in_directory(empty.clone())
                .with_config_dir(config.clone())
                .with_token(suzatary_io::model::TokenSource::from_store(secrets)),
            name,
            allowance: suzatary_io::usage::Allowance::new(
                transcripts(instance),
                instance.secrets(),
                "the compactor: whether the subscription has room above the reserve",
                &instance.paths.usage_cache,
            ),
            live: Arc::clone(live),
        }
    }
}

impl CompactorModel {
    /// `text` under `system` on the plain path, only while the subscription has room above the
    /// reserve automated jobs never take; the compactor and the topics backfill ask through it.
    ///
    /// # Errors
    /// [`ModelError::Unreachable`] at the reserve, or the CLI's own failure.
    pub fn ask(
        &self,
        system: &suzatary_core::for_model::ForModel,
        text: suzatary_core::for_model::ForModel,
    ) -> Result<String, ModelError> {
        let reserve = super::locks::read(&self.live).models.reserve();
        if !subscription_has_room(&five_hour(&self.allowance), reserve) {
            return Err(ModelError::Unreachable(
                "the subscription is down to the reserve automated jobs never take".to_owned(),
            ));
        }
        self.asked.plain(system, text)
    }
}

impl Compact for CompactorModel {
    fn model(&self) -> &suzatary_core::models::ModelName {
        &self.name
    }

    fn compact(
        &self,
        ask: &suzatary_core::secretary::compact_ask::Ask,
    ) -> Result<String, ModelError> {
        self.ask(
            &suzatary_core::for_model::ForModel::of(suzatary_core::secretary::compact_ask::PROMPT),
            suzatary_core::for_model::ForModel::of(&ask.text()),
        )
    }
}

/// The server's interpreter for `job`: a [`SpillingBrain`] over `live`, so an order or a
/// provider a tap writes answers the next turn.
#[must_use]
pub fn brain_for_serving(
    instance: &Instance,
    live: &super::live::Shared,
    job: Job,
    answered: &suzatary_io::model_health::Answered,
) -> Brain {
    Brain::Spilling(Box::new(SpillingBrain::new(
        instance,
        Arc::clone(live),
        job,
        answered.clone(),
    )))
}

#[cfg(test)]
mod pick_entry_tests {
    use chrono::Utc;

    use super::*;

    const SUBSCRIPTION: &str = suzatary_core::models::CLAUDE_CLI;

    fn entry(provider: &str, model: &str) -> ModelEntry {
        let declared = [suzatary_core::settings::DeclaredProvider {
            name: "phala".to_owned(),
            base_url: "https://inference.phala.com/v1".to_owned(),
            key: "PHALA_API_KEY".to_owned(),
            attestation: suzatary_core::settings::Attestation::Unclaimed,
        }];
        ModelEntry::parse(&declared, &format!("{provider}/{model}")).expect("a well-formed entry")
    }

    fn share(reserve: f64) -> Reserve {
        Reserve::parse(reserve).expect("a share")
    }

    fn reported(utilization: f64) -> suzatary_core::usage::Allowance {
        suzatary_core::usage::Allowance::Reported(suzatary_core::usage::Reported {
            utilization,
            resets_at: None,
            as_of: Utc::now(),
        })
    }

    fn rate_limited() -> suzatary_core::usage::Allowance {
        suzatary_core::usage::Allowance::RateLimited {
            since: Utc::now(),
            retry_after_seconds: 30,
            last_reported: None,
        }
    }

    fn rate_limited_with(utilization: f64) -> suzatary_core::usage::Allowance {
        suzatary_core::usage::Allowance::RateLimited {
            since: Utc::now(),
            retry_after_seconds: 30,
            last_reported: Some(suzatary_core::usage::Reported {
                utilization,
                resets_at: None,
                as_of: Utc::now(),
            }),
        }
    }

    /// [`pick_entry`] with a key that resolves exactly where `resolves` says, answering the entry.
    fn usable_by<'a>(
        order: &'a [ModelEntry],
        allowance: &suzatary_core::usage::Allowance,
        reserve: Reserve,
        resolves: impl Fn(&Backend) -> bool,
    ) -> Option<&'a ModelEntry> {
        pick_entry(order, allowance, reserve, |entry| match &entry.backend {
            Backend::ClaudeCli => Some(()),
            keyed => resolves(keyed).then_some(()),
        })
        .map(|(entry, ())| entry)
    }

    /// The first entry of an order that is usable wins: the subscription while no reading puts
    /// its allowance over the reserve's line (a rate limit with no remembered reading, one
    /// remembered under the line and an unknown allowance all leave it usable), and a keyed
    /// backend, a declared provider or the Messages API, exactly when its key resolves, whatever
    /// the window holds.
    #[test]
    fn the_first_usable_entry_of_an_order_wins() {
        let subscription = entry(SUBSCRIPTION, "claude-opus-5");
        let phala = entry("phala", "deepseek-v4-flash");
        let api = entry(suzatary_core::models::ANTHROPIC, "claude-sonnet-5");
        let unknown = suzatary_core::usage::Allowance::Unknown;
        let no_key: fn(&Backend) -> bool = |_| false;
        let every_key: fn(&Backend) -> bool = |_| true;
        let phala_key: fn(&Backend) -> bool = |backend| backend.as_str() == "phala";
        let api_key: fn(&Backend) -> bool = |backend| *backend == Backend::AnthropicApi;
        for (order, allowance, resolves, picked) in [
            (vec![], unknown.clone(), every_key, None),
            (vec![subscription.clone()], reported(0.69), no_key, Some(0)),
            (vec![subscription.clone()], reported(0.71), no_key, None),
            (vec![subscription.clone()], rate_limited(), no_key, Some(0)),
            (
                vec![subscription.clone()],
                rate_limited_with(0.2),
                no_key,
                Some(0),
            ),
            (vec![subscription.clone()], unknown.clone(), no_key, Some(0)),
            (
                vec![subscription.clone()],
                rate_limited_with(0.9),
                no_key,
                None,
            ),
            (vec![phala.clone()], rate_limited(), phala_key, Some(0)),
            (vec![phala.clone()], rate_limited(), no_key, None),
            (vec![api.clone()], rate_limited_with(0.99), api_key, Some(0)),
            (vec![api], unknown, no_key, None),
            (
                vec![subscription.clone(), phala.clone()],
                rate_limited_with(0.9),
                phala_key,
                Some(1),
            ),
            (
                vec![subscription, phala],
                rate_limited_with(0.9),
                no_key,
                None,
            ),
        ] {
            assert_eq!(
                usable_by(&order, &allowance, share(0.30), resolves),
                picked.map(|at| &order[at]),
                "{order:?} {allowance:?}"
            );
        }
    }
}

// Two allowance values stand in for two turns, so no live endpoint or cache is needed.
#[cfg(test)]
mod brain_for_tests {
    use std::path::PathBuf;

    use super::*;

    // `key = "HOME"` makes the provider's key resolve from the environment, no subprocess.
    fn fixture_instance() -> (PathBuf, Instance) {
        let dir = std::env::temp_dir().join(format!(
            "suzatary-brain-for-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("a writable temp dir");
        let path = dir.join("instance.toml");
        let text = r#"
repo = "."
data = "."
[signatures]
root = "836884e1723c7b17f8b87e76a0272f012de6cff08f134eb9dcdb2670695b1818"

[[chains]]
name = "life"
events = "events"
proposals = "events-triage"
untrusted = []
actors = ["ada"]
owner = "ada"
[web]
viewers = ["her@example.test"]
app_url = "https://example.test/"
[web.act_as]
"her@example.test" = "ada"
[web.serving]
host = "127.0.0.1"
port = 8787
tls_dir = "tls"
dist = "web/dist"
briefings = "briefings"
[categories]
home = 0
[mail]
root = "Mail"
state = "state.json"
notmuch = "notmuch"
mbsyncrc = "mbsyncrc"
heartbeat = "triage-heartbeat"
[[mail.accounts]]
email = "her@example.test"
subdir = "her"
sync = "gmail"

[[providers]]
name = "phala"
base_url = "https://inference.phala.com/v1"
key = "HOME"
attested = false

[models]
secretary = ["claude-cli/claude-opus-5", "phala/deepseek-v4-flash"]
reserve = 0.30
"#;
        std::fs::write(&path, text).expect("a writable temp instance file");
        let instance =
            crate::instance::Instance::read(&path, &crate::paths::Env::under(&dir, &dir))
                .expect("a well-formed fixture");
        (dir, instance)
    }

    fn reported(utilization: f64) -> suzatary_core::usage::Allowance {
        suzatary_core::usage::Allowance::Reported(suzatary_core::usage::Reported {
            utilization,
            resets_at: None,
            as_of: chrono::Utc::now(),
        })
    }

    #[test]
    fn a_window_that_fills_between_two_turns_spills_from_the_subscription_to_the_provider() {
        let (dir, instance) = fixture_instance();
        let secrets =
            suzatary_io::secrets::Secretspec::new(PathBuf::from("/does/not/exist/secretspec.toml"));

        let first_turn = reported(0.10);
        let picked = brain_for(
            &instance,
            &instance.providers,
            &instance.models,
            Job::Secretary,
            &first_turn,
            &secrets,
        );
        assert!(
            matches!(picked.1, Brain::Cli(_)),
            "room under the reserve answers from the subscription"
        );

        let second_turn = reported(0.95);
        let picked = brain_for(
            &instance,
            &instance.providers,
            &instance.models,
            Job::Secretary,
            &second_turn,
            &secrets,
        );
        assert!(
            matches!(picked.1, Brain::OpenAi(_)),
            "over the reserve, the same order spills to the declared provider"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An order with nothing usable answers with the one fallback, the subscription at the
    /// secretary's model, whichever job asks.
    #[test]
    fn an_order_with_nothing_usable_answers_with_the_fallback_for_every_job() {
        let (dir, instance) = fixture_instance();
        let secrets =
            suzatary_io::secrets::Secretspec::new(PathBuf::from("/does/not/exist/secretspec.toml"));
        let empty = Models::default();
        let allowance = reported(0.99);
        let fallback = format!("claude-cli/{}", instance.secretary.model.as_str());
        let none: &[DeclaredProvider] = &[];
        for (providers, models, job) in [
            (&instance.providers[..], &instance.models, Job::Review),
            (none, &empty, Job::Secretary),
            (none, &empty, Job::Review),
        ] {
            let picked = brain_for(&instance, providers, models, job, &allowance, &secrets);
            assert_eq!(picked.1.name(), fallback, "{job:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The brain was built before a tap declared Phala and ordered it: both arrive under the one
    /// lock, so the next turn finds the provider its order names.
    #[test]
    fn a_provider_and_an_order_swapped_under_the_lock_answer_the_next_turn() {
        let (dir, declaring) = fixture_instance();
        let mut instance = declaring.clone();
        instance.providers.clear();
        instance.models = Models::default();
        let live = super::super::Live::of(&instance).shared();
        let brain = SpillingBrain::new(
            &instance,
            Arc::clone(&live),
            Job::Title,
            suzatary_io::model_health::beside(&instance.mail.heartbeat),
        );
        let fallback = format!("claude-cli/{}", instance.secretary.model.as_str());
        let named = |pick: &Pick<'_>| match pick {
            Pick::Held(model) => model.name(),
            Pick::Turn(model) => model.name(),
        };
        let held = brain.pick();
        assert!(
            matches!(held, Pick::Held(_)),
            "the fallback is the held subprocess"
        );
        assert_eq!(named(&held), fallback);

        let phala = ModelEntry::parse(&declaring.providers, "phala/deepseek-v4-flash")
            .expect("a declared entry");
        {
            let mut tapped = crate::commands::serve::locks::write(&live);
            tapped.providers.clone_from(&declaring.providers);
            tapped.models = Models::default().with_order(Job::Title, vec![phala]);
        }
        let picked = brain.pick();
        assert!(
            matches!(picked, Pick::Turn(_)),
            "a provider holds no session"
        );
        assert_eq!(named(&picked), "openai-compatible/deepseek-v4-flash");
        assert_eq!(picked.model().name(), named(&picked));
        assert_eq!(
            brain.name(),
            "the title order",
            "naming the job reads nothing"
        );

        // That round ends with nothing asked; the next one reads the order as it stands now.
        drop(picked);
        crate::commands::serve::locks::write(&live).models = Models::default();
        let next = brain.pick();
        assert!(
            matches!(next, Pick::Held(_)),
            "nothing of the last round's pick is kept"
        );
        assert_eq!(named(&next), fallback);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The box's mirrors, built one way for the server and the CLI alike.
#[must_use]
pub fn mirrors(instance: &Instance) -> std::sync::Arc<suzatary_io::repos::Mirrors> {
    std::sync::Arc::new(suzatary_io::repos::Mirrors::new(
        instance.paths.repos.clone(),
        suzatary_io::repos::Source::GitHub,
        instance.repos.clone(),
        Box::new(instance.secrets()),
    ))
}
