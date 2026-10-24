//! What reading one file for the model answers: the file itself, or its text.
//!
//! No held number the box can find reaches a model: a PDF whose text layer names one
//! ([`crate::redact`]) is withheld and answered by that text, redacted. [`FileRead::of`] is the only
//! way to a [`Readable`], so a file that skipped the check does not typecheck.
//!
//! A scan's pixels cannot be redacted: a PDF with no text layer, and an image, are handed over
//! as they are, by the owner's choice.

use crate::enclosed::Enclosure;
use crate::redact::{Redacted, redact};

/// The largest PDF handed to a model, in bytes, under the request size its API takes.
pub const MAX_PDF: usize = 20 * 1024 * 1024;

/// The most pages of a PDF handed to a model, as its API takes them.
pub const MAX_PAGES: usize = 100;

/// The most characters of text a read answers, with the cut declared.
pub const MAX_CHARS: usize = crate::mail::MAX_ATTACHMENT_CHARS;

/// What a read answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRead {
    /// The file itself, for the model to read.
    File(Readable),
    /// Its text only, redacted: a text file, or a PDF that is not handed over, and why.
    Text {
        /// The text.
        text: Redacted,
        /// Whether it was cut at [`MAX_CHARS`].
        truncated: bool,
        /// Why the file itself was withheld, for a PDF; `None` for a text file.
        withheld: Option<Withheld>,
    },
    /// Nothing a model reads, named so the refusal is a sentence.
    Unsupported {
        /// What it is: its type, or its extension.
        what: String,
    },
}

/// Why a PDF itself is not handed to a model, said to neither the model nor the owner in
/// particular: each reader frames it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Withheld {
    /// Its text names a number [`crate::redact`] holds.
    #[error("its text names an identity number, a code, or a card or bank number")]
    HeldNumber,
    /// Over [`MAX_PDF`], in bytes.
    #[error("it is {}, over the 20 MB a model reads", megabytes(*.0))]
    TooLarge(usize),
    /// Over [`MAX_PAGES`].
    #[error("it has {0} pages, over the {MAX_PAGES} a model reads")]
    TooManyPages(usize),
}

/// A file a model may read, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readable {
    media: Enclosure,
    bytes: Vec<u8>,
    text: Redacted,
}

impl Readable {
    /// What it is.
    #[must_use]
    pub fn media(&self) -> Enclosure {
        self.media
    }

    /// Its bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Its text layer, redacted and capped, for a model that cannot read the file.
    #[must_use]
    pub fn text(&self) -> &Redacted {
        &self.text
    }
}

impl FileRead {
    /// Read `bytes`, which their source calls `mime`. `layer` is a PDF's text layer as
    /// `pdftotext` read it, a form feed ending each page; `None` for anything else.
    #[must_use]
    pub fn of(mime: &str, bytes: Vec<u8>, layer: Option<&str>) -> Self {
        match Enclosure::sniff(&bytes) {
            Some(Enclosure::Pdf) => pdf(bytes, layer.unwrap_or_default()),
            Some(Enclosure::Image(media)) if bytes.len() > crate::attachment::MAX_BYTES => {
                FileRead::Unsupported {
                    what: format!(
                        "a {} image of {}, over the 5 MB a model reads",
                        media.as_str(),
                        megabytes(bytes.len())
                    ),
                }
            }
            Some(media @ Enclosure::Image(_)) => FileRead::File(Readable {
                media,
                bytes,
                text: redact(""),
            }),
            None if mime.starts_with("text/") => {
                let decoded = String::from_utf8_lossy(&bytes);
                let (text, truncated) = crate::mail::clip(&decoded, MAX_CHARS);
                FileRead::Text {
                    text: redact(&text),
                    truncated,
                    withheld: None,
                }
            }
            None => FileRead::Unsupported {
                what: if mime.is_empty() {
                    "a file of no stated type".to_owned()
                } else {
                    mime.to_owned()
                },
            },
        }
    }
}

fn pdf(bytes: Vec<u8>, layer: &str) -> FileRead {
    let (text, truncated) = crate::mail::clip(layer, MAX_CHARS);
    let redacted = redact(&text);
    let pages = layer.matches('\u{c}').count();
    let withheld = if redacted.as_str() != text {
        Some(Withheld::HeldNumber)
    } else if bytes.len() > MAX_PDF {
        Some(Withheld::TooLarge(bytes.len()))
    } else if pages > MAX_PAGES {
        Some(Withheld::TooManyPages(pages))
    } else {
        None
    };
    match withheld {
        Some(why) => FileRead::Text {
            text: redacted,
            truncated,
            withheld: Some(why),
        },
        None => FileRead::File(Readable {
            media: Enclosure::Pdf,
            bytes,
            text: redacted,
        }),
    }
}

/// A size as a sentence names it.
fn megabytes(size: usize) -> String {
    crate::secretary::card::bytes(size as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PDF: &[u8] = b"%PDF-1.7\n%%EOF\n";

    #[test]
    fn a_pdf_whose_text_names_an_identity_number_is_withheld_and_its_text_redacted() {
        let FileRead::Text { text, withheld, .. } = FileRead::of(
            "application/pdf",
            PDF.to_vec(),
            Some("Employee SSN: 123-45-6789. Wages: 70,000.\u{c}"),
        ) else {
            panic!("withheld");
        };
        assert!(!text.as_str().contains("6789"), "{text:?}");
        assert!(text.as_str().contains("70,000"));
        assert_eq!(withheld, Some(Withheld::HeldNumber));
    }

    #[test]
    fn text_is_text_and_anything_else_is_named() {
        assert!(matches!(
            FileRead::of("text/plain", b"SSN: 123-45-6789".to_vec(), None),
            FileRead::Text { text, withheld: None, .. } if text.as_str() == "SSN: [redacted]"
        ));
        assert_eq!(
            FileRead::of("application/zip", b"PK\x03\x04".to_vec(), None),
            FileRead::Unsupported {
                what: "application/zip".to_owned()
            }
        );
        assert!(
            matches!(
                FileRead::of("application/pdf", b"<html>".to_vec(), None),
                FileRead::Unsupported { .. }
            ),
            "a PDF by name only is not one"
        );
    }
}
