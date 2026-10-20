//! `suzatary up` — enable the fetcher's services and run the doctor.
//!
//! The fetcher lock lives in `scripts/up.sh`: it refuses to activate services on a box other
//! than the one the data repository's `FETCHER` names, since a vault-restored box holds every
//! credential and two live fetchers race pushes and invalidate each other's sessions.
//! Hostname-based, so it prevents accidents, not adversaries.

use std::path::Path;

use crate::instance::Instance;

/// `suzatary up`.
pub fn run(path: &Path) -> anyhow::Result<()> {
    super::script(&Instance::open(path)?, "up.sh")
}
