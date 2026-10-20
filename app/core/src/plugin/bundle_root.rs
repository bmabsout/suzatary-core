//! Which repository a plugin's bundle is read from.

use super::Digest;

/// Where a manifest's `component` and `schema` resolve: the app's reference plugins, or the
/// owner's own repository (the data repository). Either way the bundle is loaded and admitted
/// by the same laws, and a path that resolves outside its root is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleRoot {
    /// The app's reference plugins, built by the flake from this repository.
    App,
    /// The user repository, beside the instance file, each file pinned by the sha256 the owner
    /// approved: a file that hashes otherwise is refused, not run with the origins and
    /// credentials approved for the one pinned.
    User {
        /// The component's.
        component: Digest,
        /// The declared schema's.
        schema: Digest,
    },
}
