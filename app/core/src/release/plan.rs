//! Which todos are releases: the declaration `app/core/releases.toml` holds.

use std::collections::BTreeSet;

use prodrome_core::event::TodoId;
use serde::Deserialize;

/// The releases the version counts toward, in the order they ship, on one chain.
///
/// Only [`Plan::new`] makes one, so every plan has distinct todos and majors that never go down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    chain: String,
    base: u32,
    releases: Vec<Release>,
}

/// One release: the todo whose spec composes its items, and the major its completion moves the
/// version to (`2` for 2.0, and for 2.1, whose completion moves no major).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The release's own todo.
    pub todo: TodoId,
    /// The version's major once it is completed.
    pub major: u32,
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
    /// A release's major is below the one before it, or not above the base.
    #[error("releases.toml: {todo} moves to major {major}, below {floor}")]
    Backwards {
        /// The release.
        todo: String,
        /// Its major.
        major: u32,
        /// What it may not go below.
        floor: u32,
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
}

/// The declaration this build carries.
const DECLARED: &str = include_str!("../../releases.toml");

impl Plan {
    /// The plan of `releases` on `chain`, the version's major being `base` until one completes.
    ///
    /// Every release moves past `base`, so while the major is `base` no release has completed
    /// and every release from the first counts toward the next; that is what keeps
    /// [`super::Reading`] monotone.
    ///
    /// # Errors
    /// A todo twice, or a major below the one before it or not above `base`.
    pub fn new(
        chain: impl Into<String>,
        base: u32,
        releases: Vec<Release>,
    ) -> Result<Plan, PlanError> {
        let mut seen = BTreeSet::new();
        let mut floor = base.saturating_add(1);
        for release in &releases {
            if !seen.insert(release.todo.as_str()) {
                return Err(PlanError::Twice(release.todo.as_str().to_owned()));
            }
            if release.major < floor {
                return Err(PlanError::Backwards {
                    todo: release.todo.as_str().to_owned(),
                    major: release.major,
                    floor,
                });
            }
            floor = release.major;
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
                        major: r.major,
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

    /// The major before any release completes.
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
