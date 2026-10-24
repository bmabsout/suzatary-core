//! The topic topics as a page reads and writes them: the two stores' replicas, the
//! `suzatary_core::secretary::topics::Reading` over what they hold, and its planners, which answer
//! events for the replicas' `append`. The box plans with the same functions, so a tab writing
//! offline writes what the box would write holding the same objects.
//!
//! The replicas are classes of their own rather than the Prodrome's `schema!` exports: a
//! membership's key is a pair, which that macro's register readings cannot name, and no page
//! reads a topic store's registers but through [`TopicReading`].

use prodrome_core::event::{Actor, canonical};
use prodrome_core::fpl::datetime_of;
use prodrome_core::literal::Datetime;
use prodrome_wasm_exports::wire::parse_instant;
use prodrome_wasm_exports::{Replica, thrown};
use suzatary_core::secretary::topics::alias_event::{Alias, AliasEvent};
use suzatary_core::secretary::topics::dispute::Dispute;
use suzatary_core::secretary::topics::membership_event::MembershipEvent;
use suzatary_core::secretary::topics::reading::Reading;
use suzatary_core::secretary::topics::target::Target;
use suzatary_core::secretary::topics::topic::Topic;
use wasm_bindgen::prelude::*;

use crate::{json, refused};

macro_rules! replica {
    ($name:ident = $schema:ty) => {
        #[doc = concat!("A replica of the store at `", stringify!($schema), "`.")]
        #[wasm_bindgen]
        pub struct $name(Replica<$schema>);

        #[wasm_bindgen]
        impl $name {
            /// The objects, `[{hash, text}]`, read once.
            #[wasm_bindgen(constructor)]
            pub fn new(objects: &str) -> Result<$name, JsError> {
                Replica::of(objects).map($name).map_err(refused)
            }

            /// The objects' tips, and each prodrome's heads by its genesis.
            pub fn tips(&self) -> Result<String, JsError> {
                thrown(prodrome_wasm_exports::tips(&self.0))
            }

            /// What a replica holding `tips` (a JSON array of names) lacks, in causal order.
            pub fn since(&self, tips: &str) -> Result<String, JsError> {
                thrown(prodrome_wasm_exports::since(&self.0, tips))
            }

            /// `event`, its print, sealed into this replica as a `Change` over its own fold:
            /// `{hash, objects}`, the objects the box lacks of it.
            pub fn append(
                &mut self,
                event: &str,
                genesis: Option<String>,
            ) -> Result<String, JsError> {
                thrown(prodrome_wasm_exports::append(&mut self.0, event, genesis))
            }
        }
    };
}

replica!(Memberships = MembershipEvent);
replica!(TopicAliases = AliasEvent);

/// The topics both classes' objects read as, read once.
#[wasm_bindgen]
pub struct TopicReading(Reading);

#[wasm_bindgen]
impl TopicReading {
    /// The memberships and the aliases read together.
    #[wasm_bindgen(constructor)]
    pub fn new(memberships: &Memberships, aliases: &TopicAliases) -> Result<TopicReading, JsError> {
        let memberships = memberships.0.read().map_err(refused)?;
        let aliases = aliases.0.read().map_err(refused)?;
        Ok(Self(Reading::of(memberships.nodes, aliases.nodes)))
    }

    /// Every target with a topic, `{<target>: [<topic>, …]}`, each topic in its current spelling.
    pub fn memberships(&self) -> Result<String, JsError> {
        json(
            &self
                .0
                .memberships()
                .into_iter()
                .map(|(target, topics)| (target.to_string(), topics))
                .collect::<std::collections::BTreeMap<_, _>>(),
        )
    }

    /// Every place concurrent renames disagree: `{type: "Aliases", topic, chosen, others}` or
    /// `{type: "Ring", names, spelled}`, an alias printed as its topic or `null` for itself.
    pub fn disputes(&self) -> Result<String, JsError> {
        let alias = |alias: &Alias| match alias {
            Alias::Itself => serde_json::Value::Null,
            Alias::As(topic) => serde_json::Value::from(topic.as_str()),
        };
        json(
            &self
                .0
                .disputes()
                .iter()
                .map(|dispute| match dispute {
                    Dispute::Aliases {
                        topic,
                        chosen,
                        others,
                    } => serde_json::json!({
                        "type": "Aliases",
                        "topic": topic,
                        "chosen": alias(chosen),
                        "others": others.iter().map(alias).collect::<Vec<_>>(),
                    }),
                    Dispute::Ring { names, spelled } => serde_json::json!({
                        "type": "Ring",
                        "names": names,
                        "spelled": spelled,
                    }),
                })
                .collect::<Vec<_>>(),
        )
    }

    /// The `Added` event that puts `topic` on `target`, printed, or `undefined` where it is on.
    pub fn add(
        &self,
        target: &str,
        topic: &str,
        at: &str,
        actor: &str,
    ) -> Result<Option<String>, JsError> {
        let (target, topic, at, actor) = (
            target_of(target)?,
            topic_of(topic)?,
            at_of(at)?,
            actor_of(actor)?,
        );
        Ok(self
            .0
            .add(&target, &topic, at, &actor)
            .map(|event| canonical(&event)))
    }

    /// The `Removed` events that take `topic` off `target`, printed, as a JSON array.
    pub fn remove(
        &self,
        target: &str,
        topic: &str,
        at: &str,
        actor: &str,
    ) -> Result<String, JsError> {
        let (target, topic, at, actor) = (
            target_of(target)?,
            topic_of(topic)?,
            at_of(at)?,
            actor_of(actor)?,
        );
        json(
            &self
                .0
                .remove(&target, &topic, at, &actor)
                .iter()
                .map(canonical)
                .collect::<Vec<_>>(),
        )
    }

    /// The alias events that rename `from` to `to`, printed, as a JSON array.
    pub fn rename(&self, from: &str, to: &str, at: &str, actor: &str) -> Result<String, JsError> {
        let (from, to, at, actor) = (topic_of(from)?, topic_of(to)?, at_of(at)?, actor_of(actor)?);
        json(
            &self
                .0
                .rename(&from, &to, at, &actor)
                .iter()
                .map(canonical)
                .collect::<Vec<_>>(),
        )
    }
}

fn target_of(text: &str) -> Result<Target, JsError> {
    text.parse()
        .map_err(|e: suzatary_core::secretary::topics::target::NotATarget| refused(e.to_string()))
}

fn topic_of(text: &str) -> Result<Topic, JsError> {
    Topic::new(text).map_err(|e| refused(e.to_string()))
}

fn at_of(text: &str) -> Result<Datetime, JsError> {
    let at = parse_instant("at", text).map_err(refused)?;
    datetime_of(at).map_err(|e| refused(format!("at: {e}")))
}

fn actor_of(text: &str) -> Result<Actor, JsError> {
    Actor::new(text).map_err(|e| refused(format!("actor: {e}")))
}
