//! The binary's modules as a library, so its wiring can be integration-tested.
//!
//! `verb.rs` is the argument tree and a `match`, which both binaries parse; every verb
//! is here, one module per subcommand family, and an unknown verb is a usage error with
//! no fallback dispatch.
//! `serve::State` is generic so a test can hand the router fakes the binary never chooses.

#![forbid(unsafe_code)]

pub mod commands;
pub mod instance;
pub mod paths;
pub mod stamp;
pub mod verb;
pub mod wiring;

/// The HTTP surface, re-exported because it is the one verb a test drives directly.
pub use commands::serve;
