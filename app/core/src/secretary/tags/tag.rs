//! A topic's name, in one spelling: words of lowercase letters and digits in any script, joined
//! by single hyphens, at most [`MAX`] characters, in NFC. So `Travel`, `travel ` and `travel`
//! cannot be three tags.

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization as _;

pub const MAX: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Tag(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotATag {
    #[error("a tag is 1 to {MAX} characters, got {0}")]
    Length(usize),
    #[error("a tag is written in Unicode's composed form (NFC): {0:?}")]
    NotComposed(String),
    #[error("a tag is lowercase letters and digits, with single hyphens between words: {0:?}")]
    Spelling(String),
}

impl Tag {
    pub fn new(text: impl Into<String>) -> Result<Self, NotATag> {
        let text = text.into();
        let length = text.chars().count();
        if !(1..=MAX).contains(&length) {
            return Err(NotATag::Length(length));
        }
        if !unicode_normalization::is_nfc(&text) {
            return Err(NotATag::NotComposed(text));
        }
        if text
            .split('-')
            .all(|word| !word.is_empty() && word.chars().all(letter))
        {
            Ok(Self(text))
        } else {
            Err(NotATag::Spelling(text))
        }
    }

    /// Every run of characters that are not letters becomes one hyphen; `None` when `text` has
    /// no letter or digit. A tag's own text slugs to itself.
    #[must_use]
    pub fn slug(text: &str) -> Option<Self> {
        let lowered: String = text
            .nfc()
            .collect::<String>()
            .to_lowercase()
            .nfc()
            .collect();
        let words: Vec<String> = lowered
            .split(|c: char| !letter(c))
            .filter(|word| !word.is_empty())
            .map(str::to_owned)
            .collect();
        let mut out = String::new();
        for word in words {
            let room = MAX - out.chars().count();
            let needed = word.chars().count() + usize::from(!out.is_empty());
            if needed > room {
                if out.is_empty() {
                    out = word.chars().take(MAX).collect();
                }
                break;
            }
            if !out.is_empty() {
                out.push('-');
            }
            out.push_str(&word);
        }
        Self::new(out).ok()
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A letter or a digit that is its own lowercase: `A` is not, `a`, `ß`, `٣` and `ع` are.
fn letter(c: char) -> bool {
    c.is_alphanumeric() && c.to_lowercase().eq(std::iter::once(c))
}

impl TryFrom<String> for Tag {
    type Error = NotATag;

    fn try_from(text: String) -> Result<Self, NotATag> {
        Self::new(text)
    }
}

impl From<Tag> for String {
    fn from(tag: Tag) -> Self {
        tag.0
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

crate::literal::text_literal!(
    Tag => |tag| tag.as_str().to_owned(),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_has_one_spelling() {
        for good in ["travel", "trip-abroad", "2026", "straße", "سفر", "日本"] {
            assert_eq!(Tag::new(good).map(String::from), Ok(good.to_owned()));
        }
        for bad in [
            "Travel", "travel ", " travel", "-travel", "travel-", "trip--to", "trip to", "a/b",
            "a_b", "İ",
        ] {
            assert!(matches!(Tag::new(bad), Err(NotATag::Spelling(_))), "{bad}");
        }
        assert_eq!(Tag::new(""), Err(NotATag::Length(0)));
        assert_eq!(Tag::new("a".repeat(MAX + 1)), Err(NotATag::Length(MAX + 1)));
        assert!(Tag::new("a".repeat(MAX)).is_ok());
        // `e` and a combining acute accent read as `é`, which NFC writes as one character.
        assert!(matches!(
            Tag::new("cafe\u{301}"),
            Err(NotATag::NotComposed(_))
        ));
    }
}
