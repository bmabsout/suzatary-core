//! Where a plugin's component lives, relative to the root the binary resolves it against.

use std::fmt;

use super::ManifestError;

/// A relative path of plain segments ending in `.wasm`: no `..`, no absolute path, no empty
/// segment, so a manifest cannot point outside the root it is resolved against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentPath(String);

impl ComponentPath {
    /// Parse one.
    ///
    /// # Errors
    /// [`ManifestError::BadComponent`] for any other path.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        if bundled(text, "wasm") {
            Ok(Self(text.to_owned()))
        } else {
            Err(ManifestError::BadComponent(text.to_owned()))
        }
    }

    /// The path, `/`-separated.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether `text` is a relative path of plain segments ending in `.<extension>`: a file of a
/// plugin's bundle, which cannot name anything outside the root it is resolved against.
pub(super) fn bundled(text: &str, extension: &str) -> bool {
    let plain = |segment: &str| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    text.rsplit_once('.')
        .is_some_and(|(_, found)| found == extension)
        && text.split('/').all(plain)
}

impl fmt::Display for ComponentPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_component_stays_under_its_root() {
        assert!(ComponentPath::parse("plugins/ics.wasm").is_ok());
        assert!(ComponentPath::parse("ics.wasm").is_ok());
        for bad in [
            "/etc/ics.wasm",
            "../ics.wasm",
            "plugins/../../ics.wasm",
            "plugins//ics.wasm",
            "./ics.wasm",
            "plugins/ics.so",
            "plugins\\ics.wasm",
            "",
        ] {
            assert!(ComponentPath::parse(bad).is_err(), "{bad:?}");
        }
    }
}
