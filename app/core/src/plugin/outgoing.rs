//! A request the fence admitted, with its credentials in it: the one value that holds them.

use std::fmt;

use crate::capability::Secret;

use super::{CredentialName, HeaderName, PluginUrl, Refusal, Response, Said, Verb};

/// A request [`super::Request::admit`] let through, ready for a transport.
///
/// It holds credential values, so its `Debug` prints the origin and the header names only, and
/// what comes back reaches the plugin only through [`Outgoing::answer`].
pub struct Outgoing {
    verb: Verb,
    url: PluginUrl,
    headers: Vec<(HeaderName, Secret)>,
    secrets: Vec<(CredentialName, Secret)>,
}

impl Outgoing {
    pub(super) fn new(
        verb: Verb,
        url: PluginUrl,
        headers: Vec<(HeaderName, Secret)>,
        secrets: Vec<(CredentialName, Secret)>,
    ) -> Self {
        Self {
            verb,
            url,
            headers,
            secrets,
        }
    }

    /// The verb and its body.
    #[must_use]
    pub fn verb(&self) -> &Verb {
        &self.verb
    }

    /// The URL to send, as reprinted from what was checked.
    #[must_use]
    pub fn url(&self) -> &PluginUrl {
        &self.url
    }

    /// The headers to send, a credential's among them; each value is a [`Secret`] because any
    /// of them may be one.
    #[must_use]
    pub fn headers(&self) -> &[(HeaderName, Secret)] {
        &self.headers
    }

    /// The answer as the plugin may see it: withheld whole if its body or a header carries the
    /// value of a credential this request held, since a server that echoes a token would
    /// otherwise hand it to the plugin. Refused, never edited, like every tripwire here.
    pub fn answer(&self, response: Response) -> Result<Response, Refusal> {
        let carries = |value: &[u8]| {
            response.body.windows(value.len()).any(|at| at == value)
                || response
                    .headers
                    .iter()
                    .any(|(_, said)| said.as_bytes().windows(value.len()).any(|at| at == value))
        };
        match self
            .secrets
            .iter()
            .find(|(_, value)| carries(value.expose().as_bytes()))
        {
            Some((name, _)) => Err(Refusal::Echoed(Said::clip(name.to_string()))),
            None => Ok(response),
        }
    }
}

impl fmt::Debug for Outgoing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verb = match self.verb {
            Verb::Get => "GET",
            Verb::Post(_) => "POST",
        };
        f.debug_struct("Outgoing")
            .field("verb", &verb)
            .field("url", &self.url)
            .field(
                "headers",
                &self
                    .headers
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}
