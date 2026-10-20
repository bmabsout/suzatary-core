//! What one of the triage agent's verbs runs against.

use crate::capability::Agent;
use crate::chains::ChainPolicy;
use crate::deployment::Deployment;

/// One call's world: the agent's record over the chain the call named, and that chain's policy.
#[derive(Clone, Copy)]
pub struct Call<'a> {
    /// The effects, none of which reaches outward.
    pub caps: &'a Agent<'a>,
    /// This deployment.
    pub deployment: &'a Deployment,
    /// The policy of the store `caps.chain` holds.
    pub chain: &'a ChainPolicy,
}
