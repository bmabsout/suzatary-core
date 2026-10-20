//! The syncer: pull every account by its strategy, index once, and tag the
//! leaves whose sync path carries no tags. The one module that writes the
//! maildir and the index, beside `crate::older`, which writes what a server
//! search matched into leaves of its own and indexes through [`Syncer::index`].
//!
//! One account's failure is an [`AccountSync`] with `ok: false`, and the run
//! goes on. `mbsync` is pinned to the repository's configuration with `-c`:
//! which file it reads is a security property.

use std::path::{Path, PathBuf};

use suzatary_core::connect::Failure;
use suzatary_core::mail::account::{MailAccount, Roster, Sync, folder_slug};
use suzatary_core::mail::health::Outcome;
use suzatary_core::mail::imap::{ImapError, Pull, SyncState, window_start};
use suzatary_core::mail::verbs::{AccountSync, Backfilled, SyncReport};

use crate::imap::{Folder, Outlook};
use crate::oauth::TokenStore;
use crate::present::Present as _;
use crate::run::{Cmd, succeed};

/// The file whose hold a sync and a removed account's mail delete both take, in the sync's own
/// state directory.
#[must_use]
pub fn lock(state_dir: &Path) -> PathBuf {
    state_dir.join("sync.lock")
}

/// The syncer, bound to one deployment.
pub struct Syncer {
    accounts: Roster,
    mail_root: PathBuf,
    notmuch: PathBuf,
    mbsyncrc: PathBuf,
    state_dir: PathBuf,
    tokens: TokenStore,
    grants: crate::google::Grants,
}

impl std::fmt::Debug for Syncer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Syncer")
            .field("mail_root", &self.mail_root)
            .field("accounts", &self.accounts.accounts().len())
            .finish_non_exhaustive()
    }
}

impl Syncer {
    /// Build one out of what the instance file said.
    #[must_use]
    pub fn new(
        accounts: Roster,
        mail_root: impl Into<PathBuf>,
        notmuch: impl Into<PathBuf>,
        mbsyncrc: impl Into<PathBuf>,
        state_dir: impl Into<PathBuf>,
        (tokens, grants): (TokenStore, crate::google::Grants),
    ) -> Self {
        Self {
            accounts,
            mail_root: mail_root.into(),
            notmuch: notmuch.into(),
            mbsyncrc: mbsyncrc.into(),
            state_dir: state_dir.into(),
            tokens,
            grants,
        }
    }

    /// The roster this syncer pulls.
    #[must_use]
    pub fn accounts(&self) -> &Roster {
        &self.accounts
    }

    /// Pull every account, index once, tag the leaves that need it, under [`lock`]'s hold, so a
    /// removed account's mail deleted meanwhile is not pulled back.
    #[must_use]
    pub fn sync(&self, today: chrono::NaiveDate) -> SyncReport {
        // A sync that could not take the hold still syncs, and its report says so.
        let held = crate::durable::hold(&lock(&self.state_dir));
        let unheld = held
            .as_ref()
            .err()
            .map(|failed| format!("the sync's hold could not be taken: {failed}"));
        let since = window_start(today);
        let synced: Vec<AccountSync> = self
            .accounts
            .accounts()
            .iter()
            .map(|account| self.one(account, since))
            .collect();

        let indexed = said(&self.notmuch(&["new"])).unwrap_or_else(|failed| failed);
        // lieer maps Gmail's labels to tags; IMAP and mbsync leave a folder
        // untagged, and untagged mail never reaches the queue, so each account's failure to
        // tag joins the report's line beside what indexing said.
        let untagged = self
            .accounts
            .accounts()
            .iter()
            .filter(|account| account.label_from_folder())
            .filter_map(|account| {
                said(&self.notmuch(&["tag", "+inbox", &account.inbox_query()]))
                    .err()
                    .map(|failed| format!("tagging {}'s inbox: {}", account.email, tail(&failed)))
            });
        SyncReport {
            synced,
            notmuch: notmuch_line(&tail(&indexed), untagged),
            unheld,
        }
    }

    fn notmuch(&self, args: &[&str]) -> Cmd {
        Cmd::new(&self.notmuch)
            .args(args)
            .pass(crate::mailbox::NOTMUCH)
    }

    fn one(&self, account: &MailAccount, since: chrono::NaiveDate) -> AccountSync {
        let pulled = |said: Result<String, String>| match said {
            Ok(output) => (Outcome::Succeeded, tail(&output)),
            Err(failed) => (Outcome::Failed(Failure::Unclassified), tail(&failed)),
        };
        let (outcome, tail) = match &account.sync {
            Sync::Gmail => pulled(said(
                &Cmd::new("gmi")
                    .arg("sync")
                    .cwd(self.mail_root.join(&account.subdir))
                    .pass(crate::run::NETWORK),
            )),
            Sync::Google => match self.google(account) {
                Ok(()) => pulled(said(
                    &Cmd::new("gmi")
                        .arg("sync")
                        .cwd(self.mail_root.join(&account.subdir))
                        .pass(crate::run::NETWORK),
                )),
                Err(failed) => (Outcome::Failed(failed.class()), failed.to_string()),
            },
            // mbsync's `PassCmd` asks the secret store, so it is given what the store reads.
            Sync::Proton(proton) => pulled(said(
                &Cmd::new("mbsync")
                    .arg("-c")
                    .arg(&self.mbsyncrc)
                    .arg(&proton.channel)
                    .pass(crate::secrets::STORE),
            )),
            Sync::MsImap(ms) => match self.microsoft(account, &ms.sent_folder, since) {
                Ok(said) => (Outcome::Succeeded, said),
                Err(failed) => (Outcome::Failed(failed.class()), failed.to_string()),
            },
        };
        AccountSync {
            account: account.email.to_string(),
            outcome,
            tail,
        }
    }

    /// Ready a Google account's maildir for lieer: an access token minted from its grant and
    /// handed to lieer (never the grant), and lieer's repository begun without its own sign-in
    /// where none is.
    fn google(&self, account: &MailAccount) -> Result<(), crate::google::NotReady> {
        let address = suzatary_core::mail::address::EmailAddress::new(account.email.as_str())
            .map_err(|refused| crate::google::NotReady::Failed(refused.message().to_owned()))?;
        let maildir = self.mail_root.join(&account.subdir);
        self.grants.ready(&address, &maildir)?;
        if !maildir.join(".gmailieer.json").exists() {
            said(
                &Cmd::new("gmi")
                    .args(["init", "--no-auth", account.email.as_str()])
                    .cwd(&maildir)
                    .pass(crate::run::NETWORK),
            )
            .map_err(|failed| crate::google::NotReady::Failed(tail(&failed)))?;
        }
        Ok(())
    }

    /// The account's IMAP token, minted silently; an account that needs a
    /// sign-in is this sync's failure, in the sentence that names the verb.
    fn imap_token(
        &self,
        account: &MailAccount,
    ) -> Result<suzatary_core::capability::ImapToken, ImapError> {
        self.tokens
            .access::<suzatary_core::mail::oauth::Imap>(account)
            .map_err(|failed| ImapError::Unauthorised(failed.to_string()))?
            .required()
            .map_err(ImapError::NeedsLogin)
    }

    /// The INBOX window, then Sent, read-only into its own leaf and already
    /// read, so it is searchable and never queued.
    fn microsoft(
        &self,
        account: &MailAccount,
        sent_folder: &str,
        since: chrono::NaiveDate,
    ) -> Result<String, ImapError> {
        let token = self.imap_token(account)?;
        let mut session = Outlook::connect(
            crate::imap::IMAP_HOST,
            crate::imap::IMAP_PORT,
            account.email.as_str(),
            &token,
        )?;

        let inbox = self.pull_into(&mut session, account, "Inbox", None, &Pull::Inbox { since })?;
        let sent = self.pull_into(
            &mut session,
            account,
            "Sent",
            Some(sent_folder),
            &Pull::Backfill {
                folder: sent_folder.to_owned(),
                limit: None,
            },
        )?;
        session.logout()?;
        Ok(format!("{inbox} new, {sent} sent"))
    }

    /// One [`Pull`] into one leaf, with its own state file so a backfill and
    /// the INBOX sync never share UID sets; `state_key` is `None` for INBOX.
    fn pull_into(
        &self,
        session: &mut Outlook,
        account: &MailAccount,
        leaf: &str,
        state_key: Option<&str>,
        plan: &Pull,
    ) -> Result<usize, ImapError> {
        let folder = Folder::new(self.mail_root.join(&account.subdir).join(leaf));
        folder.ensure()?;
        let path = self.state_path(account, state_key);
        let state = read_sync_state(&path)?;
        let done = suzatary_core::mail::imap::pull(session, &folder, state, plan)?;
        write_sync_state(&path, &done.state)?;
        Ok(done.fetched)
    }

    fn state_path(&self, account: &MailAccount, key: Option<&str>) -> PathBuf {
        state_file(&self.state_dir, account, key)
    }

    /// `notmuch new`, as a sync runs it, for files written outside a sync; what it said.
    ///
    /// # Errors
    /// What notmuch said when it failed, or why it could not be run.
    pub fn index(&self) -> Result<String, String> {
        said(&self.notmuch(&["new"]))
    }

    /// A one-off widening of the corpus for one server folder, kept out of
    /// the routine sync because it is expensive.
    pub fn backfill(
        &self,
        account: &MailAccount,
        folder: &str,
        subdir: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Backfilled, ImapError> {
        let token = self.imap_token(account)?;
        let mut session = Outlook::connect(
            crate::imap::IMAP_HOST,
            crate::imap::IMAP_PORT,
            account.email.as_str(),
            &token,
        )?;
        let leaf = subdir.map_or_else(|| folder_slug(folder), str::to_owned);
        let fetched = self.pull_into(
            &mut session,
            account,
            &leaf,
            Some(folder),
            &Pull::Backfill {
                folder: folder.to_owned(),
                limit,
            },
        )?;
        session.logout()?;
        self.index().map_err(|failed| ImapError::Maildir {
            what: "indexed",
            why: format!(
                "{fetched} messages were written and notmuch could not index them: {failed}"
            ),
        })?;
        Ok(Backfilled {
            account: account.email.to_string(),
            folder: folder.to_owned(),
            fetched,
        })
    }
}

/// Where one leaf's [`SyncState`] is kept under `state_dir`: `key` is the server folder a leaf
/// other than the INBOX pull's is kept for, `None` for INBOX.
pub(crate) fn state_file(state_dir: &Path, account: &MailAccount, key: Option<&str>) -> PathBuf {
    let name = match key {
        None => format!("{}.json", account.slug()),
        Some(folder) => format!("{}-{}.json", account.slug(), folder_slug(folder)),
    };
    state_dir.join(name)
}

pub(crate) fn read_sync_state(path: &Path) -> Result<SyncState, ImapError> {
    let failed = |why: &dyn std::fmt::Display| ImapError::State {
        what: "read",
        why: format!("{}: {why}", path.display()),
    };
    // A state file that is not there is a first sync, which is a fact.
    match std::fs::read_to_string(path)
        .present()
        .map_err(|e| failed(&e))?
    {
        None => Ok(SyncState::default()),
        Some(text) => serde_json::from_str(&text).map_err(|e| failed(&e)),
    }
}

pub(crate) fn write_sync_state(path: &Path, state: &SyncState) -> Result<(), ImapError> {
    let failed = |why: String| ImapError::State { what: "write", why };
    let text = serde_json::to_string_pretty(state).map_err(|e| failed(e.to_string()))?;
    crate::durable::replace(
        path,
        (text + "\n").as_bytes(),
        crate::durable::Mode::Private,
    )
    .map_err(|e| failed(format!("{}: {e}", path.display())))
}

/// What `cmd` said on both streams when it exited zero (`gmi` reports progress on stderr), or
/// why it did not. A program that is not there is one failed account, not a crashed sweep.
fn said(cmd: &Cmd) -> Result<String, String> {
    succeed(cmd)
        .map(|ran| format!("{}{}", ran.out(), ran.err()))
        .map_err(|failed| failed.to_string())
}

/// The report's notmuch line: what indexing said, then each account that could not be tagged,
/// leaving out an index line that is empty so the line never opens with a separator.
fn notmuch_line(indexed: &str, untagged: impl Iterator<Item = String>) -> String {
    std::iter::once(indexed.to_owned())
        .filter(|line| !line.is_empty())
        .chain(untagged)
        .collect::<Vec<_>>()
        .join("; ")
}

/// The last line of some output, as an [`Excerpt`](suzatary_core::excerpt::Excerpt): it is kept in
/// the sync report and read by the agent, so it names no address.
fn tail(text: &str) -> String {
    suzatary_core::excerpt::Excerpt::of(text.trim().lines().next_back().unwrap_or(""))
        .as_str()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notmuch_line_never_opens_with_a_separator() {
        let untagged = || vec!["tagging a's inbox: no".to_owned()].into_iter();
        assert_eq!(notmuch_line("", untagged()), "tagging a's inbox: no");
        assert_eq!(
            notmuch_line("Added 2 new messages.", untagged()),
            "Added 2 new messages.; tagging a's inbox: no"
        );
        assert_eq!(notmuch_line("", std::iter::empty()), "");
    }

    /// No state is a first sync; a state that is there and cannot be read is an error, never
    /// a first sync that would fetch the whole folder again.
    #[test]
    fn only_a_missing_sync_state_is_a_first_sync() {
        let dir = tempfile::tempdir().expect("a scratch dir");
        let path = dir.path().join("account.json");
        assert_eq!(
            read_sync_state(&path).expect("a first sync"),
            SyncState::default()
        );
        std::fs::create_dir(&path).expect("a directory where the file belongs");
        assert!(read_sync_state(&path).is_err());
    }

    #[test]
    fn the_tail_is_the_last_line_of_either_stream() {
        assert_eq!(tail("one\ntwo\nthree\n"), "three");
        assert_eq!(tail("  only  "), "only");
        assert_eq!(tail(""), "");
        assert_eq!(tail("\n\n"), "");
    }

    #[test]
    fn a_program_that_is_not_there_reports_rather_than_panicking() {
        let output =
            said(&Cmd::new("/nonexistent/mbsync").args(["-c", "x", "y"])).expect_err("not there");
        assert!(output.contains("/nonexistent/mbsync"), "{output}");
    }

    #[test]
    fn a_sync_state_file_round_trips_and_an_absent_one_is_a_first_sync() {
        let dir = std::env::temp_dir().join(format!(
            "suzatary-sync-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let path = dir.join("her-at-example-test.json");
        assert_eq!(
            read_sync_state(&path).expect("a first sync"),
            SyncState::default()
        );

        let state = SyncState {
            uidvalidity: Some("7".to_owned()),
            synced: [1u32, 2, 3].into_iter().collect(),
            seen_pushed: [1u32].into_iter().collect(),
        };
        write_sync_state(&path, &state).expect("writes");
        assert_eq!(read_sync_state(&path).expect("reads"), state);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
