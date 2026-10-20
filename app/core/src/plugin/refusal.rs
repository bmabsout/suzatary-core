//! Why the host did not send a plugin's request, or did not hand back its answer.

use super::Said;

/// One arm per `refusal` case of `app/wit/plugin.wit`'s `fetch` interface.
///
/// A refusal names an origin, a header or a credential's NAME, never a credential's value or a
/// secret URL's path; whatever it repeats is [`Said`], so bounded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// Not a URL the host's grammar reads.
    #[error("not a URL this host reads: {0:?}")]
    BadUrl(Said),
    /// An origin the manifest does not allow.
    #[error("{0} is not an origin this plugin may reach")]
    OriginNotAllowed(Said),
    /// A name the manifest does not declare for this use and this origin.
    #[error("{0:?} is not a credential this plugin declares for this request")]
    CredentialNotDeclared(Said),
    /// Declared, and not set on this box.
    #[error("{0} is declared and not set")]
    CredentialUnset(Said),
    /// Declared, and the secret store could not say what it holds (locked, gone or broken).
    /// The host keeps it as this; the plugin's world has no such case and is told `unreachable`,
    /// which is what the request was.
    #[error("the secret store could not answer for {0}")]
    CredentialUnavailable(Said),
    /// A secret URL whose value is not a URL the host reads.
    #[error("the value of {0} is not a URL this host reads")]
    CredentialNotAUrl(Said),
    /// A secret URL that is neither https nor loopback: its value would travel in the clear.
    #[error("the value of {0} is not an https or loopback URL, and is never sent in the clear")]
    InTheClear(Said),
    /// A header the plugin may not write.
    #[error("{0:?} is not a header this plugin may write")]
    BadHeader(Said),
    /// The answer contained a credential's value, and is withheld.
    #[error("the answer contained the value of {0}, and is withheld")]
    Echoed(Said),
    /// The request never left.
    #[error("unreachable")]
    Unreachable,
    /// No answer before the call's deadline.
    #[error("timed out")]
    TimedOut,
    /// The answer was lost, or larger than the host reads.
    #[error("lost")]
    Lost,
}
