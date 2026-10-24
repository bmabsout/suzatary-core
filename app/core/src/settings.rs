//! The closed list of secret names this app may write, and the gates and edits around it.
//!
//! The list is derived from what the instance file says the deployment reads, never from
//! `secretspec.toml`, so release and mail-transport secrets are refused by absence.

pub mod configure;
mod declaration;
mod help;
pub mod overview;
pub mod remote;

use std::collections::{BTreeMap, BTreeSet};

pub use declaration::Declaration;
pub use help::{Help, Page, Words};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The built-in `anthropic/` backend's key, settable on every deployment.
pub const ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";

/// The built-in `claude-cli/` backend's token, settable on every deployment.
pub const CLAUDE_CODE_OAUTH_TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// One `[[providers]]` entry's public shape, never the key's resolved value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredProvider {
    /// The name a `[models]` entry's `"<provider>/<model>"` names.
    pub name: String,
    /// The OpenAI-compatible endpoint's base URL.
    pub base_url: String,
    /// The name of the secret holding this provider's key.
    pub key: String,
    /// Whether this provider claims attested serving, and against what.
    pub attestation: Attestation,
}

/// A provider's attestation claim; an attested provider without a commit is not a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attestation {
    /// No claim: a response is used as it comes.
    Unclaimed,
    /// A response without a receipt from this gateway source is refused.
    Required {
        /// The gateway source commit a receipt must name.
        expect: SourceCommit,
    },
}

impl Attestation {
    /// Whether a claim is made: the wire's and the file's `attested`.
    #[must_use]
    pub const fn claimed(&self) -> bool {
        matches!(self, Self::Required { .. })
    }
}

/// A gateway source commit: exactly 40 lowercase hex characters, as startup parses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCommit(String);

impl SourceCommit {
    /// Parse one.
    ///
    /// # Errors
    /// [`SetupError::BadCommit`] for any other shape.
    pub fn parse(text: &str) -> Result<Self, SetupError> {
        if text.len() == 40 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Ok(Self(text.to_owned()))
        } else {
            Err(SetupError::BadCommit(text.to_owned()))
        }
    }

    /// The commit id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The token `scripts/safe-push.sh` pushes the data repository with, through
/// `scripts/git-credential-secretspec.sh`: scoped to that one repository, never the code's.
pub const DATA_PUSH_TOKEN: &str = "SUZATARY_DATA_PUSH_TOKEN";

/// Every secret name this deployment has declared it needs, each with what declares it; the
/// authority for [`Settable::mint`].
///
/// The free join-semilattice on (name, declaration) pairs, read as a map: [`Settables::join`] is
/// the pointwise union, [`Settables::default`] its identity, and [`Settables::derive`] the join
/// of one pair per declaring table, so deriving from two deployments' tables is joining what
/// each derives.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settables(BTreeMap<String, BTreeSet<Declaration>>);

impl Settables {
    /// One name, declared by one table.
    #[must_use]
    pub fn declared(name: &str, by: Declaration) -> Self {
        Self(BTreeMap::from([(name.to_owned(), BTreeSet::from([by]))]))
    }

    /// The join: every name of either, declared by every table either says declares it.
    #[must_use]
    pub fn join(mut self, other: Self) -> Self {
        for (name, by) in other.0 {
            self.0.entry(name).or_default().extend(by);
        }
        self
    }

    /// Every name a deployment may set: what [`Settables::derive`] reads of its tables, the
    /// GitHub tokens its repositories need and what its Google accounts read. The one derivation a server starts and re-derives
    /// with, so a name never drops out after a tap that moved the instance file.
    #[must_use]
    pub fn of_deployment<'a>(
        providers: impl IntoIterator<Item = &'a DeclaredProvider>,
        channels: impl IntoIterator<Item = crate::connect::Channel>,
        coding: Option<&crate::coding::Coding>,
        (plugins, accounts): (
            &[crate::plugin::Manifest],
            &[crate::mail::account::MailAccount],
        ),
        repos: &crate::repos::Declared,
    ) -> Self {
        Self::derive(providers, channels, coding, plugins)
            .join(Self::github(repos))
            .join(Self::google(accounts))
    }

    /// Derives the list from declared providers, notify channels, `[coding]` and every
    /// credential a `[[plugins]]` manifest declares (a calendar's link, by name; a manifest can
    /// only name its own `PLUGIN_<NAME>_` namespace, so no plugin widens this to the app's keys).
    ///
    /// The two built-in keys are always present.
    #[must_use]
    pub fn derive<'a>(
        providers: impl IntoIterator<Item = &'a DeclaredProvider>,
        channels: impl IntoIterator<Item = crate::connect::Channel>,
        coding: Option<&crate::coding::Coding>,
        plugins: &[crate::plugin::Manifest],
    ) -> Self {
        let built_in = [Preset::AnthropicApi, Preset::ClaudeSubscription]
            .map(|preset| (preset.key_name().to_owned(), Declaration::Backend(preset)));
        let routine =
            coding.map(|_| (crate::coding::ROUTINE_TOKEN.to_owned(), Declaration::Coding));
        let keys = providers.into_iter().map(|provider| {
            (
                provider.key.clone(),
                Declaration::Provider(provider.name.clone()),
            )
        });
        let notified = channels.into_iter().flat_map(|channel| {
            crate::notify::channel_secrets(channel)
                .into_iter()
                .map(move |secret| (secret.to_owned(), Declaration::Channel(channel)))
        });
        let credentials = plugins.iter().flat_map(|manifest| {
            manifest.credentials().map(|(name, placement)| {
                (
                    name.as_str().to_owned(),
                    Declaration::Plugin {
                        plugin: manifest.name().clone(),
                        into: placement.into(),
                    },
                )
            })
        });
        built_in
            .into_iter()
            .chain(routine)
            .chain(keys)
            .chain(notified)
            .chain(credentials)
            .map(|(name, by)| Self::declared(&name, by))
            .fold(Self::default(), Self::join)
    }

    /// What a deployment's Google accounts read: the box's one Google Cloud client while any is
    /// declared, and each account's own sign-in with Google. Each is asked for by its own card,
    /// never a Secret card ([`Declaration::own_card`]).
    #[must_use]
    pub fn google(accounts: &[crate::mail::account::MailAccount]) -> Self {
        let addresses: Vec<crate::mail::address::EmailAddress> = accounts
            .iter()
            .filter(|account| matches!(account.sync, crate::mail::account::Sync::Google))
            .filter_map(|account| {
                crate::mail::address::EmailAddress::new(account.email.as_str()).ok()
            })
            .collect();
        let client = addresses.first().map(|_| {
            Self::declared(crate::google::CLIENT_ID, Declaration::GoogleClient).join(
                Self::declared(crate::google::CLIENT_SECRET, Declaration::GoogleClient),
            )
        });
        addresses
            .iter()
            .map(|account| {
                Self::declared(
                    &crate::google::grant_name(account),
                    Declaration::GoogleGrant(account.as_str().to_owned()),
                )
            })
            .chain(client)
            .fold(Self::default(), Self::join)
    }

    /// A deployment's GitHub tokens, one per purpose so no token does another's job: the data
    /// repository's push, which every deployment makes, and, when the secretary reaches
    /// `repos`, the token it reaches them with.
    #[must_use]
    pub fn github(repos: &crate::repos::Declared) -> Self {
        let reached = repos
            .iter()
            .next()
            .map(|_| Self::declared(crate::repos::TOKEN, Declaration::Repos));
        reached.into_iter().fold(
            Self::declared(DATA_PUSH_TOKEN, Declaration::DataPush),
            Self::join,
        )
    }

    /// The declarations `keep` keeps, a name none of its declarations kept being dropped: the
    /// restriction, a homomorphism of the join.
    #[must_use]
    pub fn restricted(&self, keep: impl Fn(&Declaration) -> bool) -> Self {
        Self(
            self.0
                .iter()
                .filter_map(|(name, by)| {
                    let kept: BTreeSet<Declaration> =
                        by.iter().filter(|d| keep(d)).cloned().collect();
                    (!kept.is_empty()).then(|| (name.clone(), kept))
                })
                .collect(),
        )
    }

    /// Every declared name, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }

    /// Every declared name as a [`Settable`], sorted; a member is settable by construction.
    pub fn iter(&self) -> impl Iterator<Item = Settable> + '_ {
        self.0.keys().cloned().map(Settable)
    }

    /// Every declared name as a [`Settable`], with what declares it.
    pub fn declarations(&self) -> impl Iterator<Item = (Settable, &BTreeSet<Declaration>)> + '_ {
        self.0.iter().map(|(name, by)| (Settable(name.clone()), by))
    }

    /// The tables that declare `name`, `None` for a name no longer declared.
    #[must_use]
    pub fn declaration(&self, name: &Settable) -> Option<&BTreeSet<Declaration>> {
        self.0.get(name.name())
    }
}

/// A secret name this deployment declared, the only name a write route accepts.
///
/// [`Settable::mint`] is deliberately its only constructor, with no `From<String>` and no
/// reader: a card stores the name as text, and its tap mints it against the names declared then.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Settable(String);

impl Settable {
    /// Mint one against the declared list, or say which name was refused.
    ///
    /// # Errors
    /// [`SettableError::Undeclared`] when `candidate` is not a member of `declared`.
    pub fn mint(declared: &Settables, candidate: &str) -> Result<Self, SettableError> {
        if declared.0.contains_key(candidate) {
            Ok(Self(candidate.to_owned()))
        } else {
            Err(SettableError::Undeclared(candidate.to_owned()))
        }
    }

    /// The name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// Why a candidate name did not mint a [`Settable`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettableError {
    /// Not a name this deployment's `[[providers]]` or `[notify]` declared.
    #[error("{0:?} is not a name this deployment has declared it needs")]
    Undeclared(String),
}

/// Writes a secret through the one store; held in no capability bundle, so no verb reaches it.
pub trait Writes {
    /// Writes `name` to `value`: a name this deployment declares, so the store's other secrets
    /// cannot be named.
    ///
    /// # Errors
    /// The store's own words, summarised, never carrying the value.
    fn write_secret(
        &self,
        name: &Settable,
        value: &crate::capability::Secret,
    ) -> Result<(), String>;

    /// Deletes what is stored under `name`, so a removed connection's secret resolves no more.
    ///
    /// # Errors
    /// The store's own words, summarised.
    fn clear_secret(&self, name: &Settable) -> Result<(), String>;
}

/// Declares to the one store every name a deployment may set, since the store refuses a name
/// it was never told of; held in no capability record, like [`Writes`].
pub trait Declares {
    /// Declares exactly `names` beside the store's own, so every [`Writes`] and every read
    /// through the store takes each of them from now on.
    ///
    /// # Errors
    /// Why they could not be declared; the names declared before stay declared.
    fn declare(&self, names: &Settables) -> Result<(), String>;
}

/// One test call before a model's credential is kept; held in no capability record, like
/// [`Writes`], so only a tap reaches it.
pub trait Verify {
    /// Whether `value`, kept under `name`, answers one call of a model a job orders that reads
    /// it; `Ok` for a name no ordered model reads, since there is nothing to call.
    ///
    /// # Errors
    /// Why the call did not answer, in words that never carry the value.
    fn verify(&self, name: &Settable, value: &crate::capability::Secret) -> Result<(), String>;
}

/// What runs a plugin's sync once, at the tap that stored one of its links, so the link's line
/// says what its check found then and not at the plugin's next timer; held in no capability
/// record, as [`Verify`] is not.
pub trait Checks {
    /// Sync `plugin` once: every link of it is fetched, and what each came to is its ledger's.
    fn check(&self, plugin: &crate::plugin::PluginName);
}

/// The closed list of backends the app knows, so a browser cannot aim a job at any endpoint.
///
/// The two Anthropic ones are built in; the rest are declared through a [`ProviderSetup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// The subscription, through the `claude` CLI (`claude-cli/`).
    ClaudeSubscription,
    /// The Anthropic Messages API (`anthropic/`).
    AnthropicApi,
    /// Databricks Model Serving, under a workspace's own hostname.
    Databricks,
    /// Phala's confidential inference gateway, attested.
    Phala,
    /// The `OpenRouter` gateway.
    #[serde(rename = "openrouter")]
    OpenRouter,
}

impl Preset {
    /// Every preset, in the order the settings page draws them.
    pub const ALL: [Preset; 5] = [
        Preset::ClaudeSubscription,
        Preset::AnthropicApi,
        Preset::Databricks,
        Preset::Phala,
        Preset::OpenRouter,
    ];

    /// The wire's spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeSubscription => "claude_subscription",
            Self::AnthropicApi => "anthropic_api",
            Self::Databricks => "databricks",
            Self::Phala => "phala",
            Self::OpenRouter => "openrouter",
        }
    }

    /// Whose models these are, in a sentence: "the Claude subscription's opus model".
    #[must_use]
    pub const fn whose(self) -> &'static str {
        match self {
            Self::ClaudeSubscription => "the Claude subscription's",
            Self::AnthropicApi => "the Anthropic API's",
            Self::Databricks => "Databricks'",
            Self::Phala => "Phala's",
            Self::OpenRouter => "OpenRouter's",
        }
    }

    /// What the owner calls it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClaudeSubscription => "Claude subscription",
            Self::AnthropicApi => "Anthropic API",
            Self::Databricks => "Databricks",
            Self::Phala => "Phala",
            Self::OpenRouter => "OpenRouter",
        }
    }

    /// The backend a `[models]` entry names it by.
    #[must_use]
    pub const fn backend(self) -> &'static str {
        match self {
            Self::ClaudeSubscription => crate::models::CLAUDE_CLI,
            Self::AnthropicApi => crate::models::ANTHROPIC,
            Self::Databricks => "databricks",
            Self::Phala => "phala",
            Self::OpenRouter => "openrouter",
        }
    }

    /// The name of the secret this preset's key is stored under.
    #[must_use]
    pub const fn key_name(self) -> &'static str {
        match self {
            Self::ClaudeSubscription => CLAUDE_CODE_OAUTH_TOKEN,
            Self::AnthropicApi => ANTHROPIC_API_KEY,
            Self::Databricks => "DATABRICKS_TOKEN",
            Self::Phala => "PHALA_API_KEY",
            Self::OpenRouter => "OPENROUTER_API_KEY",
        }
    }
}

/// An `https://` Databricks workspace host and nothing else, built by [`WorkspaceUrl::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceUrl(String);

/// The hostname suffixes a Databricks workspace may end with.
const DATABRICKS_HOST_SUFFIXES: [&str; 3] = [
    ".cloud.databricks.com",
    ".azuredatabricks.net",
    ".gcp.databricks.com",
];

impl WorkspaceUrl {
    /// Parse a workspace URL, or say by name what was wrong with it.
    ///
    /// # Errors
    /// [`WorkspaceUrlError`] naming which rule `candidate` broke.
    pub fn parse(candidate: &str) -> Result<Self, WorkspaceUrlError> {
        let Some(after_scheme) = candidate.strip_prefix("https://") else {
            return Err(WorkspaceUrlError::NotHttps(candidate.to_owned()));
        };
        if after_scheme.is_empty() {
            return Err(WorkspaceUrlError::UnknownHost(candidate.to_owned()));
        }
        let host_end = after_scheme
            .find(['/', '?', '#'])
            .unwrap_or(after_scheme.len());
        let (host, rest) = after_scheme.split_at(host_end);
        if !rest.is_empty() {
            return Err(WorkspaceUrlError::PathOrQuery(candidate.to_owned()));
        }
        if host.contains('@') {
            return Err(WorkspaceUrlError::Userinfo(candidate.to_owned()));
        }
        if host.contains(':') {
            return Err(WorkspaceUrlError::Port(candidate.to_owned()));
        }
        if !DATABRICKS_HOST_SUFFIXES
            .iter()
            .any(|suffix| host.ends_with(suffix) && host.len() > suffix.len())
        {
            return Err(WorkspaceUrlError::UnknownHost(candidate.to_owned()));
        }
        Ok(Self(format!("https://{host}")))
    }

    /// The URL, as parsed.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a candidate string did not parse into a [`WorkspaceUrl`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceUrlError {
    /// No `https://` prefix.
    #[error("{0:?} is not an https:// URL")]
    NotHttps(String),
    /// The host carried `user:pass@`.
    #[error("{0:?} names userinfo, which a workspace URL never carries")]
    Userinfo(String),
    /// The host carried a `:port`.
    #[error("{0:?} names a port, which a workspace URL never carries")]
    Port(String),
    /// Anything after the host: a path, a query or a fragment.
    #[error("{0:?} names a path or query, and a workspace URL is a bare host")]
    PathOrQuery(String),
    /// The host is not under one of Databricks' three cloud suffixes.
    #[error(
        "{0:?}'s host does not end with .cloud.databricks.com, .azuredatabricks.net, \
         or .gcp.databricks.com"
    )]
    UnknownHost(String),
}

/// A declarable [`Preset`] with the only data a person types for it; built-ins have no arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSetup {
    /// Databricks, under a checked workspace host.
    Databricks {
        /// Where it lives.
        workspace: WorkspaceUrl,
    },
    /// Phala, attested against a commit the owner supplies, never one learned from Phala.
    Phala {
        /// The commit a receipt must name.
        expect: SourceCommit,
    },
    /// `OpenRouter`, at its one fixed URL.
    OpenRouter,
}

/// Phala's gateway.
pub const PHALA_BASE_URL: &str = "https://inference.phala.com/v1";

/// The `OpenRouter` gateway.
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

impl ProviderSetup {
    /// Parses a request's preset and the one field it reads.
    ///
    /// # Errors
    /// A built-in preset, a missing field, or the field's own parse error.
    pub fn parse(
        preset: Preset,
        workspace: Option<&str>,
        expect_commit: Option<&str>,
    ) -> Result<Self, SetupError> {
        match preset {
            Preset::ClaudeSubscription | Preset::AnthropicApi => Err(SetupError::BuiltIn(preset)),
            Preset::Databricks => Ok(Self::Databricks {
                workspace: WorkspaceUrl::parse(workspace.ok_or(SetupError::Missing("workspace"))?)?,
            }),
            Preset::Phala => Ok(Self::Phala {
                expect: SourceCommit::parse(
                    expect_commit.ok_or(SetupError::Missing("expect_commit"))?,
                )?,
            }),
            Preset::OpenRouter => Ok(Self::OpenRouter),
        }
    }

    /// Which preset this is.
    #[must_use]
    pub const fn preset(&self) -> Preset {
        match self {
            Self::Databricks { .. } => Preset::Databricks,
            Self::Phala { .. } => Preset::Phala,
            Self::OpenRouter => Preset::OpenRouter,
        }
    }

    /// The one field it was parsed from: a workspace, or a commit a receipt must name.
    #[must_use]
    pub fn fields(&self) -> (Option<String>, Option<String>) {
        match self {
            Self::Databricks { workspace } => (Some(workspace.as_str().to_owned()), None),
            Self::Phala { expect } => (None, Some(expect.as_str().to_owned())),
            Self::OpenRouter => (None, None),
        }
    }

    /// The `[[providers]]` entry this setup derives.
    #[must_use]
    pub fn declared(&self) -> DeclaredProvider {
        let preset = self.preset();
        let (base_url, attestation) = match self {
            Self::Databricks { workspace } => (
                format!("{}/serving-endpoints", workspace.as_str()),
                Attestation::Unclaimed,
            ),
            Self::Phala { expect } => (
                PHALA_BASE_URL.to_owned(),
                Attestation::Required {
                    expect: expect.clone(),
                },
            ),
            Self::OpenRouter => (OPENROUTER_BASE_URL.to_owned(), Attestation::Unclaimed),
        };
        DeclaredProvider {
            name: preset.backend().to_owned(),
            base_url,
            key: preset.key_name().to_owned(),
            attestation,
        }
    }
}

/// Why a request did not name a provider to declare.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetupError {
    /// A built-in backend: no `[[providers]]` row, only its key.
    #[error("{} is built in — there is nothing to declare, only its key to set", .0.as_str())]
    BuiltIn(Preset),
    /// The preset's required field was absent.
    #[error("this preset needs {0:?}")]
    Missing(&'static str),
    /// The workspace URL did not parse.
    #[error(transparent)]
    Workspace(#[from] WorkspaceUrlError),
    /// The expected commit was not 40 lowercase hex characters.
    #[error("{0:?} is not a commit: 40 lowercase hex characters")]
    BadCommit(String),
}

/// Edits the instance file; held in no capability bundle, like [`Writes`].
pub trait Edits: configure::Configuration {
    /// Reads the file once, asks `decide` what to write of what it read, and writes that, all
    /// under one lock, so no other edit of the file comes between the read and the write.
    ///
    /// # Errors
    /// `decide`'s refusal, which writes nothing, or [`configure::Refused::Edit`] for the file.
    fn swap(
        &self,
        decide: &mut dyn FnMut(
            &configure::Configured,
        ) -> Result<Option<configure::Write>, configure::Refused>,
    ) -> Result<configure::Performed, configure::Refused>;

    /// Lets the secretary read what `rule` covers without asking; allowing twice is no error.
    ///
    /// # Errors
    /// [`EditError::Io`] or [`EditError::Malformed`].
    fn allow_web(&self, rule: &crate::web::AllowRule) -> Result<(), EditError>;

    /// Lets the secretary read files under `folder` without asking; allowing twice is no error.
    ///
    /// # Errors
    /// [`EditError::Io`] or [`EditError::Malformed`].
    fn allow_drive(&self, folder: &crate::drive::DrivePath) -> Result<(), EditError>;

    /// Writes `connection`'s table, keeping every other byte of the file: an add followed by
    /// its remove leaves the file as it was.
    ///
    /// # Errors
    /// [`EditError::Connection`] for what the algebra refuses (a duplicate first), else
    /// [`EditError::Io`] or [`EditError::Malformed`].
    fn add_connection(&self, connection: &crate::connect::Connection) -> Result<(), EditError>;

    /// Takes `connection`'s table out, the same way.
    ///
    /// # Errors
    /// As [`Edits::add_connection`].
    fn remove_connection(&self, connection: &crate::connect::Connection) -> Result<(), EditError>;

    /// Lets `plugin` reach `origin`; granting twice is no error.
    ///
    /// # Errors
    /// As [`Edits::add_connection`].
    fn add_origin(
        &self,
        plugin: &crate::plugin::PluginName,
        origin: &crate::plugin::Origin,
    ) -> Result<(), EditError>;
}

/// Why an edit to the instance file did not happen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// The file could not be read or written.
    #[error("could not edit the instance file: {0}")]
    Io(String),
    /// The file did not parse as TOML, or as the document this edit expects.
    #[error("the instance file is malformed: {0}")]
    Malformed(String),
    /// The connection algebra refused the edit, which touched nothing.
    #[error(transparent)]
    Connection(#[from] crate::connect::Refused),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(name: &str, key: &str) -> DeclaredProvider {
        DeclaredProvider {
            name: name.to_owned(),
            base_url: format!("https://{name}.example.test/v1"),
            key: key.to_owned(),
            attestation: Attestation::Unclaimed,
        }
    }

    fn declared() -> Settables {
        Settables::derive(
            [provider("phala", "PHALA_API_KEY")].iter(),
            crate::connect::Channel::ALL,
            None,
            &[],
        )
    }

    #[test]
    fn every_declared_name_mints() {
        let declared = declared();
        for name in [
            "PHALA_API_KEY",
            ANTHROPIC_API_KEY,
            CLAUDE_CODE_OAUTH_TOKEN,
            "FCM_SERVICE_ACCOUNT_B64",
            "NTFY_TOPIC",
        ] {
            assert_eq!(
                Settable::mint(&declared, name).as_ref().map(Settable::name),
                Ok(name),
                "{name} should have minted"
            );
        }
    }

    /// Pinned against the widest list this module can build.
    #[test]
    fn the_five_forbidden_names_are_always_refused() {
        let declared = declared();
        for name in [
            "GITHUB_PAT",
            "ANDROID_KEYSTORE_B64",
            "ANDROID_KEYSTORE_PASSWORD",
            "PROTON_BRIDGE_PASSWORD",
            "SUZATARY_GMAIL_PASSWORD",
        ] {
            assert_eq!(
                Settable::mint(&declared, name),
                Err(SettableError::Undeclared(name.to_owned())),
                "{name} must never be settable"
            );
        }
    }

    #[test]
    fn an_arbitrary_name_is_refused() {
        assert_eq!(
            Settable::mint(&declared(), "WHATEVER_I_TYPE"),
            Err(SettableError::Undeclared("WHATEVER_I_TYPE".to_owned()))
        );
    }

    #[test]
    fn an_empty_deployment_still_settles_the_builtin_keys() {
        let declared = Settables::derive(std::iter::empty(), std::iter::empty(), None, &[]);
        assert_eq!(
            declared.names(),
            vec![ANTHROPIC_API_KEY, CLAUDE_CODE_OAUTH_TOKEN]
        );
        assert!(Settable::mint(&declared, "GITHUB_PAT").is_err());
    }

    /// One token per purpose: the data repository's push always, the repositories' token only
    /// where the secretary reaches one, and neither is the other's name.
    #[test]
    fn each_github_token_is_declared_for_its_own_purpose() {
        let none = Settables::github(&crate::repos::Declared::default());
        assert_eq!(none.names(), vec![DATA_PUSH_TOKEN]);
        let repos = crate::repos::Declared::new([(
            "ada/notes".parse().expect("a name"),
            crate::repos::RepoTarget::default(),
        )])
        .expect("a list");
        let both = Settables::github(&repos);
        assert_eq!(both.names(), vec![DATA_PUSH_TOKEN, crate::repos::TOKEN]);
        for name in [DATA_PUSH_TOKEN, crate::repos::TOKEN] {
            assert!(Settable::mint(&both, name).is_ok(), "{name}");
        }
        assert_ne!(DATA_PUSH_TOKEN, crate::repos::TOKEN);
    }

    #[test]
    fn a_declared_routine_makes_its_token_settable() {
        let coding = crate::coding::Coding {
            routine: "trig_01fake".parse().expect("a routine id"),
            repo: "bmabsout/Suzatary".parse().expect("a repository"),
            chain: crate::chains::ChainName::new("suzatary").expect("a chain"),
        };
        let with = Settables::derive(std::iter::empty(), std::iter::empty(), Some(&coding), &[]);
        assert!(Settable::mint(&with, crate::coding::ROUTINE_TOKEN).is_ok());
        let without = Settables::derive(std::iter::empty(), std::iter::empty(), None, &[]);
        assert!(Settable::mint(&without, crate::coding::ROUTINE_TOKEN).is_err());
    }

    /// A calendar's link is set by name like any other secret, and only the names its manifest
    /// declares, in its own namespace, are added.
    #[test]
    fn a_plugin_s_declared_credentials_are_settable_by_name() {
        let manifest: crate::plugin::Manifest = toml::from_str(
            r#"
            name = "ics"
            component = "ics.wasm"
            schema = "ics.schema"
            world = "source@0.3.0"
            credentials = [{ name = "PLUGIN_ICS_WORK", into = "url" }]
            "#,
        )
        .expect("a manifest");
        let declared = Settables::derive(std::iter::empty(), std::iter::empty(), None, &[manifest]);
        assert!(Settable::mint(&declared, "PLUGIN_ICS_WORK").is_ok());
        assert!(Settable::mint(&declared, "PLUGIN_ICS_HOME").is_err());
        assert_eq!(declared.names().len(), 3);
    }

    #[test]
    fn a_preset_parses_its_own_spelling_only() {
        for preset in Preset::ALL {
            let spelled = serde_json::to_value(preset).expect("prints");
            assert_eq!(spelled, preset.as_str());
            assert_eq!(serde_json::from_value::<Preset>(spelled).ok(), Some(preset));
        }
        assert!(serde_json::from_value::<Preset>(serde_json::json!("vertex")).is_err());
    }

    #[test]
    fn a_setup_reads_only_its_presets_field() {
        for builtin in [Preset::ClaudeSubscription, Preset::AnthropicApi] {
            assert_eq!(
                ProviderSetup::parse(builtin, Some("https://acme.cloud.databricks.com"), None),
                Err(SetupError::BuiltIn(builtin))
            );
        }
        assert_eq!(
            ProviderSetup::parse(Preset::Databricks, None, None),
            Err(SetupError::Missing("workspace"))
        );
        assert_eq!(
            ProviderSetup::parse(
                Preset::Phala,
                Some("https://acme.cloud.databricks.com"),
                None
            ),
            Err(SetupError::Missing("expect_commit"))
        );
        assert_eq!(
            ProviderSetup::parse(Preset::Phala, None, Some("8D0A")),
            Err(SetupError::BadCommit("8D0A".to_owned()))
        );
        // A workspace offered to OpenRouter is not read: its URL is fixed.
        assert_eq!(
            ProviderSetup::parse(Preset::OpenRouter, Some("https://evil.example"), None),
            Ok(ProviderSetup::OpenRouter)
        );
    }

    #[test]
    fn the_fixed_presets_derive_their_rows() {
        let commit = "8d0a666a2418898a8c823a9af49a634edd122a64";
        let phala = ProviderSetup::parse(Preset::Phala, None, Some(commit))
            .expect("a commit")
            .declared();
        assert_eq!(
            phala,
            DeclaredProvider {
                name: "phala".to_owned(),
                base_url: PHALA_BASE_URL.to_owned(),
                key: "PHALA_API_KEY".to_owned(),
                attestation: Attestation::Required {
                    expect: SourceCommit::parse(commit).expect("a commit"),
                },
            }
        );
        let openrouter = ProviderSetup::OpenRouter.declared();
        assert_eq!(openrouter.name, "openrouter");
        assert_eq!(openrouter.base_url, OPENROUTER_BASE_URL);
        assert_eq!(openrouter.key, "OPENROUTER_API_KEY");
        assert!(!openrouter.attestation.claimed());
        // Every declared preset's name is the backend a `[models]` entry spells.
        for setup in [
            ProviderSetup::OpenRouter,
            ProviderSetup::Phala {
                expect: SourceCommit::parse(commit).expect("a commit"),
            },
        ] {
            assert_eq!(setup.declared().name, setup.preset().backend());
        }
    }

    #[test]
    fn the_subscription_preset_names_a_key_the_page_can_set() {
        let declared = Settables::derive(
            [ProviderSetup::OpenRouter.declared()].iter(),
            std::iter::empty(),
            None,
            &[],
        );
        assert!(Settable::mint(&declared, Preset::ClaudeSubscription.key_name()).is_ok());
        assert!(Settable::mint(&declared, Preset::AnthropicApi.key_name()).is_ok());
        assert!(Settable::mint(&declared, Preset::OpenRouter.key_name()).is_ok());
    }

    /// A name is declared by every table naming it, and a built-in key by its backend.
    #[test]
    fn a_name_says_what_declares_it() {
        let declared = Settables::derive(
            [provider("anthropic", ANTHROPIC_API_KEY)].iter(),
            [crate::connect::Channel::Ntfy],
            None,
            &[],
        );
        let by = |name: &str| {
            declared
                .declarations()
                .find(|(settable, _)| settable.name() == name)
                .map(|(_, by)| by.iter().cloned().collect::<Vec<_>>())
        };
        assert_eq!(
            by(ANTHROPIC_API_KEY),
            Some(vec![
                Declaration::Backend(Preset::AnthropicApi),
                Declaration::Provider("anthropic".to_owned()),
            ])
        );
        assert_eq!(
            by("NTFY_TOPIC"),
            Some(vec![Declaration::Channel(crate::connect::Channel::Ntfy)])
        );
    }

    fn generated() -> impl proptest::strategy::Strategy<Value = Settables> {
        use proptest::prelude::*;
        let declaration = prop_oneof![
            Just(Declaration::Coding),
            "[a-c]".prop_map(Declaration::Provider),
            proptest::sample::select(crate::connect::Channel::ALL.to_vec())
                .prop_map(Declaration::Channel),
        ];
        proptest::collection::vec(("[A-C]", declaration), 0..6).prop_map(|pairs| {
            pairs
                .into_iter()
                .map(|(name, by)| Settables::declared(&name, by))
                .fold(Settables::default(), Settables::join)
        })
    }

    proptest::proptest! {
        /// Restricting the declarations commutes with the join and keeps all when it keeps all.
        #[test]
        fn restricting_is_a_homomorphism(
            a in generated(),
            b in generated(),
            keep in proptest::sample::subsequence(vec!["a", "b", "c"], 0..=3),
            channels in proptest::sample::subsequence(crate::connect::Channel::ALL.to_vec(), 0..=2),
        ) {
            let k = |by: &Declaration| match by {
                Declaration::Provider(name) => keep.contains(&name.as_str()),
                Declaration::Channel(channel) => channels.contains(channel),
                Declaration::Backend(_)
                | Declaration::Coding
                | Declaration::DataPush
                | Declaration::Repos
                | Declaration::Plugin { .. }
                | Declaration::GoogleClient
                | Declaration::GoogleGrant(_) => true,
            };
            proptest::prop_assert_eq!(
                a.clone().join(b.clone()).restricted(k),
                a.restricted(k).join(b.restricted(k))
            );
            proptest::prop_assert_eq!(a.restricted(|_| true), a.clone());
        }

        /// The join is a semilattice with the empty list as its identity.
        #[test]
        fn the_join_is_a_semilattice(a in generated(), b in generated(), c in generated()) {
            proptest::prop_assert_eq!(a.clone().join(b.clone()), b.clone().join(a.clone()));
            proptest::prop_assert_eq!(
                a.clone().join(b.clone()).join(c.clone()),
                a.clone().join(b.clone().join(c))
            );
            proptest::prop_assert_eq!(a.clone().join(a.clone()), a.clone());
            proptest::prop_assert_eq!(a.clone().join(Settables::default()), a);
        }

        /// Deriving is a homomorphism: the tables of two deployments derive the join of what
        /// each derives.
        #[test]
        fn deriving_is_a_homomorphism(
            left in proptest::collection::vec(("[a-c]", "[A-C]"), 0..4),
            right in proptest::collection::vec(("[a-c]", "[A-C]"), 0..4),
            channels in proptest::collection::vec(
                proptest::sample::select(crate::connect::Channel::ALL.to_vec()), 0..3),
        ) {
            let providers = |pairs: &[(String, String)]| -> Vec<DeclaredProvider> {
                pairs.iter().map(|(name, key)| provider(name, key)).collect()
            };
            let (left, right) = (providers(&left), providers(&right));
            let both: Vec<DeclaredProvider> = left.iter().chain(&right).cloned().collect();
            proptest::prop_assert_eq!(
                Settables::derive(both.iter(), channels.iter().copied(), None, &[]),
                Settables::derive(left.iter(), channels.iter().copied(), None, &[])
                    .join(Settables::derive(right.iter(), std::iter::empty(), None, &[]))
            );
        }
    }
}
