//! The maildir leaves an account's sync counts as triage input: at least one, each named.
//! An account with none would read nothing, so a list that is empty is not one.

use serde::{Deserialize, Serialize};

use crate::mail::account::RosterError;

/// One or more folder names, none of them blank, in the order the instance file gave them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct Folders(Vec<String>);

impl Folders {
    /// Check a list of folder names.
    ///
    /// # Errors
    /// [`RosterError::NoFolders`] for an empty list and [`RosterError::BlankFolder`] for a
    /// blank name.
    pub fn new(folders: Vec<String>) -> Result<Self, RosterError> {
        if folders.is_empty() {
            return Err(RosterError::NoFolders);
        }
        if folders.iter().any(|folder| folder.trim().is_empty()) {
            return Err(RosterError::BlankFolder);
        }
        Ok(Self(folders))
    }

    /// `Inbox` alone, what a new account counts.
    #[must_use]
    pub fn inbox() -> Self {
        Self(vec!["Inbox".to_owned()])
    }

    /// The names, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

/// Printed as its list, as `doctor` and the instance's golden reading show it.
impl std::fmt::Debug for Folders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl TryFrom<Vec<String>> for Folders {
    type Error = RosterError;
    fn try_from(folders: Vec<String>) -> Result<Self, RosterError> {
        Self::new(folders)
    }
}

impl From<Folders> for Vec<String> {
    fn from(folders: Folders) -> Self {
        folders.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_list_or_a_blank_name_is_refused() {
        assert_eq!(Folders::new(Vec::new()), Err(RosterError::NoFolders));
        assert_eq!(
            Folders::new(vec!["Inbox".to_owned(), "  ".to_owned()]),
            Err(RosterError::BlankFolder)
        );
        let folders = Folders::new(vec!["Inbox".to_owned()]).expect("one folder");
        assert_eq!(folders.as_slice(), ["Inbox".to_owned()]);
        assert!(serde_json::from_str::<Folders>("[]").is_err());
    }
}
