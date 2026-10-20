//! A loaded `source` plugin, called once per sync in a store of its own.

use std::sync::Arc;
use std::time::Instant;

use prodrome_core::declared::Declaration;
use suzatary_core::plugin::{Manifest, Said, Synced};
use wasmtime::{Store, Trap};

use crate::budget::Limits;
use crate::state::State;
use crate::world::{self, SourcePre};
use crate::{Budget, Effects, Log, Run, RunError};

/// A component that passed [`crate::Host::load`], ready to instantiate.
pub struct Source {
    manifest: Arc<Manifest>,
    schema: Declaration,
    pre: SourcePre<State>,
    budget: Budget,
}

impl Source {
    pub(crate) fn new(
        manifest: Arc<Manifest>,
        schema: Declaration,
        pre: SourcePre<State>,
        budget: Budget,
    ) -> Self {
        Self {
            manifest,
            schema,
            pre,
            budget,
        }
    }

    /// The manifest it was loaded with.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Its declared schema, admitted: what every item it emits must parse into.
    #[must_use]
    pub fn schema(&self) -> &Declaration {
        &self.schema
    }

    /// Read the outside world from `cursor`, or from the beginning, lending the plugin the names
    /// of the credentials its manifest declares.
    ///
    /// A fresh store and instance per call, so nothing survives it but the cursor returned, under
    /// the host's [`Budget`]: past its time or its memory the plugin is stopped and the answer is
    /// [`RunError::Deadline`] or [`RunError::Memory`].
    #[must_use]
    pub fn sync(&self, effects: Effects, cursor: Option<&[u8]>) -> Run {
        let state = State {
            manifest: Arc::clone(&self.manifest),
            effects,
            deadline: Instant::now() + self.budget.time,
            limits: Limits::new(self.budget),
            refused: Vec::new(),
            fetches: Vec::new(),
            log: Log::default(),
        };
        let mut store = Store::new(self.pre.engine(), state);
        store.limiter(|state| &mut state.limits);
        store.set_epoch_deadline(self.budget.ticks());
        store.epoch_deadline_trap();
        let result = self.call(&mut store, cursor);
        let State {
            refused,
            fetches,
            log,
            ..
        } = store.into_data();
        Run {
            result,
            refused,
            fetches,
            log,
        }
    }

    fn call(&self, store: &mut Store<State>, cursor: Option<&[u8]>) -> Result<Synced, RunError> {
        let instance = self.pre.instantiate(&mut *store).map_err(|e| {
            stopped(store, &e).unwrap_or_else(|| RunError::Instantiate(Said::clip(e.to_string())))
        })?;
        let lent = world::credentials(&self.manifest);
        let answer = instance
            .suzatary_plugin_sync()
            .call_sync(&mut *store, cursor, &lent)
            .map_err(|e| {
                stopped(store, &e).unwrap_or_else(|| RunError::Trapped(Said::clip(e.to_string())))
            })?;
        answer.map(world::synced).map_err(world::sync_error)
    }
}

/// Whether the host's budget is what ended the call: the limiter's own record for memory, the
/// epoch trap for time.
pub(crate) fn stopped(store: &Store<State>, error: &wasmtime::Error) -> Option<RunError> {
    if store.data().limits.exceeded {
        Some(RunError::Memory)
    } else if error.downcast_ref::<Trap>() == Some(&Trap::Interrupt) {
        Some(RunError::Deadline)
    } else {
        None
    }
}
