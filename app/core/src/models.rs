//! Which model answers each job: `[models]`'s order per job and its `reserve`, parsed once at
//! a boundary into values that cannot name an undeclared backend, a model name that could carry
//! bytes anywhere, or a reserve outside `[0, 1]`.
//!
//! Each backend is spelled once: `claude-cli/` is only the subscription, `anthropic/` only the
//! Messages API; a job that wants both lists both, in order.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::settings::DeclaredProvider;

/// A job whose model is chosen, spelled on the wire as `[models]`'s own key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Job {
    /// The conversation in the app.
    Secretary,
    /// The triage agent's batches, settled by `suzatary secretary review`.
    Review,
    /// Naming a topic after its first answer.
    Title,
}

impl Job {
    /// Every job, in the order a page and `doctor` list them.
    pub const ALL: [Job; 3] = [Job::Secretary, Job::Review, Job::Title];

    /// The key `[models]` and a log line spell it by.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Job::Secretary => "secretary",
            Job::Review => "review",
            Job::Title => "title",
        }
    }

    /// What the owner calls it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Job::Secretary => "Chat",
            Job::Review => "Review",
            Job::Title => "Titles",
        }
    }

    /// The job a `[models]` key or a request names.
    ///
    /// # Errors
    /// [`ModelsError::UnknownJob`] for anything but the three keys.
    pub fn parse(text: &str) -> Result<Self, ModelsError> {
        Self::ALL
            .into_iter()
            .find(|job| job.as_str() == text)
            .ok_or_else(|| ModelsError::UnknownJob(text.to_owned()))
    }
}

/// The subscription, through the `claude` CLI subprocess.
pub const CLAUDE_CLI: &str = "claude-cli";

/// The Anthropic Messages API, over `ANTHROPIC_API_KEY`.
pub const ANTHROPIC: &str = "anthropic";

/// Who answers an entry: a built-in backend or a declared provider. A `Declared` name is built
/// only by [`Backend::parse`] against the declared list.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Backend {
    /// The subscription (`claude-cli/`), metered by the account's window.
    ClaudeCli,
    /// The Messages API (`anthropic/`), billed per token on its own key.
    AnthropicApi,
    /// A declared OpenAI-compatible provider, by its name.
    Declared(String),
}

impl Backend {
    /// Read a backend's spelling against the providers this deployment
    /// declares. The two built-in names are reserved in `[[providers]]`, so
    /// the arms cannot overlap.
    ///
    /// # Errors
    /// [`ModelsError::Undeclared`] naming the backend, when it is neither
    /// built in nor declared.
    pub fn parse(declared: &[DeclaredProvider], text: &str) -> Result<Self, ModelsError> {
        match text {
            CLAUDE_CLI => Ok(Self::ClaudeCli),
            ANTHROPIC => Ok(Self::AnthropicApi),
            name if declared.iter().any(|provider| provider.name == name) => {
                Ok(Self::Declared(name.to_owned()))
            }
            other => Err(ModelsError::Undeclared(other.to_owned())),
        }
    }

    /// The spelling a `[models]` entry uses.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::ClaudeCli => CLAUDE_CLI,
            Self::AnthropicApi => ANTHROPIC,
            Self::Declared(name) => name,
        }
    }
}

impl Serialize for Backend {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// A model name, `[A-Za-z0-9._:/-]{1,128}`: what vendors' ids use and nothing a request or a
/// TOML string could be confused by. The backend, not the name, fixes the endpoint.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ModelName(String);

impl ModelName {
    /// Longest model name accepted.
    pub const MAX: usize = 128;

    /// Parse one.
    ///
    /// # Errors
    /// [`ModelsError::BadModel`] for an empty, overlong or out-of-class name.
    pub fn parse(text: &str) -> Result<Self, ModelsError> {
        let fits = (1..=Self::MAX).contains(&text.len())
            && text.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'/' | b'-')
            });
        if fits {
            Ok(Self(text.to_owned()))
        } else {
            Err(ModelsError::BadModel(text.to_owned()))
        }
    }

    /// The name, as the backend's own API takes it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

crate::literal::text_literal!(
    ModelName => |name| name.as_str().to_owned(),
);

impl TryFrom<String> for ModelName {
    type Error = ModelsError;
    fn try_from(text: String) -> Result<Self, ModelsError> {
        Self::parse(&text)
    }
}

impl From<ModelName> for String {
    fn from(name: ModelName) -> Self {
        name.0
    }
}

/// One entry in a job's order, `"<backend>/<model>"`; the first `/` splits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS)]
#[ts(rename = "ModelEntryRow")]
pub struct ModelEntry {
    /// Who answers: `claude-cli`, `anthropic`, or a declared provider's name.
    #[ts(type = "string")]
    pub backend: Backend,
    /// Which of its models.
    #[ts(type = "string")]
    pub model: ModelName,
}

impl ModelEntry {
    /// Parse `"<backend>/<model>"` against the declared providers.
    ///
    /// # Errors
    /// [`ModelsError::NoSlash`] with no `/`, [`ModelsError::Undeclared`] for
    /// a backend nobody declared, [`ModelsError::BadModel`] for the name.
    pub fn parse(declared: &[DeclaredProvider], text: &str) -> Result<Self, ModelsError> {
        let (backend, model) = text
            .split_once('/')
            .ok_or_else(|| ModelsError::NoSlash(text.to_owned()))?;
        Ok(Self {
            backend: Backend::parse(declared, backend)?,
            model: ModelName::parse(model)?,
        })
    }
}

impl std::fmt::Display for ModelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.backend.as_str(), self.model.as_str())
    }
}

/// The subscription's share automated jobs never take, in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Reserve(f64);

impl Reserve {
    /// Parse one.
    ///
    /// # Errors
    /// [`ModelsError::BadReserve`] outside `[0, 1]`, NaN included.
    pub fn parse(share: f64) -> Result<Self, ModelsError> {
        if (0.0..=1.0).contains(&share) {
            Ok(Self(share))
        } else {
            Err(ModelsError::BadReserve(share))
        }
    }

    /// The share.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

// `parse` refuses NaN, so every reserve equals itself.
impl Eq for Reserve {}

impl TryFrom<f64> for Reserve {
    type Error = ModelsError;
    fn try_from(share: f64) -> Result<Self, ModelsError> {
        Self::parse(share)
    }
}

impl From<Reserve> for f64 {
    fn from(reserve: Reserve) -> Self {
        reserve.0
    }
}

impl Default for Reserve {
    /// 30% held back when the file names none.
    fn default() -> Self {
        Self(0.30)
    }
}

/// `[models]`, parsed. An empty order means the binary's fallback.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Models {
    secretary: Vec<ModelEntry>,
    review: Vec<ModelEntry>,
    title: Vec<ModelEntry>,
    reserve: Reserve,
}

impl Models {
    /// The order a job walks.
    #[must_use]
    pub fn order(&self, job: Job) -> &[ModelEntry] {
        match job {
            Job::Secretary => &self.secretary,
            Job::Review => &self.review,
            Job::Title => &self.title,
        }
    }

    /// Whether any job's order names the declared provider `name`.
    #[must_use]
    pub fn orders(&self, name: &str) -> bool {
        Job::ALL.into_iter().any(|job| {
            self.order(job).iter().any(
                |entry| matches!(&entry.backend, Backend::Declared(declared) if declared == name),
            )
        })
    }

    /// The reserve.
    #[must_use]
    pub const fn reserve(&self) -> Reserve {
        self.reserve
    }

    /// The same table with `job`'s order replaced and nothing else touched.
    #[must_use]
    pub fn with_order(mut self, job: Job, entries: Vec<ModelEntry>) -> Self {
        *match job {
            Job::Secretary => &mut self.secretary,
            Job::Review => &mut self.review,
            Job::Title => &mut self.title,
        } = entries;
        self
    }

    /// The same table with the reserve replaced.
    #[must_use]
    pub fn with_reserve(self, reserve: Reserve) -> Self {
        Self { reserve, ..self }
    }
}

/// Why a job, an entry or a reserve did not parse, each naming what was typed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ModelsError {
    /// Not one of the three jobs.
    #[error("{0:?} is not a job — secretary, review or title")]
    UnknownJob(String),
    /// No `/` between backend and model.
    #[error("{0:?} is not \"<backend>/<model>\"")]
    NoSlash(String),
    /// A backend neither built in nor declared in `[[providers]]`.
    #[error(
        "{0:?} is not a backend this deployment has: claude-cli, anthropic, or a declared provider"
    )]
    Undeclared(String),
    /// A model name outside `[A-Za-z0-9._:/-]{{1,128}}`.
    #[error("{0:?} is not a model name (1 to 128 of A-Z a-z 0-9 . _ : / -)")]
    BadModel(String),
    /// A reserve outside `[0, 1]`.
    #[error("a reserve is a share between 0 and 1, got {0}")]
    BadReserve(f64),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared() -> Vec<DeclaredProvider> {
        vec![DeclaredProvider {
            name: "databricks".to_owned(),
            base_url: "https://acme.cloud.databricks.com/serving-endpoints".to_owned(),
            key: "DATABRICKS_TOKEN".to_owned(),
            attestation: crate::settings::Attestation::Unclaimed,
        }]
    }

    fn entry(text: &str) -> ModelEntry {
        ModelEntry::parse(&declared(), text).expect("a well-formed entry")
    }

    /// Each backend is spelled once, and the spelling round-trips.
    #[test]
    fn every_backend_spelling_prints_back() {
        for text in [
            "claude-cli/claude-opus-5-5",
            "anthropic/claude-sonnet-5",
            "databricks/databricks-deepseek-v4.1-flash",
            "databricks/org/model:tag",
        ] {
            assert_eq!(entry(text).to_string(), text);
        }
        assert_eq!(entry("claude-cli/x").backend, Backend::ClaudeCli);
        assert_eq!(entry("anthropic/x").backend, Backend::AnthropicApi);
        assert_eq!(
            entry("databricks/x").backend,
            Backend::Declared("databricks".to_owned())
        );
    }

    /// An undeclared backend is refused by name.
    #[test]
    fn an_undeclared_backend_is_refused_by_name() {
        assert_eq!(
            ModelEntry::parse(&declared(), "openrouter/some-model"),
            Err(ModelsError::Undeclared("openrouter".to_owned()))
        );
        assert_eq!(
            ModelEntry::parse(&[], "databricks/x"),
            Err(ModelsError::Undeclared("databricks".to_owned()))
        );
    }

    /// A model name cannot carry anything but its class.
    #[test]
    fn a_model_name_is_its_class_and_length() {
        for bad in [
            "",
            "has space",
            "a\"b",
            "a\nb",
            "x?y=z",
            "é",
            &"m".repeat(129),
        ] {
            assert_eq!(
                ModelName::parse(bad),
                Err(ModelsError::BadModel(bad.to_owned())),
                "{bad:?}"
            );
        }
        assert!(ModelName::parse(&"m".repeat(128)).is_ok());
        assert_eq!(
            ModelEntry::parse(&declared(), "claude-cli"),
            Err(ModelsError::NoSlash("claude-cli".to_owned()))
        );
        assert_eq!(
            ModelEntry::parse(&declared(), "claude-cli/"),
            Err(ModelsError::BadModel(String::new()))
        );
    }

    #[test]
    fn a_reserve_is_a_share() {
        for good in [0.0, 0.3, 1.0] {
            assert_eq!(Reserve::parse(good).map(Reserve::get), Ok(good));
        }
        for bad in [-0.01, 1.01, f64::INFINITY] {
            assert_eq!(Reserve::parse(bad), Err(ModelsError::BadReserve(bad)));
        }
        assert!(Reserve::parse(f64::NAN).is_err());
    }

    #[test]
    fn a_job_parses_its_own_spelling_only() {
        for job in Job::ALL {
            assert_eq!(Job::parse(job.as_str()), Ok(job));
        }
        assert_eq!(
            Job::parse("chat"),
            Err(ModelsError::UnknownJob("chat".to_owned()))
        );
    }

    /// `with_order` replaces one job's order and leaves every other one, and
    /// the reserve, exactly where they were; setting twice is setting once.
    #[test]
    fn with_order_touches_one_job() {
        let before = Models::default()
            .with_order(Job::Review, vec![entry("databricks/a")])
            .with_reserve(Reserve::parse(0.5).expect("a share"));
        let after = before.clone().with_order(
            Job::Secretary,
            vec![entry("claude-cli/b"), entry("anthropic/c")],
        );
        assert_eq!(after.order(Job::Review), before.order(Job::Review));
        assert_eq!(after.order(Job::Title), before.order(Job::Title));
        assert_eq!(after.reserve(), before.reserve());
        assert_eq!(
            after.order(Job::Secretary),
            &[entry("claude-cli/b"), entry("anthropic/c")]
        );
        assert_eq!(
            after
                .clone()
                .with_order(Job::Secretary, after.order(Job::Secretary).to_vec()),
            after
        );
    }
}
