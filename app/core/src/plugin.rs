//! A plugin's authorities, as data: what its manifest may name and what the host may do for it.
//!
//! A plugin is a WebAssembly component targeting a world of `app/wit/plugin.wit`, and the
//! component model gives it exactly the functions it imports. What those functions may reach is
//! decided here, purely: a [`Manifest`] is parsed once at the boundary into a value that cannot
//! hold an origin or a credential name it did not declare, and the fence over each request is a
//! function of that value. The runtime that links the imports is `suzatary-plugin`'s.

mod bundle_root;
mod component;
mod credential;
mod digest;
mod header;
mod item;
mod knowledge;
mod law;
mod learnt;
mod ledger;
mod manifest;
mod name;
mod origin;
mod outgoing;
mod proposal;
mod refusal;
mod request;
mod response;
mod said;
mod schema_path;
mod synced;
mod url;
mod world;

pub use bundle_root::BundleRoot;
pub use component::ComponentPath;
pub use credential::{CredentialName, Placement};
pub use digest::Digest;
pub use header::HeaderName;
pub use item::{ItemRefusal, MAX_ITEM, admit};
pub use knowledge::Knowledge;
pub use law::{Answers, Broken, Law};
pub use learnt::{Learnt, learn};
pub use ledger::{Admitted, Answer, Came, LastRun, Ledger};
pub use manifest::{Manifest, ManifestError};
pub use name::PluginName;
pub use origin::{Origin, Scheme};
pub use outgoing::Outgoing;
pub use proposal::{
    MAX_OFFER, MAX_OFFERS, MAX_SOURCE, MAX_SOURCES, MAX_WHY, Offer, ProposalRefusal, Refused,
    propose,
};
pub use refusal::Refusal;
pub use request::{Request, Target, Verb};
pub use response::Response;
pub use said::{MAX_SAID, Said};
pub use schema_path::SchemaPath;
pub use synced::{Item, Synced};
pub use url::{MAX_URL, PluginUrl};
pub use world::{PACKAGE_VERSION, World};
