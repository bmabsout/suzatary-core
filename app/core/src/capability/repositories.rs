//! Reading the owner's repositories; publishing is a tap's outward act and lives elsewhere.

pub use crate::repos::{CommitId, Patch, RepoName, RepoPath, RepoRow, Rev, Visibility};

/// The most of one file [`Repositories::blob`] answers: what one email may carry.
pub const MAX_FILE_BYTES: u64 = crate::secretary::attached::MAX_TOTAL;

/// The most of one file [`readable`] shows.
pub const MAX_TEXT_BYTES: usize = 100 * 1024;

/// Why a read produced nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoFailure {
    /// The instance file does not declare it, the token cannot reach it, or it does not exist.
    #[error("no repository {0} among those this deployment declares and its GitHub token reaches")]
    Unknown(String),
    /// The path or commit is not in the repository.
    #[error("not found: {0}")]
    Missing(String),
    /// The file is larger than [`MAX_FILE_BYTES`].
    #[error("{path} is {size} bytes; at most {MAX_FILE_BYTES} of one file is read")]
    TooLarge {
        /// The file.
        path: String,
        /// Its size.
        size: u64,
    },
    /// The patch does not apply to the commit it names.
    #[error("the patch does not apply: {0}")]
    Conflict(String),
    /// The repository is public and its entry does not say `public = true`.
    #[error(
        "{0} is public, and its [[secretary.repos]] entry does not say public = true, so no \
         patch goes there"
    )]
    Public(String),
    /// GitHub, the network or `git` itself.
    #[error("unavailable: {0}")]
    Unavailable(String),
}

/// The repositories this deployment declares, as text.
pub trait Repositories: Sync {
    /// Every repository the token can reach, by name.
    ///
    /// # Errors
    /// [`RepoFailure::Unavailable`] when GitHub cannot be asked.
    fn repos(&self) -> Result<Vec<RepoRow>, RepoFailure>;

    /// The commit `at` names, or the default branch's tip when `at` is `None`, freshly fetched.
    /// A commit resolves to itself, so a resolved revision means the same thing at any time.
    ///
    /// # Errors
    /// [`RepoFailure`]; a name the repository does not hold is [`RepoFailure::Missing`].
    fn resolve(&self, repo: &RepoName, at: Option<&Rev>) -> Result<CommitId, RepoFailure>;

    /// The entries under `dir` at `at`, directories ending in `/`.
    ///
    /// # Errors
    /// [`RepoFailure`].
    fn tree(
        &self,
        repo: &RepoName,
        at: &CommitId,
        dir: &RepoPath,
    ) -> Result<Vec<String>, RepoFailure>;

    /// One file's bytes at `at`, whole, text or not.
    ///
    /// # Errors
    /// [`RepoFailure`]; one over [`MAX_FILE_BYTES`] is [`RepoFailure::TooLarge`].
    fn blob(&self, repo: &RepoName, at: &CommitId, path: &RepoPath)
    -> Result<Vec<u8>, RepoFailure>;

    /// Lines containing `needle` at `at`, as `path:line:text`, capped.
    ///
    /// # Errors
    /// [`RepoFailure`].
    fn grep(
        &self,
        repo: &RepoName,
        at: &CommitId,
        needle: &str,
    ) -> Result<Vec<String>, RepoFailure>;

    /// Whether `patch` applies cleanly to `base`, touching nothing.
    ///
    /// # Errors
    /// [`RepoFailure::Conflict`] with `git apply`'s own words.
    fn applies(&self, repo: &RepoName, base: &CommitId, patch: &Patch) -> Result<(), RepoFailure>;

    /// Whether a patch may be pushed to `repo`, read as `visibility`, as the instance file
    /// declares it ([`crate::repos::Declared::pushable`]).
    ///
    /// # Errors
    /// [`RepoFailure::Unknown`] or [`RepoFailure::Public`].
    fn pushable(&self, repo: &RepoName, visibility: Visibility) -> Result<(), RepoFailure>;
}

/// A file's bytes as text to read, cut at [`MAX_TEXT_BYTES`] and saying so.
///
/// # Errors
/// [`RepoFailure::Missing`] in words for a binary file, which is attached, never read.
pub fn readable(path: &RepoPath, bytes: &[u8]) -> Result<String, RepoFailure> {
    if bytes.contains(&0) {
        return Err(RepoFailure::Missing(format!(
            "{} is binary; only text files can be read, and propose_email attaches one",
            path.as_str()
        )));
    }
    let text = String::from_utf8_lossy(bytes);
    if text.len() <= MAX_TEXT_BYTES {
        return Ok(text.into_owned());
    }
    let mut cut = MAX_TEXT_BYTES;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    Ok(format!(
        "{}\n[cut at {MAX_TEXT_BYTES} of {} bytes]",
        &text[..cut],
        text.len()
    ))
}
