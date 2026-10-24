//! The two tag stores, read together, since a membership's tag reads through the aliases.

use crate::secretary::tags::alias_event::AliasEvent;
use crate::secretary::tags::membership_event::MembershipEvent;
use crate::secretary::tags::reading::Reading;
use crate::store::{Prodrome, StoreError};

#[derive(Clone, Copy)]
pub struct TagStores<'a> {
    pub memberships: &'a dyn Prodrome<MembershipEvent>,
    pub aliases: &'a dyn Prodrome<AliasEvent>,
}

impl TagStores<'_> {
    /// # Errors
    /// Either store unreadable.
    pub fn reading(&self) -> Result<Reading, StoreError> {
        Ok(Reading::of(
            &self.memberships.nodes()?,
            &self.aliases.nodes()?,
        ))
    }
}
