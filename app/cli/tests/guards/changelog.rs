//! The changelog cannot silently rot: it parses, its versions are semver in strictly descending
//! order, its dates never go forwards, its headings are Keep a Changelog's, and its newest
//! release is `app/release.toml`'s, the file `flake.nix` also reads.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("app/cli/../.. is the repository")
        .to_path_buf()
}

/// The tree's release, `app/release.toml`'s version.
fn declared() -> Option<String> {
    let text = std::fs::read_to_string(repo().join("app/release.toml")).ok()?;
    suzatary_core::release::Released::parse(&text)
        .ok()
        .map(|released| released.version.to_string())
}

/// One release heading: `## [1.2.3] - 2026-08-31`.
struct Release {
    version: (u32, u32, u32),
    date: String,
}

fn releases(text: &str) -> Vec<Release> {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("## [")?;
            let (version, rest) = rest.split_once("] - ")?;
            let date = rest.trim();
            let mut parts = version.split('.');
            let mut number = || parts.next()?.parse::<u32>().ok();
            let (major, minor, patch) = (number()?, number()?, number()?);
            if parts.next().is_some() || date.len() != 10 {
                return None;
            }
            Some(Release {
                version: (major, minor, patch),
                date: date.to_owned(),
            })
        })
        .collect()
}

fn changelog() -> Option<String> {
    std::fs::read_to_string(repo().join("CHANGELOG.md")).ok()
}

#[test]
fn it_has_an_unreleased_section() {
    let Some(text) = changelog() else {
        eprintln!("skipped: this checkout has no CHANGELOG.md");
        return;
    };
    assert!(
        text.lines().any(|line| line == "## [Unreleased]"),
        "no `## [Unreleased]` heading — the push gate has nothing to check"
    );
}

/// Versions are strictly descending, so a release cut twice or out of order fails.
#[test]
fn versions_are_strictly_descending() {
    let Some(text) = changelog() else {
        eprintln!("skipped: this checkout has no CHANGELOG.md");
        return;
    };
    let found = releases(&text);
    // One release is enough: the public tree's changelog starts at the public release.
    assert!(!found.is_empty(), "no release parsed");
    for pair in found.windows(2) {
        assert!(
            pair[0].version > pair[1].version,
            "{:?} does not come after {:?}",
            pair[0].version,
            pair[1].version
        );
    }
}

/// Dates do not go forwards as the file goes down, and each one is a real
/// `YYYY-MM-DD`.
#[test]
fn dates_are_non_increasing_and_look_like_dates() {
    let Some(text) = changelog() else {
        eprintln!("skipped: this checkout has no CHANGELOG.md");
        return;
    };
    for release in releases(&text) {
        assert!(
            chrono::NaiveDate::parse_from_str(&release.date, "%Y-%m-%d").is_ok(),
            "{:?} is not a date",
            release.date
        );
    }
    // Lexical order is chronological order for `YYYY-MM-DD`.
    for pair in releases(&text).windows(2) {
        assert!(
            pair[0].date >= pair[1].date,
            "{} is dated before {}",
            pair[0].date,
            pair[1].date
        );
    }
}

/// Section headings come from Keep a Changelog's fixed vocabulary; an invented one is a typo
/// that hides its entries from a reader skimming.
#[test]
fn section_headings_use_the_fixed_vocabulary() {
    const ALLOWED: [&str; 6] = [
        "Added",
        "Changed",
        "Deprecated",
        "Removed",
        "Fixed",
        "Security",
    ];
    let Some(text) = changelog() else {
        eprintln!("skipped: this checkout has no CHANGELOG.md");
        return;
    };
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("### ") {
            assert!(
                ALLOWED.contains(&heading.trim()),
                "{heading:?} is not one of {ALLOWED:?}"
            );
        }
    }
}

/// The newest release is the declared version, so widening a contract without bumping it fails.
#[test]
fn the_newest_release_matches_the_declared_version() {
    let (Some(text), Some(version)) = (changelog(), declared()) else {
        eprintln!("skipped: this checkout has no CHANGELOG.md or no app/release.toml");
        return;
    };
    let newest = releases(&text).into_iter().next().expect("a release");
    let spelled = format!(
        "{}.{}.{}",
        newest.version.0, newest.version.1, newest.version.2
    );
    assert_eq!(
        spelled, version,
        "the newest release and app/release.toml's version disagree"
    );
}

/// The roadmap keeps its Rejected section: a deleted idea gets re-proposed; a
/// rejected one, with its reason, does not.
#[test]
fn the_roadmap_keeps_its_rejected_section() {
    let Ok(text) = std::fs::read_to_string(repo().join("ROADMAP.md")) else {
        eprintln!("skipped: this checkout has no ROADMAP.md");
        return;
    };
    assert!(
        text.contains("Rejected"),
        "ROADMAP.md lost its Rejected section"
    );
}

/// `CLAUDE.md` points at both files, so a reader who starts there finds them.
#[test]
fn the_constitution_points_at_both_files() {
    let Ok(text) = std::fs::read_to_string(repo().join("CLAUDE.md")) else {
        eprintln!("skipped: this checkout has no CLAUDE.md");
        return;
    };
    assert!(text.contains("CHANGELOG.md"), "CLAUDE.md never names it");
    assert!(text.contains("ROADMAP.md"), "CLAUDE.md never names it");
}

/// The tree's release file reads, and its data lines are the cut's print under the header the
/// file holds, which a cut keeps as it is: a binary of an older revision cuts this tree without
/// moving its header's words. A CLI test, not the core's: no library crate reads the release
/// file, so a release moves no library's inputs.
#[test]
fn the_tree_s_release_reads_and_prints_as_itself() {
    let path = repo().join("app/release.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: no {}", path.display());
        return;
    };
    let released = suzatary_core::release::Released::parse(&text).expect("app/release.toml reads");
    assert_eq!(
        released.print_over(&text),
        text,
        "app/release.toml's data is the cut's print, under the header it holds"
    );
}
