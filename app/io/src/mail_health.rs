//! Each account's sync health on disk, as a [`Doc`]: the value and its join are
//! [`suzatary_core::mail::health`]'s. It lives beside the triage heartbeat, box-local and never
//! pushed, because it is this box's account of its own syncs.
//!
//! [`Doc`]: suzatary_core::store::Doc

use std::path::Path;

use suzatary_core::mail::health::Healths;

use crate::docs::{DocCodec, Docs};

/// The register, one document; absent is a box that has synced nothing yet.
pub type Health = Docs<(), Healths>;

/// Its file's name, beside the heartbeat as the triage outcome is.
pub const FILE: &str = "mail-health.json";

impl Health {
    /// The document beside `heartbeat`.
    #[must_use]
    pub fn beside(heartbeat: &Path) -> Self {
        let path = heartbeat.with_file_name(FILE);
        Docs::new("mail health", move |()| path.clone(), &CODEC)
    }
}

const CODEC: DocCodec<(), Healths> = DocCodec {
    print: |(), healths| serde_json::to_vec_pretty(healths).map_err(|failed| failed.to_string()),
    parse: |(), bytes| {
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|failed| failed.to_string())
    },
};

#[cfg(test)]
mod tests {
    use suzatary_core::connect::Failure;
    use suzatary_core::mail::health::{self, Outcome, Reading};
    use suzatary_core::mail::verbs::{AccountSync, SyncReport};
    use suzatary_core::store::Doc as _;

    use super::*;

    fn report(ok: bool) -> SyncReport {
        SyncReport {
            unheld: None,
            synced: vec![AccountSync {
                account: "ada@example.test".to_owned(),
                outcome: if ok {
                    Outcome::Succeeded
                } else {
                    Outcome::Failed(Failure::NeedsLogin)
                },
                tail: if ok { "done" } else { "IMAP: not connected" }.to_owned(),
            }],
            notmuch: String::new(),
        }
    }

    /// Two syncs recorded one after the other read back as their join, from the file.
    #[test]
    fn syncs_recorded_into_the_file_read_back_as_their_join() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let store = Health::beside(&dir.path().join("triage-heartbeat"));
        let at = |hour| {
            chrono::NaiveDate::from_ymd_opt(2026, 10, 1)
                .and_then(|day| day.and_hms_opt(hour, 0, 0))
                .expect("an instant")
        };
        health::record(&store, &report(true), at(1)).expect("recorded");
        health::record(&store, &report(false), at(2)).expect("recorded");
        assert!(dir.path().join(FILE).is_file());
        let read = store.get(&()).expect("readable").expect("written");
        assert_eq!(
            read.reading("ada@example.test"),
            Some(Reading::Failing {
                at: at(2),
                class: Failure::NeedsLogin,
                words: "IMAP: not connected".to_owned(),
                since: at(1),
            })
        );
    }
}
