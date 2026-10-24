//! The model over the Anthropic Messages API, with no `claude` subprocess.
//!
//! A separate API key bills to a separate bucket, so the secretary does not compete with an
//! interactive session for the subscription's allowance. The request streams, but `Model::turn`
//! is synchronous, so the stream is read whole by `SseAccumulator`; this crate does not log, so
//! token usage is handed back to the caller through `AnthropicMessages::turn_with_usage`.

pub mod stream_event;

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use suzatary_core::capability::{Conversation, Model, ModelError, Reply, Secret, TokenUsage};
use suzatary_core::excerpt::Excerpt;

use crate::api_error::message_of;
use crate::blocks::{Blocks, Stores};
use crate::http::{Http, HttpError, Req, Ureq};
use crate::prompt::prompt_of;
use stream_event::{Delivered, Delta, Prompted, StreamEvent};

/// The secret this interpreter reads: an Anthropic Console API key, billed
/// separately from the CLI subscription's weekly allowance.
pub const KEY: &str = "ANTHROPIC_API_KEY";

/// The reason string recorded in secretspec's audit log.
pub const REASON: &str = "the secretary answers a conversation over the Messages API";

/// The one endpoint this interpreter calls.
pub const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";

/// Pinned so a server-side default change cannot silently reshape the response.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// A wedged connection must not hold a route's blocking task forever.
const DEFAULT_TIMEOUT: Duration = Duration::from_mins(5);

/// Long enough for a proposal carrying a full email body, short enough to bound a turn's cost.
const DEFAULT_MAX_TOKENS: u32 = 8192;

/// The Anthropic Messages API, called directly over HTTP.
#[derive(Debug, Clone)]
pub struct AnthropicMessages {
    model: String,
    max_tokens: u32,
    key: Secret,
    timeout: Duration,
    http: Ureq,
    stores: Stores,
}

impl AnthropicMessages {
    /// Bind the interpreter to a model id and the key it authenticates with.
    #[must_use]
    pub fn new(model: impl Into<String>, key: Secret) -> Self {
        Self {
            model: model.into(),
            max_tokens: DEFAULT_MAX_TOKENS,
            key,
            timeout: DEFAULT_TIMEOUT,
            http: Ureq::new(),
            stores: Stores::default(),
        }
    }

    /// A different ceiling on the reply's length.
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// A different patience.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Where the photos and files a turn names are read from, so they cross as blocks.
    #[must_use]
    pub fn with_stores(mut self, stores: Stores) -> Self {
        self.stores = stores;
        self
    }

    /// `Model::turn` plus the reported token counts, because this crate does not log and the
    /// caller does.
    ///
    /// # Errors
    /// An unreachable or timed-out endpoint, a non-2xx status, or an `error` event in the stream.
    pub fn turn_with_usage(
        &self,
        conversation: &Conversation,
        tools: &serde_json::Value,
    ) -> Result<(Reply, Option<Usage>), ModelError> {
        let prompt = prompt_of(conversation, tools, crate::prompt::Reads::Files);
        let content = self.content(conversation, prompt.user)?;
        let body = MessagesRequest {
            model: &self.model,
            max_tokens: self.max_tokens,
            system: prompt.system.as_str(),
            stream: true,
            messages: vec![MessageParam {
                role: "user",
                content,
            }],
        };

        let resp = self
            .http
            .send(
                Req::json(MESSAGES_URL, &body)
                    .header("x-api-key", self.key.expose())
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .timeout(self.timeout),
            )
            .map_err(|failed| match failed {
                HttpError::TimedOut(_) => ModelError::TimedOut(self.timeout),
                other => ModelError::Unreachable(other.to_string()),
            })?;
        if resp.status == 429 {
            return Err(ModelError::Failed(throttled_message(
                resp.header("retry-after"),
            )));
        }
        if !resp.ok() {
            return Err(error_of(resp.status, &resp.text()));
        }
        let mut accumulator = SseAccumulator::new();
        accumulator.feed(&resp.body)?;
        Ok((Reply::parse(accumulator.text()), accumulator.usage()))
    }
}

impl AnthropicMessages {
    /// The user message's content: the text alone, or the turns' blocks and then the text.
    fn content(
        &self,
        conversation: &Conversation,
        user: suzatary_core::for_model::ForModel,
    ) -> Result<Value, ModelError> {
        let blocks = Blocks::of(&self.stores, &conversation.turns)?;
        Ok(if blocks.is_empty() {
            Value::String(user.into_string())
        } else {
            Value::Array(blocks.content(user.as_str()))
        })
    }
}

impl Model for AnthropicMessages {
    fn name(&self) -> String {
        format!("anthropic-api/{}", self.model)
    }

    fn turn(
        &self,
        conversation: &Conversation,
        tools: &serde_json::Value,
    ) -> Result<Reply, ModelError> {
        let (reply, usage) = self.turn_with_usage(conversation, tools)?;
        Ok(reply.with_usage(usage.map(TokenUsage::from)))
    }
}

/// A 429 surfaces the wait rather than retrying, so the caller decides whether to try again.
fn throttled_message(retry_after: Option<&str>) -> String {
    match retry_after {
        Some(seconds) => format!("the API throttled this request (retry after {seconds}s)"),
        None => "the API throttled this request".to_owned(),
    }
}

/// A 4xx/5xx's message as an [`Excerpt`]: nothing here prints a header, so the key cannot leak,
/// and a raw body is quoted only as far as a log line may.
fn error_of(status: u16, body: &str) -> ModelError {
    crate::model::failed(format!(
        "the API answered {status}: {}",
        Excerpt::of(&message_of(body))
    ))
}

/// The Messages API request body.
#[derive(Debug, Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<MessageParam>,
    stream: bool,
}

/// Always one `user` message: the transcript is text this side rendered, not a multi-turn
/// exchange the API replays. Its content is that text, or the blocks and then the text.
#[derive(Debug, Serialize)]
struct MessageParam {
    role: &'static str,
    content: Value,
}

/// Token counts the API reported for one turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Tokens the prompt cost, from the stream's `message_start`.
    pub input_tokens: u64,
    /// Tokens the reply cost, from the stream's (cumulative) `message_delta`.
    pub output_tokens: u64,
    /// Prompt tokens served from Anthropic's prompt cache, from `message_start`; `None` where
    /// it did not say.
    pub cache_read_input_tokens: Option<u64>,
}

impl From<Usage> for TokenUsage {
    fn from(usage: Usage) -> Self {
        Self {
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
            cached_prompt_tokens: usage.cache_read_input_tokens,
        }
    }
}

/// An incremental reader over one streaming response body.
///
/// A frame may span chunks or share one, so it buffers the incomplete tail and acts only on
/// complete frames, making the result independent of how the bytes are chunked.
#[derive(Debug, Default)]
pub struct SseAccumulator {
    buffer: String,
    text: String,
    prompted: Option<Prompted>,
    delivered: Option<Delivered>,
}

impl SseAccumulator {
    /// An accumulator with nothing read yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one chunk of bytes, decoded lossily because a chunk boundary can split a UTF-8
    /// character and only the reassembled buffer is read as text.
    ///
    /// # Errors
    /// The stream reported an `error` event, which ends the turn, or sent a frame whose data is
    /// not the event its `type` names.
    pub fn feed(&mut self, chunk: &[u8]) -> Result<(), ModelError> {
        self.buffer.push_str(&String::from_utf8_lossy(chunk));
        while let Some(boundary) = self.buffer.find("\n\n") {
            let frame = self.buffer[..boundary].to_owned();
            self.buffer.drain(..boundary + 2);
            self.handle_frame(&frame)?;
        }
        Ok(())
    }

    /// The text accumulated so far.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The token counts, once the stream has said both what the prompt and what the reply
    /// cost; `None` before, never a zero standing in for an unsent figure.
    #[must_use]
    pub fn usage(&self) -> Option<Usage> {
        let (prompted, delivered) = (self.prompted?, self.delivered?);
        Some(Usage {
            input_tokens: prompted.input_tokens,
            output_tokens: delivered.output_tokens,
            cache_read_input_tokens: prompted.cache_read_input_tokens,
        })
    }

    /// A frame's data lines, concatenated (the API sends no multi-line `data:`), read as the
    /// event its own `type` names; the `event:` line repeats that name and is not read. A frame
    /// with no data is a comment; one whose data is not an event the API defines fails the turn.
    fn handle_frame(&mut self, frame: &str) -> Result<(), ModelError> {
        let data: String = frame
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim)
            .collect();
        if data.is_empty() {
            return Ok(());
        }
        let event = serde_json::from_str::<StreamEvent>(&data).map_err(|_| {
            ModelError::Failed(format!(
                "the model's stream sent a frame this box cannot read: {}",
                Excerpt::of(&data)
            ))
        })?;
        match event {
            StreamEvent::ContentBlockDelta {
                delta: Delta::TextDelta { text },
            } => self.text.push_str(&text),
            StreamEvent::MessageStart { message } => self.prompted = Some(message.usage),
            StreamEvent::MessageDelta { usage } => self.delivered = Some(usage),
            StreamEvent::Error { error } => {
                return Err(ModelError::Failed(format!(
                    "the model's stream reported an error: {}",
                    error.message
                )));
            }
            StreamEvent::ContentBlockDelta {
                delta: Delta::Other,
            }
            | StreamEvent::Other => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use suzatary_core::for_model::ForModel;

    /// A PDF a verb handed over crosses as a document block, before the text.
    #[test]
    fn a_handed_over_pdf_is_a_document_block_in_the_user_message() {
        use suzatary_core::capability::Turn;
        use suzatary_core::enclosed::{Enclosed, Enclosure};
        let dir = tempfile::tempdir().expect("scratch");
        let files = crate::blobs::Blobs::open(dir.path().to_path_buf());
        let id = files.put(b"%PDF-1.7\n").expect("kept");
        let conversation = Conversation {
            system: String::new(),
            turns: vec![Turn::Tool {
                verb: "drive_read".into(),
                answer: "{}".to_owned(),
                outcome: suzatary_core::secretary::outcome::Outcome::Answered,
                files: vec![Enclosed {
                    id,
                    name: "lease.pdf".to_owned(),
                    media: Enclosure::Pdf,
                    size: 9,
                    text: String::new(),
                }],
            }],
        };
        let api = AnthropicMessages::new("m", Secret::new("k".to_owned())).with_stores(Stores {
            files: Some(files),
            ..Stores::default()
        });
        let content = api
            .content(&conversation, ForModel::of("the transcript"))
            .expect("built");
        assert_eq!(content[0]["type"], "document");
        assert_eq!(content[0]["source"]["media_type"], "application/pdf");
        assert_eq!(content[1]["text"], "the transcript");
        let plain = AnthropicMessages::new("m", Secret::new("k".to_owned()))
            .content(
                &Conversation {
                    system: String::new(),
                    turns: Vec::new(),
                },
                ForModel::of("only text"),
            )
            .expect("built");
        assert_eq!(plain, Value::from("only text"));
    }

    fn delta(text: &str) -> String {
        format!(
            "event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":\"{text}\"}}}}\n\n"
        )
    }

    #[test]
    fn a_frame_split_across_chunks_still_accumulates() {
        let mut accumulator = SseAccumulator::new();
        let whole = delta("split");
        let midpoint = whole.len() / 2;
        accumulator
            .feed(&whole.as_bytes()[..midpoint])
            .expect("half a frame feeds with nothing to show yet");
        assert_eq!(accumulator.text(), "");
        accumulator
            .feed(&whole.as_bytes()[midpoint..])
            .expect("the second half completes the frame");
        assert_eq!(accumulator.text(), "split");
    }

    #[test]
    fn an_error_event_is_a_failure_naming_the_message() {
        let mut accumulator = SseAccumulator::new();
        let failed = accumulator
            .feed(b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"the API is overloaded\"}}\n\n")
            .expect_err("an error event must not be swallowed");
        assert!(failed.to_string().contains("the API is overloaded"));
    }

    #[test]
    fn usage_folds_from_start_and_delta_events() {
        let mut accumulator = SseAccumulator::new();
        assert!(accumulator.usage().is_none());
        accumulator
            .feed(
                b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":100}}}\n\n",
            )
            .expect("message_start feeds cleanly");
        accumulator
            .feed(b"event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{},\"usage\":{\"output_tokens\":42}}\n\n")
            .expect("message_delta feeds cleanly");
        let usage = accumulator.usage().expect("both events reported usage");
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 42);
        assert_eq!(
            usage.cache_read_input_tokens, None,
            "not said, so not a zero"
        );
    }
}
