//! The calendar's events read back: the inverse of [`crate::literal`] over what [`crate::item`]
//! prints, as far as an actor of this schema needs it, a call's string fields by name.
//!
//! The guest cannot link the Prodrome's reader for the reason it cannot link its printer
//! (`literal.rs`); `tests/host.rs` holds this one to the Prodrome's over every fixture's events
//! and over generated text, as the printer is held.

use std::collections::BTreeMap;

/// A call's fields whose values are strings, by name; any other value is skipped whole.
///
/// # Errors
/// A text that is not one call of the objects' grammar, saying where.
pub fn strings(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut reader = Reader { text, at: 0 };
    reader.name()?;
    reader.expect('(')?;
    let mut fields = BTreeMap::new();
    while !reader.eat(')') {
        let field = reader.name()?;
        reader.expect('=')?;
        if let Some(value) = reader.value()? {
            fields.insert(field, value);
        }
        if !reader.eat(',') {
            reader.expect(')')?;
            break;
        }
        reader.eat(' ');
    }
    if reader.at == text.len() {
        Ok(fields)
    } else {
        Err(reader.wrong("the end of the call"))
    }
}

struct Reader<'t> {
    text: &'t str,
    at: usize,
}

impl Reader<'_> {
    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    fn wrong(&self, wanted: &str) -> String {
        format!("expected {wanted} at byte {}", self.at)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.rest().starts_with(c) {
            self.at += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(self.wrong(&format!("{c:?}")))
        }
    }

    fn name(&mut self) -> Result<String, String> {
        let length = self
            .rest()
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(self.rest().len());
        if length == 0 {
            return Err(self.wrong("a name"));
        }
        let name = self.rest()[..length].to_owned();
        self.at += length;
        Ok(name)
    }

    /// A string's text, or `None` past any other value: a call, a tuple, a number.
    fn value(&mut self) -> Result<Option<String>, String> {
        match self.rest().chars().next() {
            Some(quote @ ('\'' | '"')) => {
                self.at += 1;
                self.string(quote).map(Some)
            }
            Some(_) => self.skip().map(|()| None),
            None => Err(self.wrong("a value")),
        }
    }

    /// Past one value that is no string, strings inside it included.
    fn skip(&mut self) -> Result<(), String> {
        let mut depth = 0usize;
        while let Some(c) = self.rest().chars().next() {
            match c {
                '\'' | '"' => {
                    self.at += 1;
                    self.string(c)?;
                    continue;
                }
                '(' => depth += 1,
                ')' | ',' if depth == 0 => return Ok(()),
                ')' => depth -= 1,
                _ => {}
            }
            self.at += c.len_utf8();
        }
        Err(self.wrong("the end of a value"))
    }

    /// The rest of a string opened by `quote`, its escapes undone.
    fn string(&mut self, quote: char) -> Result<String, String> {
        let mut out = String::new();
        let mut chars = self.rest().char_indices();
        while let Some((offset, c)) = chars.next() {
            match c {
                _ if c == quote => {
                    self.at += offset + 1;
                    return Ok(out);
                }
                '\\' => {
                    let escaped = match chars.next().map(|(_, e)| e) {
                        Some('\\') => '\\',
                        Some('\'') => '\'',
                        Some('"') => '"',
                        Some('t') => '\t',
                        Some('n') => '\n',
                        Some('r') => '\r',
                        Some(width @ ('x' | 'u' | 'U')) => {
                            let digits = match width {
                                'x' => 2,
                                'u' => 4,
                                _ => 8,
                            };
                            let hex: String = (0..digits)
                                .filter_map(|_| chars.next().map(|(_, h)| h))
                                .collect();
                            u32::from_str_radix(&hex, 16)
                                .ok()
                                .filter(|_| hex.len() == digits)
                                .and_then(char::from_u32)
                                .ok_or_else(|| self.wrong("a code point"))?
                        }
                        _ => return Err(self.wrong("an escape")),
                    };
                    out.push(escaped);
                }
                _ => out.push(c),
            }
        }
        Err(self.wrong("the string's closing quote"))
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::literal;

    #[test]
    fn a_calls_strings_are_read_and_its_other_values_skipped() {
        let read = strings(
            "Learned(occurrence='a b', at=datetime(2026, 10, 5, 9, 0, 0), actor='ics', \
             title=\"It's (due)\", attendees=('x, y', 'z'), status='confirmed')",
        )
        .expect("a call");
        assert_eq!(read.get("occurrence").map(String::as_str), Some("a b"));
        assert_eq!(read.get("title").map(String::as_str), Some("It's (due)"));
        assert_eq!(read.get("status").map(String::as_str), Some("confirmed"));
        assert!(!read.contains_key("at") && !read.contains_key("attendees"));
        assert!(strings("Learned(title='open").is_err());
        assert!(strings("Learned(title='a') and more").is_err());
    }

    proptest! {
        /// Reading is the inverse of the printer, whatever the text.
        #[test]
        fn a_printed_string_reads_back(text in any::<String>()) {
            let call = literal::call("Learned", &[("title", literal::string(&text))]);
            let read = strings(&call).expect("a call");
            prop_assert_eq!(read.get("title"), Some(&text));
        }
    }
}
