//! `suzatary-dev bench run`: replay a case through a candidate, without scoring.
//!
//! [`replay`] hands a candidate [`Model`] the manual and the case's input and dispatches every
//! call through the real agent surface until it stops asking; a real [`Model`] already builds its
//! prompt inside `turn`, so this loop builds none of its own.
//!
//! The candidate never sees [`crate::cases::Truths`]: `opening` is built from the case's input
//! alone.
//!
//! A throttle is not an answer about the case: [`ModelError`] flattens it, so [`Throttling`] is
//! the type that tells it apart, and `ask_with_backoff` retries it rather than failing the case.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use prodrome_core::event::Hash;
use serde::{Deserialize, Serialize};
use suzatary_core::agent;
use suzatary_core::capability::{
    Agent, Conversation, Model, ModelError, ReceiptId, Reply, TokenUsage, Turn,
};
use suzatary_core::deployment::Deployment;

use crate::cases::CaseId;

/// The round bound: a candidate that keeps asking stops, and the last round records what it said.
pub const MAX_ROUNDS: u32 = 8;

/// The default `--throttle-attempts`, whose five waits sum to about twelve and a half minutes.
pub const DEFAULT_THROTTLE_ATTEMPTS: u32 = 6;

/// One try at a round, typed enough to tell a throttle apart from a real failure.
#[derive(Debug)]
pub enum Attempt {
    /// The candidate answered.
    Answered(Reply),
    /// The provider asked for time rather than answering, so the round is retried.
    Throttled {
        /// The provider's own wait, honoured over the backoff schedule when present.
        retry_after: Option<Duration>,
    },
    /// Anything else [`Model::turn`] answered, which ends the case.
    Failed(ModelError),
}

/// A [`Model`] that can say when a reply was a throttle.
///
/// The default never answers `Throttled`; a model holding the unflattened error overrides it, and
/// each model opts in explicitly so such an override does not conflict with a blanket impl.
pub trait Throttling: Model {
    /// One try; the default is a plain [`Model::turn`].
    fn attempt(&self, conversation: &Conversation, tools: &serde_json::Value) -> Attempt {
        match self.turn(conversation, tools) {
            Ok(reply) => Attempt::Answered(reply),
            Err(error) => Attempt::Failed(error),
        }
    }
}

/// One verb call the candidate made, dispatched, and what came back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallRecord {
    /// The verb's name as the candidate spelled it, recorded even when unknown.
    pub verb: String,
    /// Its arguments, as the candidate supplied them.
    pub args: BTreeMap<String, serde_json::Value>,
    /// What the real dispatch answered.
    pub outcome: CallOutcome,
}

/// The two kinds of "no" `suzatary_core::agent::call` draws, kept apart for the scorer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum CallOutcome {
    /// The verb ran; an `ok: false` body is carried in `value` unchanged.
    Answered {
        /// Whatever the verb answered.
        value: serde_json::Value,
    },
    /// The call was refused before a handler ran.
    Refused {
        /// The refusal's words, which teach the right shape.
        message: String,
    },
}

/// One round: what the candidate said, and every call it asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundRecord {
    /// What the candidate said to the owner this round. May be empty.
    pub say: String,
    /// The calls it asked for, in order, each with its outcome.
    pub calls: Vec<CallRecord>,
    /// The provider's receipt for this round's response, kept per round because each round is
    /// separately attested.
    pub receipt: Option<ReceiptId>,
}

/// What a replay produced for one case, never the incumbent's answer nor a message body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    /// Which model answered, so a run directory mixing candidates says who wrote which file.
    pub model: String,
    /// Every round, in order.
    pub rounds: Vec<RoundRecord>,
    /// The objects produced, as the difference of every chain's objects before and after, since
    /// the verbs' answers differ in shape; empty means the candidate wrote nothing.
    pub objects: Vec<String>,
    /// Token usage summed across rounds, `None` when no round reported any rather than a zero.
    pub usage: Option<TokenUsage>,
}

/// One case's outcome: whether it was answered at all.
///
/// A closed sum, so a transport failure never reads as a candidate declining, and resume can ask
/// it again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum Outcome {
    /// The candidate answered — every round it took, even an empty one.
    Answered(Answer),
    /// [`Model::turn`] failed before this case reached an [`Answer`].
    Failed {
        /// What [`suzatary_core::capability::ModelError`] said.
        error: String,
    },
}

/// One case's record as a run directory holds it, keyed by [`CaseId`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEntry {
    /// Which case this answers.
    pub case: CaseId,
    /// When this replay ran, as an RFC 3339 stamp.
    pub recorded_at: String,
    /// What happened: an answer, or a failure that never reached one.
    pub outcome: Outcome,
}

/// Replays one case against `model`, dispatching every call through the real agent surface.
///
/// A call naming a chain missing from `chains` is refused; a model failure ends the case as
/// [`Outcome::Failed`], after retrying throttles through `sleep` up to `throttle_attempts` times.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn replay<M>(
    model: &M,
    chains: &BTreeMap<String, Agent<'_>>,
    deployment: &Deployment,
    system: &str,
    opening: String,
    case: &str,
    throttle_attempts: u32,
    sleep: &dyn Fn(Duration),
) -> Outcome
where
    M: Throttling,
{
    let tools = agent::tools();
    let before = objects_of(chains);

    let mut turns = vec![Turn::Person {
        text: opening,
        references: Vec::new(),
        attachments: Vec::new(),
    }];
    let mut rounds = Vec::new();
    let mut usages: Vec<Option<TokenUsage>> = Vec::new();
    let mut round_no = 0u32;
    loop {
        round_no += 1;
        let asking = Conversation {
            system: system.to_owned(),
            turns: turns.clone(),
        };
        let reply: Reply =
            match ask_with_backoff(model, &asking, &tools, case, throttle_attempts, sleep) {
                Ok(reply) => reply,
                Err(failed) => {
                    return Outcome::Failed {
                        error: failed.to_string(),
                    };
                }
            };
        usages.push(reply.usage);
        turns.push(Turn::Model {
            text: reply.say.clone(),
            calls: reply.calls.clone(),
        });
        let mut call_records = Vec::with_capacity(reply.calls.len());
        for call in &reply.calls {
            let args: serde_json::Map<String, serde_json::Value> =
                call.args.clone().into_iter().collect();
            let outcome = dispatch(chains, deployment, call.verb.as_str(), &args);
            turns.push(Turn::Tool {
                verb: call.verb.clone(),
                answer: shown(&outcome),
                // The bench runs the agent's verbs, none of which remembers a fact.
                outcome: match outcome {
                    CallOutcome::Refused { .. } => {
                        suzatary_core::secretary::outcome::Outcome::Refused
                    }
                    CallOutcome::Answered { .. } => {
                        suzatary_core::secretary::outcome::Outcome::Answered
                    }
                },
                files: Vec::new(),
            });
            call_records.push(CallRecord {
                verb: call.verb.to_string(),
                args: call.args.clone(),
                outcome,
            });
        }
        let calls_asked = !reply.calls.is_empty();
        rounds.push(RoundRecord {
            say: reply.say,
            calls: call_records,
            receipt: reply.receipt,
        });
        if !calls_asked || round_no >= MAX_ROUNDS {
            break;
        }
    }

    let after = objects_of(chains);
    let objects = after
        .difference(&before)
        .map(Hash::as_str)
        .map(str::to_owned)
        .collect();
    Outcome::Answered(Answer {
        model: model.name(),
        rounds,
        objects,
        usage: sum_usage(usages.iter()),
    })
}

/// Ask for one round's reply, retrying [`Attempt::Throttled`] with backoff; a throttle on the
/// last try fails at once rather than waiting.
fn ask_with_backoff<M: Throttling>(
    model: &M,
    conversation: &Conversation,
    tools: &serde_json::Value,
    case: &str,
    throttle_attempts: u32,
    sleep: &dyn Fn(Duration),
) -> Result<Reply, ModelError> {
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match model.attempt(conversation, tools) {
            Attempt::Answered(reply) => return Ok(reply),
            Attempt::Failed(error) => return Err(error),
            Attempt::Throttled { retry_after } => {
                if attempt >= throttle_attempts {
                    return Err(ModelError::Failed(format!(
                        "the endpoint throttled this request after {attempt} attempt(s)"
                    )));
                }
                let wait = retry_after.unwrap_or_else(|| backoff_schedule(attempt));
                eprintln!(
                    "case {case}: throttled, waiting {}s (attempt {attempt}/{throttle_attempts})",
                    wait.as_secs()
                );
                sleep(wait);
            }
        }
    }
}

/// The untimed schedule: 30s doubling, capped at 5 minutes.
fn backoff_schedule(attempt: u32) -> Duration {
    let doublings = attempt.saturating_sub(1).min(20);
    let seconds = 30u64.saturating_mul(1u64 << doublings);
    Duration::from_secs(seconds.min(300))
}

/// Sum what was reported across rounds, keeping "no round mentioned caching" apart from zero.
fn sum_usage<'a>(usages: impl Iterator<Item = &'a Option<TokenUsage>>) -> Option<TokenUsage> {
    let mut total = TokenUsage::default();
    let mut any = false;
    let mut any_cached = false;
    for usage in usages.filter_map(Option::as_ref) {
        any = true;
        total.prompt_tokens += usage.prompt_tokens;
        total.completion_tokens += usage.completion_tokens;
        if let Some(cached) = usage.cached_prompt_tokens {
            any_cached = true;
            total.cached_prompt_tokens = Some(total.cached_prompt_tokens.unwrap_or(0) + cached);
        }
    }
    any.then(|| TokenUsage {
        cached_prompt_tokens: any_cached.then_some(total.cached_prompt_tokens.unwrap_or(0)),
        ..total
    })
}

/// One call, dispatched to the real handler over the chain the candidate named.
fn dispatch(
    chains: &BTreeMap<String, Agent<'_>>,
    deployment: &Deployment,
    verb: &str,
    args: &serde_json::Map<String, serde_json::Value>,
) -> CallOutcome {
    match agent::call(chains, deployment, verb, args) {
        Ok(value) => CallOutcome::Answered { value },
        Err(refused) => CallOutcome::Refused {
            message: refused.message().to_owned(),
        },
    }
}

/// A call's outcome as the transcript shows it, so a string does not reach the next round quoted.
fn shown(outcome: &CallOutcome) -> String {
    match outcome {
        CallOutcome::Answered {
            value: serde_json::Value::String(text),
        } => text.clone(),
        CallOutcome::Answered { value } => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
        CallOutcome::Refused { message } => message.clone(),
    }
}

/// Every object every bundle holds; an unreadable chain answers none rather than panicking.
fn objects_of(chains: &BTreeMap<String, Agent<'_>>) -> BTreeSet<Hash> {
    chains
        .values()
        .flat_map(|caps| caps.chain.objects().unwrap_or_default())
        .map(|(hash, _)| hash)
        .collect()
}

#[cfg(test)]
#[allow(clippy::zero_sized_map_values)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use prodrome_core::event::Actor;
    use prodrome_core::policy::Untrusted;
    use suzatary_core::capability::{ModelError, Reply};
    use suzatary_core::chains::{ChainName, ChainPolicy, Chains};
    use suzatary_core::deployment::Deployment;

    use super::*;

    /// A deployment naming one chain, enough for [`agent::chain_of`] to resolve a `chain`
    /// argument.
    fn deployment(chain_name: &str) -> Deployment {
        let policy = ChainPolicy {
            name: ChainName::new(chain_name).expect("a valid chain name"),
            untrusted: Untrusted::none(),
            roster: BTreeSet::<Actor>::new(),
            root: suzatary_core::cast::keys::root(),
        };
        Deployment {
            viewers: BTreeSet::new(),
            actors: BTreeMap::new(),
            chains: Chains::of(vec![policy]).expect("one chain is a valid list"),
            categories: BTreeMap::new(),
            app_url: String::new(),
            coding: None,
            zone: suzatary_core::zone::Zone::UTC,
        }
    }

    /// A [`Model`] that answers from a fixed script, a stand-in since this crate does not depend
    /// on `Scripted`.
    struct Fixed(Vec<&'static str>);

    impl Model for Fixed {
        fn name(&self) -> String {
            "fixed/test".to_owned()
        }
        fn turn(
            &self,
            conversation: &Conversation,
            _tools: &serde_json::Value,
        ) -> Result<Reply, ModelError> {
            let round = conversation
                .turns
                .iter()
                .filter(|t| matches!(t, Turn::Model { .. }))
                .count();
            self.0
                .get(round)
                .map(|text| Reply::parse(text))
                .ok_or_else(|| ModelError::Failed("script exhausted".to_owned()))
        }
    }

    /// `Fixed` never throttles, so the default [`Throttling::attempt`] is right for it.
    impl Throttling for Fixed {}

    /// A no-op `sleep` for tests that do not exercise a throttle.
    fn no_sleep(_: Duration) {}

    /// A reply with no calls ends the loop after one round, and the round is recorded.
    #[test]
    fn a_reply_with_no_calls_ends_the_loop_at_one_round() {
        let model = Fixed(vec![r#"{"say": "declined"}"#]);
        let outcome = replay(
            &model,
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &no_sleep,
        );
        let Outcome::Answered(answer) = outcome else {
            panic!("a reply with no calls is an answer: {outcome:?}");
        };
        assert_eq!(answer.rounds.len(), 1);
        assert_eq!(answer.rounds[0].say, "declined");
        assert!(answer.objects.is_empty());
        assert_eq!(answer.model, "fixed/test");
        assert!(answer.usage.is_none(), "a Fixed candidate reports no usage");
    }

    /// A `Model::turn` error ends the case as [`Outcome::Failed`], never as an [`Answer`].
    #[test]
    fn a_turn_error_is_failed_not_a_prose_answer() {
        struct Unreachable;
        impl Model for Unreachable {
            fn name(&self) -> String {
                "unreachable/test".to_owned()
            }
            fn turn(
                &self,
                _conversation: &Conversation,
                _tools: &serde_json::Value,
            ) -> Result<Reply, ModelError> {
                Err(ModelError::Failed("404 not found".to_owned()))
            }
        }
        impl Throttling for Unreachable {}
        let outcome = replay(
            &Unreachable,
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &no_sleep,
        );
        match outcome {
            Outcome::Failed { error } => assert!(error.contains("404"), "{error}"),
            Outcome::Answered(answer) => {
                panic!("a turn error must not become an answer: {answer:?}")
            }
        }
    }

    /// A call naming a chain this replay holds no bundle for is refused, not a panic.
    #[test]
    fn an_unbundled_chain_is_refused_and_the_loop_still_ends() {
        let model = Fixed(vec![
            r#"{"say": "looking", "calls": [{"verb": "context", "args": {"chain": "nowhere"}}]}"#,
            r#"{"say": "given up"}"#,
        ]);
        let outcome = replay(
            &model,
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &no_sleep,
        );
        let Outcome::Answered(answer) = outcome else {
            panic!("a refused call is still an answer: {outcome:?}");
        };
        assert_eq!(answer.rounds.len(), 2);
        assert_eq!(answer.rounds[0].calls.len(), 1);
        assert!(matches!(
            answer.rounds[0].calls[0].outcome,
            CallOutcome::Refused { .. }
        ));
    }

    /// The round bound stops an unbounded script.
    #[test]
    fn the_round_bound_stops_a_candidate_that_never_finishes() {
        struct Insistent;
        impl Model for Insistent {
            fn name(&self) -> String {
                "insistent/test".to_owned()
            }
            fn turn(
                &self,
                _conversation: &Conversation,
                _tools: &serde_json::Value,
            ) -> Result<Reply, ModelError> {
                Ok(Reply::parse(
                    r#"{"say": "again", "calls": [{"verb": "context", "args": {}}]}"#,
                ))
            }
        }
        impl Throttling for Insistent {}
        let outcome = replay(
            &Insistent,
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &no_sleep,
        );
        let Outcome::Answered(answer) = outcome else {
            panic!("the round bound ends in an answer, not a failure: {outcome:?}");
        };
        assert_eq!(
            u32::try_from(answer.rounds.len()).expect("a small round count"),
            MAX_ROUNDS
        );
    }

    /// A [`Model`] reporting usage, so [`sum_usage`] is tested over `replay`'s loop.
    struct WithUsage(Vec<(&'static str, Option<TokenUsage>)>);

    impl Model for WithUsage {
        fn name(&self) -> String {
            "with-usage/test".to_owned()
        }
        fn turn(
            &self,
            conversation: &Conversation,
            _tools: &serde_json::Value,
        ) -> Result<Reply, ModelError> {
            let round = conversation
                .turns
                .iter()
                .filter(|t| matches!(t, Turn::Model { .. }))
                .count();
            let (text, usage) = self
                .0
                .get(round)
                .ok_or_else(|| ModelError::Failed("script exhausted".to_owned()))?;
            Ok(Reply::parse(text).with_usage(*usage))
        }
    }

    impl Throttling for WithUsage {}

    /// Usage sums across rounds, and a round that reported nothing contributes nothing.
    #[test]
    fn usage_sums_across_rounds() {
        let model = WithUsage(vec![
            (
                r#"{"say": "looking", "calls": [{"verb": "context", "args": {}}]}"#,
                Some(TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 10,
                    cached_prompt_tokens: Some(20),
                }),
            ),
            (
                r#"{"say": "declined"}"#,
                Some(TokenUsage {
                    prompt_tokens: 50,
                    completion_tokens: 5,
                    cached_prompt_tokens: None,
                }),
            ),
        ]);
        let outcome = replay(
            &model,
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &no_sleep,
        );
        let Outcome::Answered(answer) = outcome else {
            panic!("a scripted candidate answers: {outcome:?}");
        };
        let usage = answer.usage.expect("two rounds reported usage");
        assert_eq!(usage.prompt_tokens, 150);
        assert_eq!(usage.completion_tokens, 15);
        // One round reported a cached figure and the other none, and the sum keeps it.
        assert_eq!(usage.cached_prompt_tokens, Some(20));
    }

    /// No round reporting usage is `None`, never a zero.
    #[test]
    fn no_round_reporting_usage_is_none() {
        let outcome = replay(
            &Fixed(vec![r#"{"say": "declined"}"#]),
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &no_sleep,
        );
        let Outcome::Answered(answer) = outcome else {
            panic!("Fixed answers: {outcome:?}");
        };
        assert!(answer.usage.is_none());
    }

    /// A [`Model`] with a receipt on some rounds and not others.
    struct WithReceipt(Vec<(&'static str, Option<&'static str>)>);

    impl Model for WithReceipt {
        fn name(&self) -> String {
            "with-receipt/test".to_owned()
        }
        fn turn(
            &self,
            conversation: &Conversation,
            _tools: &serde_json::Value,
        ) -> Result<Reply, ModelError> {
            let round = conversation
                .turns
                .iter()
                .filter(|t| matches!(t, Turn::Model { .. }))
                .count();
            let (text, receipt) = self
                .0
                .get(round)
                .ok_or_else(|| ModelError::Failed("script exhausted".to_owned()))?;
            Ok(Reply::parse(text).with_receipt(receipt.map(ReceiptId::new)))
        }
    }

    impl Throttling for WithReceipt {}

    /// Every round's receipt is recorded on that round.
    #[test]
    fn every_rounds_receipt_is_recorded_on_that_round() {
        let model = WithReceipt(vec![
            (
                r#"{"say": "looking", "calls": [{"verb": "context", "args": {}}]}"#,
                Some("receipt-1"),
            ),
            (r#"{"say": "declined"}"#, Some("receipt-2")),
        ]);
        let outcome = replay(
            &model,
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &no_sleep,
        );
        let Outcome::Answered(answer) = outcome else {
            panic!("a scripted candidate answers: {outcome:?}");
        };
        assert_eq!(answer.rounds.len(), 2);
        assert_eq!(
            answer.rounds[0]
                .receipt
                .as_ref()
                .expect("round 1's own receipt")
                .as_str(),
            "receipt-1"
        );
        assert_eq!(
            answer.rounds[1]
                .receipt
                .as_ref()
                .expect("round 2's own receipt")
                .as_str(),
            "receipt-2"
        );
    }

    /// No receipt is `None` on that round, never an empty string.
    #[test]
    fn a_round_with_no_receipt_is_none() {
        let outcome = replay(
            &Fixed(vec![r#"{"say": "declined"}"#]),
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &no_sleep,
        );
        let Outcome::Answered(answer) = outcome else {
            panic!("Fixed answers: {outcome:?}");
        };
        assert!(answer.rounds[0].receipt.is_none());
    }

    /// A [`Model`] that answers `Attempt::Throttled` a fixed number of times, then answers.
    struct ThrottleThenAnswer {
        /// How many more `attempt` calls answer `Throttled` before this answers.
        remaining: std::sync::atomic::AtomicU32,
    }

    impl Model for ThrottleThenAnswer {
        fn name(&self) -> String {
            "throttle-then-answer/test".to_owned()
        }
        fn turn(
            &self,
            _conversation: &Conversation,
            _tools: &serde_json::Value,
        ) -> Result<Reply, ModelError> {
            unreachable!("attempt() is overridden and never falls back to turn()")
        }
    }

    impl Throttling for ThrottleThenAnswer {
        fn attempt(&self, _conversation: &Conversation, _tools: &serde_json::Value) -> Attempt {
            let left = self.remaining.load(std::sync::atomic::Ordering::SeqCst);
            if left == 0 {
                Attempt::Answered(Reply::parse(r#"{"say": "done"}"#))
            } else {
                self.remaining
                    .store(left - 1, std::sync::atomic::Ordering::SeqCst);
                Attempt::Throttled { retry_after: None }
            }
        }
    }

    /// A [`Model`] that always answers `Attempt::Throttled`.
    struct AlwaysThrottled;

    impl Model for AlwaysThrottled {
        fn name(&self) -> String {
            "always-throttled/test".to_owned()
        }
        fn turn(
            &self,
            _conversation: &Conversation,
            _tools: &serde_json::Value,
        ) -> Result<Reply, ModelError> {
            unreachable!("attempt() is overridden and never falls back to turn()")
        }
    }

    impl Throttling for AlwaysThrottled {
        fn attempt(&self, _conversation: &Conversation, _tools: &serde_json::Value) -> Attempt {
            Attempt::Throttled { retry_after: None }
        }
    }

    /// A throttle is retried with the untimed schedule's 30s then 60s, and ends as one answer.
    #[test]
    fn a_throttle_is_retried_with_backoff_then_answers() {
        let model = ThrottleThenAnswer {
            remaining: std::sync::atomic::AtomicU32::new(2),
        };
        let waits = std::cell::RefCell::new(Vec::new());
        let sleep = |duration: Duration| waits.borrow_mut().push(duration);
        let outcome = replay(
            &model,
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &sleep,
        );
        let Outcome::Answered(answer) = outcome else {
            panic!("a throttle that clears is still an answer: {outcome:?}");
        };
        // The throttled tries were retries of one round, not rounds of their own.
        assert_eq!(answer.rounds.len(), 1);
        assert_eq!(answer.rounds[0].say, "done");
        assert_eq!(
            *waits.borrow(),
            vec![Duration::from_secs(30), Duration::from_mins(1)]
        );
    }

    /// A throttle that never clears fails after exactly `throttle_attempts` tries and one fewer
    /// wait.
    #[test]
    fn a_throttle_that_never_clears_fails_after_the_attempt_cap() {
        let waits = std::cell::RefCell::new(Vec::new());
        let sleep = |duration: Duration| waits.borrow_mut().push(duration);
        let attempts = 3;
        let outcome = replay(
            &AlwaysThrottled,
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            attempts,
            &sleep,
        );
        match outcome {
            Outcome::Failed { error } => {
                assert!(error.contains(&attempts.to_string()), "{error}");
            }
            Outcome::Answered(answer) => {
                panic!("a throttle that never clears must not become an answer: {answer:?}")
            }
        }
        assert_eq!(waits.borrow().len(), (attempts - 1) as usize);
    }

    /// A [`Model`] whose one throttle carries a `retry_after`.
    struct ThrottleWithRetryAfter {
        /// Whether the one scripted throttle has already been answered.
        thrown: std::sync::atomic::AtomicBool,
    }

    impl Model for ThrottleWithRetryAfter {
        fn name(&self) -> String {
            "throttle-with-retry-after/test".to_owned()
        }
        fn turn(
            &self,
            _conversation: &Conversation,
            _tools: &serde_json::Value,
        ) -> Result<Reply, ModelError> {
            unreachable!("attempt() is overridden and never falls back to turn()")
        }
    }

    impl Throttling for ThrottleWithRetryAfter {
        fn attempt(&self, _conversation: &Conversation, _tools: &serde_json::Value) -> Attempt {
            if self.thrown.swap(true, std::sync::atomic::Ordering::SeqCst) {
                Attempt::Answered(Reply::parse(r#"{"say": "done"}"#))
            } else {
                Attempt::Throttled {
                    retry_after: Some(Duration::from_secs(5)),
                }
            }
        }
    }

    /// The provider's own `retry_after` is honoured over the schedule.
    #[test]
    fn retry_after_is_honoured_over_the_schedule() {
        let model = ThrottleWithRetryAfter {
            thrown: std::sync::atomic::AtomicBool::new(false),
        };
        let waits = std::cell::RefCell::new(Vec::new());
        let sleep = |duration: Duration| waits.borrow_mut().push(duration);
        let outcome = replay(
            &model,
            &BTreeMap::<String, Agent<'_>>::new(),
            &deployment("life"),
            "system prompt",
            "a message needs triage".to_owned(),
            "test-case",
            DEFAULT_THROTTLE_ATTEMPTS,
            &sleep,
        );
        assert!(
            matches!(outcome, Outcome::Answered(_)),
            "a cleared throttle still answers: {outcome:?}"
        );
        assert_eq!(*waits.borrow(), vec![Duration::from_secs(5)]);
    }
}
