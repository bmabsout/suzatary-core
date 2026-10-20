//! What a tapped card or a hand edit can change in the instance file while the server runs,
//! held under one lock that the routes and every brain read.

use std::sync::{Arc, RwLock};

use suzatary_core::connect::Channel;
use suzatary_core::mail::account::Roster;
use suzatary_core::models::Models;
use suzatary_core::nonempty::NonEmpty;
use suzatary_core::plugin::Manifest;
use suzatary_core::settings::{DeclaredProvider, Settables};

use crate::instance::Instance;

/// The instance file's declarations a running server reads. A move of the file replaces them
/// whole, so no reader sees a new provider without its key's name, an order naming a provider
/// it cannot find, or a calendar without the name its link is set under.
#[derive(Debug, Clone)]
pub struct Live {
    /// The secret names the deployment may set.
    pub settables: Settables,
    /// Every declared `[[providers]]` entry.
    pub providers: Vec<DeclaredProvider>,
    /// Each job's model order and the reserve.
    pub models: Models,
    /// `[notify] channels`, `None` for no push.
    pub channels: Option<NonEmpty<Channel>>,
    /// `[[mail.accounts]]`.
    pub accounts: Roster,
    /// `[[plugins]]`, each name once.
    pub plugins: Vec<Manifest>,
}

/// The one lock over [`Live`], shared by the routes, the brains and the compactor.
pub type Shared = Arc<RwLock<Live>>;

impl Live {
    /// What `instance` declares.
    #[must_use]
    pub fn of(instance: &Instance) -> Self {
        Self {
            settables: instance.settables.clone(),
            providers: instance.providers.clone(),
            models: instance.models.clone(),
            channels: instance.notify.clone(),
            accounts: instance.mail.accounts.clone(),
            plugins: instance.plugins.manifests.clone(),
        }
    }

    /// Behind the one lock.
    #[must_use]
    pub fn shared(self) -> Shared {
        Arc::new(RwLock::new(self))
    }
}
