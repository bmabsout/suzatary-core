//! The authorities a call lends its plugin's imports, as one record passed by value.

use std::sync::Arc;

use suzatary_core::capability::Clock;
use suzatary_io::http::Http;
use suzatary_io::secrets::Resolves;

/// What `fetch`, `clock` and `log` may reach for one call. A plugin holds none of these: each
/// import is the host's, and these are what the host uses to answer it.
///
/// `http` is the transport, and its reach bounds every plugin's: the deployment passes
/// `suzatary_io::http::Ureq::public_only()`, which follows no redirect and refuses any name
/// resolving off the public internet, so no manifest can aim a plugin at this box or the
/// tailnet. A test passes a transport to a loopback server it started.
#[derive(Clone)]
pub struct Effects {
    /// The one way a request leaves.
    pub http: Arc<dyn Http>,
    /// The secret store, asked by name only for a credential the manifest declares.
    pub secrets: Arc<dyn Resolves + Send + Sync>,
    /// The wall clock.
    pub clock: Arc<dyn Clock + Send>,
}
