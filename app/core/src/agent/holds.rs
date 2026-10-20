//! A surface whose every call holds the agent's record.

use serde_json::Value;

use crate::authoring::Writer;
use crate::capability::Agent;
use crate::chains::ChainPolicy;
use crate::verb::Surface;

use super::Says;

/// A surface whose calls hold the agent's record and the chain they named, so a verb both the
/// triage agent and the secretary hold is one implementation over it: the record is a value the
/// call hands the verb, and whatever else the surface's record holds the verb never sees.
pub trait Holds: Surface<Answer: From<Value>> {
    /// Who a write through this surface is.
    const WRITES_AS: Writer;
    /// What this surface's model is told each shared verb does.
    const SAYS: Says;
    /// The agent's record, as this call holds it.
    fn agent<'a>(call: &Self::Call<'a>) -> Agent<'a>;
    /// The policy of the chain the call named.
    fn chain<'a>(call: &Self::Call<'a>) -> &'a ChainPolicy;
}
