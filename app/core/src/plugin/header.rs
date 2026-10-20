//! A header name a plugin, or its manifest, may write.

use std::fmt;

/// Headers the host owns: where the request goes and how it is framed. A plugin that could set
/// `host` could ask an allowed address to serve a host the manifest never named.
const RESERVED: [&str; 7] = [
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "upgrade",
    "te",
    "trailer",
];

/// An RFC 9110 token, lowercased, and not one of the headers the host owns.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeaderName(String);

impl HeaderName {
    /// Parse one, or `None`: the caller says in its own error what was being parsed.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let token = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b);
        let name = text.to_ascii_lowercase();
        let allowed = !name.is_empty()
            && name.len() <= 64
            && name.bytes().all(token)
            && !RESERVED.contains(&name.as_str());
        allowed.then_some(Self(name))
    }

    /// The name, lowercased.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HeaderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_name_is_a_token_the_host_does_not_own() {
        assert_eq!(
            HeaderName::parse("X-Api-Key").map(|name| name.to_string()),
            Some("x-api-key".to_owned())
        );
        for bad in [
            "",
            "Host",
            "content-length",
            "Transfer-Encoding",
            "x api",
            "x:y",
            "x\r\ny",
            "é",
        ] {
            assert!(HeaderName::parse(bad).is_none(), "{bad:?}");
        }
    }
}
