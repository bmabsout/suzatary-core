//! A subscription sign-in in progress: the link the box read off `claude setup-token` and the
//! sessions the box holds open, which a code card's tap finishes.

use crate::capability::Secret;
use crate::secretary::inbox::ProposalId;

/// The hosts a sign-in link may name; a link anywhere else is never drawn as a button.
const HOSTS: [&str; 4] = [
    "claude.com",
    "claude.ai",
    "platform.claude.com",
    "console.anthropic.com",
];

/// An `https://` link on one of Anthropic's sign-in hosts, as `claude setup-token` draws it.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct SignInUrl(String);

impl SignInUrl {
    /// Parse one.
    ///
    /// # Errors
    /// [`NotASignIn`] for another scheme, another host, userinfo, a port, or whitespace.
    pub fn parse(text: &str) -> Result<Self, NotASignIn> {
        let refused = || NotASignIn(text.chars().take(80).collect());
        let rest = text.strip_prefix("https://").ok_or_else(refused)?;
        let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
        // A host is compared whole, so userinfo (`ada@`) and a port (`:8443`) are refused with it.
        if text.chars().any(char::is_whitespace) || !HOSTS.contains(&host) {
            return Err(refused());
        }
        Ok(Self(text.to_owned()))
    }

    /// The link.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A link that is not a sign-in, named by its first characters.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not an https link on a Claude sign-in host")]
pub struct NotASignIn(pub String);

/// The sign-ins the box holds open, each a `claude setup-token` waiting for its code; held in no
/// capability record, so only a tap reaches one.
pub trait Sessions: Sync {
    /// Every sign-in still open, by its link.
    fn open(&self) -> Vec<SignInUrl>;

    /// Type `code` into the sign-in at `url`, and answer the card that began it with the token
    /// the code is exchanged for, or why none came in words that never carry the code or a
    /// token; the session ends either way. `None` for a sign-in no longer open.
    fn finish(
        &self,
        url: &SignInUrl,
        code: &Secret,
    ) -> Option<(ProposalId, Result<Secret, String>)>;
}

crate::literal::text_literal!(
    SignInUrl => |url| url.as_str().to_owned(),
);

impl TryFrom<String> for SignInUrl {
    type Error = NotASignIn;
    fn try_from(text: String) -> Result<Self, NotASignIn> {
        Self::parse(&text)
    }
}

impl From<SignInUrl> for String {
    fn from(url: SignInUrl) -> String {
        url.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_https_link_on_a_sign_in_host_parses() {
        assert!(
            SignInUrl::parse("https://claude.com/cai/oauth/authorize?code=true&state=s").is_ok()
        );
        for refused in [
            "http://claude.com/cai/oauth/authorize",
            "https://claude.com.example.test/oauth",
            "https://example.test/?https://claude.com",
            "https://claude.com:8443/oauth",
            "https://ada@claude.com/oauth",
            "https://claude.com/oauth?x=1 y",
        ] {
            assert!(SignInUrl::parse(refused).is_err(), "{refused}");
        }
    }
}
