//! A membership's one register, its [`Presence`]. It is not inflationary: a removal after an add
//! goes down, and only causal order decides that it does.

use prodrome_core::fold::{Frontier, Product, Register as _, RegisterType};
use prodrome_core::literal::ProdromeError;
use prodrome_core::registers::Stamp;

use crate::secretary::tags::membership::Presence;
use crate::secretary::tags::membership_event::MembershipEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MembershipRegister;

#[derive(Debug)]
pub struct PresenceRegister;

impl RegisterType<MembershipEvent> for PresenceRegister {
    type Value<'e> = Presence;

    fn value(event: &MembershipEvent) -> Option<Presence> {
        Some(event.presence())
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MembershipRegisters<'a> {
    pub presence: Frontier<'a, MembershipEvent>,
}

impl MembershipRegisters<'_> {
    /// `Off` where nothing wrote it.
    #[must_use]
    pub fn presence(&self) -> Presence {
        self.presence
            .reading::<PresenceRegister>()
            .iter()
            .map(|stamp| stamp.event.presence())
            .max()
            .unwrap_or(Presence::Off)
    }
}

impl<'a> Product<'a> for MembershipRegisters<'a> {
    type Schema = MembershipEvent;

    fn registers(&self) -> Vec<MembershipRegister> {
        vec![MembershipRegister]
    }

    fn join(&mut self, stamp: &'a Stamp<MembershipEvent>) {
        self.presence.join(stamp);
    }

    fn frontier(&self, _: MembershipRegister) -> &Frontier<'a, MembershipEvent> {
        &self.presence
    }

    fn frontier_mut(&mut self, _: MembershipRegister) -> &mut Frontier<'a, MembershipEvent> {
        &mut self.presence
    }

    fn reading(&self, _: MembershipRegister) -> Vec<&'a Stamp<MembershipEvent>> {
        self.presence.reading::<PresenceRegister>()
    }

    fn grows(&self, _: &MembershipEvent) -> Result<(), ProdromeError> {
        Ok(())
    }
}
