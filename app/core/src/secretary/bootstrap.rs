//! The rule over the models: when no job's order resolves to a model that answers, the box asks
//! for one itself. The setup agent needs a model to run, so connecting the first one is the one
//! step no model guides.
//!
//! What it owes, and only while nothing answers: the subscription's sign-in, and the Messages
//! API's key while a job orders `anthropic/`, named for the jobs that do (a declared provider's
//! key is the secrets rule's). A
//! sign-in the box holds open is owed its code card whatever answers, until the session ends.
//! Each card's words name the episode, the last instant any model answered, so a model that
//! answers and later stops (an expired token) files the same cards again, and the owner's no
//! stands within one episode.

use crate::mail::health::{Healths, Instant, Reading};
use crate::models::{Backend, Job, ModelEntry, Models};
use crate::secretary::acts::Act;
use crate::secretary::noticed::Owed;
use crate::secretary::sign_in::SignInUrl;
use crate::secretary::store::Proposal;
use crate::settings::{
    ANTHROPIC_API_KEY, CLAUDE_CODE_OAUTH_TOKEN, Declaration, Settable, Settables,
};

/// What the box knows of its models.
#[derive(Clone, Copy)]
pub struct Answering<'a> {
    /// Each job's order.
    pub models: &'a Models,
    /// What a job answers with when its order holds nothing usable.
    pub fallback: &'a ModelEntry,
    /// Each entry's observed calls, keyed by its print (`claude-cli/<model>`).
    pub health: &'a Healths,
    /// Every secret name declared, with what declares it.
    pub settables: &'a Settables,
    /// Whether a declared name resolves.
    pub resolves: &'a dyn Fn(&Settable) -> bool,
}

impl Answering<'_> {
    /// Every entry some job may answer with: each order, then the fallback.
    pub(crate) fn entries(&self) -> Vec<&ModelEntry> {
        let mut entries: Vec<&ModelEntry> = Job::ALL
            .iter()
            .flat_map(|job| self.models.order(*job))
            .chain([self.fallback])
            .collect();
        entries.sort_by_key(ToString::to_string);
        entries.dedup();
        entries
    }

    /// The secret `backend` reads: the subscription's token, the Messages API's key, or the key
    /// its `[[providers]]` entry declares.
    pub(crate) fn credential(&self, backend: &Backend) -> Option<Settable> {
        self.settables
            .declarations()
            .find(|(name, by)| match backend {
                Backend::ClaudeCli => name.name() == CLAUDE_CODE_OAUTH_TOKEN,
                Backend::AnthropicApi => name.name() == ANTHROPIC_API_KEY,
                Backend::Declared(provider) => {
                    by.contains(&Declaration::Provider(provider.clone()))
                }
            })
            .map(|(name, _)| name)
    }

    /// Whether `entry` answers: its latest observed call succeeded or, never observed, the
    /// credential it reads resolves.
    fn answers(&self, entry: &ModelEntry) -> bool {
        match self.health.reading(&entry.to_string()) {
            Some(Reading::Ok { .. }) => true,
            Some(Reading::Failing { .. }) => false,
            None => self
                .credential(&entry.backend)
                .is_some_and(|name| (self.resolves)(&name)),
        }
    }

    /// Whether any job's order resolves to a model that answers.
    #[must_use]
    pub fn any(&self) -> bool {
        self.entries().into_iter().any(|entry| self.answers(entry))
    }

    /// The last instant any model answered: the episode a card belongs to.
    #[must_use]
    pub fn since(&self) -> Option<Instant> {
        self.health.succeeded()
    }

    /// Why the Messages API's key is owed: the entries and jobs that order it. `None` while no
    /// order names `anthropic/`, since an owner who chose the subscription chose no key.
    fn keyed(&self) -> Option<String> {
        let ordering: Vec<String> = self
            .entries()
            .into_iter()
            .filter(|entry| entry.backend == Backend::AnthropicApi)
            .map(|entry| {
                let jobs = Job::ALL
                    .into_iter()
                    .filter(|job| self.models.order(*job).contains(entry))
                    .map(Job::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{entry} answers {jobs} with this key")
            })
            .collect();
        (!ordering.is_empty()).then(|| ordering.join("; "))
    }
}

/// The cards the models owe: each open sign-in's code card, and while nothing answers, the
/// subscription's sign-in and a key for each ordered built-in keyed backend.
#[must_use]
pub fn owed(answering: &Answering<'_>, open: &[SignInUrl]) -> Owed {
    let codes = open
        .iter()
        .map(|url| {
            Owed::one(
                Proposal::Act(Act::SignInCode { url: url.clone() }),
                "a sign-in waits for the code its page shows",
            )
        })
        .fold(Owed::default(), Owed::join);
    if answering.any() {
        return codes;
    }
    let since = answering.since().map(|at| {
        format!(
            "no model has answered since {} UTC",
            at.format("%Y-%m-%d %H:%M")
        )
    });
    let episode = since
        .clone()
        .unwrap_or_else(|| "no model has answered yet".to_owned());
    let connect = Owed::one(
        Proposal::Act(Act::ClaudeSignIn {
            since: since.clone(),
        }),
        format!("{episode}, so nothing can answer a conversation, a review or a title"),
    );
    let keys = answering
        .credential(&Backend::AnthropicApi)
        .and_then(|name| {
            let why = answering.keyed()?;
            Some(Owed::one(
                Proposal::Act(Act::Secret {
                    name: name.name().to_owned(),
                    why: format!("{episode}, and {why}"),
                }),
                format!("{episode}, and the Messages API reads this key"),
            ))
        })
        .unwrap_or_default();
    codes.join(connect).join(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::health::Outcome;
    use crate::secretary::noticed::Noticed;

    fn refused() -> Outcome {
        Outcome::Failed(crate::connect::Failure::KeyRefused)
    }

    fn at(hour: u32) -> Instant {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 7)
            .and_then(|day| day.and_hms_opt(hour, 0, 0))
            .expect("an instant")
    }

    fn entry(spelled: &str) -> ModelEntry {
        ModelEntry::parse(&[], spelled).expect("an entry")
    }

    fn settables() -> Settables {
        Settables::derive(std::iter::empty(), std::iter::empty(), None, &[])
    }

    fn acts(owed: &Owed) -> Vec<Act> {
        owed.iter()
            .filter_map(|Noticed { proposal, .. }| match proposal {
                Proposal::Act(act) => Some(act.clone()),
                _ => None,
            })
            .collect()
    }

    fn reading<'a>(
        models: &'a Models,
        fallback: &'a ModelEntry,
        health: &'a Healths,
        settables: &'a Settables,
        resolves: &'a dyn Fn(&Settable) -> bool,
    ) -> Answering<'a> {
        Answering {
            models,
            fallback,
            health,
            settables,
            resolves,
        }
    }

    /// A deployment whose subscription token resolves and which nothing has called yet answers;
    /// one whose token is missing, or whose model's last call failed, does not, and is owed the
    /// sign-in.
    #[test]
    fn nothing_answers_until_a_model_is_connected() {
        let (models, fallback, settables) =
            (Models::default(), entry("claude-cli/a-model"), settables());
        let none = Healths::default();
        let resolved = |_: &Settable| true;
        let missing = |_: &Settable| false;
        assert!(reading(&models, &fallback, &none, &settables, &resolved).any());
        assert!(!reading(&models, &fallback, &none, &settables, &missing).any());
        let failed = Healths::one("claude-cli/a-model", at(9), refused(), "401");
        assert!(!reading(&models, &fallback, &failed, &settables, &resolved).any());
        let owed = owed(
            &reading(&models, &fallback, &failed, &settables, &missing),
            &[],
        );
        let acts = acts(&owed);
        assert_eq!(
            acts[0],
            Act::ClaudeSignIn { since: None },
            "nothing has ever answered, so the episode is the first"
        );
        assert_eq!(
            acts.len(),
            1,
            "no job orders anthropic/, so no API key is asked beside the subscription: {acts:?}"
        );
    }

    /// An ordered built-in keyed backend owes its key's card too, named for the jobs it answers.
    #[test]
    fn an_ordered_anthropic_entry_owes_its_key() {
        let fallback = entry("claude-cli/a-model");
        let models = Models::default().with_order(Job::Secretary, vec![entry("anthropic/m")]);
        let settables = settables();
        let missing = |_: &Settable| false;
        let owed = owed(
            &reading(
                &models,
                &fallback,
                &Healths::default(),
                &settables,
                &missing,
            ),
            &[],
        );
        let acts = acts(&owed);
        assert_eq!(acts.len(), 2, "{acts:?}");
        assert!(matches!(&acts[1], Act::Secret { name, why }
            if name == ANTHROPIC_API_KEY && why.ends_with("anthropic/m answers secretary with this key")));
    }

    /// Once a model answers nothing is owed but the code of a sign-in still open, and a model that
    /// stops answering later owes the cards of a new episode.
    #[test]
    fn a_model_that_answers_owes_nothing_and_one_that_stops_is_a_new_episode() {
        let (models, fallback, settables) =
            (Models::default(), entry("claude-cli/a-model"), settables());
        let resolved = |_: &Settable| true;
        let open =
            [SignInUrl::parse("https://claude.com/cai/oauth/authorize?state=s").expect("a link")];
        let ok = Healths::one("claude-cli/a-model", at(9), Outcome::Succeeded, "");
        let answering = owed(
            &reading(&models, &fallback, &ok, &settables, &resolved),
            &open,
        );
        assert_eq!(
            acts(&answering),
            vec![Act::SignInCode {
                url: open[0].clone()
            }]
        );
        let lapsed = ok.join(&Healths::one(
            "claude-cli/a-model",
            at(12),
            refused(),
            "expired",
        ));
        let first = Healths::one("claude-cli/a-model", at(8), refused(), "no token");
        let before = owed(
            &reading(&models, &fallback, &first, &settables, &resolved),
            &[],
        );
        let after = owed(
            &reading(&models, &fallback, &lapsed, &settables, &resolved),
            &[],
        );
        assert_ne!(acts(&before), acts(&after), "another episode, another card");
    }
}
