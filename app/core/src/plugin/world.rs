//! The worlds of `app/wit/plugin.wit` this app can host, and what each one imports.

use std::fmt;

use super::ManifestError;

/// The version of the `suzatary:plugin` package the host was built against.
pub const PACKAGE_VERSION: &str = "0.3.0";

/// A world a component may target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum World {
    /// Reads the outside world from a cursor: monotone, so it needs no lock.
    Source,
    /// Proposes acts for the owner to tap, from what its store holds: it fetches nothing and
    /// performs nothing.
    Actor,
}

impl World {
    /// Parse `<world>@<version>`; the version must be [`PACKAGE_VERSION`].
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let (world, version) = text
            .split_once('@')
            .ok_or_else(|| ManifestError::UnknownWorld(text.to_owned()))?;
        let world = match world {
            "source" => World::Source,
            "actor" => World::Actor,
            _ => return Err(ManifestError::UnknownWorld(text.to_owned())),
        };
        if version == PACKAGE_VERSION {
            Ok(world)
        } else {
            Err(ManifestError::WorldVersion {
                asked: version.to_owned(),
                have: PACKAGE_VERSION,
            })
        }
    }

    /// The instances a component of this world may import, by their component-model names.
    /// Anything else it imports is a power the world does not give, and it is not loaded.
    #[must_use]
    pub const fn imports(self) -> &'static [&'static str] {
        match self {
            World::Source => &[
                "suzatary:plugin/fetch@0.3.0",
                "suzatary:plugin/clock@0.3.0",
                "suzatary:plugin/log@0.3.0",
            ],
            World::Actor => &["suzatary:plugin/clock@0.3.0", "suzatary:plugin/log@0.3.0"],
        }
    }

    /// Whether its plugins reach the network at all, so a manifest may name origins and
    /// credentials for it.
    #[must_use]
    pub const fn fetches(self) -> bool {
        match self {
            World::Source => true,
            World::Actor => false,
        }
    }

    /// The instance a component of this world must export.
    #[must_use]
    pub const fn export(self) -> &'static str {
        match self {
            World::Source => "suzatary:plugin/sync@0.3.0",
            World::Actor => "suzatary:plugin/decide@0.3.0",
        }
    }
}

impl fmt::Display for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            World::Source => write!(f, "source@{PACKAGE_VERSION}"),
            World::Actor => write!(f, "actor@{PACKAGE_VERSION}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn another_version_or_world_is_refused_by_name() {
        assert_eq!(
            World::parse("source@0.1.0"),
            Err(ManifestError::WorldVersion {
                asked: "0.1.0".to_owned(),
                have: PACKAGE_VERSION,
            })
        );
        assert!(matches!(
            World::parse("performer@0.2.0"),
            Err(ManifestError::UnknownWorld(_))
        ));
        assert!(matches!(
            World::parse("source"),
            Err(ManifestError::UnknownWorld(_))
        ));
    }

    /// An actor's one outward power is to propose: it imports no fetch.
    #[test]
    fn an_actor_imports_nothing_that_reaches_outward() {
        assert!(
            !World::Actor
                .imports()
                .iter()
                .any(|name| name.contains("fetch"))
        );
        assert!(!World::Actor.fetches());
    }
}
