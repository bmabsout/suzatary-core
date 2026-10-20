//! A read of the API carries a validator, and a page that holds the same answer is told so.
//!
//! The tag is a digest of the answer's bytes, not of the heads the route read: a fold prices
//! every todo at the instant it is asked, so the same heads give different bytes an hour apart,
//! and a tag of the heads would tell a page its old numbers still stand. A digest changes exactly
//! when the bytes do. It is weak (`W/`), so the one tag stands for the answer under every content
//! coding the compression layer outside this one may pick, which RFC 9110 §8.8.1 allows a weak
//! validator and not a strong one.
//!
//! Every answer stays `no-store`: the browser's cache keeps nothing of the owner's, and the page
//! keeps the last answer of each read itself and sends its tag (`web/src/wire/http.ts`). A 304
//! saves the transfer and the page's parse, never the box's work.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};

/// The weak tag of an answer's bytes: the first 128 bits of their sha256, in hex.
///
/// # Panics
/// Never: hex digits in quotes are visible ASCII, which every header value may hold.
#[must_use]
pub fn tag(bytes: &[u8]) -> HeaderValue {
    let digest = Sha256::digest(bytes);
    HeaderValue::try_from(format!("W/\"{}\"", hex::encode(&digest[..16])))
        .expect("hex digits in quotes are a header value")
}

/// Whether `If-None-Match` names `tag`, by RFC 9110's weak comparison: `*`, or any listed tag
/// whose opaque part is the same, `W/` or not.
#[must_use]
pub fn held(headers: &HeaderMap, tag: &HeaderValue) -> bool {
    let opaque = |t: &[u8]| {
        let t = t.trim_ascii();
        t.strip_prefix(b"W/").unwrap_or(t).to_vec()
    };
    let ours = opaque(tag.as_bytes());
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .flat_map(|value| value.as_bytes().split(|&b| b == b','))
        .any(|listed| listed.trim_ascii() == b"*" || opaque(listed) == ours)
}

/// Tags a successful JSON answer to a GET, and answers 304 with no body when the request
/// already holds it. A stream, a file and a refusal pass as they are.
pub async fn revalidated(request: Request, next: Next) -> Response {
    let get = request.method() == Method::GET;
    let asked = request.headers().clone();
    let response = next.run(request).await;
    let json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json"));
    if !get || response.status() != StatusCode::OK || !json {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    // The answer is a `Json` already serialised in memory, so reading it whole holds nothing more.
    let Ok(bytes) = axum::body::to_bytes(body, usize::MAX).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let tag = tag(&bytes);
    if held(&asked, &tag) {
        let mut unchanged = StatusCode::NOT_MODIFIED.into_response();
        unchanged.headers_mut().insert(header::ETAG, tag);
        return unchanged;
    }
    parts.headers.insert(header::ETAG, tag);
    Response::from_parts(parts, Body::from(bytes))
}

#[cfg(test)]
mod tests {
    use super::{held, tag};
    use axum::http::{HeaderMap, HeaderValue, header};
    use proptest::prelude::*;

    fn asking(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(value).expect("a test's header is visible ASCII"),
        );
        headers
    }

    proptest! {
        // The law: the tag changes exactly when the answer's bytes change.
        #[test]
        fn the_tag_changes_exactly_when_the_bytes_do(a in any::<Vec<u8>>(), b in any::<Vec<u8>>()) {
            prop_assert_eq!(tag(&a) == tag(&b), a == b);
        }

        // An answer one byte apart is never the answer held.
        #[test]
        fn one_byte_moved_is_a_new_answer(mut a in proptest::collection::vec(any::<u8>(), 1..256), at in any::<prop::sample::Index>(), by in 1u8..) {
            let was = tag(&a);
            let i = at.index(a.len());
            a[i] = a[i].wrapping_add(by);
            prop_assert!(!held(&asking(was.to_str().expect("ascii")), &tag(&a)));
        }
    }

    #[test]
    fn a_tag_is_held_under_either_strength_in_a_list_or_as_a_star() {
        let ours = tag(b"{\"entries\":[]}");
        let text = ours.to_str().expect("ascii");
        let opaque = text.trim_start_matches("W/");
        assert!(held(&asking(text), &ours));
        assert!(held(&asking(opaque), &ours));
        assert!(held(&asking(&format!("W/\"other\", {text}")), &ours));
        assert!(held(&asking("*"), &ours));
        assert!(!held(&asking("W/\"other\""), &ours));
        assert!(!held(&HeaderMap::new(), &ours));
    }
}
