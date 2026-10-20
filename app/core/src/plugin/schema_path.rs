//! Where a plugin's schema lives: its declaration, shipped beside its component.

use std::fmt;

use super::ManifestError;
use super::component::bundled;

/// A relative path of plain segments ending in `.schema`, resolved against the same root as the
/// component: the text of the plugin's declared schema, which the host admits before it loads the
/// component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaPath(String);

impl SchemaPath {
    /// Parse one.
    ///
    /// # Errors
    /// [`ManifestError::BadSchema`] for any other path.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        if bundled(text, "schema") {
            Ok(Self(text.to_owned()))
        } else {
            Err(ManifestError::BadSchema(text.to_owned()))
        }
    }

    /// The path, `/`-separated.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SchemaPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_schema_stays_under_its_root_beside_its_component() {
        assert!(SchemaPath::parse("ics.schema").is_ok());
        assert!(SchemaPath::parse("plugins/ics.schema").is_ok());
        for bad in [
            "../ics.schema",
            "/ics.schema",
            "ics.wasm",
            "ics.schema.txt",
            "",
        ] {
            assert!(SchemaPath::parse(bad).is_err(), "{bad:?}");
        }
    }
}
