//! The morning briefing saved as a file, `briefings/YYYY-MM-DD.typ`: typst markup with no
//! actor and no chain event, trusted the way the CLI is because only a human-directed session
//! writes one (see `suzatary_core::capability::Briefings`).

use std::path::PathBuf;

use crate::present::Present as _;
use chrono::NaiveDate;
use suzatary_core::capability::{BriefingError, Briefings};

/// A `NaiveDate` always prints as exactly this many characters, none of them `/`, which is what
/// makes [`BriefingFiles::read`] safe against a traversal.
const DATE_LEN: usize = "YYYY-MM-DD".len();

/// The briefing directory this deployment reads from and writes to.
#[derive(Debug, Clone)]
pub struct BriefingFiles {
    dir: PathBuf,
}

impl BriefingFiles {
    /// The store over `dir`, ordinarily the data tree's `briefings/`.
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl Briefings for BriefingFiles {
    fn all(&self) -> Result<Vec<(NaiveDate, String)>, BriefingError> {
        let unreadable = |err: std::io::Error| BriefingError::Unreadable(err.to_string());
        // No directory yet is no briefings yet.
        let Some(entries) = std::fs::read_dir(&self.dir).present().map_err(unreadable)? else {
            return Ok(Vec::new());
        };
        let mut rows: Vec<(NaiveDate, String)> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(unreadable)?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("typ") {
                continue;
            }
            // A stray file is skipped, so it cannot take the whole listing down.
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(day) = NaiveDate::parse_from_str(stem, "%Y-%m-%d") else {
                continue;
            };
            // A file that vanishes between the listing and the read loses one entry, not all.
            let Some(source) = std::fs::read_to_string(&path)
                .present()
                .map_err(unreadable)?
            else {
                continue;
            };
            rows.push((day, source));
        }
        // Newest first; how many a page gets is the route's decision about a response size.
        rows.sort_by_key(|(day, _)| std::cmp::Reverse(*day));
        Ok(rows)
    }

    fn read(&self, day: NaiveDate) -> Result<String, BriefingError> {
        // A parsed `NaiveDate` formats to `DATE_LEN` characters with no `/`, so no path this
        // builds leaves `self.dir`.
        let stamp = day.format("%Y-%m-%d").to_string();
        debug_assert_eq!(stamp.len(), DATE_LEN);
        let path = self.dir.join(format!("{stamp}.typ"));
        std::fs::read_to_string(&path)
            .present()
            .map_err(|err| BriefingError::Unreadable(err.to_string()))?
            .ok_or(BriefingError::Absent(stamp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "suzatary-io-briefings-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn write(dir: &std::path::Path, stem: &str, source: &str) {
        std::fs::create_dir_all(dir).expect("dir exists");
        std::fs::write(dir.join(format!("{stem}.typ")), source).expect("writes");
    }

    #[test]
    fn a_listing_is_newest_first() {
        let dir = scratch("listing");
        write(&dir, "2026-09-01", "= Day one");
        write(&dir, "2026-09-03", "= Day three");
        write(&dir, "2026-09-02", "= Day two");
        let files = BriefingFiles::new(dir.clone());
        let all = files.all().expect("reads");
        let dates: Vec<NaiveDate> = all.iter().map(|(d, _)| *d).collect();
        assert_eq!(
            dates,
            vec![
                NaiveDate::from_ymd_opt(2026, 9, 3).expect("valid"),
                NaiveDate::from_ymd_opt(2026, 9, 2).expect("valid"),
                NaiveDate::from_ymd_opt(2026, 9, 1).expect("valid"),
            ]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stem_that_is_not_a_date_is_skipped() {
        let dir = scratch("bad-stem");
        write(&dir, "2026-09-01", "= Day one");
        write(&dir, "not-a-date", "= Stray");
        let files = BriefingFiles::new(dir.clone());
        let all = files.all().expect("reads");
        assert_eq!(all.len(), 1);
        assert_eq!(
            all[0].0,
            NaiveDate::from_ymd_opt(2026, 9, 1).expect("valid")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_day_is_absent() {
        let dir = scratch("missing");
        let files = BriefingFiles::new(dir.clone());
        let day = NaiveDate::from_ymd_opt(2026, 9, 7).expect("valid");
        assert!(matches!(files.read(day), Err(BriefingError::Absent(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_source_comes_back_whole() {
        let dir = scratch("whole");
        let source = "= Morning\n\nSome #strong[bold] text.\n";
        write(&dir, "2026-09-07", source);
        let files = BriefingFiles::new(dir.clone());
        let day = NaiveDate::from_ymd_opt(2026, 9, 7).expect("valid");
        assert_eq!(files.read(day).expect("reads"), source);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
