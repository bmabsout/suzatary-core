//! The owner's repositories, as bare mirrors read through `git` subprocesses.
//!
//! The instance file's declared list is the whole universe, met with what the owner's token
//! lists as his own: a name outside the list is refused before GitHub or `git` is asked, and
//! reading a mirror never executes a file in it. The token never touches argv or a file; it
//! reaches `git` through `GIT_CONFIG_*` in the child's environment.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine as _;
use suzatary_core::capability::{MAX_FILE_BYTES, RepoFailure, Repositories, Secret};
use suzatary_core::coding::{BRANCH_PREFIX, Branch, FileStat, Lines, REPORTS_DIR, Seen};
use suzatary_core::repos::{
    CommitId, Declared, Patch, RepoName, RepoPath, RepoRow, RepoTarget, Rev, TOKEN, Visibility,
};

use crate::http::{Http, Req, Ureq};
use crate::run::{Cmd, Ran, RunError, run, succeed};
use crate::scratch::Scratch;
use crate::secrets::Resolves;

/// How long a listing or a fetch is trusted before asking again.
const FRESH: Duration = Duration::from_mins(2);

/// How long a fetch or a push may take before it is a failure, not a request that never ends.
const REMOTE: Duration = Duration::from_mins(5);

/// The most entries a tree or a grep returns.
const MAX_LINES: usize = 400;

/// Where the repositories come from.
pub enum Source {
    /// GitHub, listed through its API with the token.
    GitHub,
    /// Bare repositories under a local directory (`<base>/<owner>/<name>.git`), each declared
    /// one present there listed as public on `main`: the fixture's and the tests' source, with no
    /// network and no token.
    Local {
        /// The directory holding them.
        base: PathBuf,
    },
}

/// The mirrors, the listing and the token that reaches both.
pub struct Mirrors {
    root: PathBuf,
    source: Source,
    declared: Declared,
    secrets: Box<dyn Resolves + Send + Sync>,
    http: Ureq,
    listed: Mutex<Option<(Instant, Vec<RepoRow>)>>,
    fetched: Mutex<HashMap<RepoName, Instant>>,
}

impl Mirrors {
    /// Mirrors of the `declared` repositories kept under `root`, with the token resolved through
    /// `secrets`.
    #[must_use]
    pub fn new(
        root: impl Into<PathBuf>,
        source: Source,
        declared: Declared,
        secrets: Box<dyn Resolves + Send + Sync>,
    ) -> Self {
        Self {
            root: root.into(),
            source,
            declared,
            secrets,
            http: Ureq::new(),
            listed: Mutex::new(None),
            fetched: Mutex::new(HashMap::new()),
        }
    }

    fn token(&self) -> Result<Secret, RepoFailure> {
        self.secrets
            .get(TOKEN, "the secretary reading the owner's repositories")
            .map_err(|locked| RepoFailure::Unavailable(locked.to_string()))?
            .ok_or_else(|| {
                RepoFailure::Unavailable(format!("{TOKEN} does not resolve on this box"))
            })
    }

    /// The listed row for `repo`, or the refusal that it is not reachable; one the instance file
    /// does not declare is refused here, before anything is asked.
    fn row(&self, repo: &RepoName) -> Result<RepoRow, RepoFailure> {
        if self.declared.target(repo).is_none() {
            return Err(RepoFailure::Unknown(repo.to_string()));
        }
        self.repos()?
            .into_iter()
            .find(|row| &row.repo == repo)
            .ok_or_else(|| RepoFailure::Unknown(repo.to_string()))
    }

    /// `git` with the token in its environment, never in argv.
    fn git(token: Option<&Secret>) -> Cmd {
        let command = Cmd::new("git")
            .pass(crate::run::NETWORK)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1");
        let Some(token) = token else {
            return command;
        };
        let basic = base64::engine::general_purpose::STANDARD
            .encode(format!("x-access-token:{}", token.expose()));
        command
            .env("GIT_CONFIG_COUNT", "2")
            .env("GIT_CONFIG_KEY_0", "http.https://github.com/.extraheader")
            .env(
                "GIT_CONFIG_VALUE_0",
                format!("AUTHORIZATION: basic {basic}"),
            )
            .env("GIT_CONFIG_KEY_1", "credential.helper")
            .env("GIT_CONFIG_VALUE_1", "")
    }

    fn dir(&self, repo: &RepoName) -> PathBuf {
        self.root
            .join(repo.owner())
            .join(format!("{}.git", repo.name()))
    }

    /// The mirror, cloned on first use and fetched when stale.
    fn mirror(&self, repo: &RepoName) -> Result<PathBuf, RepoFailure> {
        self.row(repo)?;
        let dir = self.dir(repo);
        let fresh = self
            .fetched
            .lock()
            .map_err(|_| RepoFailure::Unavailable("the fetch table is poisoned".to_owned()))?
            .get(repo)
            .is_some_and(|at| at.elapsed() < FRESH);
        if fresh && dir.is_dir() {
            return Ok(dir);
        }
        let (token, url) = self.remote(repo)?;
        if !dir.is_dir() {
            std::fs::create_dir_all(&dir).map_err(|e| RepoFailure::Unavailable(e.to_string()))?;
            Self::local(&dir, &["init", "--bare", "--quiet"])?;
        }
        // Branches and tags only, never `--mirror`: GitHub serves `refs/pull/*` too, whose
        // commits are anyone's forks, and a commit here is one the secretary may name as his.
        succeed(
            &Self::git(token.as_ref())
                .arg("--git-dir")
                .arg(&dir)
                .args([
                    "fetch",
                    "--prune",
                    "--quiet",
                    &url,
                    "+refs/heads/*:refs/heads/*",
                    "+refs/tags/*:refs/tags/*",
                ])
                .timeout(REMOTE),
        )?;
        let default = format!("refs/heads/{}", self.row(repo)?.default_branch);
        Self::local(&dir, &["symbolic-ref", "HEAD", &default])?;
        self.fetched
            .lock()
            .map_err(|_| RepoFailure::Unavailable("the fetch table is poisoned".to_owned()))?
            .insert(repo.clone(), Instant::now());
        Ok(dir)
    }

    /// `git --git-dir <mirror> <args>`, with no token: every read is local.
    fn local(dir: &Path, args: &[&str]) -> Result<Ran, RunError> {
        succeed(&Self::git(None).arg("--git-dir").arg(dir).args(args))
    }

    /// [`Mirrors::local`] for a question whose exit status is the answer.
    fn asked(dir: &Path, args: &[&str]) -> Result<Ran, RunError> {
        run(&Self::git(None).arg("--git-dir").arg(dir).args(args))
    }

    /// The patch applied to `base` in a scratch index, touching no ref:
    /// `Ok` with the index file when it applies.
    fn staged(&self, dir: &Path, base: &CommitId, patch: &Patch) -> Result<Index, RepoFailure> {
        let index = Index(self.scratch()?);
        let indexed = || {
            Self::git(None)
                .env("GIT_INDEX_FILE", index.path())
                .arg("--git-dir")
                .arg(dir)
        };
        succeed(&indexed().args(["read-tree", base.as_str()]))
            .map_err(refused_as_missing(|| format!("commit {}", base.short())))?;
        let applied = run(&indexed()
            .args(["apply", "--cached", "--whitespace=nowarn", "-"])
            .stdin(patch.as_str()))
        .map_err(|e| RepoFailure::Unavailable(e.to_string()))?;
        if !applied.ok() {
            return Err(RepoFailure::Conflict(applied.err().trim().to_owned()));
        }
        Ok(index)
    }
}

impl Mirrors {
    /// Every `claude/` branch of `repo`: its tip, whether that tip is on the default branch,
    /// the reports it adds under `docs/reports/`, and its diff stat. A branch or path that
    /// does not parse is skipped, since it names no session.
    ///
    /// # Errors
    /// [`RepoFailure`] when the repository is unknown or `git` fails.
    pub fn seen(&self, repo: &RepoName) -> Result<Vec<Seen>, RepoFailure> {
        let default = format!("refs/heads/{}", self.row(repo)?.default_branch);
        let dir = self.mirror(repo)?;
        let refs = Self::local(
            &dir,
            &[
                "for-each-ref",
                "--format=%(refname:strip=2) %(objectname)",
                &format!("refs/heads/{BRANCH_PREFIX}"),
            ],
        )?;
        let mut seen = Vec::new();
        for line in lines(&refs) {
            let Some((name, tip)) = line.split_once(' ') else {
                continue;
            };
            let (Ok(branch), Ok(tip)) =
                (name.parse::<Branch>(), CommitId::try_from(tip.to_owned()))
            else {
                continue;
            };
            let range = format!("{default}...{}", tip.as_str());
            let merged = Self::asked(
                &dir,
                &["merge-base", "--is-ancestor", tip.as_str(), &default],
            )
            .map_err(|e| RepoFailure::Unavailable(e.to_string()))?
            .ok();
            let reports = lines(&Self::local(
                &dir,
                &[
                    "diff",
                    "--name-only",
                    &range,
                    "--",
                    &format!("{REPORTS_DIR}/"),
                ],
            )?)
            .into_iter()
            .filter(|path| {
                Path::new(path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            })
            .filter_map(|path| {
                let text = Self::local(
                    &dir,
                    &["cat-file", "blob", &format!("{}:{path}", tip.as_str())],
                )
                .map_err(RepoFailure::from)
                .ok()?;
                Some((RepoPath::try_from(path).ok()?, text.out()))
            })
            .collect();
            let stat = lines(&Self::local(&dir, &["diff", "--numstat", &range])?)
                .iter()
                .filter_map(|line| file_stat(line))
                .collect();
            seen.push(Seen {
                branch,
                tip,
                merged,
                reports,
                stat,
            });
        }
        Ok(seen)
    }
}

/// One `git diff --numstat` line: `added<TAB>removed<TAB>path`, `-` for a
/// binary file's counts.
fn file_stat(line: &str) -> Option<FileStat> {
    let mut parts = line.splitn(3, '\t');
    let (added, removed, path) = (parts.next()?, parts.next()?, parts.next()?);
    let lines = match (added.parse(), removed.parse()) {
        (Ok(added), Ok(removed)) => Lines::Text { added, removed },
        _ => Lines::Binary,
    };
    Some(FileStat {
        path: RepoPath::try_from(path.to_owned()).ok()?,
        lines,
    })
}

/// What a publish pushed, and the pull request it opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    /// The new commit.
    pub commit: CommitId,
    /// The branch of its own it landed on.
    pub branch: String,
    /// The request to merge that branch into the default one, or why none was opened: the
    /// branch is pushed either way, so the failure is an answer, not a reason to push again.
    pub pull: Result<PullRequest, RepoFailure>,
}

/// A pull request, by its number and where a person reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    /// Its number in the repository.
    pub number: u64,
    /// Its page.
    pub url: String,
}

/// Who the commit is by: the box's own git identity, which is the owner's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    /// `user.name`.
    pub name: String,
    /// `user.email`.
    pub email: String,
}

impl Author {
    /// The identity `git config` answers on this box, if it has one.
    #[must_use]
    pub fn of_this_box() -> Option<Self> {
        let get = |key: &str| {
            run(&Cmd::new("git").args(["config", "--get", key]))
                .ok()
                .filter(Ran::ok)
                .map(|ran| ran.out().trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        Some(Self {
            name: get("user.name")?,
            email: get("user.email")?,
        })
    }
}

/// How long a declared check may run.
const CHECK_TIMEOUT: Duration = Duration::from_mins(10);

impl Mirrors {
    /// What the instance file declares about `repo`, or `None` when it is not reachable.
    #[must_use]
    pub fn target(&self, repo: &RepoName) -> Option<RepoTarget> {
        self.declared.target(repo).cloned()
    }

    /// Publish `patch` on `base` as a pull request: checked by the repository's declared check,
    /// committed by `author`, pushed to the new `branch` (never forced, so a branch that already
    /// holds something else is refused by the remote), and opened as a request to merge it into
    /// the default branch, whose title is the message's first line. Nothing lands on the default
    /// branch; a person merges it.
    ///
    /// # Errors
    /// [`RepoFailure::Conflict`] when it no longer applies or the check
    /// fails (with its output), [`RepoFailure::Unavailable`] for `git` and
    /// the network, including a push the remote refused.
    pub fn publish(
        &self,
        repo: &RepoName,
        base: &CommitId,
        patch: &Patch,
        message: &str,
        author: &Author,
        branch: &str,
    ) -> Result<Published, RepoFailure> {
        // Asked again at the tap: the card was drawn from the listing, and the file or the
        // repository may have changed since.
        self.declared.pushable(repo, self.row(repo)?.visibility)?;
        let check = self
            .target(repo)
            .ok_or_else(|| RepoFailure::Unknown(repo.to_string()))?
            .check;
        self.fetched
            .lock()
            .map_err(|_| RepoFailure::Unavailable("the fetch table is poisoned".to_owned()))?
            .remove(repo);
        let dir = self.mirror(repo)?;
        let index = self.staged(&dir, base, patch)?;
        let tree = succeed(
            &Self::git(None)
                .env("GIT_INDEX_FILE", index.path())
                .arg("--git-dir")
                .arg(&dir)
                .arg("write-tree"),
        )?
        .out()
        .trim()
        .to_owned();
        if let Some(check) = &check {
            self.checked_tree(&dir, &tree, check)?;
        }
        let made = succeed(
            &Self::git(None)
                .env("GIT_AUTHOR_NAME", &author.name)
                .env("GIT_AUTHOR_EMAIL", &author.email)
                .env("GIT_COMMITTER_NAME", &author.name)
                .env("GIT_COMMITTER_EMAIL", &author.email)
                .arg("--git-dir")
                .arg(&dir)
                .args(["commit-tree", &tree, "-p", base.as_str(), "-F", "-"])
                .stdin(message),
        )?;
        let id = CommitId::try_from(made.out().trim().to_owned())
            .map_err(|e| RepoFailure::Unavailable(e.to_string()))?;
        let (token, url) = self.remote(repo)?;
        succeed(
            &Self::git(token.as_ref())
                .arg("--git-dir")
                .arg(&dir)
                .args([
                    "push",
                    "--quiet",
                    &url,
                    &format!("{}:refs/heads/{branch}", id.as_str()),
                ])
                .timeout(REMOTE),
        )
        .map_err(RepoFailure::from)
        .map_err(|failed| RepoFailure::Unavailable(format!("the push was refused: {failed}")))?;
        self.fetched
            .lock()
            .map_err(|_| RepoFailure::Unavailable("the fetch table is poisoned".to_owned()))?
            .remove(repo);
        Ok(Published {
            commit: id,
            branch: branch.to_owned(),
            pull: self.open_pull(repo, branch, message),
        })
    }

    /// Open a request to merge `branch` into the default branch: through GitHub's API with the
    /// token, or, for a local remote, as GitHub spells one in git, `refs/pull/<n>/head`.
    fn open_pull(
        &self,
        repo: &RepoName,
        branch: &str,
        message: &str,
    ) -> Result<PullRequest, RepoFailure> {
        let into = self.row(repo)?.default_branch;
        let (title, body) = message.split_once('\n').unwrap_or((message, ""));
        match &self.source {
            Source::GitHub => {
                let token = self.token()?;
                let resp = self
                    .http
                    .send(
                        Req::json(
                            format!("https://api.github.com/repos/{repo}/pulls"),
                            &serde_json::json!({
                                "title": title.trim(),
                                "head": branch,
                                "base": into,
                                "body": body.trim(),
                            }),
                        )
                        .header("Authorization", format!("Bearer {}", token.expose()))
                        .header("Accept", "application/vnd.github+json")
                        .header("User-Agent", "suzatary-secretary")
                        .timeout(Duration::from_secs(20)),
                    )
                    .map_err(|e| RepoFailure::Unavailable(format!("GitHub: {e}")))?;
                if resp.status != 201 {
                    return Err(RepoFailure::Unavailable(format!(
                        "GitHub answered {} to the pull request",
                        resp.status
                    )));
                }
                let answer: serde_json::Value = serde_json::from_slice(&resp.body)
                    .map_err(|e| RepoFailure::Unavailable(format!("GitHub: {e}")))?;
                match (
                    answer.get("number").and_then(serde_json::Value::as_u64),
                    answer.get("html_url").and_then(serde_json::Value::as_str),
                ) {
                    (Some(number), Some(url)) => Ok(PullRequest {
                        number,
                        url: url.to_owned(),
                    }),
                    _ => Err(RepoFailure::Unavailable(
                        "GitHub's pull request names no number and page".to_owned(),
                    )),
                }
            }
            Source::Local { .. } => {
                let (_, url) = self.remote(repo)?;
                let remote = Path::new(&url);
                let opened = lines(&Self::local(
                    remote,
                    &["for-each-ref", "--format=%(refname)", "refs/pull/"],
                )?)
                .len();
                let number = opened as u64 + 1;
                Self::local(
                    remote,
                    &[
                        "update-ref",
                        &format!("refs/pull/{number}/head"),
                        &format!("refs/heads/{branch}"),
                    ],
                )?;
                Ok(PullRequest {
                    number,
                    url: format!("{url}#pull/{number}"),
                })
            }
        }
    }

    /// Run `check` in the tree `tree` unpacked into a scratch directory.
    fn checked_tree(&self, dir: &Path, tree: &str, check: &str) -> Result<(), RepoFailure> {
        let scratch = self.scratch()?;
        let archive = Self::local(dir, &["archive", "--format=tar", tree])?;
        succeed(
            &Cmd::new("tar")
                .arg("-x")
                .arg("-C")
                .arg(scratch.path())
                .stdin(archive.stdout),
        )?;
        // Both streams to stderr, so the tail a failure quotes is everything the check said. It
        // runs code the agent proposed, so it is passed nothing beyond `run`'s base; that keeps
        // the server's variables from it, and is not a sandbox: HOME and XDG_RUNTIME_DIR still
        // lead to the user's files, the gpg-agent socket and the secret store.
        let tail = match succeed(
            &Cmd::new("sh")
                .args(["-c", &format!("exec 1>&2\n{check}")])
                .cwd(scratch.path())
                .timeout(CHECK_TIMEOUT),
        ) {
            Ok(_) => return Ok(()),
            Err(RunError::Exited { stderr, .. }) => stderr.as_str().to_owned(),
            Err(RunError::TimedOut { after, .. }) => format!("timed out after {after:?}"),
            Err(failed) => return Err(RepoFailure::Unavailable(format!("the check: {failed}"))),
        };
        Err(RepoFailure::Conflict(format!(
            "the check `{check}` failed:\n{tail}"
        )))
    }

    /// A scratch directory under the mirrors' root, so a tree unpacked for a check never
    /// leaves the filesystem the mirrors are on.
    fn scratch(&self) -> Result<Scratch, RepoFailure> {
        Scratch::under(&self.root, ".scratch-").map_err(|e| RepoFailure::Unavailable(e.to_string()))
    }

    /// Where to push, and the token when the source needs one.
    fn remote(&self, repo: &RepoName) -> Result<(Option<Secret>, String), RepoFailure> {
        Ok(match &self.source {
            Source::GitHub => (
                Some(self.token()?),
                format!("https://github.com/{repo}.git"),
            ),
            Source::Local { base, .. } => (
                None,
                base.join(repo.owner())
                    .join(format!("{}.git", repo.name()))
                    .display()
                    .to_string(),
            ),
        })
    }
}

/// A scratch index file, in a scratch directory of its own that goes with it.
struct Index(Scratch);

impl Index {
    fn path(&self) -> PathBuf {
        self.0.path().join("index")
    }
}

fn lines(ran: &Ran) -> Vec<String> {
    ran.out()
        .lines()
        .take(MAX_LINES)
        .map(str::to_owned)
        .collect()
}

impl From<RunError> for RepoFailure {
    fn from(failed: RunError) -> Self {
        RepoFailure::Unavailable(failed.to_string())
    }
}

/// git's refusal, which says the thing it was asked for is not there; git failing to run, or
/// outliving its timeout, says nothing about the thing and is the mirror unavailable.
fn refused_as_missing(what: impl FnOnce() -> String) -> impl FnOnce(RunError) -> RepoFailure {
    move |failed| match failed {
        RunError::Exited { .. } => RepoFailure::Missing(what()),
        failed => RepoFailure::from(failed),
    }
}

/// One repository from GitHub's listing, parsed rather than trusted.
fn row_of(value: &serde_json::Value) -> Option<RepoRow> {
    let repo: RepoName = value.get("full_name")?.as_str()?.parse().ok()?;
    let private = value.get("private")?.as_bool()?;
    Some(RepoRow {
        repo,
        visibility: if private {
            Visibility::Private
        } else {
            Visibility::Public
        },
        default_branch: value
            .get("default_branch")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("main")
            .to_owned(),
        description: value
            .get("description")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

impl Repositories for Mirrors {
    fn repos(&self) -> Result<Vec<RepoRow>, RepoFailure> {
        let poisoned = || RepoFailure::Unavailable("the listing lock is poisoned".to_owned());
        if let Some((at, rows)) = self.listed.lock().map_err(|_| poisoned())?.as_ref()
            && at.elapsed() < FRESH
        {
            return Ok(rows.clone());
        }
        if let Source::Local { base } = &self.source {
            return Ok(self
                .declared
                .iter()
                .map(|(repo, _)| repo)
                .filter(|repo| {
                    base.join(repo.owner())
                        .join(format!("{}.git", repo.name()))
                        .is_dir()
                })
                .map(|repo| RepoRow {
                    repo: repo.clone(),
                    visibility: Visibility::Public,
                    default_branch: "main".to_owned(),
                    description: String::new(),
                })
                .collect());
        }
        let token = self.token()?;
        let mut rows = Vec::new();
        for page in 1..=10 {
            let resp = self
                .http
                .send(
                    Req::get(format!(
                        "https://api.github.com/user/repos?per_page=100&affiliation=owner&page={page}"
                    ))
                    .header("Authorization", format!("Bearer {}", token.expose()))
                    .header("Accept", "application/vnd.github+json")
                    .header("User-Agent", "suzatary-secretary")
                    .timeout(Duration::from_secs(20)),
                )
                .map_err(|e| RepoFailure::Unavailable(format!("GitHub: {e}")))?;
            if resp.status != 200 {
                return Err(RepoFailure::Unavailable(format!(
                    "GitHub answered {} to the repository listing",
                    resp.status
                )));
            }
            let body: serde_json::Value = serde_json::from_slice(&resp.body)
                .map_err(|e| RepoFailure::Unavailable(format!("GitHub: {e}")))?;
            let items = body.as_array().map_or(&[][..], Vec::as_slice);
            let last = items.len() < 100;
            // The listing is the owner's own repositories; met with the declared list, it is
            // what the secretary reaches.
            rows.extend(
                items
                    .iter()
                    .filter_map(row_of)
                    .filter(|row| self.declared.target(&row.repo).is_some()),
            );
            if last {
                break;
            }
        }
        rows.sort_by(|a, b| a.repo.cmp(&b.repo));
        *self.listed.lock().map_err(|_| poisoned())? = Some((Instant::now(), rows.clone()));
        Ok(rows)
    }

    fn resolve(&self, repo: &RepoName, at: Option<&Rev>) -> Result<CommitId, RepoFailure> {
        let name = match at {
            Some(rev) => rev.as_str().to_owned(),
            None => format!("refs/heads/{}", self.row(repo)?.default_branch),
        };
        let dir = self.mirror(repo)?;
        let output = Self::local(
            &dir,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &format!("{name}^{{commit}}"),
            ],
        )
        .map_err(refused_as_missing(|| format!("{name} in {repo}")))?;
        let commit = CommitId::try_from(output.out().trim().to_owned())
            .map_err(|e| RepoFailure::Unavailable(e.to_string()))?;
        // Every commit the secretary names is on the default branch, so what it reads, attaches
        // or writes a change against is the owner's own history and never a fork's.
        let default = format!("refs/heads/{}", self.row(repo)?.default_branch);
        let on_default = Self::asked(
            &dir,
            &["merge-base", "--is-ancestor", commit.as_str(), &default],
        )
        .map_err(|e| RepoFailure::Unavailable(e.to_string()))?
        .ok();
        if on_default {
            Ok(commit)
        } else {
            Err(RepoFailure::Missing(format!(
                "{name} in {repo} is not on its default branch"
            )))
        }
    }

    fn tree(
        &self,
        repo: &RepoName,
        at: &CommitId,
        dir: &RepoPath,
    ) -> Result<Vec<String>, RepoFailure> {
        let mirror = self.mirror(repo)?;
        let mut args = vec!["ls-tree", at.as_str()];
        let under = format!("{}/", dir.as_str());
        if !dir.as_str().is_empty() {
            args.push("--");
            args.push(&under);
        }
        let output = Self::local(&mirror, &args).map_err(refused_as_missing(|| {
            format!("{} at {}", dir.as_str(), at.short())
        }))?;
        Ok(lines(&output)
            .into_iter()
            .filter_map(|line| {
                let (meta, path) = line.split_once('\t')?;
                Some(if meta.contains(" tree ") {
                    format!("{path}/")
                } else {
                    path.to_owned()
                })
            })
            .collect())
    }

    fn blob(
        &self,
        repo: &RepoName,
        at: &CommitId,
        path: &RepoPath,
    ) -> Result<Vec<u8>, RepoFailure> {
        let mirror = self.mirror(repo)?;
        let spec = format!("{}:{}", at.as_str(), path.as_str());
        let what = || format!("{} at {}", path.as_str(), at.short());
        let size: u64 = Self::local(&mirror, &["cat-file", "-s", &spec])
            .map_err(refused_as_missing(what))?
            .out()
            .trim()
            .parse()
            .map_err(|_| RepoFailure::Missing(what()))?;
        if size > MAX_FILE_BYTES {
            return Err(RepoFailure::TooLarge {
                path: path.as_str().to_owned(),
                size,
            });
        }
        Ok(Self::local(&mirror, &["cat-file", "blob", &spec])
            .map_err(refused_as_missing(what))?
            .stdout)
    }

    fn grep(
        &self,
        repo: &RepoName,
        at: &CommitId,
        needle: &str,
    ) -> Result<Vec<String>, RepoFailure> {
        let mirror = self.mirror(repo)?;
        let output = Self::asked(
            &mirror,
            &[
                "grep",
                "-n",
                "-I",
                "-F",
                "-i",
                "-e",
                needle,
                at.as_str(),
                "--",
            ],
        )
        .map_err(|e| RepoFailure::Unavailable(e.to_string()))?;
        // `git grep` exits 1 for no match, which is an answer.
        if !output.ok() && output.status.code() != Some(1) {
            return Err(RepoFailure::Unavailable(output.err().trim().to_owned()));
        }
        let prefix = format!("{}:", at.as_str());
        Ok(lines(&output)
            .into_iter()
            .map(|line| {
                line.strip_prefix(&prefix)
                    .map_or(line.clone(), str::to_owned)
            })
            .collect())
    }

    fn applies(&self, repo: &RepoName, base: &CommitId, patch: &Patch) -> Result<(), RepoFailure> {
        let mirror = self.mirror(repo)?;
        self.staged(&mirror, base, patch).map(|_| ())
    }

    fn pushable(&self, repo: &RepoName, visibility: Visibility) -> Result<(), RepoFailure> {
        self.declared.pushable(repo, visibility)
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    /// git's refusal names the thing missing; git that could not run, or ran out of time, says
    /// the mirror is unavailable, never that the thing is not there.
    #[test]
    fn only_git_s_refusal_is_a_missing_thing() {
        use std::os::unix::process::ExitStatusExt as _;
        let what = || "README.md at abc1234".to_owned();
        let refused = RunError::Exited {
            program: "git".to_owned(),
            status: std::process::ExitStatus::from_raw(128 << 8),
            stderr: suzatary_core::excerpt::Excerpt::of("fatal: not a valid object name"),
        };
        assert!(matches!(
            refused_as_missing(what)(refused),
            RepoFailure::Missing(said) if said == what()
        ));
        let unrun = RunError::Spawn {
            program: "git".to_owned(),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };
        assert!(matches!(
            refused_as_missing(what)(unrun),
            RepoFailure::Unavailable(_)
        ));
        let slow = RunError::TimedOut {
            program: "git".to_owned(),
            after: Duration::from_secs(1),
        };
        assert!(matches!(
            refused_as_missing(what)(slow),
            RepoFailure::Unavailable(_)
        ));
    }

    /// A local source needs no token: asking for one fails the test.
    struct NoToken;

    impl Resolves for NoToken {
        fn get(
            &self,
            name: &str,
            _reason: &str,
        ) -> Result<Option<Secret>, crate::secrets::SecretError> {
            panic!("a local source asked for {name}");
        }
    }

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.test"])
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            status.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    fn declared(repo: &RepoName) -> Declared {
        checked_by(repo, None)
    }

    fn checked_by(repo: &RepoName, check: Option<&str>) -> Declared {
        Declared::new([(
            repo.clone(),
            RepoTarget {
                check: check.map(str::to_owned),
                // A local remote lists as public.
                public: true,
            },
        )])
        .expect("one repository")
    }

    fn fixture(tag: &str) -> (tempfile::TempDir, Mirrors, RepoName) {
        checked_fixture(tag, None)
    }

    fn checked_fixture(tag: &str, check: Option<&str>) -> (tempfile::TempDir, Mirrors, RepoName) {
        let temp = tempfile::tempdir().expect("a temp dir");
        let work = temp.path().join("work");
        std::fs::create_dir_all(work.join("docs")).expect("work dir");
        std::fs::write(work.join("cv.typ"), "= Ada\nResearch engineer\n").expect("cv");
        std::fs::write(work.join("docs/notes.md"), "notes\n").expect("notes");
        run(&work, &["init", "-q", "-b", "main"]);
        run(&work, &["add", "."]);
        run(&work, &["commit", "-q", "-m", "first"]);
        let base = temp.path().join("remotes");
        std::fs::create_dir_all(base.join("bmabsout")).expect("remotes");
        run(
            temp.path(),
            &[
                "clone",
                "-q",
                "--bare",
                "work",
                &format!("remotes/bmabsout/{tag}.git"),
            ],
        );
        let repo: RepoName = format!("bmabsout/{tag}").parse().expect("a name");
        let mirrors = Mirrors::new(
            temp.path().join("mirrors"),
            Source::Local { base },
            checked_by(&repo, check),
            Box::new(NoToken),
        );
        (temp, mirrors, repo)
    }

    #[test]
    fn every_read_answers_from_the_mirror() {
        let (_temp, mirrors, repo) = fixture("cv");
        assert_eq!(mirrors.repos().expect("listed").len(), 1);
        let head = mirrors.resolve(&repo, None).expect("a head");
        let root = RepoPath::try_from(String::new()).expect("root");
        assert_eq!(
            mirrors.tree(&repo, &head, &root).expect("a tree"),
            vec!["cv.typ", "docs/"]
        );
        let docs = RepoPath::try_from("docs".to_owned()).expect("docs");
        assert_eq!(
            mirrors.tree(&repo, &head, &docs).expect("a tree"),
            vec!["docs/notes.md"]
        );
        let cv = RepoPath::try_from("cv.typ".to_owned()).expect("cv");
        assert_eq!(
            mirrors.blob(&repo, &head, &cv).expect("read"),
            b"= Ada\nResearch engineer\n"
        );
        let named = |name: &str| Rev::try_from(name.to_owned()).expect("a revision");
        assert_eq!(
            mirrors
                .resolve(&repo, Some(&named("main")))
                .expect("a branch"),
            head
        );
        assert_eq!(
            mirrors
                .resolve(&repo, Some(&Rev::from(head.clone())))
                .expect("a commit"),
            head,
            "a commit resolves to itself"
        );
        assert!(matches!(
            mirrors.resolve(&repo, Some(&named("nowhere"))),
            Err(RepoFailure::Missing(_))
        ));
        assert!(matches!(
            mirrors.blob(
                &repo,
                &head,
                &RepoPath::try_from("docs".to_owned()).expect("a dir")
            ),
            Err(RepoFailure::Missing(_))
        ));
        assert_eq!(
            mirrors.grep(&repo, &head, "research").expect("grep"),
            vec!["cv.typ:2:Research engineer"]
        );
        assert!(
            mirrors
                .grep(&repo, &head, "nowhere")
                .expect("no match")
                .is_empty()
        );
    }

    /// A fork's commit served under `refs/pull/*`, or a branch not merged into the default one,
    /// names nothing the secretary may read, attach or write against.
    #[test]
    fn only_the_default_branchs_history_resolves() {
        let (temp, mirrors, repo) = fixture("forks");
        let work = temp.path().join("work");
        let remote = temp.path().join("remotes/bmabsout/forks.git");
        let remote = remote.to_str().expect("a utf-8 path");
        run(&work, &["checkout", "-q", "-b", "outsider"]);
        std::fs::write(work.join("cv.typ"), "= Mallory\n").expect("a fork's cv");
        run(&work, &["commit", "-qam", "a fork's change"]);
        run(&work, &["push", "-q", remote, "outsider:refs/pull/1/head"]);
        run(
            &work,
            &["push", "-q", remote, "outsider:refs/heads/unmerged"],
        );
        let fork = String::from_utf8(
            Command::new("git")
                .current_dir(&work)
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("git runs")
                .stdout,
        )
        .expect("utf-8")
        .trim()
        .to_owned();
        for name in ["pull/1/head", "refs/pull/1/head", "unmerged", fork.as_str()] {
            let rev = Rev::try_from(name.to_owned()).expect("a revision");
            assert!(
                matches!(
                    mirrors.resolve(&repo, Some(&rev)),
                    Err(RepoFailure::Missing(_))
                ),
                "{name}"
            );
        }
        let mirror = temp.path().join("mirrors/bmabsout/forks.git");
        let refs = Command::new("git")
            .arg("--git-dir")
            .arg(&mirror)
            .args(["for-each-ref", "--format=%(refname)"])
            .output()
            .expect("git runs");
        let refs = String::from_utf8_lossy(&refs.stdout);
        assert!(
            !refs.contains("refs/pull/"),
            "a pull request's ref is never fetched: {refs}"
        );
        assert!(mirrors.resolve(&repo, None).is_ok());
    }

    #[test]
    fn a_patch_is_checked_against_its_base_and_touches_nothing() {
        let (_temp, mirrors, repo) = fixture("cv2");
        let head = mirrors.resolve(&repo, None).expect("a head");
        let good = Patch::try_from(
            "--- a/cv.typ\n+++ b/cv.typ\n@@ -1,2 +1,2 @@\n = Ada\n-Research engineer\n+Principal investigator\n"
                .to_owned(),
        )
        .expect("a patch");
        mirrors.applies(&repo, &head, &good).expect("applies");
        assert_eq!(
            mirrors.resolve(&repo, None).expect("head"),
            head,
            "checking moved nothing"
        );
        let stale = Patch::try_from(
            "--- a/cv.typ\n+++ b/cv.typ\n@@ -1,2 +1,2 @@\n = Ada\n-Not what it says\n+x\n"
                .to_owned(),
        )
        .expect("a patch");
        assert!(matches!(
            mirrors.applies(&repo, &head, &stale),
            Err(RepoFailure::Conflict(_))
        ));
    }

    #[test]
    fn a_claude_branch_is_seen_with_its_report_and_then_merged() {
        let (temp, mirrors, repo) = fixture("coding");
        let work = temp.path().join("work");
        let remote = temp.path().join("remotes/bmabsout/coding.git");
        let remote = remote.to_str().expect("a utf-8 path");
        run(&work, &["checkout", "-q", "-b", "claude/retitle"]);
        std::fs::create_dir_all(work.join("docs/reports")).expect("reports dir");
        std::fs::write(
            work.join("docs/reports/retitle.md"),
            "Session: https://claude.ai/code/session_01fake\nGates: green\n",
        )
        .expect("report");
        std::fs::write(work.join("cv.typ"), "= Ada\nPrincipal investigator\n").expect("cv");
        run(&work, &["add", "."]);
        run(&work, &["commit", "-q", "-m", "retitle"]);
        run(&work, &["push", "-q", remote, "claude/retitle"]);
        run(
            &work,
            &["push", "-q", remote, "main:refs/heads/not-a-session"],
        );

        let seen = mirrors.seen(&repo).expect("seen");
        assert_eq!(seen.len(), 1, "only claude/ branches: {seen:?}");
        let branch = &seen[0];
        assert_eq!(branch.branch.as_str(), "claude/retitle");
        assert!(!branch.merged);
        assert_eq!(branch.reports.len(), 1);
        assert_eq!(branch.reports[0].0.as_str(), "docs/reports/retitle.md");
        assert!(branch.reports[0].1.contains("session_01fake"));
        let files: Vec<(&str, Lines)> = branch
            .stat
            .iter()
            .map(|file| (file.path.as_str(), file.lines))
            .collect();
        assert_eq!(
            files,
            [
                (
                    "cv.typ",
                    Lines::Text {
                        added: 1,
                        removed: 1
                    }
                ),
                (
                    "docs/reports/retitle.md",
                    Lines::Text {
                        added: 2,
                        removed: 0
                    }
                ),
            ]
        );

        run(&work, &["checkout", "-q", "main"]);
        run(&work, &["merge", "-q", "--ff-only", "claude/retitle"]);
        run(&work, &["push", "-q", remote, "main"]);
        let refetched = Mirrors::new(
            temp.path().join("mirrors"),
            Source::Local {
                base: temp.path().join("remotes"),
            },
            declared(&repo),
            Box::new(NoToken),
        );
        let seen = refetched.seen(&repo).expect("seen again");
        assert!(seen[0].merged, "the tip is on main now");
    }

    #[test]
    fn a_repository_outside_the_declared_list_is_refused_before_git_runs() {
        let (temp, mirrors, _repo) = fixture("cv3");
        let present: RepoName = "bmabsout/present".parse().expect("a name");
        run(
            temp.path(),
            &[
                "clone",
                "-q",
                "--bare",
                "work",
                "remotes/bmabsout/present.git",
            ],
        );
        assert!(
            matches!(
                mirrors.resolve(&present, None),
                Err(RepoFailure::Unknown(_))
            ),
            "undeclared"
        );
        let other: RepoName = "someorg/secrets".parse().expect("a name");
        assert!(matches!(
            mirrors.resolve(&other, None),
            Err(RepoFailure::Unknown(_))
        ));
        assert!(!temp.path().join("mirrors/bmabsout/present.git").exists());
    }

    /// Against GitHub, an undeclared name is refused before the token is resolved, so before
    /// any request: `NoToken` fails the test if it is asked.
    #[test]
    fn an_undeclared_repository_is_refused_before_any_network() {
        let temp = tempfile::tempdir().expect("a temp dir");
        let declared_one: RepoName = "bmabsout/cv".parse().expect("a name");
        let mirrors = Mirrors::new(
            temp.path().join("mirrors"),
            Source::GitHub,
            declared(&declared_one),
            Box::new(NoToken),
        );
        for name in ["bmabsout/other", "someorg/secrets"] {
            let repo: RepoName = name.parse().expect("a name");
            assert!(
                matches!(mirrors.resolve(&repo, None), Err(RepoFailure::Unknown(_))),
                "{name}"
            );
        }
    }

    fn author() -> Author {
        Author {
            name: "Ada".to_owned(),
            email: "b@example.test".to_owned(),
        }
    }

    fn retitle() -> Patch {
        Patch::try_from(
            "--- a/cv.typ\n+++ b/cv.typ\n@@ -1,2 +1,2 @@\n = Ada\n-Research engineer\n+Principal investigator\n"
                .to_owned(),
        )
        .expect("a patch")
    }

    fn on_remote(temp: &tempfile::TempDir, tag: &str, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("--git-dir")
            .arg(temp.path().join(format!("remotes/bmabsout/{tag}.git")))
            .args(args)
            .output()
            .expect("git runs");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    #[test]
    fn a_publish_opens_a_pull_request_and_leaves_the_default_branch() {
        let (temp, mirrors, repo) = checked_fixture("pub", Some("grep -q Principal cv.typ"));
        let base = mirrors.resolve(&repo, None).expect("a head");
        let published = mirrors
            .publish(
                &repo,
                &base,
                &retitle(),
                "Retitle the CV\n\nHe is a PI now.\n",
                &author(),
                "secretary/2026-10-05-1-0",
            )
            .expect("published");
        assert_eq!(published.branch, "secretary/2026-10-05-1-0");
        assert_eq!(published.pull.as_ref().expect("opened").number, 1);
        assert!(
            on_remote(&temp, "pub", &["show", "secretary/2026-10-05-1-0:cv.typ"])
                .contains("Principal investigator")
        );
        assert_eq!(
            on_remote(&temp, "pub", &["rev-parse", "refs/pull/1/head"]),
            published.commit.as_str(),
            "the request names the pushed commit"
        );
        assert_eq!(
            mirrors.resolve(&repo, None).expect("head"),
            base,
            "the default branch is a person's to merge into"
        );
        let second = mirrors
            .publish(
                &repo,
                &base,
                &retitle(),
                "Again",
                &author(),
                "secretary/other",
            )
            .expect("a second");
        assert_eq!(second.pull.expect("opened").number, 2);
    }

    /// A branch pushed whose request could not be opened is published, saying why there is no
    /// request, so nothing is pushed again.
    #[test]
    fn a_pushed_branch_without_a_pull_request_says_so() {
        let (temp, mirrors, repo) = fixture("nopull");
        // A stale lock on the request's ref: the branch pushes, the request cannot be made.
        let pulls = temp.path().join("remotes/bmabsout/nopull.git/refs/pull/1");
        std::fs::create_dir_all(&pulls).expect("the pull refs");
        std::fs::write(pulls.join("head.lock"), "").expect("a stale lock");
        let base = mirrors.resolve(&repo, None).expect("a head");
        let published = mirrors
            .publish(
                &repo,
                &base,
                &retitle(),
                "Retitle",
                &author(),
                "secretary/x",
            )
            .expect("the branch is pushed");
        assert!(published.pull.is_err(), "{published:?}");
        assert_eq!(
            on_remote(&temp, "nopull", &["rev-parse", "secretary/x"]),
            published.commit.as_str()
        );
    }

    #[test]
    fn a_failing_check_pushes_nothing() {
        let (temp, mirrors, repo) = checked_fixture("chk", Some("echo broken; exit 3"));
        let base = mirrors.resolve(&repo, None).expect("a head");
        let refused = mirrors.publish(&repo, &base, &retitle(), "x", &author(), "secretary/x");
        assert!(
            matches!(&refused, Err(RepoFailure::Conflict(said)) if said.contains("broken")),
            "{refused:?}"
        );
        assert_eq!(
            on_remote(&temp, "chk", &["for-each-ref"]).lines().count(),
            1,
            "only main"
        );
    }

    /// A check runs in a tree unpacked for it, and the tree goes with the check, passed or
    /// failed: nothing but the mirrors is left under their root.
    #[test]
    fn a_check_leaves_nothing_behind() {
        for (tag, check) in [("kept", "true"), ("gone", "exit 3")] {
            let (temp, mirrors, repo) = checked_fixture(tag, Some(check));
            let base = mirrors.resolve(&repo, None).expect("a head");
            let _ = mirrors.publish(&repo, &base, &retitle(), "x", &author(), "secretary/x");
            let left: Vec<String> = std::fs::read_dir(temp.path().join("mirrors"))
                .expect("the mirrors' root")
                .map(|entry| {
                    entry
                        .expect("an entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            assert_eq!(left, ["bmabsout"], "`{check}` left {left:?}");
        }
    }

    #[test]
    fn a_branch_that_holds_something_else_is_refused_not_forced() {
        let (temp, mirrors, repo) = fixture("moved");
        let base = mirrors.resolve(&repo, None).expect("a head");
        let first = mirrors
            .publish(
                &repo,
                &base,
                &retitle(),
                "first",
                &author(),
                "secretary/one",
            )
            .expect("first");
        let other = Patch::try_from(
            "--- a/docs/notes.md\n+++ b/docs/notes.md\n@@ -1 +1 @@\n-notes\n+more notes\n"
                .to_owned(),
        )
        .expect("a patch");
        let stale = mirrors.publish(&repo, &base, &other, "second", &author(), "secretary/one");
        assert!(
            matches!(stale, Err(RepoFailure::Unavailable(_))),
            "{stale:?}"
        );
        assert_eq!(
            on_remote(&temp, "moved", &["rev-parse", "secretary/one"]),
            first.commit.as_str()
        );
    }
}
