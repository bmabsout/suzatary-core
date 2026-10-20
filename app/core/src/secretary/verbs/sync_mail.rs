use schemars::JsonSchema;
use serde::Deserialize;

use crate::mail::sync_now::{self, Report, SyncNow};
use crate::refusal::{Refusal, shown};
use crate::secretary::verb_answer::VerbAnswer;
use crate::secretary::{Call, SecretarySurface};
use crate::verb::Verb;

/// Start the timer's mail sync now and answer its report.
pub struct SyncMail;

#[derive(Deserialize, JsonSchema)]
pub struct SyncMailArgs {
    account: Option<String>,
}

impl Verb for SyncMail {
    type Surface = SecretarySurface;
    type Args = SyncMailArgs;
    type Answer = SyncNow;
    const NAME: &'static str = "sync_mail";
    const DESCRIPTION: &'static str = "Sync the mail NOW instead of waiting for the five-minute \
        timer: the timer's own sync, started early, so it reaches no server the timer does not. \
        Use it after the owner sends something, to confirm the copy is in Sent, or when they say \
        mail has just arrived.\n\nIt answers the sync's report: each account with `ok` and the \
        last line it said, and what `notmuch new` found. `sync` says which sync that is: `fresh` \
        (one finished in the last minute, so none was started), `ran` (started and finished), or \
        `pending` (started, not finished within the wait; ask again in a minute). Every account \
        is pulled whichever you name: `account` narrows the answer to that one and must be one \
        of this deployment's accounts. Then call `search` (for example `folder:Sent` and the \
        subject) to see what arrived.";

    fn run(call: &Call<'_>, args: SyncMailArgs) -> Result<SyncNow, Refusal> {
        let account = args.account.as_deref().map(str::trim);
        if let Some(asked) = account {
            let roster = call.caps.mail.accounts();
            if !roster.iter().any(|known| known.eq_ignore_ascii_case(asked)) {
                return Err(Refusal::bad_request(format!(
                    "account must be one of this deployment's accounts ({}), got {}",
                    roster.join(", "),
                    shown(asked)
                )));
            }
        }
        let now = call.caps.clock.now();
        let last = call
            .caps
            .sync
            .last()
            .map_err(|failed| Refusal::internal(failed.to_string()))?;
        if let Some(synced) = sync_now::fresh(last, now) {
            return Ok(SyncNow::Fresh(Report::of(&synced, account)));
        }
        Ok(
            match call
                .caps
                .sync
                .start(now)
                .map_err(|failed| Refusal::internal(failed.to_string()))?
            {
                Some(synced) => SyncNow::Ran(Report::of(&synced, account)),
                None => sync_now::pending(now),
            },
        )
    }
}

impl From<SyncNow> for VerbAnswer {
    fn from(answer: SyncNow) -> Self {
        Self::json(&answer)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use prodrome_core::event::Actor;
    use prodrome_core::policy::Untrusted;

    use super::*;
    use crate::capability::{Clock, Datetime, MailSync, Nothing, Secretary, SyncFailure, Synced};
    use crate::chains::{ChainName, ChainPolicy, Chains};
    use crate::deployment::Deployment;
    use crate::mail::verbs::{AccountSync, SyncReport};
    use crate::secretary::store::ConversationId;

    /// A sync whose last report finished `age` seconds before the clock, and whose start answers
    /// `started` and counts itself.
    struct Scripted {
        age: Option<i64>,
        started: Option<i64>,
        starts: AtomicUsize,
    }

    fn report(seconds_before_now: i64) -> Synced {
        let now = prodrome_core::fpl::instant_of(Nothing.now());
        Synced {
            at: prodrome_core::fpl::datetime_of(
                now - chrono::TimeDelta::seconds(seconds_before_now),
            )
            .expect("an instant"),
            report: SyncReport {
                unheld: None,
                synced: vec![AccountSync {
                    account: "ada@example.test".to_owned(),
                    outcome: crate::mail::health::Outcome::Succeeded,
                    tail: "Sent: 1 new".to_owned(),
                }],
                notmuch: "Added 1 new message.".to_owned(),
            },
        }
    }

    impl MailSync for Scripted {
        fn last(&self) -> Result<Option<Synced>, SyncFailure> {
            Ok(self.age.map(report))
        }
        fn start(&self, _since: Datetime) -> Result<Option<Synced>, SyncFailure> {
            self.starts.fetch_add(1, Ordering::SeqCst);
            Ok(self.started.map(report))
        }
        fn health(&self) -> Result<crate::mail::health::Healths, crate::store::StoreError> {
            Ok(crate::mail::health::Healths::default())
        }
    }

    fn answered(sync: &Scripted) -> SyncNow {
        let policy = ChainPolicy {
            name: ChainName::new("life").expect("a slug"),
            untrusted: Untrusted::of([Actor::new("triage").expect("an actor")]),
            roster: std::collections::BTreeSet::new(),
            root: crate::cast::keys::root(),
        };
        let deployment = Deployment {
            viewers: std::collections::BTreeSet::new(),
            actors: std::collections::BTreeMap::new(),
            chains: Chains::of(vec![policy.clone()]).expect("one chain"),
            categories: std::collections::BTreeMap::new(),
            app_url: "https://example.test".to_owned(),
            coding: None,
            zone: crate::zone::Zone::UTC,
        };
        let caps = Secretary {
            sync,
            ..Nothing::secretary()
        };
        SyncMail::run(
            &Call {
                caps: &caps,
                deployment: &deployment,
                settables: &crate::settings::Settables::default(),
                chain: &policy,
                chains: &[],
                conversation: &ConversationId::new("default").expect("a slug"),
            },
            SyncMailArgs { account: None },
        )
        .expect("an answer")
    }

    #[test]
    fn a_sync_in_the_last_minute_answers_and_none_is_started() {
        let sync = Scripted {
            age: Some(20),
            started: Some(0),
            starts: AtomicUsize::new(0),
        };
        assert!(matches!(answered(&sync), SyncNow::Fresh(_)));
        assert_eq!(sync.starts.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn an_older_sync_starts_one_and_answers_its_report() {
        let sync = Scripted {
            age: Some(300),
            started: Some(0),
            starts: AtomicUsize::new(0),
        };
        assert!(matches!(answered(&sync), SyncNow::Ran(_)));
        assert_eq!(sync.starts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_start_with_no_report_in_time_is_pending() {
        let sync = Scripted {
            age: None,
            started: None,
            starts: AtomicUsize::new(0),
        };
        assert!(matches!(answered(&sync), SyncNow::Pending { .. }));
        assert_eq!(sync.starts.load(Ordering::SeqCst), 1);
    }
}
