//! A Configure card's change: one setting of `[models]` or `[[providers]]`, moved from the value
//! the card read when it was filed to the value it proposes.
//!
//! Each setting is a view of what the instance file declares ([`Configured`]). One rule decides
//! every card ([`Configure::perform`]): the file must still hold the card's `from`, and then its
//! `to` is written. A file that already holds `to` is done with nothing written, so a second card
//! for the same change is no error; a file that holds anything else is written nothing, and the
//! refusal says what it holds now. The file is read, compared and written as one step under the
//! editor's lock ([`Edits::swap`]), so two taps at once cannot both read the same `from`.

use prodrome_core::literal::{ProdromeError, Value};
use serde::{Deserialize, Serialize};

use crate::literal::{Fields, Literal, Signature, arm, no_arm, record};
use crate::models::{Backend, Job, ModelEntry, ModelName, Models, ModelsError, Reserve};
use crate::settings::{
    Attestation, DeclaredProvider, EditError, Edits, Help, Preset, ProviderSetup, SourceCommit,
};

/// What the instance file declares of the settings a card changes, read when a card is filed
/// and again, under the editor's lock, when it is tapped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Configured {
    /// `[models]`.
    pub models: Models,
    /// `[[providers]]`.
    pub providers: Vec<DeclaredProvider>,
}

impl Configured {
    /// The provider declared as `name`.
    #[must_use]
    pub fn provider(&self, name: &str) -> Option<&DeclaredProvider> {
        self.providers.iter().find(|provider| provider.name == name)
    }
}

/// Reads the settings and connections a card changes from the instance file as it is now; a read
/// alone, so a record that files cards may hold it.
pub trait Configuration: Send + Sync {
    /// The settings, as the file declares them now.
    ///
    /// # Errors
    /// [`EditError::Io`] or [`EditError::Malformed`].
    fn configured(&self) -> Result<Configured, EditError>;

    /// The tables connections are read from, as the file declares them now, so a tap acts on
    /// the file rather than on what a process read when it started, and a Connect card is filed
    /// only for what the file does not hold yet.
    ///
    /// # Errors
    /// [`EditError::Io`] or [`EditError::Malformed`].
    fn declared(&self) -> Result<crate::connect::Declared, EditError>;
}

/// Which setting a card changes, as the overview's lines name them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Setting {
    /// A job's model order.
    Order(Job),
    /// The share automated jobs leave for the owner.
    Reserve,
    /// The provider of this name.
    Provider(String),
}

impl Setting {
    /// What the owner calls it.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Setting::Order(job) => job.label().to_owned(),
            Setting::Reserve => "Reserve".to_owned(),
            Setting::Provider(name) => provider_label(name),
        }
    }
}

/// What the owner calls the provider `name`: its preset's name, or its own.
#[must_use]
pub fn provider_label(name: &str) -> String {
    Preset::ALL
        .into_iter()
        .find(|preset| preset.backend() == name)
        .map_or_else(|| name.to_owned(), |preset| preset.label().to_owned())
}

/// One entry of an order as a card holds it: `"<backend>/<model>"` as [`ModelEntry::parse`]
/// accepted it when the card was filed. Whether its backend is declared is a question for the
/// moment of the tap, so it becomes a [`ModelEntry`] only through [`Entry::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    backend: String,
    model: ModelName,
}

impl Entry {
    fn of(entry: &ModelEntry) -> Self {
        Self {
            backend: entry.backend.as_str().to_owned(),
            model: entry.model.clone(),
        }
    }

    /// An entry a stored card printed: a backend's name and a model name, the backend's
    /// declaration left to [`Entry::parse`].
    fn stored(text: &str) -> Result<Self, ModelsError> {
        match text.split_once('/') {
            Some((backend, model)) if !backend.is_empty() => Ok(Self {
                backend: backend.to_owned(),
                model: ModelName::parse(model)?,
            }),
            Some(_) => Err(ModelsError::Undeclared(String::new())),
            None => Err(ModelsError::NoSlash(text.to_owned())),
        }
    }

    /// The entry against the providers declared now.
    ///
    /// # Errors
    /// [`ModelsError::Undeclared`] for a backend no longer declared.
    fn parse(&self, providers: &[DeclaredProvider]) -> Result<ModelEntry, ModelsError> {
        ModelEntry::parse(providers, &self.to_string())
    }
}

impl std::fmt::Display for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.backend, self.model.as_str())
    }
}

/// One change of a value: what the card read, and what it writes. The two differ.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Move<T> {
    from: T,
    to: T,
}

impl<T: PartialEq> Move<T> {
    fn new(from: T, to: T) -> Result<Self, Refused> {
        if from == to {
            Err(Refused::Unmoved)
        } else {
            Ok(Self { from, to })
        }
    }

    /// `None` when the file already holds `to`, `to` when it holds `from`, else what it holds
    /// now in words.
    fn against(&self, held: &T, words: impl Fn(&T) -> String) -> Result<Option<&T>, Refused> {
        if *held == self.to {
            Ok(None)
        } else if *held == self.from {
            Ok(Some(&self.to))
        } else {
            Err(Refused::Moved { now: words(held) })
        }
    }
}

/// A provider's change: declared from a preset (where it was absent or declared otherwise), or
/// removed. What it declares is a [`ProviderSetup`], so a card can only declare what a preset
/// derives, and never an endpoint typed by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Provided {
    Declare {
        from: Option<DeclaredProvider>,
        to: ProviderSetup,
    },
    Remove {
        from: DeclaredProvider,
    },
}

impl Provided {
    fn new(from: Option<DeclaredProvider>, to: Option<ProviderSetup>) -> Result<Self, Refused> {
        match (from, to) {
            (None, None) => Err(Refused::Unmoved),
            (Some(from), None) => Ok(Provided::Remove { from }),
            (from, Some(to)) => {
                let derived = to.declared();
                match &from {
                    Some(held) if *held == derived => Err(Refused::Unmoved),
                    Some(held) if held.name != derived.name => Err(Refused::Renamed {
                        from: held.name.clone(),
                        to: derived.name,
                    }),
                    None | Some(_) => Ok(Provided::Declare { from, to }),
                }
            }
        }
    }

    fn name(&self) -> &str {
        match self {
            Provided::Declare { to, .. } => to.preset().backend(),
            Provided::Remove { from } => &from.name,
        }
    }

    /// The change as the file holds it: the provider before, and the one its preset derives.
    fn moved(&self) -> Move<Option<DeclaredProvider>> {
        match self {
            Provided::Declare { from, to } => Move {
                from: from.clone(),
                to: Some(to.declared()),
            },
            Provided::Remove { from } => Move {
                from: Some(from.clone()),
                to: None,
            },
        }
    }

    fn parts(&self) -> (Option<&DeclaredProvider>, Option<&ProviderSetup>) {
        match self {
            Provided::Declare { from, to } => (from.as_ref(), Some(to)),
            Provided::Remove { from } => (Some(from), None),
        }
    }
}

/// One setting a tap changes, built only by [`Configure::order`], [`Configure::reserve`],
/// [`Configure::declare`] and [`Configure::remove`], or read back from what one of them stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configure(Change);

/// The closed sum of what a card can change.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Change {
    Order { job: Job, order: Move<Vec<Entry>> },
    Reserve(Move<Reserve>),
    Provider(Provided),
}

/// What a card asks of the file once the file has been read: the one value it writes.
#[derive(Debug, Clone, PartialEq)]
pub enum Write {
    /// `[models].<job>`.
    Order(Job, Vec<ModelEntry>),
    /// `[models].reserve`.
    Reserve(Reserve),
    /// The `[[providers]]` table named `name`, declared as `to` or taken out.
    Provider {
        /// Its name.
        name: String,
        /// What it becomes, `None` for removed.
        to: Option<DeclaredProvider>,
    },
}

impl Configure {
    /// `job`'s order changed to `to`, each entry parsed against the providers `now` declares.
    ///
    /// # Errors
    /// [`Refused::Entry`] for an entry no parser accepts, [`Refused::Unmoved`] for the order
    /// already held.
    pub fn order(now: &Configured, job: Job, to: &[&str]) -> Result<Self, Refused> {
        let to = to
            .iter()
            .map(|text| ModelEntry::parse(&now.providers, text).map(|entry| Entry::of(&entry)))
            .collect::<Result<Vec<_>, _>>()?;
        let from = now.models.order(job).iter().map(Entry::of).collect();
        Ok(Self(Change::Order {
            job,
            order: Move::new(from, to)?,
        }))
    }

    /// The reserve changed to `to`.
    ///
    /// # Errors
    /// [`Refused::Unmoved`] for the reserve already held.
    pub fn reserve(now: &Configured, to: Reserve) -> Result<Self, Refused> {
        Ok(Self(Change::Reserve(Move::new(now.models.reserve(), to)?)))
    }

    /// The provider `setup` derives, declared, replacing one declared under its name.
    ///
    /// # Errors
    /// [`Refused::Unmoved`] when it is declared so already.
    pub fn declare(now: &Configured, setup: ProviderSetup) -> Result<Self, Refused> {
        let held = now.provider(setup.preset().backend()).cloned();
        Ok(Self(Change::Provider(Provided::new(held, Some(setup))?)))
    }

    /// The provider `name` removed: what a card offering to take out a provider no job uses
    /// files, whether the secretary or the box's own rules ask. A job still ordering it at the
    /// tap refuses it.
    ///
    /// # Errors
    /// [`Refused::Undeclared`] when no provider of that name is declared.
    pub fn remove(now: &Configured, name: &str) -> Result<Self, Refused> {
        let held = now
            .provider(name)
            .cloned()
            .ok_or_else(|| Refused::Undeclared {
                provider: provider_label(name),
            })?;
        Ok(Self(Change::Provider(Provided::Remove { from: held })))
    }

    /// The provider it declares, `None` for any other change.
    #[must_use]
    pub fn declares(&self) -> Option<&str> {
        match &self.0 {
            Change::Provider(provided @ Provided::Declare { .. }) => Some(provided.name()),
            Change::Provider(Provided::Remove { .. })
            | Change::Order { .. }
            | Change::Reserve(_) => None,
        }
    }

    /// Which setting it changes.
    #[must_use]
    pub fn setting(&self) -> Setting {
        match &self.0 {
            Change::Order { job, .. } => Setting::Order(*job),
            Change::Reserve(_) => Setting::Reserve,
            Change::Provider(provider) => Setting::Provider(provider.name().to_owned()),
        }
    }

    /// The value it read and the value it writes, in the owner's words. Two values that differ
    /// never read the same.
    #[must_use]
    pub fn words(&self) -> (String, String) {
        match &self.0 {
            Change::Order { order, .. } => (order_words(&order.from), order_words(&order.to)),
            Change::Reserve(reserve) => (reserve_words(reserve.from), reserve_words(reserve.to)),
            Change::Provider(provider) => {
                let moved = provider.moved();
                (
                    provider_words(moved.from.as_ref()),
                    provider_words(moved.to.as_ref()),
                )
            }
        }
    }

    /// The imperative a card and a turn print: a provider added or removed is said so, and any
    /// other change from and to.
    #[must_use]
    pub fn sentence(&self) -> String {
        let label = self.setting().label();
        match &self.0 {
            Change::Provider(Provided::Declare { from: None, .. }) => {
                format!("add {label}: {}", self.words().1)
            }
            Change::Provider(Provided::Remove { .. }) => format!("remove {label}"),
            Change::Provider(Provided::Declare { from: Some(_), .. })
            | Change::Order { .. }
            | Change::Reserve(_) => {
                let (from, to) = self.words();
                format!("change {label}: {from} → {to}")
            }
        }
    }

    /// Change the file through `edits` while it still holds `from`, read and written as one
    /// step under the editor's lock.
    ///
    /// # Errors
    /// [`Refused::Moved`] when the file holds neither `from` nor `to`, [`Refused::Entry`] for an
    /// entry whose provider is no longer declared, [`Refused::Ordered`] for a provider a job
    /// still orders, and [`Refused::Edit`] for the file's own failure. Each wrote nothing.
    pub fn perform(&self, edits: &dyn Edits) -> Result<Performed, Refused> {
        edits.swap(&mut |now| self.write(now))
    }

    /// What the file `now` should be written, `None` when it already holds `to`.
    ///
    /// # Errors
    /// As [`Configure::perform`], but [`Refused::Edit`].
    pub fn write(&self, now: &Configured) -> Result<Option<Write>, Refused> {
        match &self.0 {
            Change::Order { job, order } => {
                let held: Vec<Entry> = now.models.order(*job).iter().map(Entry::of).collect();
                order
                    .against(&held, |held| order_words(held))?
                    .map(|to| {
                        // The providers may have moved since the card was filed.
                        let to = to
                            .iter()
                            .map(|entry| entry.parse(&now.providers))
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(Write::Order(*job, to))
                    })
                    .transpose()
            }
            Change::Reserve(reserve) => Ok(reserve
                .against(&now.models.reserve(), |held| reserve_words(*held))?
                .map(|to| Write::Reserve(*to))),
            Change::Provider(provider) => {
                let name = provider.name();
                provider
                    .moved()
                    .against(&now.provider(name).cloned(), |held| {
                        provider_words(held.as_ref())
                    })?
                    .map(|to| {
                        let jobs = ordering(&now.models, name);
                        if to.is_none() && !jobs.is_empty() {
                            return Err(Refused::Ordered {
                                provider: provider_label(name),
                                jobs,
                            });
                        }
                        Ok(Write::Provider {
                            name: name.to_owned(),
                            to: to.clone(),
                        })
                    })
                    .transpose()
            }
        }
    }
}

/// What a performed card did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Performed {
    /// The file held `from`, and now holds `to`.
    Written,
    /// The file already held `to`; nothing was written.
    Already,
}

/// The jobs whose order names the provider `name`, in the owner's words.
fn ordering(models: &Models, name: &str) -> Vec<String> {
    Job::ALL
        .into_iter()
        .filter(|job| {
            models
                .order(*job)
                .iter()
                .any(|entry| matches!(&entry.backend, Backend::Declared(held) if held == name))
        })
        .map(|job| job.label().to_owned())
        .collect()
}

fn order_words(order: &[Entry]) -> String {
    if order.is_empty() {
        "nothing chosen, so the fallback answers".to_owned()
    } else {
        order
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", then ")
    }
}

/// A reserve in words: a whole percent when it is one, else the share itself as its shortest
/// exact print, so two reserves that differ never read the same.
fn reserve_words(reserve: Reserve) -> String {
    match whole_percent(reserve) {
        Some(percent) => format!("keeps {percent}% for you"),
        None => format!("keeps a share of {} for you", reserve.get()),
    }
}

/// The reserve as a whole percent, when it is exactly one.
fn whole_percent(reserve: Reserve) -> Option<f64> {
    let share = reserve.get();
    let percent = (share * 100.0).round();
    // Exactly, never within a margin: a reserve a hair off a whole percent is another value.
    ((percent / 100.0).to_bits() == share.to_bits()).then_some(percent)
}

/// A provider in words: where it is, which key it reads in the owner's words (a key's stored
/// name never leaves the box), and the source commit an attested receipt must name, whole.
fn provider_words(provider: Option<&DeclaredProvider>) -> String {
    provider.map_or_else(
        || "not added".to_owned(),
        |provider| {
            let attested = match &provider.attestation {
                Attestation::Unclaimed => String::new(),
                Attestation::Required { expect } => {
                    format!(", attested to source commit {}", expect.as_str())
                }
            };
            format!(
                "answers at {}, with {}{attested}",
                provider.base_url,
                Help::label(&provider.key)
            )
        },
    )
}

/// Why a card was not filed, or a tap wrote nothing.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Refused {
    /// The card would change nothing.
    #[error("that is what it holds already")]
    Unmoved,
    /// A provider removed that is not declared.
    #[error("no provider named {provider} is declared")]
    Undeclared {
        /// What the owner calls it.
        provider: String,
    },
    /// A provider's change names two providers.
    #[error("a provider's change names one provider, not {from} and {to}")]
    Renamed {
        /// The one it read.
        from: String,
        /// The one it would write.
        to: String,
    },
    /// An entry no parser accepts against the providers declared.
    #[error(transparent)]
    Entry(#[from] ModelsError),
    /// The file holds neither what the card read nor what it writes.
    #[error("the setting has changed since this card was filed: it now reads {now}")]
    Moved {
        /// What the file holds now, in the owner's words.
        now: String,
    },
    /// A provider a job still orders.
    #[error("{} still orders {provider}: change that first", .jobs.join(" and "))]
    Ordered {
        /// The provider.
        provider: String,
        /// The jobs that order it.
        jobs: Vec<String>,
    },
    /// The file could not be read or written.
    #[error(transparent)]
    Edit(#[from] EditError),
}

/// `ConfigureOrder(job, from, to)`.
pub const CONFIGURE_ORDER: Signature = ("ConfigureOrder", &["job", "from", "to"]);
/// `ConfigureReserve(from, to)`.
pub const CONFIGURE_RESERVE: Signature = ("ConfigureReserve", &["from", "to"]);
/// `ConfigureProvider(from, to)`.
pub const CONFIGURE_PROVIDER: Signature = ("ConfigureProvider", &["from", "to"]);
/// `DeclaredProvider(name, base_url, key, expect)`, a provider as the file held it.
pub const DECLARED_PROVIDER: Signature =
    ("DeclaredProvider", &["name", "base_url", "key", "expect"]);
/// `ProviderSetup(preset, workspace, expect)`, a preset and the one field it reads.
pub const PROVIDER_SETUP: Signature = ("ProviderSetup", &["preset", "workspace", "expect"]);

fn invalid(failed: impl std::fmt::Display) -> ProdromeError {
    ProdromeError::invalid(failed.to_string())
}

impl Literal for Configure {
    fn to_value(&self) -> Value {
        match &self.0 {
            Change::Order { job, order } => record(
                CONFIGURE_ORDER,
                vec![
                    Value::str(job.as_str()),
                    order.from.to_value(),
                    order.to.to_value(),
                ],
            ),
            Change::Reserve(reserve) => record(
                CONFIGURE_RESERVE,
                vec![reserve.from.to_value(), reserve.to.to_value()],
            ),
            Change::Provider(provider) => {
                let (from, to) = provider.parts();
                record(
                    CONFIGURE_PROVIDER,
                    vec![from.cloned().to_value(), to.cloned().to_value()],
                )
            }
        }
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        match arm(value)? {
            "ConfigureOrder" => {
                let fields = Fields::of(value, CONFIGURE_ORDER)?;
                Ok(Self(Change::Order {
                    job: Job::parse(&fields.get::<String>("job")?).map_err(invalid)?,
                    order: Move::new(fields.get("from")?, fields.get("to")?).map_err(invalid)?,
                }))
            }
            "ConfigureReserve" => {
                let fields = Fields::of(value, CONFIGURE_RESERVE)?;
                Ok(Self(Change::Reserve(
                    Move::new(fields.get("from")?, fields.get("to")?).map_err(invalid)?,
                )))
            }
            "ConfigureProvider" => {
                let fields = Fields::of(value, CONFIGURE_PROVIDER)?;
                Ok(Self(Change::Provider(
                    Provided::new(fields.get("from")?, fields.get("to")?).map_err(invalid)?,
                )))
            }
            other => Err(no_arm("a setting a card changes", other)),
        }
    }
}

impl Literal for Entry {
    fn to_value(&self) -> Value {
        Value::str(self.to_string())
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        Entry::stored(&String::from_value(value)?).map_err(invalid)
    }
}

impl Literal for Reserve {
    fn to_value(&self) -> Value {
        Value::float(self.get()).expect("a reserve is a share between 0 and 1")
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        match value {
            Value::Float(share) => Reserve::parse(share.get()).map_err(invalid),
            other => Err(invalid(format!("expected a share, got {other:?}"))),
        }
    }
}

impl Literal for DeclaredProvider {
    fn to_value(&self) -> Value {
        let expect = match &self.attestation {
            Attestation::Unclaimed => None,
            Attestation::Required { expect } => Some(expect.as_str().to_owned()),
        };
        record(
            DECLARED_PROVIDER,
            vec![
                Value::str(self.name.as_str()),
                Value::str(self.base_url.as_str()),
                Value::str(self.key.as_str()),
                expect.to_value(),
            ],
        )
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        let fields = Fields::of(value, DECLARED_PROVIDER)?;
        Ok(DeclaredProvider {
            name: fields.get("name")?,
            base_url: fields.get("base_url")?,
            key: fields.get("key")?,
            attestation: match fields.get::<Option<String>>("expect")? {
                None => Attestation::Unclaimed,
                Some(expect) => Attestation::Required {
                    expect: SourceCommit::parse(&expect).map_err(invalid)?,
                },
            },
        })
    }
}

/// Read back through [`ProviderSetup::parse`], so a stored card names only what a preset
/// derives.
impl Literal for ProviderSetup {
    fn to_value(&self) -> Value {
        let (workspace, expect) = self.fields();
        record(
            PROVIDER_SETUP,
            vec![
                Value::str(self.preset().as_str()),
                workspace.to_value(),
                expect.to_value(),
            ],
        )
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        let fields = Fields::of(value, PROVIDER_SETUP)?;
        let preset = fields.get::<String>("preset")?;
        let preset = Preset::ALL
            .into_iter()
            .find(|known| known.as_str() == preset)
            .ok_or_else(|| invalid(format!("{preset:?} is no preset")))?;
        ProviderSetup::parse(
            preset,
            fields.get::<Option<String>>("workspace")?.as_deref(),
            fields.get::<Option<String>>("expect")?.as_deref(),
        )
        .map_err(invalid)
    }
}

/// A card's text form, as a page or a verb spells it: each value as the file spells it, read back
/// through the same constructors as its literal.
#[derive(Serialize, Deserialize)]
#[serde(tag = "setting", rename_all = "lowercase")]
enum Spelled {
    Order {
        job: Job,
        from: Vec<String>,
        to: Vec<String>,
    },
    Reserve {
        from: Reserve,
        to: Reserve,
    },
    Provider {
        from: Option<SpelledProvider>,
        to: Option<SpelledSetup>,
    },
}

#[derive(Serialize, Deserialize)]
struct SpelledProvider {
    name: String,
    base_url: String,
    key: String,
    expect: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SpelledSetup {
    preset: Preset,
    workspace: Option<String>,
    expect: Option<String>,
}

impl Serialize for Configure {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let spelled = |order: &[Entry]| order.iter().map(ToString::to_string).collect();
        match &self.0 {
            Change::Order { job, order } => Spelled::Order {
                job: *job,
                from: spelled(&order.from),
                to: spelled(&order.to),
            },
            Change::Reserve(reserve) => Spelled::Reserve {
                from: reserve.from,
                to: reserve.to,
            },
            Change::Provider(provider) => {
                let (from, to) = provider.parts();
                Spelled::Provider {
                    from: from.map(|from| SpelledProvider {
                        name: from.name.clone(),
                        base_url: from.base_url.clone(),
                        key: from.key.clone(),
                        expect: match &from.attestation {
                            Attestation::Unclaimed => None,
                            Attestation::Required { expect } => Some(expect.as_str().to_owned()),
                        },
                    }),
                    to: to.map(|to| {
                        let (workspace, expect) = to.fields();
                        SpelledSetup {
                            preset: to.preset(),
                            workspace,
                            expect,
                        }
                    }),
                }
            }
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Configure {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let stored = |order: Vec<String>| {
            order
                .iter()
                .map(|text| Entry::stored(text))
                .collect::<Result<Vec<_>, _>>()
                .map_err(D::Error::custom)
        };
        match Spelled::deserialize(deserializer)? {
            Spelled::Order { job, from, to } => Ok(Self(Change::Order {
                job,
                order: Move::new(stored(from)?, stored(to)?).map_err(D::Error::custom)?,
            })),
            Spelled::Reserve { from, to } => Ok(Self(Change::Reserve(
                Move::new(from, to).map_err(D::Error::custom)?,
            ))),
            Spelled::Provider { from, to } => {
                let from = from
                    .map(|from| {
                        Ok::<_, D::Error>(DeclaredProvider {
                            name: from.name,
                            base_url: from.base_url,
                            key: from.key,
                            attestation: match from.expect {
                                None => Attestation::Unclaimed,
                                Some(expect) => Attestation::Required {
                                    expect: SourceCommit::parse(&expect)
                                        .map_err(D::Error::custom)?,
                                },
                            },
                        })
                    })
                    .transpose()?;
                let to = to
                    .map(|to| {
                        ProviderSetup::parse(
                            to.preset,
                            to.workspace.as_deref(),
                            to.expect.as_deref(),
                        )
                        .map_err(D::Error::custom)
                    })
                    .transpose()?;
                Ok(Self(Change::Provider(
                    Provided::new(from, to).map_err(D::Error::custom)?,
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use proptest::prelude::*;

    use super::*;
    use crate::settings::WorkspaceUrl;

    fn databricks() -> DeclaredProvider {
        ProviderSetup::Databricks {
            workspace: WorkspaceUrl::parse("https://acme.cloud.databricks.com").expect("a host"),
        }
        .declared()
    }

    fn phala(commit: &str) -> ProviderSetup {
        ProviderSetup::Phala {
            expect: SourceCommit::parse(commit).expect("a commit"),
        }
    }

    /// An instance file in memory: what a card reads and what its write leaves.
    #[derive(Default)]
    struct File(Mutex<Configured>);

    impl Configuration for File {
        fn configured(&self) -> Result<Configured, EditError> {
            Ok(self.0.lock().expect("never poisoned").clone())
        }

        fn declared(&self) -> Result<crate::connect::Declared, EditError> {
            unreachable!("a card reads its settings alone")
        }
    }

    impl Edits for File {
        fn swap(
            &self,
            decide: &mut dyn FnMut(&Configured) -> Result<Option<Write>, Refused>,
        ) -> Result<Performed, Refused> {
            let mut file = self.0.lock().expect("never poisoned");
            Ok(match decide(&file)? {
                None => Performed::Already,
                Some(Write::Order(job, entries)) => {
                    file.models = file.models.clone().with_order(job, entries);
                    Performed::Written
                }
                Some(Write::Reserve(reserve)) => {
                    file.models = file.models.clone().with_reserve(reserve);
                    Performed::Written
                }
                Some(Write::Provider { name, to }) => {
                    file.providers.retain(|provider| provider.name != name);
                    file.providers.extend(to);
                    Performed::Written
                }
            })
        }

        fn allow_web(&self, _: &crate::web::AllowRule) -> Result<(), EditError> {
            unreachable!("a card changes no allow list")
        }

        fn allow_drive(&self, _: &crate::drive::DrivePath) -> Result<(), EditError> {
            unreachable!("a card changes no allow list")
        }

        fn add_connection(&self, _: &crate::connect::Connection) -> Result<(), EditError> {
            unreachable!("a card changes no connection")
        }

        fn remove_connection(&self, _: &crate::connect::Connection) -> Result<(), EditError> {
            unreachable!("a card changes no connection")
        }

        fn add_origin(
            &self,
            _: &crate::plugin::PluginName,
            _: &crate::plugin::Origin,
        ) -> Result<(), EditError> {
            unreachable!("a card changes no origin")
        }
    }

    fn file() -> File {
        File(Mutex::new(Configured {
            models: Models::default(),
            providers: vec![databricks()],
        }))
    }

    fn read(file: &File) -> Configured {
        file.0.lock().expect("never poisoned").clone()
    }

    /// The literal reads back as the card it printed, and so does its text form.
    fn round_trips(card: &Configure) {
        assert_eq!(Configure::from_value(&card.to_value()).as_ref(), Ok(card));
        let printed = serde_json::to_string(card).expect("prints");
        assert_eq!(
            serde_json::from_str::<Configure>(&printed).expect("reads back"),
            *card
        );
    }

    #[test]
    fn an_order_card_writes_its_order_and_says_from_and_to() {
        let file = file();
        let card = Configure::order(
            &read(&file),
            Job::Secretary,
            &["anthropic/claude-b", "databricks/served"],
        )
        .expect("a card");
        round_trips(&card);
        assert_eq!(
            card.sentence(),
            "change Chat: nothing chosen, so the fallback answers → anthropic/claude-b, then \
             databricks/served"
        );
        assert_eq!(card.perform(&file), Ok(Performed::Written));
        assert_eq!(
            read(&file)
                .models
                .order(Job::Secretary)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["anthropic/claude-b", "databricks/served"]
        );
        // Tapped again, or a second card for the same change: done, and nothing written.
        assert_eq!(card.perform(&file), Ok(Performed::Already));
    }

    #[test]
    fn no_card_names_an_entry_no_parser_accepted() {
        let now = read(&file());
        for bad in ["openrouter/m", "anthropic/", "no-slash", "anthropic/a b"] {
            assert!(
                matches!(
                    Configure::order(&now, Job::Review, &[bad]),
                    Err(Refused::Entry(_))
                ),
                "{bad:?} is refused"
            );
        }
        for stored in ["anthropic/a b", "/m", "no-slash"] {
            let value = record(
                CONFIGURE_ORDER,
                vec![
                    Value::str("review"),
                    Value::Tuple(Vec::new()),
                    Value::Tuple(vec![Value::str(stored)]),
                ],
            );
            assert!(Configure::from_value(&value).is_err(), "{stored:?}");
        }
    }

    #[test]
    fn a_card_that_changes_nothing_is_never_filed() {
        let now = read(&file());
        assert_eq!(
            Configure::order(&now, Job::Title, &[]),
            Err(Refused::Unmoved)
        );
        assert_eq!(
            Configure::reserve(&now, Reserve::default()),
            Err(Refused::Unmoved)
        );
        assert_eq!(
            Configure::remove(&now, "phala"),
            Err(Refused::Undeclared {
                provider: "Phala".to_owned()
            })
        );
        let stored = record(
            CONFIGURE_RESERVE,
            vec![Reserve::default().to_value(), Reserve::default().to_value()],
        );
        assert!(Configure::from_value(&stored).is_err());
    }

    /// Two cards read the same setting; the first tap moves it, and the second, which still
    /// reads the old value, writes nothing and says what the file holds now.
    #[test]
    fn a_card_whose_setting_moved_since_it_was_filed_writes_nothing() {
        let file = file();
        let now = read(&file);
        let first = Configure::reserve(&now, Reserve::parse(0.2).expect("a share")).expect("one");
        let second = Configure::reserve(&now, Reserve::parse(0.5).expect("a share")).expect("two");
        assert_eq!(first.perform(&file), Ok(Performed::Written));
        assert_eq!(
            second.perform(&file),
            Err(Refused::Moved {
                now: "keeps 20% for you".to_owned()
            })
        );
        assert_eq!(
            read(&file).models.reserve(),
            Reserve::parse(0.2).expect("a share")
        );
    }

    #[test]
    fn an_order_whose_provider_went_since_the_card_was_filed_writes_nothing() {
        let file = file();
        let card =
            Configure::order(&read(&file), Job::Review, &["databricks/served"]).expect("a card");
        let removal = Configure::remove(&read(&file), "databricks").expect("a removal");
        assert_eq!(removal.perform(&file), Ok(Performed::Written));
        assert!(matches!(
            card.perform(&file),
            Err(Refused::Entry(ModelsError::Undeclared(_)))
        ));
        assert!(read(&file).models.order(Job::Review).is_empty());
    }

    #[test]
    fn a_provider_a_job_orders_is_not_removed() {
        let file = file();
        Configure::order(&read(&file), Job::Review, &["databricks/served"])
            .expect("a card")
            .perform(&file)
            .expect("written");
        let removal = Configure::remove(&read(&file), "databricks").expect("a removal");
        round_trips(&removal);
        assert_eq!(
            removal.perform(&file),
            Err(Refused::Ordered {
                provider: "Databricks".to_owned(),
                jobs: vec!["Review".to_owned()],
            })
        );
        assert_eq!(read(&file).providers, [databricks()]);
    }

    #[test]
    fn a_provider_is_declared_only_as_its_preset_derives_it() {
        let file = file();
        let card = Configure::declare(&read(&file), ProviderSetup::OpenRouter).expect("a card");
        round_trips(&card);
        assert_eq!(card.setting(), Setting::Provider("openrouter".to_owned()));
        assert_eq!(
            card.sentence(),
            "add OpenRouter: answers at https://openrouter.ai/api/v1, with the OpenRouter key"
        );
        assert_eq!(card.perform(&file), Ok(Performed::Written));
        assert_eq!(
            read(&file).provider("openrouter"),
            Some(&ProviderSetup::OpenRouter.declared())
        );
        // A provider's `to` is a preset's: a hand-typed endpoint has no literal to be read from.
        let typed = record(
            CONFIGURE_PROVIDER,
            vec![
                Value::None,
                record(
                    PROVIDER_SETUP,
                    vec![
                        Value::str("databricks"),
                        Value::str("https://evil.example"),
                        Value::None,
                    ],
                ),
            ],
        );
        assert!(Configure::from_value(&typed).is_err());
    }

    /// Phala's receipts are checked against the source commit its table names, so a card moving
    /// only that commit moves the trust anchor, and says so: From and To differ, each whole.
    #[test]
    fn a_card_moving_only_the_attested_commit_says_both_commits() {
        let file = file();
        file.0
            .lock()
            .expect("never poisoned")
            .providers
            .push(phala(&"a".repeat(40)).declared());
        let card = Configure::declare(&read(&file), phala(&"b".repeat(40))).expect("a card");
        round_trips(&card);
        let (from, to) = card.words();
        assert_ne!(from, to);
        assert!(
            from.ends_with(&format!("source commit {}", "a".repeat(40))),
            "{from}"
        );
        assert!(
            to.ends_with(&format!("source commit {}", "b".repeat(40))),
            "{to}"
        );
        assert!(
            !from.contains("PHALA_API_KEY"),
            "a key's stored name stays on the box"
        );
    }

    fn reserve() -> impl Strategy<Value = Reserve> {
        (0u32..=100)
            .prop_map(|percent| Reserve::parse(f64::from(percent) / 100.0).expect("a share"))
    }

    fn commit() -> impl Strategy<Value = String> {
        "[0-9a-f]{40}"
    }

    /// What a card moves a declared commit to: one hex digit changed, or another commit, or
    /// nothing (removed).
    fn moved_commit() -> impl Strategy<Value = (String, Option<String>)> {
        prop_oneof![
            (commit(), 0usize..40, "[0-9a-f]").prop_map(|(from, at, digit)| {
                let to = format!("{}{digit}{}", &from[..at], &from[at + 1..]);
                (from, Some(to))
            }),
            (commit(), commit()).prop_map(|(from, to)| (from, Some(to))),
            commit().prop_map(|from| (from, None)),
        ]
    }

    fn any_reserve() -> impl Strategy<Value = Reserve> {
        prop_oneof![
            (0u32..=100).prop_map(|percent| f64::from(percent) / 100.0),
            0.0..=1.0f64,
        ]
        .prop_map(|share| Reserve::parse(share).expect("a share"))
    }

    proptest! {
        /// Whatever the file holds, a card performs at most one write, and the file then holds
        /// either the card's `to` or what it held before.
        #[test]
        fn a_tap_writes_to_or_nothing(held in reserve(), from in reserve(), to in reserve()) {
            prop_assume!(from != to);
            let file = file();
            file.0.lock().expect("never poisoned").models = Models::default().with_reserve(held);
            let card = Configure(Change::Reserve(Move::new(from, to).expect("a change")));
            round_trips(&card);
            let after = card.perform(&file).map(|_| read(&file).models.reserve());
            if held == from || held == to {
                prop_assert_eq!(after, Ok(to));
            } else {
                prop_assert!(after.is_err());
                prop_assert_eq!(read(&file).models.reserve(), held);
            }
        }

        /// Two providers that differ never read the same: whatever moves, a single digit of the
        /// commit a receipt must name included, From and To say it.
        #[test]
        fn a_provider_card_never_reads_the_same_on_both_sides((from, to) in moved_commit()) {
            let now = Configured {
                models: Models::default(),
                providers: vec![phala(&from).declared()],
            };
            let made = match to {
                Some(commit) => Configure::declare(&now, phala(&commit)),
                None => Configure::remove(&now, "phala"),
            };
            match made {
                Ok(card) => {
                    let (from, to) = card.words();
                    prop_assert_ne!(from, to);
                }
                Err(refused) => prop_assert_eq!(refused, Refused::Unmoved),
            }
        }

        /// Two reserves that differ never read the same, and a whole percent reads as one.
        #[test]
        fn two_reserves_never_read_the_same(a in any_reserve(), b in any_reserve()) {
            prop_assert_eq!(a == b, reserve_words(a) == reserve_words(b));
            if let Some(percent) = whole_percent(a) {
                prop_assert_eq!(reserve_words(a), format!("keeps {percent}% for you"));
            }
        }
    }
}
