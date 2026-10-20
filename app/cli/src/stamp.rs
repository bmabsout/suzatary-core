//! What this binary runs as: its release, read at run time from the file installed beside it
//! ([`BESIDE`]), and its build (`SUZATARY_BUILD`), a hash of the sources it was compiled from,
//! which the flake sets. The release is data, not a compiled input: the flake builds the binary
//! without it and copies it into a small package together with `app/release.toml`, so a release
//! cut rebuilds nothing. A cargo build has no release file beside it and no build stamp, and
//! says `dev`.

use std::path::{Path, PathBuf};

use suzatary_core::release::{Released, ReleasedError};

/// Where the release file is installed, relative to the directory above the executable's own
/// (`<out>/bin/suzatary` reads `<out>/share/suzatary/release.toml`).
pub const BESIDE: &str = "share/suzatary/release.toml";

/// The release a binary runs as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Release {
    /// No release file beside the executable: a cargo build, or a binary copied out alone.
    Dev,
    /// The release the file beside the executable holds.
    Released(Released),
}

/// Why the release beside an executable could not be read.
#[derive(Debug)]
pub enum Unread {
    /// The running executable's own path.
    Exe(std::io::Error),
    /// The release file is there and could not be read.
    File(PathBuf, std::io::Error),
    /// The release file does not hold a release.
    Released(ReleasedError),
}

impl std::fmt::Display for Unread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unread::Exe(e) => write!(f, "this executable's path: {e}"),
            Unread::File(path, e) => write!(f, "{}: {e}", path.display()),
            Unread::Released(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Unread {}

impl Release {
    /// The release installed beside the executable at `exe` ([`BESIDE`]).
    ///
    /// # Errors
    /// A release file that cannot be read or does not hold a release.
    pub fn beside(exe: &Path) -> Result<Release, Unread> {
        let Some(prefix) = exe.parent().and_then(Path::parent) else {
            return Ok(Release::Dev);
        };
        let path = prefix.join(BESIDE);
        match std::fs::read_to_string(&path) {
            Ok(text) => Released::parse(&text)
                .map(Release::Released)
                .map_err(Unread::Released),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Release::Dev),
            Err(e) => Err(Unread::File(path, e)),
        }
    }
}

/// The release this binary runs as, read once from beside the running executable. The one
/// reader of a binary's release: `--version`, `doctor`, the MCP handshake and `GET /api/about`
/// all ask it.
///
/// # Errors
/// The executable's path or its release file could not be read, or the file holds no release.
/// The flake's package runs `--version` before it ships, so a release that does not read
/// never ships.
pub fn release() -> Result<&'static Release, &'static Unread> {
    static RELEASE: std::sync::OnceLock<Result<Release, Unread>> = std::sync::OnceLock::new();
    RELEASE
        .get_or_init(|| {
            std::env::current_exe()
                .map_err(Unread::Exe)
                .and_then(|exe| Release::beside(&exe))
        })
        .as_ref()
}

/// The build this binary was stamped with, or `dev`.
#[must_use]
pub fn build() -> &'static str {
    option_env!("SUZATARY_BUILD").unwrap_or("dev")
}

/// The version `--version`, `doctor` and the MCP server's handshake name: the release's, `dev`
/// with no release file, or what is wrong with the file. Read once.
#[must_use]
pub fn version() -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(|| match release() {
        Ok(Release::Released(released)) => released.version.to_string(),
        Ok(Release::Dev) => "dev".to_owned(),
        Err(unread) => format!("unreadable ({unread})"),
    })
}

#[cfg(test)]
mod tests {
    use super::{BESIDE, Release, Unread};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("suzatary-stamp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).expect("a scratch bin");
        dir
    }

    #[test]
    fn a_binary_with_no_release_beside_it_is_dev() {
        let out = scratch("dev");
        assert_eq!(
            Release::beside(&out.join("bin/suzatary")).expect("reads"),
            Release::Dev
        );
        assert_eq!(
            Release::beside(std::path::Path::new("suzatary")).expect("reads"),
            Release::Dev
        );
    }

    #[test]
    fn a_binary_reads_the_release_installed_beside_it() {
        let out = scratch("released");
        std::fs::create_dir_all(out.join("share/suzatary")).expect("a share dir");
        std::fs::write(
            out.join(BESIDE),
            "version = \"3.1.4\"\ncompletes = [\"example\"]\n",
        )
        .expect("a release");
        let Release::Released(released) =
            Release::beside(&out.join("bin/suzatary")).expect("reads")
        else {
            panic!("the release beside the binary was not read");
        };
        assert_eq!(released.version.to_string(), "3.1.4");
        assert_eq!(released.completes.len(), 1);
    }

    #[test]
    fn a_release_file_that_holds_no_release_is_refused() {
        let out = scratch("unread");
        std::fs::create_dir_all(out.join("share/suzatary")).expect("a share dir");
        std::fs::write(out.join(BESIDE), "version = \"not one\"\n").expect("a file");
        assert!(matches!(
            Release::beside(&out.join("bin/suzatary")),
            Err(Unread::Released(_))
        ));
    }
}
