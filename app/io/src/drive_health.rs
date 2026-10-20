//! The Drive's listings on disk, as a [`Doc`]: the value and its join are
//! [`suzatary_core::mail::health`]'s, keyed by [`suzatary_core::drive::SOURCE`]. It lives beside
//! the mail health, box-local and never pushed, because it is this box's account of its own
//! probes.
//!
//! [`Doc`]: suzatary_core::store::Doc

use std::path::Path;

use suzatary_core::mail::health::Healths;

use crate::docs::{DocCodec, Docs};

/// The register, one document; absent is a box that has probed nothing yet.
pub type Health = Docs<(), Healths>;

/// Its file's name, beside the heartbeat as the mail health is.
pub const FILE: &str = "drive-health.json";

/// The document beside `heartbeat`.
#[must_use]
pub fn beside(heartbeat: &Path) -> Health {
    let path = heartbeat.with_file_name(FILE);
    Docs::new("drive health", move |()| path.clone(), &CODEC)
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
    use suzatary_core::store::Doc as _;

    use super::*;

    /// Two probes joined into the file read back as their join.
    #[test]
    fn probes_joined_into_the_file_read_back_as_their_join() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let store = beside(&dir.path().join("triage-heartbeat"));
        let at = |hour| {
            chrono::NaiveDate::from_ymd_opt(2026, 10, 9)
                .and_then(|day| day.and_hms_opt(hour, 0, 0))
                .expect("an instant")
        };
        let drive = suzatary_core::drive::SOURCE;
        health::join_into(&store, &Healths::one(drive, at(1), Outcome::Succeeded, ""))
            .expect("joined");
        health::join_into(
            &store,
            &Healths::one(
                drive,
                at(2),
                Outcome::Failed(Failure::Unclassified),
                "no session",
            ),
        )
        .expect("joined");
        assert!(dir.path().join(FILE).is_file());
        let read = store.get(&()).expect("readable").expect("written");
        assert_eq!(
            read.reading(drive),
            Some(Reading::Failing {
                at: at(2),
                class: Failure::Unclassified,
                words: "no session".to_owned(),
                since: at(1),
            })
        );
    }
}
