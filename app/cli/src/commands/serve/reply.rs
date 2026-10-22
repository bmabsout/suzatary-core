//! How an answer goes on the wire, beyond a plain JSON 200.

use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use suzatary_core::refusal::{Refusal, Status};

/// What a route's work returns: an answer, or an answer or the refusal it earned.
pub trait Outcome: Send + 'static {
    /// What the box keeps of a signed request's answer, for a replay of it.
    const KEEPS: Keeps = Keeps::Answer;

    /// The response, or the refusal.
    ///
    /// # Errors
    /// The work's own [`Refusal`].
    fn outcome(self) -> Result<Response, Refusal>;
}

/// What the box keeps of an answer for a replay of its request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keeps {
    /// The answer as it was given.
    Answer,
    /// This JSON body in its place, with the answer's status: an answer that must not be kept
    /// anywhere, as a held value's.
    Instead(&'static str),
}

impl<O: IntoResponse + Send + 'static> Outcome for Result<O, Refusal> {
    fn outcome(self) -> Result<Response, Refusal> {
        self.map(IntoResponse::into_response)
    }
}

impl<T: Serialize + Send + 'static> Outcome for axum::Json<T> {
    fn outcome(self) -> Result<Response, Refusal> {
        Ok(self.into_response())
    }
}

impl Outcome for StatusCode {
    fn outcome(self) -> Result<Response, Refusal> {
        Ok(self.into_response())
    }
}

/// A JSON 202: the work goes on after the answer.
pub struct Accepted<T>(pub T);

impl<T: Serialize> IntoResponse for Accepted<T> {
    fn into_response(self) -> Response {
        (StatusCode::ACCEPTED, axum::Json(self.0)).into_response()
    }
}

/// An answer that is a function of its URL alone, as an object's literal is of its name: the
/// browser keeps it for good and never asks again (`immutable`), and keeps it for this person
/// alone (`private`). The API's layer marks every answer `no-store`, and [`kept`], outside it,
/// marks this one instead, so a refusal, a stream and every other answer stay `no-store`. A
/// wipe's `Clear-Site-Data` clears what was kept.
pub struct Immutable<T>(pub T);

/// What [`Immutable`] answers with: a year, the longest a cache is asked to keep anything.
pub const IMMUTABLE: &str = "private, max-age=31536000, immutable";

/// The mark an [`Immutable`] answer carries to [`kept`].
#[derive(Debug, Clone, Copy)]
struct KeptForGood;

impl<T: IntoResponse> IntoResponse for Immutable<T> {
    fn into_response(self) -> Response {
        let mut response = self.0.into_response();
        response.extensions_mut().insert(KeptForGood);
        response
    }
}

/// Marks an [`Immutable`] answer to be kept for good, after the API's layer marked it
/// `no-store` with every other answer.
pub async fn kept(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    if response.extensions().get::<KeptForGood>().is_some() {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static(IMMUTABLE),
        );
    }
    response
}

/// A refusal on the wire: one body shape, the server's own sentence.
pub fn refused(refusal: &Refusal) -> Response {
    let mut response = (
        StatusCode::from_u16(refusal.status().code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        axum::Json(refusal.payload()),
    )
        .into_response();
    match refusal.status() {
        Status::MethodNotAllowed => {
            response
                .headers_mut()
                .insert(header::ALLOW, header::HeaderValue::from_static("POST"));
        }
        // A 410 is cacheable by default, and a cached wipe would answer the new device the tab
        // comes back as. The browser clears the origin's HTTP cache, which the tab cannot reach,
        // so what the box served goes with what the tab deletes. Not `storage`: the tab deletes
        // that itself, and a later 410 of the old page's would clear it again after the reload,
        // the new device's key with it. `no-store` here as well as on the API's layer, since a
        // refusal can come from the gate, which sits outside that layer.
        Status::Wipe => {
            let headers = response.headers_mut();
            headers.insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("no-store"),
            );
            headers.insert(
                header::HeaderName::from_static("clear-site-data"),
                header::HeaderValue::from_static("\"cache\""),
            );
        }
        _ => {}
    }
    response
}
