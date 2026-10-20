//! The model over an OpenAI-compatible chat completions endpoint.
//!
//! One interpreter for every vendor that speaks this wire: a vendor is a base URL, and the verb
//! protocol is our own JSON in the reply's text, so the request asks for no vendor tool-calling.
//! Under `Attestation::Required` the gateway's identity is established before a job is sent,
//! and every reply's receipt is checked under it and against the commit the owner recorded;
//! a quote that does not verify refuses the job, and a missing or mismatched receipt the reply.
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use suzatary_core::capability::{
    Conversation, Model, ModelError, ReceiptId, Reply, Secret, TokenUsage,
};
use suzatary_core::excerpt::Excerpt;

use crate::api_error::message_of;
use crate::attestation::collateral::Collateral;
use crate::attestation::{self, AttestationError, ExpectedSource, Identities, Identity, Nonce};
use crate::http::{Http, Req, Ureq};
use crate::prompt::prompt_of;

/// A wedged connection must not hold a route's blocking task forever.
const DEFAULT_TIMEOUT: Duration = Duration::from_mins(5);

/// The response header naming the attestation receipt, per Phala's confidential-AI docs; the
/// receipt itself is fetched by that id from the attestation endpoint.
const RECEIPT_HEADER: &str = "x-receipt-id";

/// Whether a provider declared attestation: a policy over every response, deciding whether a
/// missing receipt is ordinary or a refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attestation {
    /// No attestation claim; a response without a receipt is unremarkable.
    Unclaimed,
    /// Attestation declared: a missing receipt, or one resolving to a commit other than
    /// `expect`, refuses the reply.
    Required {
        /// The commit the owner recorded for this provider's source.
        expect: ExpectedSource,
    },
}

/// The instance file's claim, as the policy this interpreter enforces.
impl From<&suzatary_core::settings::Attestation> for Attestation {
    fn from(claim: &suzatary_core::settings::Attestation) -> Self {
        match claim {
            suzatary_core::settings::Attestation::Unclaimed => Self::Unclaimed,
            suzatary_core::settings::Attestation::Required { expect } => Self::Required {
                expect: expect.into(),
            },
        }
    }
}

/// The receipt id a response carried; an identifier, not a verified quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    /// The `x-receipt-id` header's value.
    pub identifier: ReceiptId,
}

/// The OpenAI-compatible chat completions endpoint.
#[derive(Clone)]
pub struct OpenAiCompatible<H: Http = Ureq> {
    base_url: String,
    model: String,
    key: Secret,
    attestation: Attestation,
    http: H,
    timeout: Duration,
    /// A separate seam from `http` so a test can script receipt resolution without scripting
    /// every chat completion.
    attest: Arc<dyn Http>,
    /// The identities established, which outlive one turn when the caller shares them.
    identities: Arc<Identities>,
}

impl<H: Http> std::fmt::Debug for OpenAiCompatible<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCompatible")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("attestation", &self.attestation)
            .finish_non_exhaustive()
    }
}

impl OpenAiCompatible<Ureq> {
    /// Bind the interpreter to a base URL, a model name, a key and an attestation policy.
    #[must_use]
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        key: Secret,
        attestation: Attestation,
    ) -> Self {
        Self::with_transport(base_url, model, key, attestation, Ureq::new())
    }
}

impl<H: Http> OpenAiCompatible<H> {
    /// `new` over an explicit transport, so tests open no socket.
    #[must_use]
    pub fn with_transport(
        base_url: impl Into<String>,
        model: impl Into<String>,
        key: Secret,
        attestation: Attestation,
        http: H,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            model: model.into(),
            key,
            attestation,
            http,
            timeout: DEFAULT_TIMEOUT,
            attest: Arc::new(Ureq::new()),
            identities: Arc::default(),
        }
    }

    /// A different patience.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Override the attestation endpoint's transport.
    #[must_use]
    pub fn with_attestation_transport(mut self, attest: impl Http + 'static) -> Self {
        self.attest = Arc::new(attest);
        self
    }

    /// Read and record identities in `identities`, so a gateway's report is fetched once per
    /// keyset however many interpreters a caller builds over it.
    #[must_use]
    pub fn with_identities(mut self, identities: Arc<Identities>) -> Self {
        self.identities = identities;
        self
    }

    /// The gateway's identity, from a report asked with a fresh nonce and Intel's collateral
    /// for the platform its quote names.
    fn establish(&self, now: i64) -> Result<Identity, AttestationError> {
        let attest = self.attest.as_ref();
        Identity::establish(
            attest,
            &self.base_url,
            &Nonce::fresh(),
            |platform| {
                self.identities.collateral(platform, now, || {
                    Collateral::fetch(attest, platform, now).map_err(AttestationError::Collateral)
                })
            },
            now,
        )
    }

    /// `Model::turn` plus the receipt, because this crate does not log and the caller records
    /// which enclave answered.
    ///
    /// # Errors
    /// Transport, HTTP and body failures, and under `Attestation::Required` a missing,
    /// unverifiable or mismatched receipt.
    pub fn turn_with_receipt(
        &self,
        conversation: &Conversation,
        tools: &Value,
    ) -> Result<(Reply, Option<Receipt>), OpenAiError> {
        let now = chrono::Utc::now().timestamp();
        // Everything that protects a prompt happens before it is sent (ACI §9).
        if let Attestation::Required { expect } = &self.attestation {
            let identity = self
                .identities
                .current(&self.base_url, now, || self.establish(now))
                .map_err(OpenAiError::AttestationFailed)?;
            admitted(expect, &identity)?;
        }
        let prompt = prompt_of(conversation, tools, crate::prompt::Reads::Text);
        let body = serde_json::json!({
            "model": &self.model,
            "messages": [
                { "role": "system", "content": prompt.system },
                { "role": "user", "content": prompt.user },
            ],
        });
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let resp = self
            .http
            .send(
                Req::json(url, &body)
                    .header("Authorization", format!("Bearer {}", self.key.expose()))
                    .timeout(self.timeout),
            )
            .map_err(|failed| OpenAiError::Unreachable(failed.to_string()))?;
        let (status, text) = (resp.status, resp.text());

        if status == 401 {
            return Err(OpenAiError::Unauthorized);
        }
        if status == 429 {
            return Err(OpenAiError::Throttled {
                retry_after: resp.header("retry-after").map(str::to_owned),
            });
        }
        if !(200..300).contains(&status) {
            return Err(OpenAiError::Failed {
                status,
                message: Excerpt::of(&message_of(&text)).to_string(),
            });
        }

        let Ok(Completion { choices, usage }) = serde_json::from_str(&text) else {
            return Err(OpenAiError::Malformed(prefix(&text)));
        };
        let Some(Choice { message }) = choices.into_iter().next() else {
            return Err(OpenAiError::Malformed(prefix(&text)));
        };

        let receipt = resp.header(RECEIPT_HEADER).map(|header| Receipt {
            identifier: ReceiptId::new(header.to_owned()),
        });
        if let Attestation::Required { expect } = &self.attestation {
            let Some(receipt) = &receipt else {
                return Err(OpenAiError::Unattested(format!(
                    "{} declared attestation but the response carried no \
                     {RECEIPT_HEADER} header",
                    self.base_url
                )));
            };
            // A receipt that fails to check out discards the reply, never falls back.
            let verified = attestation::Verified::resolve(
                self.attest.as_ref(),
                &self.base_url,
                &receipt.identifier,
                &self.key,
                resp.header(attestation::KEYSET_DIGEST_HEADER),
                |named| {
                    self.identities
                        .of(&self.base_url, named, now, || self.establish(now))
                },
            )
            .map_err(OpenAiError::AttestationFailed)?;
            admitted(expect, &verified.identity)?;
        }

        let reply = Reply::parse(&message.content).with_usage(usage.map(TokenUsage::from));
        Ok((reply, receipt))
    }
}

impl<H: Http> Model for OpenAiCompatible<H> {
    fn name(&self) -> String {
        format!("openai-compatible/{}", self.model)
    }

    fn turn(&self, conversation: &Conversation, tools: &Value) -> Result<Reply, ModelError> {
        self.turn_with_receipt(conversation, tools)
            .map(|(reply, receipt)| reply.with_receipt(receipt.map(|r| r.identifier)))
            .map_err(ModelError::from)
    }
}

/// `identity` is the source the owner recorded, or the job is refused naming both.
fn admitted(expect: &ExpectedSource, identity: &Identity) -> Result<(), OpenAiError> {
    if identity.repo_commit.as_deref() == Some(expect.repo_commit()) {
        Ok(())
    } else {
        Err(OpenAiError::SourceMismatch {
            expected: expect.repo_commit().to_owned(),
            actual: identity.repo_commit.clone(),
        })
    }
}

/// A 2xx body: the reply is the first choice's text.
#[derive(Deserialize)]
struct Completion {
    choices: Vec<Choice>,
    /// `None` when the endpoint sent no usage, never a zero standing in for an unsent figure.
    usage: Option<CompletionUsage>,
}

#[derive(Deserialize)]
struct Choice {
    message: Said,
}

#[derive(Deserialize)]
struct Said {
    content: String,
}

#[derive(Deserialize)]
struct CompletionUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    prompt_tokens_details: Option<PromptDetails>,
}

#[derive(Deserialize)]
struct PromptDetails {
    cached_tokens: Option<u64>,
}

impl From<CompletionUsage> for TokenUsage {
    fn from(usage: CompletionUsage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            cached_prompt_tokens: usage
                .prompt_tokens_details
                .and_then(|details| details.cached_tokens),
        }
    }
}

/// The first 200 characters of what came back, as an [`Excerpt`]: enough to see the shape of a
/// malformed body without holding the whole of it, or an address in it, in an error string.
fn prefix(text: &str) -> String {
    Excerpt::head(text, 200).to_string()
}

/// Why an OpenAI-compatible turn failed.
#[derive(Debug, thiserror::Error)]
pub enum OpenAiError {
    /// The endpoint could not be reached: no route, no DNS, a refused connection.
    #[error("the endpoint could not be reached: {0}")]
    Unreachable(String),
    /// A 401; the key is never named.
    #[error("the key did not authenticate")]
    Unauthorized,
    /// A 429, with the `retry-after` header's value when the endpoint sent one.
    #[error(
        "the endpoint throttled this request{}",
        retry_after
            .as_deref()
            .map(|seconds| format!(" (retry after {seconds}s)"))
            .unwrap_or_default()
    )]
    Throttled {
        /// The endpoint's own `retry-after` header value, verbatim.
        retry_after: Option<String>,
    },
    /// Any other non-2xx status, with the endpoint's own message.
    #[error("the endpoint answered {status}: {message}")]
    Failed {
        /// The HTTP status.
        status: u16,
        /// The endpoint's own error message, or its raw body in another shape.
        message: String,
    },
    /// A 2xx body that was not the expected shape — not JSON, or JSON with
    /// no readable `choices[0].message.content`.
    #[error("the body was not the expected shape: {0}")]
    Malformed(String),
    /// `Attestation::Required` and the response carried no receipt.
    #[error("{0}")]
    Unattested(String),
    /// The gateway's identity or the reply's receipt did not resolve or verify.
    #[error("the attestation did not verify: {0}")]
    AttestationFailed(AttestationError),
    /// The identity verified but names a different commit than the owner recorded; the job or
    /// the reply is refused.
    #[error(
        "the served source {actual:?} does not match the expected commit {expected:?} — \
         the reply is refused, not served"
    )]
    SourceMismatch {
        /// The commit `instance.toml` records.
        expected: String,
        /// The receipt's `repo_commit`, `None` when the gateway reported no source.
        actual: Option<String>,
    },
}

impl From<OpenAiError> for ModelError {
    fn from(error: OpenAiError) -> Self {
        match error {
            OpenAiError::Unreachable(text) => ModelError::Unreachable(text),
            other => crate::model::failed(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{HttpError, Resp, Scripted};
    use std::sync::Mutex;
    use suzatary_core::capability::Turn;

    /// One scripted answer, capturing the request body and timeout it was asked to send.
    #[derive(Debug)]
    struct ScriptedTransport {
        answer: Result<(u16, String, Headers), String>,
        captured: Mutex<Option<Value>>,
        captured_timeout: Mutex<Option<Duration>>,
    }

    impl ScriptedTransport {
        fn new(answer: Result<(u16, String, Headers), String>) -> Self {
            Self {
                answer,
                captured: Mutex::new(None),
                captured_timeout: Mutex::new(None),
            }
        }
    }

    impl Http for ScriptedTransport {
        fn send(&self, req: Req) -> Result<Resp, HttpError> {
            *self
                .captured
                .lock()
                .expect("no poisoned lock in a single-threaded test") =
                serde_json::from_slice(&req.body).ok();
            *self
                .captured_timeout
                .lock()
                .expect("no poisoned lock in a single-threaded test") = req.timeout;
            let (status, text, headers) = self.answer.clone().map_err(HttpError::Lost)?;
            let mut resp = Resp::new(status, text);
            if let Some(after) = headers.retry_after {
                resp = resp.with_header("retry-after", after);
            }
            if let Some(receipt) = headers.attestation {
                resp = resp.with_header(RECEIPT_HEADER, receipt);
            }
            if let Some(keyset) = headers.keyset {
                resp = resp.with_header(attestation::KEYSET_DIGEST_HEADER, keyset);
            }
            Ok(resp)
        }
    }

    /// The headers a scripted chat answer carries.
    #[derive(Debug, Clone, Default)]
    struct Headers {
        retry_after: Option<String>,
        attestation: Option<String>,
        keyset: Option<String>,
    }

    fn conversation() -> Conversation {
        Conversation {
            system: "You are the secretary.".to_owned(),
            turns: vec![Turn::Person {
                text: "what changed".to_owned(),
                references: Vec::new(),
                attachments: Vec::new(),
            }],
        }
    }

    fn model_over(
        transport: ScriptedTransport,
        attestation: Attestation,
    ) -> OpenAiCompatible<ScriptedTransport> {
        OpenAiCompatible::with_transport(
            BASE,
            "deepseek-v4-flash",
            Secret::new("a-key"),
            attestation,
            transport,
        )
    }

    fn ok_body(content: &str) -> Value {
        serde_json::json!({
            "choices": [
                { "message": { "role": "assistant", "content": content } }
            ]
        })
    }

    fn expected_source() -> ExpectedSource {
        ExpectedSource::parse(&"ab".repeat(20)).expect("40 hex characters")
    }

    const BASE: &str = "https://inference.example/v1";

    /// The one receipt key every scripted receipt is signed by.
    fn signer() -> &'static attestation::testkit::Signer {
        static SIGNER: std::sync::OnceLock<attestation::testkit::Signer> =
            std::sync::OnceLock::new();
        SIGNER.get_or_init(|| attestation::testkit::Signer::generate("receipt-test"))
    }

    /// The keyset that key is listed in.
    fn keyset() -> Value {
        attestation::testkit::keyset(&[signer().key_entry()], 2_000_000_000)
    }

    /// Identities already holding the gateway's, as a verified report naming `repo_commit`
    /// would give it, so a test reaches the receipt and never a quote it cannot sign.
    fn holding(repo_commit: &str) -> Arc<Identities> {
        let identities = Arc::new(Identities::default());
        let held = attestation::testkit::identity(&keyset(), Some(repo_commit));
        identities
            .current(BASE, 0, || Ok::<_, ()>(held))
            .expect("a held identity");
        identities
    }

    /// The receipts endpoint: a receipt for the id asked, signed by [`signer`] and binding its
    /// keyset, one byte changed when `tamper`; every other path is a 404.
    fn receipts(tamper: bool) -> Arc<Scripted> {
        Arc::new(Scripted::new(move |req| {
            let Some((_, id)) = req.url.split_once("/aci/receipts/") else {
                return Ok(Resp::new(404, "not found"));
            };
            let unsigned = attestation::testkit::receipt(
                id,
                &attestation::testkit::digest(&keyset()),
                &signer().key_id,
            );
            let signed = signer().sign(&unsigned).to_string();
            Ok(Resp::new(
                200,
                if tamper {
                    signed.replacen("\"served_at\":1750000000", "\"served_at\":1750000001", 1)
                } else {
                    signed
                },
            ))
        }))
    }

    fn required() -> Attestation {
        Attestation::Required {
            expect: expected_source(),
        }
    }

    fn with_receipt(id: &str) -> Headers {
        Headers {
            attestation: Some(id.to_owned()),
            keyset: Some(attestation::testkit::digest(&keyset())),
            ..Headers::default()
        }
    }

    /// Whether the chat endpoint was ever asked.
    fn sent(model: &OpenAiCompatible<ScriptedTransport>) -> bool {
        model
            .http
            .captured
            .lock()
            .expect("no poisoned lock in a single-threaded test")
            .is_some()
    }

    #[test]
    fn a_usage_block_is_read_into_token_usage() {
        let mut body = ok_body(r#"{"say": "hi", "calls": []}"#);
        body["usage"] = serde_json::json!({
            "prompt_tokens": 100,
            "completion_tokens": 20,
            "prompt_tokens_details": { "cached_tokens": 40 },
        });
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let model = model_over(transport, Attestation::Unclaimed);
        let reply = model
            .turn(&conversation(), &serde_json::json!([]))
            .expect("a normal reply is not an error");
        let usage = reply.usage.expect("a usage block was sent");
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.cached_prompt_tokens, Some(40));
    }

    #[test]
    fn no_usage_block_is_none() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let model = model_over(transport, Attestation::Unclaimed);
        let reply = model
            .turn(&conversation(), &serde_json::json!([]))
            .expect("a normal reply is not an error");
        assert!(reply.usage.is_none());
    }

    /// A usage block missing a count is a body of another shape, never a count read as zero.
    #[test]
    fn a_usage_block_missing_a_count_is_malformed() {
        let mut body = ok_body(r#"{"say": "hi", "calls": []}"#);
        body["usage"] = serde_json::json!({ "prompt_tokens": 100 });
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        assert!(matches!(
            model_over(transport, Attestation::Unclaimed)
                .turn_with_receipt(&conversation(), &serde_json::json!([])),
            Err(OpenAiError::Malformed(_))
        ));
    }

    #[test]
    fn a_normal_reply_parses() {
        let body = ok_body(r#"{"say": "on it", "calls": []}"#);
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let model = model_over(transport, Attestation::Unclaimed);
        let reply = model
            .turn(&conversation(), &serde_json::json!([]))
            .expect("a normal reply is not an error");
        assert_eq!(reply.say, "on it");
        assert!(reply.calls.is_empty());
    }

    /// Unfencing is `Reply::parse`'s job; this checks the content is handed through whole.
    #[test]
    fn a_fenced_json_reply_parses() {
        let body = ok_body("```json\n{\"say\": \"hello\", \"calls\": []}\n```");
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let model = model_over(transport, Attestation::Unclaimed);
        let reply = model
            .turn(&conversation(), &serde_json::json!([]))
            .expect("a fenced reply is not an error");
        assert_eq!(reply.say, "hello");
    }

    #[test]
    fn a_401_says_the_key_did_not_authenticate() {
        let transport = ScriptedTransport::new(Ok((
            401,
            "{\"error\":{\"message\":\"invalid api key: a-key\"}}".to_owned(),
            Headers::default(),
        )));
        let model = model_over(transport, Attestation::Unclaimed);
        let failed = model
            .turn_with_receipt(&conversation(), &serde_json::json!([]))
            .expect_err("a 401 is a failure");
        assert!(matches!(failed, OpenAiError::Unauthorized));
        assert!(!failed.to_string().contains("a-key"));
    }

    #[test]
    fn a_429_carries_the_retry_after() {
        let transport = ScriptedTransport::new(Ok((
            429,
            String::new(),
            Headers {
                retry_after: Some("30".to_owned()),
                ..Headers::default()
            },
        )));
        let model = model_over(transport, Attestation::Unclaimed);
        let failed = model
            .turn_with_receipt(&conversation(), &serde_json::json!([]))
            .expect_err("a 429 is a failure");
        match failed {
            OpenAiError::Throttled { retry_after } => {
                assert_eq!(retry_after.as_deref(), Some("30"));
            }
            other => panic!("expected Throttled, got {other:?}"),
        }
    }

    #[test]
    fn a_malformed_body_names_its_prefix() {
        let long = "not json at all, ".repeat(20);
        let transport = ScriptedTransport::new(Ok((200, long.clone(), Headers::default())));
        let model = model_over(transport, Attestation::Unclaimed);
        let failed = model
            .turn_with_receipt(&conversation(), &serde_json::json!([]))
            .expect_err("a malformed body is a failure");
        match failed {
            OpenAiError::Malformed(prefix) => {
                assert_eq!(
                    prefix,
                    long.chars().take(200).chain(['…']).collect::<String>()
                );
                assert!(prefix.chars().count() <= 201);
            }
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn a_body_with_no_choices_is_malformed() {
        let body = serde_json::json!({ "id": "x" });
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let model = model_over(transport, Attestation::Unclaimed);
        assert!(matches!(
            model.turn_with_receipt(&conversation(), &serde_json::json!([])),
            Err(OpenAiError::Malformed(_))
        ));
    }

    #[test]
    fn required_attestation_with_no_receipt_is_refused() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let model = model_over(transport, required())
            .with_identities(holding(expected_source().repo_commit()));
        let failed = model
            .turn_with_receipt(&conversation(), &serde_json::json!([]))
            .expect_err("a required-but-missing receipt is a refusal");
        assert!(matches!(failed, OpenAiError::Unattested(_)));
        assert!(failed.to_string().contains("attestation"));
    }

    #[test]
    fn required_attestation_with_a_matching_receipt_succeeds() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let transport =
            ScriptedTransport::new(Ok((200, body.to_string(), with_receipt("receipt-abc123"))));
        let model = model_over(transport, required())
            .with_identities(holding(expected_source().repo_commit()))
            .with_attestation_transport(receipts(false));
        let (reply, receipt) = model
            .turn_with_receipt(&conversation(), &serde_json::json!([]))
            .expect("a matching receipt under Required succeeds");
        assert_eq!(reply.say, "hi");
        assert_eq!(
            receipt.expect("a receipt").identifier.as_str(),
            "receipt-abc123"
        );
    }

    /// `turn` must carry the receipt onto the reply rather than drop it.
    #[test]
    fn turn_carries_the_receipt_onto_the_reply() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let transport =
            ScriptedTransport::new(Ok((200, body.to_string(), with_receipt("receipt-abc123"))));
        let model = model_over(transport, required())
            .with_identities(holding(expected_source().repo_commit()))
            .with_attestation_transport(receipts(false));
        let reply = model
            .turn(&conversation(), &serde_json::json!([]))
            .expect("a matching receipt under Required succeeds");
        assert_eq!(
            reply.receipt.expect("the receipt survives turn").as_str(),
            "receipt-abc123"
        );
    }

    /// The reply names the keyset it was served under, which must be the one its receipt
    /// names.
    #[test]
    fn a_reply_served_under_another_keyset_than_its_receipt_is_refused() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let headers = Headers {
            keyset: Some(format!("sha256:{}", "ee".repeat(32))),
            ..with_receipt("receipt-abc123")
        };
        let model = model_over(
            ScriptedTransport::new(Ok((200, body.to_string(), headers))),
            required(),
        )
        .with_identities(holding(expected_source().repo_commit()))
        .with_attestation_transport(receipts(false));
        assert!(matches!(
            model.turn_with_receipt(&conversation(), &serde_json::json!([])),
            Err(OpenAiError::AttestationFailed(
                AttestationError::ServedKeysetMismatch { .. }
            ))
        ));
    }

    /// A gateway serving another source is refused before the job reaches it.
    #[test]
    fn a_wrong_source_refuses_the_job_naming_both() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let transport =
            ScriptedTransport::new(Ok((200, body.to_string(), with_receipt("receipt-abc123"))));
        let wrong = "cd".repeat(20);
        let model = model_over(transport, required())
            .with_identities(holding(&wrong))
            .with_attestation_transport(receipts(false));
        let failed = model
            .turn_with_receipt(&conversation(), &serde_json::json!([]))
            .expect_err("a wrong repo_commit is a refusal");
        match failed {
            OpenAiError::SourceMismatch { expected, actual } => {
                assert_eq!(expected, expected_source().repo_commit());
                assert_eq!(actual.as_deref(), Some(wrong.as_str()));
            }
            other => panic!("expected SourceMismatch, got {other:?}"),
        }
        assert!(!sent(&model), "the job was never sent");
    }

    /// The provider declares attestation, so a report that does not establish its identity
    /// refuses the job: it is never sent there, and the refusal says why.
    #[test]
    fn a_report_that_does_not_verify_refuses_the_job_before_it_is_sent() {
        let report = attestation::testkit::report(&keyset(), expected_source().repo_commit());
        let tampered = report.replacen(
            "\"workload_keyset_digest\":\"sha256:",
            "\"workload_keyset_digest\":\"sha256:ff",
            1,
        );
        for (served, refused) in [
            (report, "the quote's layout"),
            (tampered, "the keyset digest"),
        ] {
            let body = ok_body(r#"{"say": "hi", "calls": []}"#);
            let transport =
                ScriptedTransport::new(Ok((200, body.to_string(), with_receipt("receipt-abc123"))));
            let model = model_over(transport, required())
                .with_attestation_transport(Scripted::always(Ok(Resp::new(200, served))));
            let failed = model
                .turn_with_receipt(&conversation(), &serde_json::json!([]))
                .expect_err("an unverified gateway is refused");
            match (&failed, refused) {
                (
                    OpenAiError::AttestationFailed(AttestationError::QuoteLayout(_)),
                    "the quote's layout",
                )
                | (
                    OpenAiError::AttestationFailed(AttestationError::KeysetDigestMismatch {
                        ..
                    }),
                    "the keyset digest",
                ) => {}
                other => panic!("expected a refusal over {refused}, got {other:?}"),
            }
            assert!(!sent(&model), "the job was never sent");
        }
    }

    /// A receipt is credential-bound, so it is fetched with the provider's key; its signature
    /// gates the reply.
    #[test]
    fn the_receipt_is_fetched_with_the_key_and_its_signature_gates_the_turn() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        for tamper in [false, true] {
            let attest = receipts(tamper);
            let transport =
                ScriptedTransport::new(Ok((200, body.to_string(), with_receipt("receipt-abc123"))));
            let model = model_over(transport, required())
                .with_identities(holding(expected_source().repo_commit()))
                .with_attestation_transport(Arc::clone(&attest));
            let outcome = model.turn_with_receipt(&conversation(), &serde_json::json!([]));
            if tamper {
                assert!(matches!(
                    outcome,
                    Err(OpenAiError::AttestationFailed(AttestationError::Signature(
                        attestation::SignatureError::BadSignature { .. }
                    )))
                ));
            } else {
                outcome.expect("a signed receipt under Required succeeds");
            }
            let sent = attest.sent();
            assert_eq!(sent.len(), 1, "the receipt alone; the identity was held");
            assert_eq!(
                sent[0].header_value("authorization"),
                Some(format!("Bearer {}", model.key.expose()).as_str())
            );
        }
    }

    /// Every reply mints a fresh receipt, and every receipt is fetched; the report is not,
    /// while the keyset the receipts name is one already established.
    #[test]
    fn turns_under_one_keyset_fetch_receipts_and_never_the_report_again() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let identities = holding(expected_source().repo_commit());
        let attest = receipts(false);
        for receipt in ["receipt-one", "receipt-two"] {
            let transport =
                ScriptedTransport::new(Ok((200, body.to_string(), with_receipt(receipt))));
            model_over(transport, required())
                .with_identities(Arc::clone(&identities))
                .with_attestation_transport(Arc::clone(&attest))
                .turn_with_receipt(&conversation(), &serde_json::json!([]))
                .expect("each turn resolves its own receipt");
        }
        let urls: Vec<String> = attest.sent().into_iter().map(|req| req.url).collect();
        assert_eq!(
            urls,
            vec![
                format!("{BASE}/aci/receipts/receipt-one"),
                format!("{BASE}/aci/receipts/receipt-two"),
            ]
        );
    }

    #[test]
    fn turn_reports_no_receipt_when_unclaimed_and_none_came() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let model = model_over(transport, Attestation::Unclaimed);
        let reply = model
            .turn(&conversation(), &serde_json::json!([]))
            .expect("no receipt under Unclaimed is not a failure");
        assert!(reply.receipt.is_none());
    }

    #[test]
    fn unclaimed_attestation_with_no_receipt_is_ordinary() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let model = model_over(transport, Attestation::Unclaimed);
        let (reply, receipt) = model
            .turn_with_receipt(&conversation(), &serde_json::json!([]))
            .expect("no receipt under Unclaimed is not a failure");
        assert_eq!(reply.say, "hi");
        assert!(receipt.is_none());
    }

    #[test]
    fn the_request_carries_two_messages_and_no_tools_key() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let model = model_over(transport, Attestation::Unclaimed);
        let _ = model
            .turn_with_receipt(&conversation(), &serde_json::json!([]))
            .expect("a normal reply is not an error");
        let sent = model
            .http
            .captured
            .lock()
            .expect("no poisoned lock in a single-threaded test")
            .clone()
            .expect("the transport was called");
        assert_eq!(
            sent.get("model").and_then(Value::as_str),
            Some("deepseek-v4-flash")
        );
        assert!(sent.get("tools").is_none(), "{sent:?}");
        assert!(sent.get("temperature").is_none(), "{sent:?}");
        let messages = sent
            .get("messages")
            .and_then(Value::as_array)
            .expect("messages is an array");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
    }

    /// No socket, so this proves only that the configured timeout reaches the transport.
    #[test]
    fn with_timeout_reaches_the_transport() {
        let body = ok_body(r#"{"say": "hi", "calls": []}"#);
        let transport = ScriptedTransport::new(Ok((200, body.to_string(), Headers::default())));
        let configured = Duration::from_secs(17);
        let model = model_over(transport, Attestation::Unclaimed).with_timeout(configured);
        let _ = model
            .turn_with_receipt(&conversation(), &serde_json::json!([]))
            .expect("a normal reply is not an error");
        let captured = *model
            .http
            .captured_timeout
            .lock()
            .expect("no poisoned lock in a single-threaded test");
        assert_eq!(captured, Some(configured));
    }

    #[test]
    fn name_carries_the_configured_model() {
        let transport = ScriptedTransport::new(Err("unused".to_owned()));
        let model = model_over(transport, Attestation::Unclaimed);
        assert!(model.name().contains("deepseek-v4-flash"));
    }

    #[test]
    fn model_error_conversion_keeps_unreachable_distinct() {
        let transport = ScriptedTransport::new(Err("connection refused".to_owned()));
        let model = model_over(transport, Attestation::Unclaimed);
        let failed = model
            .turn(&conversation(), &serde_json::json!([]))
            .expect_err("an unreachable transport is a failure");
        assert!(matches!(failed, ModelError::Unreachable(_)));
    }
}
