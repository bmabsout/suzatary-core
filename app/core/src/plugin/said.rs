//! What a plugin said, as the host keeps it: its words, bounded.

use std::fmt::{self, Write as _};

/// The most bytes a [`Said`] holds, its marker included.
pub const MAX_SAID: usize = 512;

/// The marker's longest form, `… (<usize::MAX> bytes cut)`, held back from the kept prefix so the
/// whole fits [`MAX_SAID`].
const MARKER: usize = "… ( bytes cut)".len() + 20;

/// Text a plugin put before the host (an error, a refusal's subject, a log line), at most
/// [`MAX_SAID`] bytes.
///
/// [`Said::clip`] is the one way to make one, so whatever the host keeps or prints of a plugin's
/// words is bounded by type, never by a caller remembering to cut it. Text that fits is kept
/// whole; text that does not is its longest prefix ending on a character that leaves room for
/// `… (N bytes cut)`, so the reader sees that it was cut and by how much.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Said(String);

impl Said {
    /// `text`, whole when it fits and cut when it does not.
    #[must_use]
    pub fn clip(text: impl Into<String>) -> Self {
        let mut text = text.into();
        if text.len() > MAX_SAID {
            let kept = text.floor_char_boundary(MAX_SAID - MARKER);
            let cut = text.len() - kept;
            text.truncate(kept);
            // Writing to a `String` cannot fail.
            let _ = write!(text, "… ({cut} bytes cut)");
        }
        Self(text)
    }

    /// The words as kept.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Said {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// As the text's own: a refusal quoting a plugin's words with `{:?}` escapes them as a string.
impl fmt::Debug for Said {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn a_megabyte_is_cut_to_the_bound_and_says_by_how_much() {
        let said = Said::clip("é".repeat(1 << 19));
        assert!(said.as_str().len() <= MAX_SAID);
        let kept = said.as_str().split('…').next().expect("a prefix").len();
        assert_eq!(
            said.as_str(),
            format!("{}… ({} bytes cut)", "é".repeat(kept / 2), (1 << 20) - kept)
        );
    }

    proptest! {
        /// Whatever a plugin says, what is kept fits the bound.
        #[test]
        fn what_is_kept_is_bounded(text in "\\PC{0,700}") {
            prop_assert!(Said::clip(text).as_str().len() <= MAX_SAID);
        }

        /// Text that fits is kept whole, so clipping is the identity on bounded text and
        /// clipping twice is clipping once.
        #[test]
        fn clipping_is_a_projection(text in "\\PC{0,700}") {
            let once = Said::clip(text.clone());
            if text.len() <= MAX_SAID {
                prop_assert_eq!(once.as_str(), text.as_str());
            }
            prop_assert_eq!(Said::clip(once.as_str()), once);
        }

        /// Cut text is a prefix of what was said, ending on a character, and the marker counts
        /// exactly the bytes that are not kept.
        #[test]
        fn a_cut_keeps_a_prefix_and_counts_the_rest(text in "\\PC{120,700}") {
            let said = Said::clip(text.clone());
            if text.len() > MAX_SAID {
                let (kept, marker) = said.as_str().rsplit_once("… (").expect("a marker");
                prop_assert!(text.starts_with(kept));
                prop_assert_eq!(marker, format!("{} bytes cut)", text.len() - kept.len()));
            } else {
                prop_assert_eq!(said.as_str(), text.as_str());
            }
        }
    }
}
