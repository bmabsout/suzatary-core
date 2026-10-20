//! Where a line came from: written here, or carried from an object of an earlier store.
//!
//! The earlier store is gone; a carried line keeps the NAME of the object it carried, which the
//! digest reads as its leaf, so the summaries written before the carry still name their nodes.
//! It is never read for meaning, and nothing writes one any more.

use prodrome_core::event::Hash;

/// A line's provenance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Carried {
    /// Written in this store.
    Written,
    /// Carried from the object of the earlier store this names.
    From(Hash),
}

/// A line prints `None`, or the name of the object it carries.
impl crate::literal::Literal for Carried {
    fn to_value(&self) -> prodrome_core::literal::Value {
        match self {
            Carried::Written => None::<Hash>.to_value(),
            Carried::From(object) => Some(object.clone()).to_value(),
        }
    }

    fn from_value(
        value: &prodrome_core::literal::Value,
    ) -> Result<Self, prodrome_core::literal::ProdromeError> {
        Ok(Option::<Hash>::from_value(value)?.map_or(Carried::Written, Carried::From))
    }
}
