//! `suzatary weather` — the one line the briefing reads.
//!
//! One line on stdout, caveat included: the only consumer reads a line, so a warning
//! about an untrusted feed rides on it rather than on stderr, where nobody sees it.
//! The city and timezone come from the instance's `[persona]`.

use std::path::Path;

use crate::instance;

/// `suzatary weather`.
pub fn run(path: &Path) -> anyhow::Result<()> {
    let instance = instance::Instance::open(path)?;
    println!(
        "{}",
        suzatary_io::weather::line(chrono::Utc::now(), &instance.persona)?
    );
    Ok(())
}
