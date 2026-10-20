//! Where the data repository is pushed, as the owner reads it: a host and a path, with any
//! credential a URL carries left behind; and whether what it holds was pushed there.

use crate::connect::Failure;
use crate::mail::health::{Health, Instant, Outcome};

/// Where the data repository is pushed, as far as its git configuration could be read: "pushed
/// nowhere" is a fact read from it, never what a failed read stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pushed {
    /// To its `origin` on a host.
    To(Remote),
    /// To its `origin` in a folder: a local path or a `file://` URL, as written.
    Folder(String),
    /// Nowhere: its configuration names no `origin`, or it is no git repository.
    Nowhere,
    /// Its configuration could not be read, and why.
    Unread(String),
}

impl Pushed {
    /// Where the `origin` a repository's git configuration names is: on a host, read as
    /// [`Remote::parse`] reads it, or in a folder (a `file://` URL, or a path that is neither a
    /// URL nor scp-like). Any other origin is unread, and its words never repeat it, since a URL
    /// may carry a credential.
    #[must_use]
    pub fn of_git_config(config: &str) -> Self {
        let mut origin = false;
        let url = config.lines().map(str::trim).find_map(|line| {
            if line.starts_with('[') {
                origin = line == "[remote \"origin\"]";
                return None;
            }
            let (key, value) = line.split_once('=')?;
            (origin && key.trim() == "url").then(|| value.trim())
        });
        match url {
            None | Some("") => Self::Nowhere,
            Some(url) => match url.strip_prefix("file://") {
                Some(path) => Self::Folder(path.to_owned()),
                None if !url.contains("://") && !scp_like(url) => Self::Folder(url.to_owned()),
                None => Remote::parse(url).map_or_else(
                    || Self::Unread("an origin that names no host and path".to_owned()),
                    Self::To,
                ),
            },
        }
    }

    /// Whether the data repository's push token is needed: while it pushes to GitHub, the one
    /// host the token is for, and while its configuration could not be read, since it may.
    #[must_use]
    pub fn needs_push_token(&self) -> bool {
        match self {
            Pushed::To(remote) => remote.host().eq_ignore_ascii_case("github.com"),
            Pushed::Unread(_) => true,
            Pushed::Folder(_) | Pushed::Nowhere => false,
        }
    }

    /// The directory a `.git` FILE points at (a worktree's or a submodule's `gitdir: <path>`).
    #[must_use]
    pub fn gitdir(dot_git: &str) -> Option<&str> {
        dot_git
            .lines()
            .find_map(|line| line.trim().strip_prefix("gitdir:"))
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
    }
}

/// `host:path`, as git reads a URL with no scheme: a colon before any slash.
fn scp_like(url: &str) -> bool {
    url.split_once(':')
        .is_some_and(|(host, _)| !host.is_empty() && !host.contains('/'))
}

/// A git remote's host and path, `forge.example.test/ada/data`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote(String);

impl Remote {
    /// The remote a URL names, `https://`, `ssh://` or scp-like, its user, password, port and
    /// `.git` dropped; `None` for a local path, a `file://` URL or anything without a host.
    #[must_use]
    pub fn parse(url: &str) -> Option<Self> {
        let url = url.trim();
        let (host, path) = match url.split_once("://") {
            Some(("file", _)) => return None,
            Some((_, rest)) => rest.split_once('/')?,
            None => url
                .split_once(':')
                .filter(|(host, _)| !host.contains('/'))?,
        };
        let host = host.rsplit('@').next()?.split(':').next()?;
        let path = path.trim_matches('/').trim_end_matches(".git");
        (!host.is_empty() && !path.is_empty()).then(|| Self(format!("{host}/{path}")))
    }

    /// The host and path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The host alone.
    #[must_use]
    pub fn host(&self) -> &str {
        self.0
            .split_once('/')
            .map_or(self.0.as_str(), |(host, _)| host)
    }
}

/// Where a ref last moved, as its reflog's last line says: the commit it moved to, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tip {
    /// The commit.
    pub commit: String,
    /// When it moved there, UTC.
    pub at: Instant,
}

impl Tip {
    /// The last move a reflog's text records: `<old> <new> <name> <<email>> <seconds> <zone>`,
    /// then a tab and the message; `None` for an empty log or a line not of that shape.
    #[must_use]
    pub fn of_reflog(log: &str) -> Option<Self> {
        let line = log.lines().rev().find(|line| !line.trim().is_empty())?;
        let (moved, _message) = line.split_once('\t').unwrap_or((line, ""));
        let mut fields = moved.split(' ');
        let (_old, commit) = (fields.next()?, fields.next()?);
        let seconds: i64 = moved.rsplit(' ').nth(1)?.parse().ok()?;
        Some(Self {
            commit: commit.to_owned(),
            at: chrono::DateTime::from_timestamp(seconds, 0)?.naive_utc(),
        })
    }
}

/// What the data repository's pushes read, as the push's own record has it: its upstream's
/// last move is a push that succeeded then, and a commit after it that the upstream does not
/// name is one not pushed yet, so failing since that push. `None` while it has no upstream to
/// push to, which observes nothing.
#[must_use]
pub fn pushes(head: &Tip, upstream: Option<&Tip>) -> Option<Health> {
    let upstream = upstream?;
    let pushed = Health::of(upstream.at, Outcome::Succeeded, "");
    Some(if head.commit == upstream.commit {
        pushed
    } else {
        pushed.join(&Health::of(
            head.at,
            Outcome::Failed(Failure::Unclassified),
            "its latest commit is not on its remote yet",
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::health::Reading;

    /// Every spelling of one remote reads the same, and a credential in the URL is never kept.
    #[test]
    fn a_remote_reads_as_host_and_path_and_drops_its_credential() {
        for url in [
            "https://forge.example.test/ada/data.git",
            "https://ada:token-not-shown@forge.example.test/ada/data",
            "ssh://git@forge.example.test:22/ada/data.git",
            "git@forge.example.test:ada/data.git",
        ] {
            assert_eq!(
                Remote::parse(url).map(|remote| remote.0),
                Some("forge.example.test/ada/data".to_owned()),
                "{url}"
            );
        }
        assert_eq!(Remote::parse("/srv/git/data"), None);
        assert_eq!(Remote::parse("../data"), None);
        assert_eq!(Remote::parse("file:///srv/git/data.git"), None);
    }

    #[test]
    fn the_origin_is_read_from_a_git_config() {
        let origin = |url: &str| {
            Pushed::of_git_config(&format!(
                "[core]\n\turl = nothing\n[remote \"backup\"]\n\turl = https://example.test/b\n\
                 [remote \"origin\"]\n\turl = {url}\n"
            ))
        };
        assert_eq!(
            origin("git@example.test:ada/data.git"),
            Pushed::To(Remote("example.test/ada/data".to_owned()))
        );
        assert_eq!(
            origin("/srv/git/data.git"),
            Pushed::Folder("/srv/git/data.git".to_owned())
        );
        assert_eq!(
            origin("file:///srv/git/data.git"),
            Pushed::Folder("/srv/git/data.git".to_owned())
        );
        assert_eq!(origin("../data"), Pushed::Folder("../data".to_owned()));
        // A URL with a credential and no path is refused whole, and never repeated.
        for refused in ["https://user:tok@host", "git@host:", "ssh://host"] {
            let read = origin(refused);
            assert_eq!(
                read,
                Pushed::Unread("an origin that names no host and path".to_owned()),
                "{refused}"
            );
            assert!(!format!("{read:?}").contains("tok"));
        }
        assert_eq!(Pushed::of_git_config("[core]\n"), Pushed::Nowhere);
    }

    #[test]
    fn a_dot_git_file_names_its_gitdir() {
        assert_eq!(
            Pushed::gitdir("gitdir: /srv/data/.git/worktrees/w\n"),
            Some("/srv/data/.git/worktrees/w")
        );
        assert_eq!(Pushed::gitdir("gitdir:\n"), None);
        assert_eq!(Pushed::gitdir("[core]\n"), None);
    }

    const LOG: &str = "0000 1111 Ada <ada@example.test> 1791363600 +0000\tcommit: one\n\
                       1111 2222 Ada <ada@example.test> 1791367200 +0200\tcommit: two\n";

    fn at(hour: u32) -> Instant {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 7)
            .and_then(|day| day.and_hms_opt(hour, 0, 0))
            .expect("an instant")
    }

    /// A reflog's last line is where its ref moved last, in UTC whatever zone it was written in.
    #[test]
    fn a_reflog_names_its_last_move() {
        assert_eq!(
            Tip::of_reflog(LOG),
            Some(Tip {
                commit: "2222".to_owned(),
                at: at(10),
            })
        );
        assert_eq!(Tip::of_reflog(""), None);
        assert_eq!(Tip::of_reflog("not a reflog\n"), None);
    }

    /// A head its upstream names was pushed when the upstream moved; one it does not name and
    /// that moved after is failing since that push; no upstream observes nothing.
    #[test]
    fn a_push_reads_as_its_upstream_last_moved() {
        let tip = |commit: &str, hour| Tip {
            commit: commit.to_owned(),
            at: at(hour),
        };
        let read = |head: Tip, upstream: Option<Tip>| {
            pushes(&head, upstream.as_ref()).map(|health| health.reading())
        };
        assert_eq!(
            read(tip("a", 9), Some(tip("a", 10))),
            Some(Reading::Ok { at: at(10) })
        );
        assert_eq!(
            read(tip("b", 11), Some(tip("a", 10))),
            Some(Reading::Failing {
                at: at(11),
                class: Failure::Unclassified,
                words: "its latest commit is not on its remote yet".to_owned(),
                since: at(10),
            })
        );
        assert_eq!(
            read(tip("b", 9), Some(tip("a", 10))),
            Some(Reading::Ok { at: at(10) }),
            "an upstream that moved after the head is ahead of it, not behind"
        );
        assert_eq!(read(tip("a", 9), None), None);
    }

    /// The push token is asked for only where the data repository pushes to GitHub: a folder, no
    /// origin or another host needs none, and an unread configuration may, so it still asks.
    #[test]
    fn only_a_push_to_github_needs_the_token() {
        let read = |config: &str| Pushed::of_git_config(config).needs_push_token();
        assert!(read(
            "[remote \"origin\"]\n\turl = https://github.com/ada/data.git\n"
        ));
        assert!(read(
            "[remote \"origin\"]\n\turl = https://GitHub.com/ada/data\n"
        ));
        assert!(!read(
            "[remote \"origin\"]\n\turl = git@forge.example.test:ada/data.git\n"
        ));
        assert!(!read("[remote \"origin\"]\n\turl = /srv/backup/data\n"));
        assert!(!read("[core]\n\tbare = false\n"));
        assert!(Pushed::Unread("permission denied".to_owned()).needs_push_token());
    }
}
