//! Owner-only files and directories for every bench write, since they hold the owner's mail
//! and the umask alone is not enough.
//!
//! The mode is set at the `open`/`mkdir` syscall, not by a later `chmod`, which would leave a
//! window where the file exists at whatever the umask gave it.

use std::fs::{DirBuilder, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The underlying `open` or `mkdir` failed.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PrivacyError(#[from] std::io::Error);

/// Create (or truncate) `path` as a `0o600` file; every mail-derived write opens through this.
///
/// `open(2)`'s mode applies only on creation, so a file left by an interrupted write keeps its
/// old mode; the mode is re-asserted on the handle unconditionally.
///
/// # Errors
/// [`PrivacyError`] when `open` or the permission re-assertion fails.
pub fn private_file(path: &Path) -> Result<File, PrivacyError> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(PrivacyError)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(PrivacyError)?;
    Ok(file)
}

/// Create `path` and every missing ancestor as `0o700` directories, so a private file is
/// never reachable through a world-readable parent.
///
/// # Errors
/// [`PrivacyError`] when `mkdir` fails.
pub fn private_dir(path: &Path) -> Result<(), PrivacyError> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(PrivacyError)
}

/// The git worktree `path` would sit in, if any: the corpus is never committed, so a run
/// directory inside one is refused before anything is written there.
#[must_use]
pub fn worktree_of(path: &Path) -> Option<PathBuf> {
    std::path::absolute(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::{private_dir, private_file, worktree_of};

    #[test]
    fn private_file_is_mode_600() {
        let dir = tempfile_dir();
        let path = dir.join("case.json");
        let file = private_file(&path).expect("create");
        drop(file);
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn private_file_truncates_an_existing_file() {
        let dir = tempfile_dir();
        let path = dir.join("case.json");
        std::fs::write(&path, b"stale and much longer than what follows").expect("seed");
        {
            let mut file = private_file(&path).expect("create");
            std::io::Write::write_all(&mut file, b"fresh").expect("write");
        }
        let text = std::fs::read_to_string(&path).expect("read");
        assert_eq!(text, "fresh");
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn private_dir_is_mode_700() {
        let dir = tempfile_dir();
        let nested = dir.join("run").join(".chains");
        private_dir(&nested).expect("create");
        let mode = std::fs::metadata(&nested)
            .expect("stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
        // The intermediate ancestor `mkdir -p` created is private too.
        let mode = std::fs::metadata(dir.join("run"))
            .expect("stat")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[test]
    fn a_directory_below_a_worktree_is_found_and_one_outside_is_not() {
        let dir = tempfile_dir();
        assert_eq!(worktree_of(&dir.join("run")), None);
        std::fs::create_dir(dir.join(".git")).expect("a synthetic worktree");
        assert_eq!(worktree_of(&dir.join("not-yet").join("run")), Some(dir));
    }

    /// A fresh scratch directory, named per run so parallel tests never collide.
    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "suzatary-bench-privacy-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }
}
