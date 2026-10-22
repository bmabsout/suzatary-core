//! `POST /api/held/reveal`: one held value (`suzatary_core::held_value`), for the owner's tap.
//!
//! The answer carries the value once. The box keeps only `{"type":"NotKept"}` for a replay of
//! the request, and nothing logs, stores or pushes the value.

use axum::Json;
use axum::response::{IntoResponse as _, Response};
use suzatary_core::held_value::{Handle, Revealed as Read};
use suzatary_core::refusal::Refusal;
use suzatary_core::secretary::tap::Tap;
use suzatary_core::wire::{self, RevealPayload};

use super::State;
use super::body::{Body, MAX_ACT_BYTES};
use super::call::Call;
use super::reply::{Keeps, Outcome};

/// What the box keeps for a replay of a reveal, in the value's place.
pub const NOT_KEPT: &str = r#"{"type":"NotKept"}"#;

/// The handle a reveal names.
pub struct Revealing(Handle);

impl Body for Revealing {
    const CAP: usize = MAX_ACT_BYTES;

    type Wire = wire::Revealing;

    fn parse(revealing: wire::Revealing, _: &State) -> Result<Self, Refusal> {
        Handle::parse(&revealing.handle)
            .map(Self)
            .map_err(|refused| Refusal::bad_request(refused.to_string()))
    }
}

/// A reveal's answer, which the box never keeps.
pub struct Revealed(Result<RevealPayload, Refusal>);

impl Outcome for Revealed {
    const KEEPS: Keeps = Keeps::Instead(NOT_KEPT);

    fn outcome(self) -> Result<Response, Refusal> {
        self.0.map(|payload| Json(payload).into_response())
    }
}

/// `POST /api/held/reveal`: the value a handle names, read again from the message that holds
/// it, for a trusted device of an actor the gate maps.
pub fn reveal(call: &Call<Tap, (), Revealing>) -> Revealed {
    Revealed(
        call.state
            .reveal
            .reveal(&call.body.0)
            .map(|read| match read {
                Read::Value(shown) => RevealPayload::Shown {
                    value: shown.into_string(),
                },
                Read::Gone => RevealPayload::Gone,
            })
            .map_err(|failed| Refusal::internal(failed.to_string())),
    )
}
