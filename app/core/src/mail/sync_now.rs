//! A mail sync asked for now: the answer, and when the last one is fresh enough to be it.
//!
//! A sync is a join: pulling a server into the maildir twice in a row holds what pulling it once
//! does. So asking again within [`WINDOW`] of a sync is answered by that sync, and starting one
//! is idempotent too, since the unit never runs twice at once.

use serde::Serialize;

use super::verbs::{AccountSync, SyncReport};
use crate::capability::{Datetime, Synced};

/// How recent a sync must be to answer a request for one without starting another.
pub const WINDOW: chrono::TimeDelta = chrono::TimeDelta::seconds(60);

/// What a request for a sync now answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "sync", rename_all = "snake_case")]
pub enum SyncNow {
    /// One finished within [`WINDOW`], so none was started; its report.
    Fresh(Report),
    /// It was started, and this is its report.
    Ran(Report),
    /// It was started and did not report within the wait; the pass carries on without us.
    Pending {
        /// When it was asked for.
        started: String,
        /// What to do next.
        note: &'static str,
    },
}

/// One sync's report, as much of it as the request asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// When it finished.
    pub at: String,
    /// The accounts asked about, each as it synced.
    pub accounts: Vec<AccountSync>,
    /// What `notmuch new` said of the whole pull.
    pub notmuch: String,
    /// Why the sync ran without its hold, when it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unheld: Option<String>,
}

impl Report {
    /// `synced`, narrowed to `account` when one was asked about.
    #[must_use]
    pub fn of(synced: &Synced, account: Option<&str>) -> Self {
        let SyncReport {
            synced: all,
            notmuch,
            unheld,
        } = &synced.report;
        Self {
            at: iso(synced.at),
            accounts: all
                .iter()
                .filter(|row| account.is_none_or(|asked| row.account.eq_ignore_ascii_case(asked)))
                .cloned()
                .collect(),
            notmuch: notmuch.clone(),
            unheld: unheld.clone(),
        }
    }
}

/// The last sync, when it finished within [`WINDOW`] before `now`.
#[must_use]
pub fn fresh(last: Option<Synced>, now: Datetime) -> Option<Synced> {
    let now = prodrome_core::fpl::instant_of(now);
    last.filter(|synced| {
        let age = now - prodrome_core::fpl::instant_of(synced.at);
        age >= chrono::TimeDelta::zero() && age < WINDOW
    })
}

/// The pending answer for a sync asked for at `at`.
#[must_use]
pub fn pending(at: Datetime) -> SyncNow {
    SyncNow::Pending {
        started: iso(at),
        note: "The sync was started and had not reported yet. Ask again in a minute; the \
               answer will be its report.",
    }
}

fn iso(at: Datetime) -> String {
    prodrome_core::fpl::iso(prodrome_core::fpl::instant_of(at))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn at(seconds: i64) -> Datetime {
        let base = chrono::NaiveDate::from_ymd_opt(2026, 10, 3)
            .and_then(|day| day.and_hms_opt(9, 0, 0))
            .expect("an instant");
        prodrome_core::fpl::datetime_of(base + chrono::TimeDelta::seconds(seconds))
            .expect("an instant")
    }

    fn synced(seconds: i64) -> Synced {
        Synced {
            at: at(seconds),
            report: SyncReport {
                unheld: None,
                synced: vec![
                    AccountSync {
                        account: "ada@example.test".to_owned(),
                        outcome: crate::mail::health::Outcome::Succeeded,
                        tail: "3 new".to_owned(),
                    },
                    AccountSync {
                        account: "grace@example.test".to_owned(),
                        outcome: crate::mail::health::Outcome::Failed(
                            crate::connect::Failure::NeedsLogin,
                        ),
                        tail: "needs a sign-in".to_owned(),
                    },
                ],
                notmuch: "Added 3 new messages.".to_owned(),
            },
        }
    }

    proptest! {
        /// A sync is fresh exactly when it finished in the window before now, never after it.
        #[test]
        fn fresh_is_the_window_before_now(finished in -120i64..120, now in -120i64..120) {
            let answered = fresh(Some(synced(finished)), at(now));
            let age = now - finished;
            prop_assert_eq!(answered.is_some(), (0..WINDOW.num_seconds()).contains(&age));
        }
    }

    #[test]
    fn no_sync_is_never_fresh() {
        assert_eq!(fresh(None, at(0)), None);
    }

    #[test]
    fn a_report_narrows_to_the_account_asked_about_and_names_every_one_otherwise() {
        let whole = Report::of(&synced(0), None);
        assert_eq!(whole.accounts.len(), 2);
        let one = Report::of(&synced(0), Some("Grace@Example.test"));
        assert_eq!(one.accounts.len(), 1);
        assert_ne!(
            one.accounts[0].outcome,
            crate::mail::health::Outcome::Succeeded
        );
        assert_eq!(one.notmuch, "Added 3 new messages.");
    }

    #[test]
    fn the_answer_says_which_sync_it_is() {
        let fresh =
            serde_json::to_value(SyncNow::Fresh(Report::of(&synced(0), None))).expect("serialises");
        assert_eq!(fresh["sync"], "fresh");
        assert_eq!(fresh["at"], "2026-10-03T09:00:00");
        let pending = serde_json::to_value(pending(at(0))).expect("serialises");
        assert_eq!(pending["sync"], "pending");
    }
}
