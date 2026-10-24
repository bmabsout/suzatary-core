//! A topic's name, in one spelling: one to [`MAX_WORDS`] words of lowercase letters and digits in
//! any script, joined by single hyphens, at most [`MAX_CHARACTERS`] characters, in NFC. So
//! `Travel`, `travel ` and `travel` cannot be three topics, and a long name cannot be one.

use serde::{Deserialize, Serialize};

pub const MAX_WORDS: usize = 3;
pub const MAX_CHARACTERS: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Topic(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotATopic {
    #[error("a topic is written in Unicode's composed form (NFC): {0:?}")]
    NotComposed(String),
    #[error("a topic is lowercase letters and digits, with single hyphens between words: {0:?}")]
    Spelling(String),
    #[error("a topic is at most {MAX_WORDS} words, and {0:?} has {1}")]
    Words(String, usize),
    #[error("a topic is at most {MAX_CHARACTERS} characters, and {0:?} has {1}")]
    Characters(String, usize),
}

impl Topic {
    pub fn new(text: impl Into<String>) -> Result<Self, NotATopic> {
        let text = text.into();
        if !unicode_normalization::is_nfc(&text) {
            return Err(NotATopic::NotComposed(text));
        }
        if !text
            .split('-')
            .all(|word| !word.is_empty() && word.chars().all(letter))
        {
            return Err(NotATopic::Spelling(text));
        }
        let words = text.split('-').count();
        if words > MAX_WORDS {
            return Err(NotATopic::Words(text, words));
        }
        let characters = text.chars().count();
        if characters > MAX_CHARACTERS {
            return Err(NotATopic::Characters(text, characters));
        }
        Ok(Self(text))
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

impl TryFrom<String> for Topic {
    type Error = NotATopic;

    fn try_from(text: String) -> Result<Self, NotATopic> {
        Self::new(text)
    }
}

impl From<Topic> for String {
    fn from(topic: Topic) -> Self {
        topic.0
    }
}

impl std::fmt::Display for Topic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

crate::literal::text_literal!(
    Topic => |topic| topic.as_str().to_owned(),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_topic_has_one_spelling_and_is_short() {
        for good in [
            "travel",
            "trip-abroad",
            "taxes-2026-filing",
            "2026",
            "straße",
            "سفر",
            "日本",
        ] {
            assert_eq!(Topic::new(good).map(String::from), Ok(good.to_owned()));
        }
        for bad in [
            "", "Travel", "travel ", " travel", "-travel", "travel-", "trip--to", "trip to", "a/b",
            "a_b", "İ",
        ] {
            assert!(
                matches!(Topic::new(bad), Err(NotATopic::Spelling(_))),
                "{bad}"
            );
        }
        assert_eq!(
            Topic::new("a-b-c-d"),
            Err(NotATopic::Words("a-b-c-d".to_owned(), 4))
        );
        let long = "a".repeat(MAX_CHARACTERS + 1);
        assert_eq!(
            Topic::new(long.clone()),
            Err(NotATopic::Characters(long, MAX_CHARACTERS + 1))
        );
        assert!(Topic::new("a".repeat(MAX_CHARACTERS)).is_ok());
        // `e` and a combining acute accent read as `é`, which NFC writes as one character.
        assert!(matches!(
            Topic::new("cafe\u{301}"),
            Err(NotATopic::NotComposed(_))
        ));
    }
}
