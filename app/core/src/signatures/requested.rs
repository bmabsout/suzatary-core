use prodrome_core::event::Hash;
use prodrome_core::sign::{PublicKey, Signature, Signed};
use serde::{Deserialize, Serialize};

/// WHICH DEVICE ASKED FOR WHAT THE BOX WROTE: one request the box performed, as provenance and
/// nothing more. Its name, method, target and instant, the device key that signed the name and
/// the signature, the actor the gate wrote it as, the status the box answered and the chain
/// objects it wrote doing it.
///
/// Kept once, under its name, in a store pushed with the data, so it holds nothing of what was
/// said either way: no body, no answer, no body's digest and not the request's nonce, since an
/// answer can carry a memory's words and anything that recomputes the name from a guessed body
/// is a check against a secret the body carried. So the name cannot be recomputed from the
/// record and a guess; it is what the device signed ([`Requested::verifies`]). What a replay is answered with is
/// [`Answer`], kept on the box alone. Not on a chain: a chain holds what a todo became, and a
/// request is what a person asked of the box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requested {
    /// The request's name, [`super::request_name`] of what it asked and the key.
    pub name: String,
    /// The method it asked with.
    pub method: String,
    /// The path and query it asked at.
    pub target: String,
    /// The instant its device stamped on it.
    pub at: String,
    /// The device key that signed it.
    pub key: String,
    /// The signature of the name.
    pub signature: String,
    /// The actor the gate wrote it as.
    pub actor: String,
    /// The HTTP status the box answered.
    pub status: u16,
    /// The chain objects the box appended performing it, in the order written.
    pub wrote: Vec<String>,
}

/// An answer as the box gave it, its status and its JSON body: what a replay of the request is
/// answered with, kept on the box only and only while the request could still be replayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    /// The HTTP status.
    pub status: u16,
    /// The body, as sent.
    pub body: String,
}

impl Requested {
    /// Does the signature the record holds verify over its name, by its key?
    #[must_use]
    pub fn verifies(&self) -> bool {
        match (
            Hash::new(self.name.clone()),
            PublicKey::new(self.key.clone()),
            Signature::new(self.signature.clone()),
        ) {
            (Ok(object), Ok(key), Ok(signature)) => Signed {
                object,
                key,
                signature,
            }
            .verifies(),
            _ => false,
        }
    }
}
