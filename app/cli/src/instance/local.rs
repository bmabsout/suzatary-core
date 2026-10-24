//! The box-local tier.

use std::path::{Path, PathBuf};

use anyhow::Context;
use suzatary_io::present::Present as _;

/// The box-local tier, `<data>/local`: files no push carries (the tier ignores itself, and
/// `safe-push.sh` stages none of it) and the vault does. Its place and each file have one name
/// here, so the box, the scripts and the guards cannot disagree about them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local(pub(super) PathBuf);

impl Local {
    /// The tier's directory, under the data root.
    pub const DIR: &str = "local";

    /// Every file the tier holds, by name.
    pub const FILES: [&str; 6] = [
        "persona.toml",
        "personal-terms.txt",
        "context-terms.txt",
        "mail.json",
        "triage-notes.md",
        "watches",
    ];

    /// The directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// The person's prose, [`suzatary_core::persona::Prose`], as TOML.
    #[must_use]
    pub fn persona(&self) -> PathBuf {
        self.0.join(Self::FILES[0])
    }

    /// The names the leak guard looks for (`scripts/public-tree.sh`), one per line.
    #[must_use]
    pub fn personal_terms(&self) -> PathBuf {
        self.0.join(Self::FILES[1])
    }

    /// The words of the owner's context the leak guard also looks for.
    #[must_use]
    pub fn context_terms(&self) -> PathBuf {
        self.0.join(Self::FILES[2])
    }

    /// What the box keeps of its mail beside the pushed decisions
    /// ([`suzatary_core::mail::local_state`]).
    #[must_use]
    pub fn mail(&self) -> PathBuf {
        self.0.join(Self::FILES[3])
    }

    /// The watches on mail the owner expects ([`suzatary_core::mail::watch`]), a directory.
    #[must_use]
    pub fn watches(&self) -> PathBuf {
        self.0.join(Self::FILES[5])
    }

    /// The prose, or none when the file is absent (a deployment that wrote none).
    ///
    /// # Errors
    /// A file that is there and does not read as prose.
    pub fn prose(&self) -> anyhow::Result<suzatary_core::persona::Prose> {
        let path = self.persona();
        let reading = || format!("reading {}", path.display());
        match std::fs::read_to_string(&path)
            .present()
            .with_context(reading)?
        {
            Some(text) => toml::from_str(&text).with_context(reading),
            None => Ok(suzatary_core::persona::Prose::default()),
        }
    }
}
