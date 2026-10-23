//! A tag's one register, its [`Alias`]. The order is discrete: two different aliases written
//! concurrently for one tag are a conflict, which the reading shows and the next rename settles.

use prodrome_core::fold::{Discrete, Frontier, Product, Register as _, RegisterType};
use prodrome_core::literal::ProdromeError;
use prodrome_core::registers::Stamp;

use crate::secretary::tags::alias_event::{Alias, AliasEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AliasRegister;

#[derive(Debug)]
pub struct AliasType;

impl RegisterType<AliasEvent> for AliasType {
    type Value<'e> = Discrete<&'e Alias>;

    fn value(event: &AliasEvent) -> Option<Discrete<&Alias>> {
        Some(Discrete(event.alias()))
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AliasRegisters<'a> {
    pub alias: Frontier<'a, AliasEvent>,
}

impl<'a> AliasRegisters<'a> {
    /// More than one write is a conflict.
    #[must_use]
    pub fn reading(&self) -> Vec<&'a Stamp<AliasEvent>> {
        self.alias.reading::<AliasType>()
    }
}

impl<'a> Product<'a> for AliasRegisters<'a> {
    type Schema = AliasEvent;

    fn registers(&self) -> Vec<AliasRegister> {
        vec![AliasRegister]
    }

    fn join(&mut self, stamp: &'a Stamp<AliasEvent>) {
        self.alias.join(stamp);
    }

    fn frontier(&self, _: AliasRegister) -> &Frontier<'a, AliasEvent> {
        &self.alias
    }

    fn frontier_mut(&mut self, _: AliasRegister) -> &mut Frontier<'a, AliasEvent> {
        &mut self.alias
    }

    fn reading(&self, _: AliasRegister) -> Vec<&'a Stamp<AliasEvent>> {
        AliasRegisters::reading(self)
    }

    fn grows(&self, _: &AliasEvent) -> Result<(), ProdromeError> {
        Ok(())
    }
}
