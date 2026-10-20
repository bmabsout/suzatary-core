//! A message a reference names that this box no longer keeps: its mail was deleted with its
//! account, or never synced here. Whatever cites it stays, and reads it in these words.

/// What every reading of a dangling mail reference says, never an error of the reading's.
pub const SAID: &str = "this email is no longer stored";

/// A message no longer kept, with what a todo citing it recorded of it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gone<'a> {
    /// Its subject, as the citing todo's source recorded it.
    pub subject: Option<&'a str>,
    /// Its sender, likewise.
    pub sender: Option<&'a str>,
}

impl Gone<'_> {
    /// The sentence a page shows in its place.
    #[must_use]
    pub fn said(&self) -> String {
        match (known(self.subject), known(self.sender)) {
            (None, None) => SAID.to_owned(),
            (Some(subject), None) => format!("{SAID} (“{subject}”)"),
            (None, Some(sender)) => format!("{SAID} (from {sender})"),
            (Some(subject), Some(sender)) => format!("{SAID} (“{subject}” from {sender})"),
        }
    }
}

/// `text`, trimmed, when it says anything.
fn known(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gone_message_says_what_its_citation_recorded() {
        assert_eq!(Gone::default().said(), SAID);
        assert_eq!(
            Gone {
                subject: Some("The lease"),
                sender: Some("landlord@example.test"),
            }
            .said(),
            "this email is no longer stored (“The lease” from landlord@example.test)"
        );
        assert_eq!(
            Gone {
                subject: Some(" "),
                sender: None,
            }
            .said(),
            SAID
        );
    }
}
