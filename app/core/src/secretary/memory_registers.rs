//! A fact's one register, its [`Life`]: written by its remembering and by its forgetting,
//! inflationary, so the memory as a whole is a two-phase set
//! ([`crate::secretary::two_phase::TwoPhase`]) whose reading only climbs.

use prodrome_core::fold::{Discrete, Frontier, Product, Register as _, RegisterType};
use prodrome_core::literal::ProdromeError;
use prodrome_core::registers::Stamp;

use crate::secretary::fact_life::Life;
use crate::secretary::memory_event::{MemoryEvent, Said};

/// The register every memory event supersedes in: the fact's life, the only one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FactRegister;

/// The register's type: a [`Life`].
#[derive(Debug)]
pub struct LifeRegister;

impl RegisterType<MemoryEvent> for LifeRegister {
    type Value<'e> = Life<'e>;

    fn value(event: &MemoryEvent) -> Option<Life<'_>> {
        Some(match event.said() {
            Said::Remembered { .. } => Life::Remembered(Discrete(event)),
            Said::Forgotten { .. } => Life::Forgotten,
        })
    }
}

/// One fact's registers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FactRegisters<'a> {
    /// The writes no later write to this fact rests on.
    pub life: Frontier<'a, MemoryEvent>,
}

impl<'a> FactRegisters<'a> {
    /// The register's reading: one write, or a conflict's candidates.
    #[must_use]
    pub fn reading(&self) -> Vec<&'a Stamp<MemoryEvent>> {
        self.life.reading::<LifeRegister>()
    }
}

impl<'a> Product<'a> for FactRegisters<'a> {
    type Schema = MemoryEvent;

    fn registers(&self) -> Vec<FactRegister> {
        vec![FactRegister]
    }

    fn join(&mut self, stamp: &'a Stamp<MemoryEvent>) {
        self.life.join(stamp);
    }

    fn frontier(&self, _: FactRegister) -> &Frontier<'a, MemoryEvent> {
        &self.life
    }

    fn frontier_mut(&mut self, _: FactRegister) -> &mut Frontier<'a, MemoryEvent> {
        &mut self.life
    }

    fn reading(&self, _: FactRegister) -> Vec<&'a Stamp<MemoryEvent>> {
        FactRegisters::reading(self)
    }

    fn grows(&self, event: &MemoryEvent) -> Result<(), ProdromeError> {
        self.life.grows::<LifeRegister>(event)
    }
}
