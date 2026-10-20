//! The failure both model APIs answer with: `{"error": {"message": …}}`, from the Messages
//! API's error status and stream `error` event alike, and from an OpenAI-compatible endpoint.

use serde::Deserialize;

/// An error answer's body.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiError {
    /// What failed.
    pub error: Said,
}

/// The endpoint's own words for a failure.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Said {
    /// Its message.
    pub message: String,
}

/// The endpoint's message, or the whole body when it answered in another shape.
#[must_use]
pub fn message_of(body: &str) -> String {
    serde_json::from_str::<ApiError>(body).map_or_else(|_| body.to_owned(), |e| e.error.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_endpoint_s_own_message_or_the_body_as_it_came() {
        assert_eq!(
            message_of(r#"{"type":"error","error":{"type":"x","message":"too large"}}"#),
            "too large"
        );
        assert_eq!(message_of("upstream timed out"), "upstream timed out");
        assert_eq!(message_of(r#"{"error":{}}"#), r#"{"error":{}}"#);
    }
}
