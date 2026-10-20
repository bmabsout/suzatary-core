//! What `claude setup-token` has drawn so far, as text: the terminal's escapes removed, and the
//! two things the box reads off it, the sign-in URL and the token.

use suzatary_core::secretary::sign_in::SignInUrl;

/// The prefix of a long-lived subscription token, `sk-ant-oat01-…`.
const TOKEN_PREFIX: &str = "sk-ant-oat";

/// How the CLI says it did not take a code, lowercased.
const REFUSED: [&str; 5] = ["error", "invalid", "expired", "try again", "not accepted"];

/// Everything drawn, its escapes removed.
#[derive(Debug, Clone, Default)]
pub struct Screen {
    text: String,
    pending: Vec<u8>,
}

impl Screen {
    /// Take more of what the terminal drew; a sequence cut between two reads waits for the rest.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
        let (text, rest) = plain(&self.pending);
        self.text.push_str(&text);
        self.pending = rest;
    }

    /// The first `https://` URL drawn that is a sign-in link, whole.
    #[must_use]
    pub fn url(&self) -> Option<SignInUrl> {
        self.text.match_indices("https://").find_map(|(at, _)| {
            let url = &self.text[at..];
            SignInUrl::parse(&url[..url.find(char::is_whitespace).unwrap_or(url.len())]).ok()
        })
    }

    /// The token, once the CLI has printed it.
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        let at = self.text.find(TOKEN_PREFIX)?;
        let token = &self.text[at..];
        let end = token
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(token.len());
        // A token cut off at the end of what was read so far is not yet a token.
        (end < token.len() && end > TOKEN_PREFIX.len() + 8).then(|| &token[..end])
    }

    /// How much has been drawn: a mark [`Screen::refused_after`] reads from.
    #[must_use]
    pub fn mark(&self) -> usize {
        self.text.len()
    }

    /// Whether what was drawn after `mark` says the code typed there was refused (an error, an
    /// invalid or expired code, a request to try again), the typed `code` itself not read, so a
    /// CLI that asks again is answered at once rather than at the end of the wait.
    #[must_use]
    pub fn refused_after(&self, mark: usize, code: &str) -> bool {
        let drawn = self.text.get(mark..).unwrap_or_default();
        let drawn = if code.is_empty() {
            drawn.to_ascii_lowercase()
        } else {
            drawn.replace(code, " ").to_ascii_lowercase()
        };
        REFUSED.iter().any(|refused| drawn.contains(refused))
    }

    /// The last `n` characters drawn, for saying why no URL came; called only before a code is
    /// typed, so it can hold neither the code nor a token.
    #[must_use]
    pub fn tail(&self, n: usize) -> String {
        let chars: Vec<char> = self.text.chars().collect();
        chars[chars.len().saturating_sub(n)..]
            .iter()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// `bytes` as text with every escape sequence removed, and the bytes of a sequence or a
/// character not yet complete.
fn plain(bytes: &[u8]) -> (String, Vec<u8>) {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            0x1b => match escape_end(&bytes[i..]) {
                Some(len) => {
                    out.extend(moved(&bytes[i..i + len]));
                    i += len;
                }
                None => break,
            },
            b'\r' => {
                out.push(b'\n');
                i += 1;
            }
            b if b < 0x20 && b != b'\n' && b != b'\t' => i += 1,
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    let rest = bytes[i..].to_vec();
    match String::from_utf8(out) {
        Ok(text) => (text, rest),
        Err(failed) => {
            let valid = failed.utf8_error().valid_up_to();
            let mut bytes = failed.into_bytes();
            let cut = bytes.split_off(valid);
            let text = String::from_utf8(bytes).unwrap_or_default();
            if cut.len() < 4 {
                (text, [cut, rest].concat())
            } else {
                // Not a cut character but bytes that are not UTF-8: dropped, never waited for.
                (text, rest)
            }
        }
    }
}

/// What a cursor move stands for in the text: the CLI places each word by moving to its column,
/// so a move along a line is a space and a move to another line is a line break.
fn moved(sequence: &[u8]) -> Option<u8> {
    match sequence {
        [0x1b, b'[', .., b'C' | b'G'] => Some(b' '),
        [
            0x1b,
            b'[',
            ..,
            b'A' | b'B' | b'E' | b'F' | b'H' | b'd' | b'f',
        ] => Some(b'\n'),
        _ => None,
    }
}

/// The length of the escape sequence `bytes` starts with, `None` while it is incomplete.
fn escape_end(bytes: &[u8]) -> Option<usize> {
    match bytes.get(1)? {
        // CSI: parameters and intermediates, then one final byte in `@`..=`~`.
        b'[' => bytes[2..]
            .iter()
            .position(|b| (0x40..=0x7e).contains(b))
            .map(|at| at + 3),
        // OSC (a title, a hyperlink): ended by BEL or by ESC `\`.
        b']' => bytes[2..].windows(2).enumerate().find_map(|(at, pair)| {
            if pair[0] == 0x07 {
                Some(at + 3)
            } else {
                (pair == [0x1b, b'\\']).then_some(at + 4)
            }
        }),
        // A charset designation names one more byte.
        b'(' | b')' | b'*' | b'+' => bytes.get(2).map(|_| 3),
        // A two-byte sequence (a keypad mode, a save or restore).
        _ => Some(2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(chunks: &[&[u8]]) -> Screen {
        let mut screen = Screen::default();
        for chunk in chunks {
            screen.feed(chunk);
        }
        screen
    }

    #[test]
    fn the_url_is_read_through_colour_and_a_hyperlink() {
        let drawn = screen(&[
            b"\x1b[2J\x1b[HBrowser didn't open? Use the url below to sign in\r\n",
            b"\x1b]8;;https://claude.ai/oauth/authorize?code=true&state=s1\x1b\\",
            b"\x1b[36mhttps://claude.ai/oauth/authorize?code=true&state=s1\x1b[39m",
            b"\x1b]8;;\x1b\\\r\nPaste code here if prompted > ",
        ]);
        assert_eq!(
            drawn.url().as_ref().map(SignInUrl::as_str),
            Some("https://claude.ai/oauth/authorize?code=true&state=s1")
        );
        assert_eq!(drawn.token(), None);
    }

    /// As the real `setup-token` (2.1.292) draws it: each word placed at its column, the link in
    /// an OSC 8 hyperlink ended by BEL, its text in a 256-colour grey.
    #[test]
    fn the_real_draw_reads_as_words_and_one_link() {
        let drawn = screen(&[
            b"\x1b[2G\x1b[38;5;246mBrowser\x1b[10Gdidn't\x1b[17Gopen?\x1b[39m\r\r\n\r\r\n",
            b"\x1b]8;id=1im01g5;https://claude.com/cai/oauth/authorize?code=true&state=s\x07",
            b"\x1b[38;5;246mhttps://claude.com/cai/oauth/authorize?code=true&state=s\x1b[39m",
            b"\x1b]8;;\x07\r\r\n\r\r\n\x1b[2GPaste\x1b[8Gcode\x1b[30G>\r\r\n\x1b[>0q\x1b[?u\x1b[c",
        ]);
        assert_eq!(
            drawn.url().as_ref().map(SignInUrl::as_str),
            Some("https://claude.com/cai/oauth/authorize?code=true&state=s")
        );
        assert!(
            drawn.tail(20).ends_with("Paste code >"),
            "{}",
            drawn.tail(20)
        );
        assert!(drawn.tail(200).starts_with("Browser didn't open?"));
    }

    #[test]
    fn a_sequence_cut_between_two_reads_is_waited_for() {
        let drawn = screen(&[b"\x1b[3", b"6mhttps://claude.ai/oauth?x=1\x1b[", b"0m\r\n"]);
        assert_eq!(
            drawn.url().as_ref().map(SignInUrl::as_str),
            Some("https://claude.ai/oauth?x=1")
        );
    }

    #[test]
    fn a_url_on_another_host_is_not_a_sign_in() {
        let drawn = screen(&[b"see https://example.test/oauth?x=1 \r\n"]);
        assert_eq!(drawn.url(), None);
    }

    #[test]
    fn the_token_is_read_whole_and_only_once_it_is_whole() {
        let mut drawn =
            screen(&[b"Your OAuth token (valid for 1 year):\r\n\r\nsk-ant-oat01-abcDEF"]);
        assert_eq!(drawn.token(), None, "the token may go on");
        drawn.feed(b"123_-xyz\r\n\r\nStore this token securely.");
        assert_eq!(drawn.token(), Some("sk-ant-oat01-abcDEF123_-xyz"));
    }

    #[test]
    fn a_character_cut_between_two_reads_is_kept() {
        let tick = "✓".as_bytes();
        let drawn = screen(&[&tick[..1], &tick[1..], b" done"]);
        assert_eq!(drawn.tail(10), "✓ done");
    }
}
