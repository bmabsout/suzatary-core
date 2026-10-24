//! The two tag stores on disk, box-local and never pushed: a tag name is words.

use suzatary_core::secretary::tags::alias_event::AliasEvent;
use suzatary_core::secretary::tags::membership_event::MembershipEvent;
use suzatary_core::secretary::tags::stores::TagStores;
use suzatary_core::store::StoreError;

use crate::objects::Objects;

/// The memberships and the aliases, opened together.
#[derive(Debug)]
pub struct Tags {
    /// `secretary-tags/` by default.
    pub memberships: Objects<MembershipEvent>,
    /// `secretary-tag-aliases/` by default.
    pub aliases: Objects<AliasEvent>,
}

impl Tags {
    /// Both stores, at their directories. Nothing is read until something asks.
    #[must_use]
    pub fn open(
        memberships: impl Into<std::path::PathBuf>,
        aliases: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            memberships: Objects::tags(memberships),
            aliases: Objects::tag_aliases(aliases),
        }
    }

    /// Both begun, so a tab's first write has a prodrome to rest on.
    ///
    /// # Errors
    /// Either store unreadable or not begun.
    pub fn begin(&self) -> Result<(), StoreError> {
        self.memberships.begin()?;
        self.aliases.begin()
    }

    /// The record a reader or a writer holds.
    #[must_use]
    pub fn stores(&self) -> TagStores<'_> {
        TagStores {
            memberships: &self.memberships,
            aliases: &self.aliases,
        }
    }
}
