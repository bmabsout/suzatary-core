//! The plugin host: WebAssembly components that hold only the authority their world imports.
//!
//! One [`Host`] holds one wasmtime `Engine` and one `Linker`, which links exactly the imports of
//! `app/wit/plugin.wit`'s worlds and nothing from WASI. [`Host::load`] refuses a component that
//! imports anything else, so a plugin's powers are its imports and every one of them is the
//! host's: `fetch` crosses `suzatary_core::plugin::Request::admit`, the clock and the logger
//! are read-only, and a credential is resolved here by name, never handed in.
//!
//! Each call is a fresh `Store` and instance under a [`Budget`] of time and memory; a plugin
//! that exceeds either is stopped with a [`RunError`] and the host carries on. Nothing a plugin
//! computes survives a call except the cursor it returns, so a call is a function of its
//! cursor, the clock and its answers.
//!
//! `suzatary plugin sync` is the caller (`docs/PLUGINS-PLAN-2026-10-02.md` stage 2).

#![forbid(unsafe_code)]

mod actor;
mod budget;
mod effects;
mod host;
mod log;
mod run;
mod source;
mod state;
mod trial;
mod world;

pub use actor::{Actor, Acts, Decided, Foreign};
pub use budget::Budget;
pub use effects::Effects;
pub use host::{Host, HostError, LoadError};
pub use log::{Level, Line, Log};
pub use run::{Fetch, Run, RunError};
pub use source::Source;
pub use trial::Trial;
