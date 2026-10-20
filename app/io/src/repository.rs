//! The data tree's uncommitted writes as [`Repository`]: its `git diff --stat`,
//! with each deleted replica object explained when it was adopted.
//!
//! No method takes a program or an argument from its caller: the agent may ask
//! what is uncommitted, not have this box run something.

use std::path::PathBuf;

use suzatary_core::capability::{Repository, RepositoryError};

use crate::run::{Cmd, Ran, RunError, run};

/// One chain as `diff_summary` needs it: its directories relative to the
/// repository, as a diff names them.
#[derive(Debug, Clone)]
pub struct ChainRoster {
    /// The chain's name.
    pub name: String,
    /// Its objects' directory.
    pub events_dir: String,
    /// The triage agent's replica of it.
    pub proposals_dir: String,
}

/// The data tree the box's writes land in, and the chains it holds.
#[derive(Debug)]
pub struct Checkout {
    data: PathBuf,
    chains: Vec<ChainRoster>,
}

impl Checkout {
    /// The diff of `data`, read for the chains `chains` names.
    #[must_use]
    pub fn new(data: impl Into<PathBuf>, chains: Vec<ChainRoster>) -> Self {
        Self {
            data: data.into(),
            chains,
        }
    }
}

impl Repository for Checkout {
    fn diff_summary(&self) -> Result<String, RepositoryError> {
        let shown = self
            .git(&["diff", "--stat"])
            .map_err(|failed| RepositoryError {
                what: "git_diff_summary",
                why: failed.to_string(),
            })?
            .out();
        if shown.is_empty() {
            return Ok("(no uncommitted changes)".to_owned());
        }
        let adoptions = self.adoptions();
        Ok(if adoptions.is_empty() {
            shown
        } else {
            format!("{shown}\n{}\n", adoptions.join("\n"))
        })
    }
}

impl Checkout {
    fn git(&self, args: &[&str]) -> Result<Ran, RunError> {
        run(&Cmd::new("git").args(args).cwd(&self.data))
    }

    /// One line per deleted replica object the target chain adopted; a rejected
    /// or unexplained deletion stays as `--stat` printed it. Best effort: it only
    /// enriches an answer already given.
    fn adoptions(&self) -> Vec<String> {
        self.chains
            .iter()
            .flat_map(|chain| {
                self.deleted_under(&chain.proposals_dir)
                    .into_iter()
                    .filter_map(move |hash| self.explain(chain, &hash))
            })
            .collect()
    }

    /// Every object hash the uncommitted diff deletes under `<dir>/objects/`.
    fn deleted_under(&self, dir: &str) -> Vec<String> {
        let objects = format!("{dir}/objects");
        let Ok(out) = self.git(&["diff", "--diff-filter=D", "--name-only", "--", &objects]) else {
            return Vec::new();
        };
        out.out()
            .lines()
            .filter_map(|line| line.rsplit('/').next())
            .filter_map(|name| name.strip_suffix(".py"))
            .map(str::to_owned)
            .collect()
    }

    /// Explain one deleted object as an adoption, or nothing.
    fn explain(&self, chain: &ChainRoster, hash: &str) -> Option<String> {
        let held_by_target = self
            .data
            .join(&chain.events_dir)
            .join("objects")
            .join(format!("{hash}.py"))
            .is_file();
        held_by_target.then(|| {
            format!(
                "{}/objects/{hash}.py: adopted onto {} (accepted {})",
                chain.proposals_dir,
                chain.name,
                self.accepted_at(chain, hash)
            )
        })
    }

    /// When the target chain committed this object, or "not yet committed".
    fn accepted_at(&self, chain: &ChainRoster, hash: &str) -> String {
        let path = format!("{}/objects/{hash}.py", chain.events_dir);
        self.git(&["log", "-1", "--format=%ai", "--", &path])
            .ok()
            .map(|out| out.out().trim().to_owned())
            .filter(|when| !when.is_empty())
            .unwrap_or_else(|| "not yet committed".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn no_chains() -> Vec<ChainRoster> {
        Vec::new()
    }

    #[test]
    fn a_diff_of_a_directory_that_is_not_there_is_reported_and_never_panics() {
        let failed = Checkout::new("/nonexistent/data", no_chains())
            .diff_summary()
            .unwrap_err();
        assert_eq!(failed.what, "git_diff_summary", "{failed}");
    }

    /// The chain `life` (`events/`) and its replica `events-triage/`, as deployed.
    fn life() -> ChainRoster {
        ChainRoster {
            name: "life".to_owned(),
            events_dir: "events".to_owned(),
            proposals_dir: "events-triage".to_owned(),
        }
    }

    fn git(cwd: &std::path::Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A throwaway repository with both object directories seeded and committed, so a test
    /// stages only the deletion under test.
    fn seeded_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temp dir");
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        std::fs::create_dir_all(root.join("events/objects")).expect("events/objects");
        std::fs::create_dir_all(root.join("events-triage/objects")).expect("replica objects");
        std::fs::write(root.join("events/objects/deadbeef.py"), "Sealed(...)\n")
            .expect("target object");
        std::fs::write(
            root.join("events-triage/objects/deadbeef.py"),
            "Sealed(...)\n",
        )
        .expect("replica object");
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "seed"]);
        dir
    }

    #[test]
    fn a_replica_object_the_target_holds_is_reported_as_adopted() {
        let repo = seeded_repo();
        std::fs::remove_file(repo.path().join("events-triage/objects/deadbeef.py"))
            .expect("removes the replica's copy");
        let summary = Checkout::new(repo.path(), vec![life()])
            .diff_summary()
            .expect("a real repository answers");
        assert!(
            summary.contains("events-triage/objects/deadbeef.py: adopted onto life"),
            "{summary}"
        );
        assert!(!summary.to_lowercase().contains("rewrite"), "{summary}");
    }

    #[test]
    fn an_unexplained_deletion_stays_a_bare_deletion() {
        let repo = seeded_repo();
        std::fs::remove_file(repo.path().join("events/objects/deadbeef.py"))
            .expect("no target copy in this scene");
        std::fs::remove_file(repo.path().join("events-triage/objects/deadbeef.py"))
            .expect("removes the replica's copy");
        let summary = Checkout::new(repo.path(), vec![life()])
            .diff_summary()
            .expect("a real repository answers");
        assert!(!summary.contains("adopted onto"), "{summary}");
        assert!(
            summary.contains("events-triage/objects/deadbeef.py"),
            "{summary}"
        );
    }

    #[test]
    fn a_modified_chain_object_is_not_reclassified_as_an_adoption() {
        let repo = seeded_repo();
        std::fs::write(
            repo.path().join("events/objects/deadbeef.py"),
            "Sealed(tampered)\n",
        )
        .expect("rewrites the tracked object in place");
        let summary = Checkout::new(repo.path(), vec![life()])
            .diff_summary()
            .expect("a real repository answers");
        assert!(!summary.contains("adopted onto"), "{summary}");
        assert!(summary.contains("events/objects/deadbeef.py"), "{summary}");
    }
}
