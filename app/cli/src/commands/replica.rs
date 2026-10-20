//! `suzatary replica init|sync <host>` — the mail replica, over muchsync.
//!
//! Run on the replica against the fetcher over SSH, with zero credentials: a replica
//! holds the corpus and no way to fetch it fresh, which is what makes one safe to have.
//! It reads no instance file: it is about a maildir and a host, not a deployment.

use anyhow::Context;

/// What `suzatary replica` was told.
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    action: Action,
}

/// The two acts, each against the fetcher's SSH host.
#[derive(Debug, clap::Subcommand)]
enum Action {
    /// Seed ~/Mail from the fetcher.
    Init {
        /// The SSH host of the fetcher.
        host: String,
    },
    /// The steady-state pull.
    Sync {
        /// The SSH host of the fetcher.
        host: String,
    },
}

/// The `muchsync` arguments for a replica action, pure so it is testable.
#[must_use]
fn command(action: &Action) -> Vec<String> {
    match action {
        Action::Init { host } => {
            let home = std::env::var("HOME").unwrap_or_default();
            let maildir = std::path::Path::new(&home).join("Mail");
            vec![
                "--init".to_owned(),
                maildir.to_string_lossy().into_owned(),
                host.clone(),
            ]
        }
        Action::Sync { host } => vec![host.clone()],
    }
}

/// `suzatary replica init|sync <host>`.
pub fn run(args: &Args) -> anyhow::Result<()> {
    // The operator's whole environment, on purpose: an interactive command run at a terminal, and
    // muchsync's ssh needs the operator's agent and config to reach the other host.
    let status = std::process::Command::new("muchsync")
        .args(command(&args.action))
        .status()
        .context("running muchsync — is it on PATH? (it is a system package, not a shell one)")?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    use super::{Action, command};

    /// `init` seeds and `sync` pulls; the difference is one flag and the maildir path.
    #[test]
    fn init_seeds_the_maildir_and_sync_is_the_bare_pull() {
        let host = || "box".to_owned();
        let init = command(&Action::Init { host: host() });
        assert_eq!(init[0], "--init");
        assert!(init[1].ends_with("/Mail"), "{init:?}");
        assert_eq!(init[2], "box");
        assert_eq!(command(&Action::Sync { host: host() }), vec![host()]);
    }
}
