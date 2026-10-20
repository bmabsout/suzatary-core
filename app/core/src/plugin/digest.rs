//! A file's sha256, as a manifest pins it and the host checks it.

use std::fmt;

use sha2::{Digest as _, Sha256};

use super::ManifestError;

/// The sha256 of a file's bytes, 64 lowercase hex digits: what the owner approved, so a file
/// whose bytes hash otherwise is not the file approved.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(String);

impl Digest {
    /// Parse one as a manifest spells it.
    ///
    /// # Errors
    /// [`ManifestError::BadDigest`] for anything but 64 lowercase hex digits.
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        if text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            Ok(Self(text.to_owned()))
        } else {
            Err(ManifestError::BadDigest(text.chars().take(80).collect()))
        }
    }

    /// The digest of `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(hex::encode(Sha256::digest(bytes)))
    }

    /// The hex digits.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_digest_is_its_bytes_sha256_and_nothing_else_parses() {
        let empty = Digest::of(b"");
        assert_eq!(
            empty.as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(Digest::parse(empty.as_str()), Ok(empty.clone()));
        for bad in [
            "",
            "E3B0",
            &empty.as_str().to_uppercase(),
            &format!("{empty}0"),
        ] {
            assert!(Digest::parse(bad).is_err(), "{bad:?}");
        }
    }
}
