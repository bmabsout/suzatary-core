//! How an answer goes on the wire, beyond a plain JSON 200.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use suzatary_core::refusal::{Refusal, Status};

/// What a route's work returns: an answer, or an answer or the refusal it earned.
pub trait Outcome: Send + 'static {
    /// The response, or the refusal.
    ///
    /// # Errors
    /// The work's own [`Refusal`].
    fn outcome(self) -> Result<Response, Refusal>;
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
