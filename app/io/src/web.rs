//! The web fetcher behind `suzatary_core::capability::Web`: GET only, no
//! cookies or credentials, a timeout, a bounded body, and no redirect
//! followed (a 3xx is an answer naming a new request), over
//! [`Ureq::public_only`], which refuses any name resolving off the public
//! internet.

use std::time::Duration;

use suzatary_core::capability::{WebAnswer, WebFailure};
use suzatary_core::url::percent_encode;
use suzatary_core::web::{SEARCH_HOST, WebRequest};

use crate::http::{Http, HttpError, Req, Ureq};

/// The most of a body read.
const MAX_BODY: u64 = 2 * 1024 * 1024;

/// The most text handed back to the model.
pub const MAX_TEXT: usize = 60_000;

/// How many search results are listed.
const MAX_RESULTS: usize = 10;

/// The fetcher.
#[derive(Debug, Clone)]
pub struct Fetcher {
    http: Ureq,
    timeout: Duration,
}

impl Fetcher {
    /// A fetcher with this patience per request.
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self {
            http: Ureq::public_only(),
            timeout,
        }
    }

    /// Perform one request.
    ///
    /// # Errors
    /// [`WebFailure::Refused`] when the name resolves to a private address,
    /// [`WebFailure::Unreachable`] for everything else the network does.
    pub fn perform(&self, request: &WebRequest) -> Result<WebAnswer, WebFailure> {
        match request {
            WebRequest::Fetch { url } => self.page(url.as_str()),
            WebRequest::Search { query } => {
                let url = format!(
                    "https://{SEARCH_HOST}/html/?q={}",
                    percent_encode(query.as_str())
                );
                let page = self.get(&url)?;
                let text = if page.status == 200 {
                    listed(&results(&page.body))
                } else {
                    format!("the search answered HTTP {}", page.status)
                };
                Ok(WebAnswer {
                    url,
                    status: page.status,
                    text,
                })
            }
        }
    }

    fn page(&self, url: &str) -> Result<WebAnswer, WebFailure> {
        let page = self.get(url)?;
        let text = if let Some(location) =
            page.location.filter(|_| (300..400).contains(&page.status))
        {
            format!(
                "HTTP {} redirect to {location}. Redirects are not followed; that address is a new \
                 request.",
                page.status
            )
        } else if page.kind.contains("html") {
            capped(&crate::mailbox::text::html_to_text(&page.body))
        } else if page.kind.starts_with("text/")
            || page.kind.contains("json")
            || page.kind.contains("xml")
        {
            capped(&page.body)
        } else {
            format!(
                "HTTP {}: {} is not a page this box reads as text",
                page.status, page.kind
            )
        };
        Ok(WebAnswer {
            url: url.to_owned(),
            status: page.status,
            text,
        })
    }

    fn get(&self, url: &str) -> Result<Got, WebFailure> {
        let resp = self
            .http
            .send(
                Req::get(url)
                    .header("User-Agent", "Mozilla/5.0 (compatible; suzatary-secretary)")
                    .timeout(self.timeout)
                    .limit(MAX_BODY),
            )
            .map_err(|error| failure(&error))?;
        Ok(Got {
            status: resp.status,
            kind: resp
                .header("content-type")
                .unwrap_or_default()
                .to_ascii_lowercase(),
            location: resp.header("location").map(str::to_owned),
            body: resp.text(),
        })
    }
}

/// One response, read.
struct Got {
    status: u16,
    kind: String,
    location: Option<String>,
    body: String,
}

fn failure(error: &HttpError) -> WebFailure {
    match error {
        HttpError::Private { .. } => WebFailure::Refused(error.to_string()),
        HttpError::NotSent(_) | HttpError::TimedOut(_) | HttpError::Lost(_) => {
            WebFailure::Unreachable(error.to_string())
        }
    }
}

/// Text cut to [`MAX_TEXT`] characters, saying so when cut.
fn capped(text: &str) -> String {
    if text.chars().count() <= MAX_TEXT {
        return text.to_owned();
    }
    let kept: String = text.chars().take(MAX_TEXT).collect();
    format!("{kept}\n\n[cut at {MAX_TEXT} characters]")
}

/// Percent-decoding, lossy on bad escapes.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    let hex = |b: u8| (b as char).to_digit(16);
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| Some((hex(*bytes.get(at + 1)?)?, hex(*bytes.get(at + 2)?)?)))
            .flatten();
        match (bytes[at], escaped) {
            (b'%', Some((high, low))) => {
                out.push(u8::try_from(high * 16 + low).unwrap_or(b'?'));
                at += 3;
                continue;
            }
            (b'+', _) => out.push(b' '),
            (other, _) => out.push(other),
        }
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Hit {
    title: String,
    url: String,
    snippet: String,
}

/// The results on a `DuckDuckGo` HTML page: each `result__a` anchor, its
/// target unwrapped from `DuckDuckGo`'s redirect, and the snippet after it.
fn results(page: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    for block in page.split("class=\"result__a\"").skip(1) {
        let Some(href) = attribute(block, "href") else {
            continue;
        };
        let url = match href.split_once("uddg=") {
            Some((_, rest)) => decode(rest.split('&').next().unwrap_or(rest)),
            None => href.replace("&amp;", "&"),
        };
        let title = block
            .split_once('>')
            .and_then(|(_, rest)| rest.split_once("</a>"))
            .map(|(inner, _)| inline_text(inner))
            .unwrap_or_default();
        let snippet = block
            .split_once("class=\"result__snippet\"")
            .and_then(|(_, rest)| rest.split_once('>'))
            .and_then(|(_, rest)| rest.split_once("</a>").or_else(|| rest.split_once("</td>")))
            .map(|(inner, _)| inline_text(inner))
            .unwrap_or_default();
        if url.starts_with("http") && !title.is_empty() {
            hits.push(Hit {
                title,
                url,
                snippet,
            });
        }
        if hits.len() == MAX_RESULTS {
            break;
        }
    }
    hits
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_owned())
}

fn inline_text(html: &str) -> String {
    crate::mailbox::text::html_to_text(html)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn listed(hits: &[Hit]) -> String {
    if hits.is_empty() {
        return "no results".to_owned();
    }
    hits.iter()
        .enumerate()
        .map(|(n, hit)| {
            format!(
                "{}. {}\n   {}\n   {}",
                n + 1,
                hit.title,
                hit.url,
                hit.snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<div class="result"><h2><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.gov.uk%2Ftenancy%2Ddeposit%2Dprotection&amp;rut=abc">Tenancy deposit <b>protection</b></a></h2>
<a class="result__snippet" href="x">Your landlord must protect your deposit &amp; tell you where.</a></div>
<div class="result"><h2><a class="result__a" href="https://example.com/direct">Direct</a></h2></div>"#;

    #[test]
    fn search_results_are_unwrapped_titled_and_snipped() {
        let hits = results(PAGE);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].url, "https://www.gov.uk/tenancy-deposit-protection");
        assert_eq!(hits[0].title, "Tenancy deposit protection");
        assert_eq!(
            hits[0].snippet,
            "Your landlord must protect your deposit & tell you where."
        );
        assert_eq!(hits[1].url, "https://example.com/direct");
        assert!(listed(&hits).starts_with("1. Tenancy deposit protection\n   https://www.gov.uk/"));
        assert_eq!(listed(&[]), "no results");
    }

    /// Only the resolver's typed refusal is `Refused`; every other failure is `Unreachable`,
    /// whatever its words say.
    #[test]
    fn only_a_private_address_is_refused() {
        let private = HttpError::Private {
            host: "intranet.test".to_owned(),
            ip: std::net::IpAddr::from([10, 0, 0, 1]),
        };
        assert_eq!(failure(&private), WebFailure::Refused(private.to_string()));
        let worded = "x resolves to 10.0.0.1, which is not on the public internet".to_owned();
        for other in [
            HttpError::NotSent(worded.clone()),
            HttpError::TimedOut(worded.clone()),
            HttpError::Lost(worded.clone()),
        ] {
            assert_eq!(failure(&other), WebFailure::Unreachable(worded.clone()));
        }
    }

    #[test]
    fn a_query_is_encoded_and_a_long_page_is_cut_and_says_so() {
        assert_eq!(
            percent_encode("tenancy deposit & rules?"),
            "tenancy%20deposit%20%26%20rules%3F"
        );
        assert_eq!(decode("a%2Fb+c%zz"), "a/b c%zz");
        let long = "x".repeat(MAX_TEXT + 5);
        assert!(capped(&long).ends_with(&format!("[cut at {MAX_TEXT} characters]")));
        assert_eq!(capped("short"), "short");
    }
}
