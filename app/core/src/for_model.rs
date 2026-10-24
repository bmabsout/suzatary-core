//! Text on its way to a model. Every model client takes [`ForModel`] and nothing else, and
//! [`ForModel::of`], which writes every held number as a placeholder (`held_value::withheld`), is
//! its one constructor, so no path can send a model text the masker did not read.

use crate::held_value::withheld;

/// Text a model may read: every held number in it a placeholder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForModel(String);

impl ForModel {
    /// `text`, masked.
    #[must_use]
    pub fn of(text: &str) -> Self {
        Self(withheld(text))
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The text, owned.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::ops::Deref for ForModel {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ForModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
