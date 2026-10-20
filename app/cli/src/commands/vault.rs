//! `suzatary vault backup`: an encrypted state snapshot, human-only and on no agent trait.

use std::path::Path;

use crate::instance::Instance;

/// What `suzatary vault` was told.
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    action: Action,
}

/// The one act.
#[derive(Debug, clap::Subcommand)]
enum Action {
    /// Snapshot the box's state, encrypted, to Proton Drive.
    Backup,
}

/// `suzatary vault <action>`.
pub fn run(path: &Path, args: &Args) -> anyhow::Result<()> {
    match args.action {
        Action::Backup => super::script(&Instance::open(path)?, "vault-backup.sh"),
    }
}
