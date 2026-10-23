//! Which todos are releases: the declaration `app/core/releases.toml` holds.

use std::collections::BTreeSet;

use prodrome_core::event::TodoId;
use serde::Deserialize;

use super::Counted;

/// The releases the version counts toward, in the order they ship, on one chain.
///
/// Only [`Plan::new`] makes one, so every plan has distinct todos whose versions strictly
/// increase, the first above `base.0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    chain: String,
    base: u32,
    releases: Vec<Release>,
}

/// One release: the todo whose spec composes its items, and the `major.minor` its completion
/// moves the version to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The release's own todo.
    pub todo: TodoId,
    /// The version's `major.minor` once it is completed.
    pub version: Counted,
}

/// Why a declaration is not a plan.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// Not the file's shape.
    #[error("releases.toml: {0}")]
    Unread(String),
    /// A release's todo is not a todo id.
    #[error("releases.toml: {0:?} is not a todo id")]
    NotATodo(String),
    /// Two releases name one todo.
    #[error("releases.toml: {0} is declared twice")]
    Twice(String),
    /// A release's version is not above the one before it, or the first not above `base.0`.
    #[error("releases.toml: {todo} is {version}, not above {floor}")]
    NotAbove {
        /// The release.
        todo: String,
        /// Its version.
        version: Counted,
        /// What it must be above.
        floor: Counted,
    },
}

/// The file as written, before [`Plan::new`] reads it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declared {
    chain: String,
    base: u32,
    #[serde(default)]
    release: Vec<DeclaredRelease>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclaredRelease {
    todo: String,
    major: u32,
    minor: u32,
}

/// The declaration this build carries.
const DECLARED: &str = include_str!("../../releases.toml");

impl Plan {
    /// The plan of `releases` on `chain`, the version being `base.0` until one completes.
    ///
    /// The versions strictly increase, so the last completed release in the plan's order is
    /// also the greatest, and completing any release never lowers what the chain reads.
    ///
    /// # Errors
    /// A todo twice, or a version not above the one before it (`base.0` for the first).
    pub fn new(
        chain: impl Into<String>,
        base: u32,
        releases: Vec<Release>,
    ) -> Result<Plan, PlanError> {
        let mut seen = BTreeSet::new();
        let mut floor = Counted {
            major: base,
            minor: 0,
        };
        for release in &releases {
            if !seen.insert(release.todo.as_str()) {
                return Err(PlanError::Twice(release.todo.as_str().to_owned()));
            }
            if release.version <= floor {
                return Err(PlanError::NotAbove {
                    todo: release.todo.as_str().to_owned(),
                    version: release.version,
                    floor,
                });
            }
            floor = release.version;
        }
        Ok(Plan {
            chain: chain.into(),
            base,
            releases,
        })
    }

    /// The plan `text` declares, in `releases.toml`'s shape.
    ///
    /// # Errors
    /// Not that shape, or not a plan.
    pub fn parse(text: &str) -> Result<Plan, PlanError> {
        let declared: Declared =
            toml::from_str(text).map_err(|e| PlanError::Unread(e.message().to_owned()))?;
        let releases = declared
            .release
            .into_iter()
            .map(|r| {
                TodoId::new(r.todo.clone())
                    .map(|todo| Release {
                        todo,
                        version: Counted {
                            major: r.major,
                            minor: r.minor,
                        },
                    })
                    .map_err(|_| PlanError::NotATodo(r.todo))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Plan::new(declared.chain, declared.base, releases)
    }

    /// The plan this build was compiled with (`app/core/releases.toml`).
    ///
    /// # Errors
    /// Never for a build whose tests passed: `the_declared_plan_reads` pins it.
    pub fn declared() -> Result<Plan, PlanError> {
        Plan::parse(DECLARED)
    }

    /// The chain the releases are todos of.
    #[must_use]
    pub fn chain(&self) -> &str {
        &self.chain
    }

    /// The major before any release completes, whose minor is 0.
    #[must_use]
    pub fn base(&self) -> u32 {
        self.base
    }

    /// The releases, in the order they ship.
    #[must_use]
    pub fn releases(&self) -> &[Release] {
        &self.releases
    }
}
