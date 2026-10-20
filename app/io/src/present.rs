//! The one reading of a failed read: a path that is not there is absent, and nothing else is.

use std::io;

/// An io answer read for presence.
pub trait Present<T> {
    /// `Ok(None)` for a path that is not there; every other failure stays the error it was, so
    /// an unreadable file or directory is never read as an empty one.
    ///
    /// # Errors
    /// Every failure but [`io::ErrorKind::NotFound`].
    fn present(self) -> io::Result<Option<T>>;
}

impl<T> Present<T> for io::Result<T> {
    fn present(self) -> io::Result<Option<T>> {
        match self {
            Ok(found) => Ok(Some(found)),
            Err(absent) if absent.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(failed) => Err(failed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_missing_path_is_absent() {
        let dir = tempfile::tempdir().expect("a scratch dir");
        let file = dir.path().join("there");
        std::fs::write(&file, "x").expect("written");
        assert_eq!(
            std::fs::read(&file).present().expect("read"),
            Some(b"x".to_vec())
        );
        assert_eq!(
            std::fs::read(dir.path().join("not-there"))
                .present()
                .expect("read"),
            None
        );
        // A directory read as a file, and a file listed as a directory, fail, and neither is
        // absence: the faults these tests inject, since a mode refuses nothing to root.
        assert!(std::fs::read(dir.path()).present().is_err());
        assert!(std::fs::read_dir(&file).present().is_err());
    }
}
