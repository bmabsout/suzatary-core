//! A proposal as its conversation records it.

/// A proposal as its conversation records it: when, where among the turns, and which inbox
/// object. Whether a later proposal replaced it is the inbox's to say
/// ([`Withdrawal::Superseded`](crate::secretary::responded::Withdrawal::Superseded)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    /// Its index among the conversation's proposals: what `proposal:<conversation>/<index>`
    /// names.
    pub index: usize,
    /// When it was made.
    pub at: crate::capability::Datetime,
    /// The inbox object.
    pub object: crate::secretary::inbox::ProposalId,
    /// How many turns stood before this was proposed: the card is drawn after turn
    /// `after_turn - 1`, `0` meaning before any turn. Derived in [`Stored::of`](super::Stored::of).
    pub after_turn: usize,
}
