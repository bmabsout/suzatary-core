//! A release written into the tree: `app/release.toml` and the changelog's section.

use chrono::NaiveDate;

use super::{Completes, Released, Version};

/// One release as the tree will hold it: the version and what it completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cut {
    /// The version.
    pub version: Version,
    /// What it completed, listed first in its changelog section.
    pub completes: Completes,
    /// A version the tree declares that no tag names (set by hand, never released): its
    /// section becomes this release's, so it neither blocks the count nor stays as a release.
    pub folds: Option<Version>,
}

/// Why a tree cannot take a release.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TreeError {
    /// `CHANGELOG.md` has no `## [Unreleased]` heading to close.
    #[error("CHANGELOG.md has no `## [Unreleased]` heading to close")]
    NoUnreleased,
}

/// The heading a release closes.
const UNRELEASED: &str = "## [Unreleased]";

impl Cut {
    /// The release file this cut writes.
    #[must_use]
    pub fn released(&self) -> Released {
        Released::new(
            self.version,
            self.completes.todos().iter().map(|c| c.todo.clone()),
        )
    }

    /// `changelog` with its Unreleased section closed under this version on `day`, the
    /// completed todos (on `chain`) first, and a new empty Unreleased above it; the section of
    /// the version it folds, if any, joins it under the same heading.
    ///
    /// # Errors
    /// No Unreleased heading.
    pub fn changelog(
        &self,
        changelog: &str,
        day: NaiveDate,
        chain: &str,
    ) -> Result<String, TreeError> {
        let folded = self.folds.map(|v| format!("## [{v}] - "));
        let all: Vec<&str> = changelog.split_inclusive('\n').collect();
        let heading = |i: usize| {
            folded
                .as_deref()
                .is_some_and(|h| all.get(i).is_some_and(|line| line.starts_with(h)))
        };
        // The folded heading goes with the blank line under it, so its text joins the section
        // above with one blank line between.
        let under = |i: usize| i > 0 && heading(i - 1) && all[i].trim().is_empty();
        let lines: Vec<&str> = (0..all.len())
            .filter(|&i| !heading(i) && !under(i))
            .map(|i| all[i])
            .collect();
        let at = lines
            .iter()
            .position(|line| line.trim_end() == UNRELEASED)
            .ok_or(TreeError::NoUnreleased)?;
        let listed = self.completes.changelog(chain);
        let listed = if listed.is_empty() {
            String::new()
        } else {
            format!("\n{}\n", listed.join("\n"))
        };
        Ok(format!(
            "{before}{UNRELEASED}\n\n## [{version}] - {day}\n{listed}{after}",
            before = lines[..at].concat(),
            version = self.version,
            day = day.format("%Y-%m-%d"),
            after = lines[at + 1..].concat(),
        ))
    }
}

/// `merged` with every entry line the train's merges added to a released section moved back
/// under Unreleased, each under its `###` heading there: a PR branched before a release wrote
/// its entries under an Unreleased heading that release has since closed, and git merges them in
/// where they were, inside the closed section (several releases back, for a PR branched long
/// ago). Each released section is compared with the same section of `base` (the changelog the
/// train started from): a line `base` held there stays, blank lines stay, and a section `base`
/// does not hold stays whole, since no merge could have been written against it.
#[must_use]
pub fn reopened(base: &str, merged: &str) -> String {
    let lines: Vec<&str> = merged.split_inclusive('\n').collect();
    let Some(unreleased) = lines.iter().position(|l| l.trim_end() == UNRELEASED) else {
        return merged.to_owned();
    };
    let base_lines: Vec<&str> = base.split_inclusive('\n').collect();
    let headings: Vec<usize> = (unreleased + 1..lines.len())
        .filter(|i| lines[*i].starts_with("## ["))
        .collect();
    let Some(first) = headings.first().copied() else {
        return merged.to_owned();
    };

    // Each released section's kept lines, and every added line beside its `###` heading.
    let mut sections: Vec<Vec<&str>> = Vec::new();
    let mut moved: Vec<(&str, &str)> = Vec::new();
    for (k, &at) in headings.iter().enumerate() {
        let end = headings.get(k + 1).copied().unwrap_or(lines.len());
        let body = &lines[at + 1..end];
        let mut kept = vec![lines[at]];
        match section(&base_lines, lines[at]) {
            None => kept.extend(body),
            Some(mut held) => {
                let mut under = "";
                for line in body {
                    if line.starts_with("### ") {
                        under = line;
                    }
                    if let Some(i) = held.iter().position(|h| h == line) {
                        held.remove(i);
                        kept.push(line);
                    } else if line.trim().is_empty() {
                        // One blank where a moved line left two.
                        if kept.last().is_none_or(|l| !l.trim().is_empty()) {
                            kept.push(line);
                        }
                    } else if !line.starts_with("### ") {
                        moved.push((under, line));
                    }
                }
            }
        }
        sections.push(kept);
    }
    if moved.is_empty() {
        return merged.to_owned();
    }

    let mut open: Vec<String> = lines[unreleased + 1..first]
        .iter()
        .map(|l| (*l).to_owned())
        .collect();
    for (under, line) in moved {
        let at = open.iter().position(|l| l == under).map(|h| {
            // After the last entry line of that subsection.
            let next = open[h + 1..]
                .iter()
                .position(|l| l.starts_with("### ") || l.starts_with("## ["))
                .map_or(open.len(), |i| h + 1 + i);
            open[h + 1..next]
                .iter()
                .rposition(|l| !l.trim().is_empty())
                .map_or(h + 1, |i| h + 2 + i)
        });
        if let Some(at) = at {
            open.insert(at, line.to_owned());
        } else {
            while open.last().is_some_and(|l| l.trim().is_empty()) {
                open.pop();
            }
            open.push("\n".to_owned());
            if !under.is_empty() {
                open.push(under.to_owned());
            }
            open.push(line.to_owned());
            open.push("\n".to_owned());
        }
    }
    let mut out: Vec<String> = lines[..=unreleased]
        .iter()
        .map(|l| (*l).to_owned())
        .collect();
    out.extend(open);
    out.extend(sections.into_iter().flatten().map(str::to_owned));
    out.concat()
}

/// The body of `base`'s section headed `heading`, or `None` where `base` has no such section.
fn section<'a>(base: &[&'a str], heading: &str) -> Option<Vec<&'a str>> {
    let at = base.iter().position(|l| *l == heading)?;
    let end = base[at + 1..]
        .iter()
        .position(|l| l.starts_with("## ["))
        .map_or(base.len(), |i| at + 1 + i);
    Some(base[at + 1..end].to_vec())
}
