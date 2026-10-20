//! A loaded `actor` plugin, asked once per decision in a store of its own.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

use prodrome_core::declared::{Declaration, Declared};
use prodrome_core::event::canonical;
use suzatary_core::capability::{Clock, Secret};
use suzatary_core::plugin::{MAX_OFFERS, Manifest, Offer, ProposalRefusal, Refused, Said, propose};
use suzatary_io::http::{Http, HttpError, Req, Resp};
use suzatary_io::secrets::Resolves;
use wasmtime::Store;

use crate::budget::Limits;
use crate::state::State;
use crate::world::actor::{ActorPre, decide};
use crate::{Budget, Effects, Log, RunError};

/// A component that passed [`crate::Host::load_actor`], ready to instantiate.
pub struct Actor {
    manifest: Arc<Manifest>,
    schema: Declaration,
    pre: ActorPre<State>,
    budget: Budget,
}

/// The events of one actor's own store: the only thing an actor is handed. Minted by
/// [`Actor::acts`] alone, from events of that actor's declared schema, each once and in the
/// order of its print, so an actor is never handed another store's events, nor an order a
/// replica happened to read them in.
#[derive(Debug, Clone)]
pub struct Acts {
    plugin: String,
    prints: BTreeSet<String>,
}

/// An event offered to an actor that is not of its own schema.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("an event of the schema {found}, not {plugin}'s own")]
pub struct Foreign {
    /// The actor.
    pub plugin: String,
    /// The schema the event was of.
    pub found: String,
}

/// One decision, as the host saw it: the proposals admitted, as a set, and every one refused.
#[derive(Debug, Clone, PartialEq)]
pub struct Decided {
    /// The admitted offers, one per act (an act offered twice is the one whose reasons come
    /// first), in the order of their acts' prints; or why the call proposed nothing.
    pub result: Result<Vec<Offer>, RunError>,
    /// Every proposal the host refused, naming the plugin, in the order the plugin made them.
    pub refused: Vec<Refused>,
    /// What the plugin logged.
    pub log: Log,
}

impl Actor {
    pub(crate) fn new(
        manifest: Arc<Manifest>,
        schema: Declaration,
        pre: ActorPre<State>,
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

    /// Its declared schema, admitted: the type of the store it reads, whose events it is handed.
    #[must_use]
    pub fn schema(&self) -> &Declaration {
        &self.schema
    }

    /// The acts this actor may be handed: `events`, every one of its declared schema.
    ///
    /// # Errors
    /// [`Foreign`] for the first event of another schema.
    pub fn acts(&self, events: impl IntoIterator<Item = Declared>) -> Result<Acts, Foreign> {
        let own = self.schema.name().as_str();
        let prints = events
            .into_iter()
            .map(|event| {
                let found = event.schema().name().as_str().to_owned();
                if found == own {
                    Ok(canonical(&event))
                } else {
                    Err(Foreign {
                        plugin: self.manifest.name().to_string(),
                        found,
                    })
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Acts {
            plugin: self.manifest.name().to_string(),
            prints,
        })
    }

    /// Ask it what to propose, handing it `acts`, the events its store holds, and lending it
    /// `clock` and nothing else that reaches outside the call.
    ///
    /// A fresh store per call under the host's [`Budget`], as a source's sync is. At most
    /// [`MAX_OFFERS`] proposals are read, and each passes [`propose`] or is refused naming the
    /// plugin; what it proposes is a value, and nothing here performs it.
    #[must_use]
    pub fn decide(&self, clock: Arc<dyn Clock + Send>, acts: &Acts) -> Decided {
        let state = State {
            manifest: Arc::clone(&self.manifest),
            // An actor imports no fetch, so these are never asked; they answer nothing if they
            // were.
            effects: Effects {
                http: Arc::new(Nowhere),
                secrets: Arc::new(Nowhere),
                clock,
            },
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
        let plugin = self.manifest.name();
        let mut refused = Vec::new();
        let result = self.call(&mut store, acts).map(|mut proposed| {
            if proposed.len() > MAX_OFFERS {
                refused.push(Refused {
                    plugin: plugin.clone(),
                    refusal: ProposalRefusal::Beyond(proposed.len() - MAX_OFFERS),
                });
                proposed.truncate(MAX_OFFERS);
            }
            let mut offers: Vec<Offer> = proposed
                .into_iter()
                .filter_map(|proposal| {
                    match propose(plugin, &proposal.act, &proposal.why, &proposal.sources) {
                        Ok(offer) => Some(offer),
                        Err(refusal) => {
                            refused.push(refusal);
                            None
                        }
                    }
                })
                .collect();
            offers.sort_by_cached_key(|offer| (offer.key(), offer.reasons().clone()));
            offers.dedup_by_key(|offer| offer.key());
            offers
        });
        Decided {
            result,
            refused,
            log: store.into_data().log,
        }
    }

    fn call(
        &self,
        store: &mut Store<State>,
        acts: &Acts,
    ) -> Result<Vec<decide::Proposal>, RunError> {
        if acts.plugin != self.manifest.name().as_str() {
            return Err(RunError::Malformed(Said::clip(format!(
                "handed {}'s acts, not its own",
                acts.plugin
            ))));
        }
        let instance = self.pre.instantiate(&mut *store).map_err(|e| {
            crate::source::stopped(store, &e)
                .unwrap_or_else(|| RunError::Instantiate(Said::clip(e.to_string())))
        })?;
        let schema = self.schema.name().as_str().to_owned();
        let handed: Vec<decide::Act> = acts
            .prints
            .iter()
            .map(|print| decide::Act {
                schema: schema.clone(),
                bytes: print.clone().into_bytes(),
            })
            .collect();
        let answer = instance
            .suzatary_plugin_decide()
            .call_decide(&mut *store, &handed)
            .map_err(|e| {
                crate::source::stopped(store, &e)
                    .unwrap_or_else(|| RunError::Trapped(Said::clip(e.to_string())))
            })?;
        answer.map_err(crate::world::decide_error)
    }
}

/// What an actor's call is lent in place of a network and a secret store: nothing.
struct Nowhere;

impl Http for Nowhere {
    fn send(&self, _req: Req) -> Result<Resp, HttpError> {
        Err(HttpError::NotSent("an actor fetches nothing".to_owned()))
    }
}

impl Resolves for Nowhere {
    fn get(
        &self,
        _name: &str,
        _reason: &str,
    ) -> Result<Option<Secret>, suzatary_io::secrets::SecretError> {
        Ok(None)
    }
}
