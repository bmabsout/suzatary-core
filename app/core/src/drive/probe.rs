//! The Drive's check: each folder the secretary may read lists, observed into a register as a
//! mail account's syncs are.

use super::{DrivePath, Port};
use crate::capability::DriveFailure;
use crate::connect::Failure;
use crate::mail::health::{Healths, Instant, Outcome};

/// The source the register keys the Drive's results by.
pub const SOURCE: &str = "drive";

/// What listing each of `folders` through `port` at `at` says: a success when every one lists,
/// else a failure naming the first that did not, in the Drive's words. No folder to read is
/// nothing observed, since there is nothing to check.
#[must_use]
pub fn probe(port: &dyn Port, folders: &[DrivePath], at: Instant) -> Healths {
    if folders.is_empty() {
        return Healths::default();
    }
    match folders
        .iter()
        .find_map(|folder| port.list(folder).err().map(|failed| (folder, failed)))
    {
        None => Healths::one(SOURCE, at, Outcome::Succeeded, ""),
        Some((folder, failed)) => Healths::one(
            SOURCE,
            at,
            Outcome::Failed(class(&failed)),
            &words(folder, &failed),
        ),
    }
}

/// A failed listing's class: a CLI that asks for its sign-in needs it again, which a terminal
/// gives; anything else nothing but the next probe mends.
fn class(failed: &DriveFailure) -> Failure {
    match failed {
        DriveFailure::SignedOut(_) => Failure::NeedsLogin,
        DriveFailure::Missing(_) | DriveFailure::Refused(_) | DriveFailure::Unreachable(_) => {
            Failure::Unclassified
        }
    }
}

/// A failed listing in the owner's words, by its kind alone: the CLI's own words name paths on
/// the box, which a line never shows.
fn words(folder: &DrivePath, failed: &DriveFailure) -> String {
    match failed {
        DriveFailure::Missing(_) => format!("{folder} is not in the Drive"),
        DriveFailure::Refused(_) => format!("{folder} could not be listed"),
        DriveFailure::Unreachable(_) => "the Drive did not answer".to_owned(),
        DriveFailure::SignedOut(_) => "the Drive's sign-in has expired".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::health::Reading;

    fn at(minute: u32) -> Instant {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 9)
            .and_then(|day| day.and_hms_opt(9, minute, 0))
            .expect("an instant")
    }

    fn path(text: &str) -> DrivePath {
        text.parse().expect("a path")
    }

    /// Folders that list are a success; one that does not is a failure since the last success,
    /// naming the folder; no folder is nothing observed.
    #[test]
    fn the_probe_lists_every_allowed_folder() {
        use super::super::WritePort as _;
        let drive = super::super::Memory::default();
        drive
            .create_folder(&path("/"), "my-files")
            .expect("a section");
        drive
            .create_folder(&path("/my-files"), "Docs")
            .expect("a folder");
        let docs = [path("/my-files/Docs")];
        let listed = probe(&drive, &docs, at(0));
        assert_eq!(listed.reading(SOURCE), Some(Reading::Ok { at: at(0) }));
        let gone = [path("/my-files/Docs"), path("/my-files/Gone")];
        let failed = listed.join(&probe(&drive, &gone, at(5)));
        let Some(Reading::Failing { since, words, .. }) = failed.reading(SOURCE) else {
            panic!("failing");
        };
        assert_eq!(since, at(0));
        assert_eq!(words, "/my-files/Gone is not in the Drive");
        assert_eq!(probe(&drive, &[], at(9)), Healths::default());
    }

    /// A CLI whose session expired needs its sign-in; a CLI that did not answer is an outage.
    #[test]
    fn a_session_expired_needs_the_sign_in() {
        let docs = [path("/my-files/Docs")];
        let failing = |failed: DriveFailure| {
            probe(&Failing(failed), &docs, at(0))
                .reading(SOURCE)
                .expect("observed")
        };
        assert!(matches!(
            failing(DriveFailure::SignedOut("session expired".to_owned())),
            Reading::Failing { class: Failure::NeedsLogin, ref words, .. }
                if words == "the Drive's sign-in has expired"
        ));
        assert!(matches!(
            failing(DriveFailure::Unreachable("timed out".to_owned())),
            Reading::Failing {
                class: Failure::Unclassified,
                ..
            }
        ));
    }

    /// A Drive whose every listing fails as it was told.
    struct Failing(DriveFailure);

    impl Port for Failing {
        fn list(&self, _: &DrivePath) -> Result<Vec<super::super::DriveEntry>, DriveFailure> {
            Err(self.0.clone())
        }

        fn download(&self, _: &DrivePath) -> Result<Vec<u8>, DriveFailure> {
            Err(self.0.clone())
        }
    }

    /// The CLI's own words, which name the box's paths, never reach the line.
    #[test]
    fn a_failure_is_worded_by_its_kind_alone() {
        let folder = path("/my-files/Docs");
        let said = "the Drive CLI at /opt/drive/bin/proton-drive could not be run";
        for failed in [
            DriveFailure::Missing(said.to_owned()),
            DriveFailure::Refused(said.to_owned()),
            DriveFailure::Unreachable(said.to_owned()),
            DriveFailure::SignedOut(said.to_owned()),
        ] {
            let worded = words(&folder, &failed);
            assert!(!worded.contains("/opt"), "{worded}");
        }
    }
}
