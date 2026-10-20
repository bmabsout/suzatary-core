//! Documents on disk, as an interpreter of [`Doc`]: one file per key, replaced whole. A set
//! writes a sibling, syncs it, renames it over the file and syncs the directory, so a reader sees
//! the old document or the new one, an interrupted write leaves the old one standing, and a kept
//! write survives a crash.

use std::path::PathBuf;
use std::sync::Arc;

use suzatary_core::store::{Doc, StoreError};

use crate::durable::Mode;
use crate::present::Present as _;

/// Documents, each at the path its key names.
#[derive(Clone)]
pub struct Docs<K: 'static, T: 'static> {
    store: &'static str,
    path: Arc<dyn Fn(&K) -> PathBuf + Send + Sync>,
    codec: &'static DocCodec<K, T>,
    mode: Mode,
}

/// How a document is bytes, both ways; each side sees the key, so a content-addressed store can
/// refuse a value its name does not name.
#[derive(Debug, Clone, Copy)]
pub struct DocCodec<K, T> {
    /// The bytes to keep for this value under this key.
    pub print: fn(&K, &T) -> Result<Vec<u8>, String>,
    /// The value these bytes are under this key; `None` for bytes the key does not name, which
    /// read as absent rather than as something else.
    pub parse: fn(&K, Vec<u8>) -> Result<Option<T>, String>,
}

impl<K: 'static, T: 'static> std::fmt::Debug for Docs<K, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Docs")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl<K: 'static, T: 'static> Docs<K, T> {
    /// Documents named `store` in a failure, each at `path(key)`.
    #[must_use]
    pub fn new(
        store: &'static str,
        path: impl Fn(&K) -> PathBuf + Send + Sync + 'static,
        codec: &'static DocCodec<K, T>,
    ) -> Self {
        Self {
            store,
            path: Arc::new(path),
            codec,
            mode: Mode::Readable,
        }
    }

    /// The same documents, each readable by this user alone.
    #[must_use]
    pub fn private(self) -> Self {
        Self {
            mode: Mode::Private,
            ..self
        }
    }

    /// Where one document lives.
    #[must_use]
    pub fn path(&self, key: &K) -> PathBuf {
        (self.path)(key)
    }

    /// The document's size, from the filesystem and before its bytes are read.
    ///
    /// # Errors
    /// [`StoreError::Absent`] and [`StoreError::Unreadable`].
    pub fn size(&self, key: &K) -> Result<u64, StoreError> {
        let path = self.path(key);
        std::fs::metadata(&path)
            .present()
            .map_err(|failed| self.failed_read(&path, &failed))?
            .map(|meta| meta.len())
            .ok_or_else(|| StoreError::Absent {
                what: self.store,
                name: path.display().to_string(),
            })
    }

    fn failed_read(&self, path: &std::path::Path, failed: &std::io::Error) -> StoreError {
        StoreError::unreadable(self.store, format!("{}: {failed}", path.display()))
    }
}

impl<K, T> Doc<K, T> for Docs<K, T>
where
    K: Sync + 'static,
    T: Sync + 'static,
{
    fn get(&self, key: &K) -> Result<Option<T>, StoreError> {
        let path = self.path(key);
        let Some(bytes) = std::fs::read(&path)
            .present()
            .map_err(|failed| self.failed_read(&path, &failed))?
        else {
            return Ok(None);
        };
        (self.codec.parse)(key, bytes)
            .map_err(|why| StoreError::unreadable(self.store, format!("{}: {why}", path.display())))
    }

    fn set(&self, key: &K, value: &T) -> Result<(), StoreError> {
        let path = self.path(key);
        let unwritable = |why: &dyn std::fmt::Display| {
            StoreError::unwritable(self.store, format!("{}: {why}", path.display()))
        };
        let bytes = (self.codec.print)(key, value).map_err(|why| unwritable(&why))?;
        crate::durable::replace(&path, &bytes, self.mode).map_err(|failed| unwritable(&failed))
    }

    fn remove(&self, key: &K) -> Result<(), StoreError> {
        let path = self.path(key);
        crate::durable::remove(&path).map(|_| ()).map_err(|failed| {
            StoreError::unwritable(self.store, format!("{}: {failed}", path.display()))
        })
    }
}
