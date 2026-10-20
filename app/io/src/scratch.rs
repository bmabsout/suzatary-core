//! A scratch directory: made fresh for one job, private to this user, named
//! unpredictably, and removed with everything in it when dropped.
//!
//! It is created exclusively (`mkdtemp`), so a directory another user made in
//! a shared root beforehand is never adopted, and `0o700`, so nothing a job
//! unpacks or compiles in it is readable by anyone else.

use std::fs::Permissions;
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use tempfile::TempDir;

/// One job's directory, gone when this is dropped.
#[derive(Debug)]
pub struct Scratch(TempDir);

impl Scratch {
    /// A fresh directory under `root`, which is made when absent.
    ///
    /// # Errors
    /// `root` or the directory could not be made.
    pub fn under(root: &Path, prefix: &str) -> io::Result<Self> {
        std::fs::create_dir_all(root)?;
        builder(prefix).tempdir_in(root).map(Self)
    }

    /// A fresh directory under the system's temporary directory.
    ///
    /// # Errors
    /// The directory could not be made.
    pub fn temporary(prefix: &str) -> io::Result<Self> {
        builder(prefix).tempdir().map(Self)
    }

    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.0.path()
    }
}

fn builder(prefix: &str) -> tempfile::Builder<'_, '_> {
    let mut builder = tempfile::Builder::new();
    builder
        .prefix(prefix)
        .permissions(Permissions::from_mode(0o700));
    builder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scratch_is_private_unique_and_removed_whole() {
        let root = tempfile::tempdir().expect("a root");
        let first = Scratch::under(&root.path().join("made"), "job-").expect("made");
        let second = Scratch::under(&root.path().join("made"), "job-").expect("made");
        assert_ne!(first.path(), second.path());
        let mode = std::fs::metadata(first.path())
            .expect("there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
        std::fs::create_dir_all(first.path().join("a/b")).expect("a tree");
        std::fs::write(first.path().join("a/b/c"), b"x").expect("a file");
        let gone = first.path().to_owned();
        drop(first);
        assert!(!gone.exists(), "a directory with contents is removed");
        assert!(second.path().is_dir());
    }
}
