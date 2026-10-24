//! The topic tags as a page reads and writes them: the two stores' replicas, the
//! `suzatary_core::secretary::tags::Reading` over what they hold, and its planners, which answer
//! events for the replicas' `append`. The box plans with the same functions, so a tab writing
//! offline writes what the box would write holding the same objects.
//!
//! The replicas are classes of their own rather than the Prodrome's `schema!` exports: a
//! membership's key is a pair, which that macro's register readings cannot name, and no page
//! reads a tag store's registers but through [`TagReading`].

use prodrome_core::event::{Actor, canonical};
use prodrome_core::fpl::datetime_of;
use prodrome_core::literal::Datetime;
use prodrome_wasm_exports::wire::parse_instant;
use prodrome_wasm_exports::{Replica, thrown};
use suzatary_core::secretary::tags::alias_event::{Alias, AliasEvent};
use suzatary_core::secretary::tags::dispute::Dispute;
use suzatary_core::secretary::tags::membership_event::MembershipEvent;
use suzatary_core::secretary::tags::reading::Reading;
use suzatary_core::secretary::tags::tag::Tag;
use suzatary_core::secretary::tags::target::Target;
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
replica!(TagAliases = AliasEvent);

/// The tags both classes' objects read as, read once.
#[wasm_bindgen]
pub struct TagReading(Reading);

#[wasm_bindgen]
impl TagReading {
    /// The memberships and the aliases read together.
    #[wasm_bindgen(constructor)]
    pub fn new(memberships: &Memberships, aliases: &TagAliases) -> Result<TagReading, JsError> {
        let memberships = memberships.0.read().map_err(refused)?;
        let aliases = aliases.0.read().map_err(refused)?;
        Ok(Self(Reading::of(memberships.nodes, aliases.nodes)))
    }

    /// Every target with a tag, `{<target>: [<tag>, …]}`, each tag in its current spelling.
    pub fn tagged(&self) -> Result<String, JsError> {
        json(
            &self
                .0
                .tagged()
                .into_iter()
                .map(|(target, tags)| (target.to_string(), tags))
                .collect::<std::collections::BTreeMap<_, _>>(),
        )
    }

    /// Every place concurrent renames disagree: `{type: "Aliases", tag, chosen, others}` or
    /// `{type: "Ring", names, spelled}`, an alias printed as its tag or `null` for itself.
    pub fn disputes(&self) -> Result<String, JsError> {
        let alias = |alias: &Alias| match alias {
            Alias::Itself => serde_json::Value::Null,
            Alias::As(tag) => serde_json::Value::from(tag.as_str()),
        };
        json(
            &self
                .0
                .disputes()
                .iter()
                .map(|dispute| match dispute {
                    Dispute::Aliases {
                        tag,
                        chosen,
                        others,
                    } => serde_json::json!({
                        "type": "Aliases",
                        "tag": tag,
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

    /// The `Tagged` event that puts `tag` on `target`, printed, or `undefined` where it is on.
    pub fn tag(
        &self,
        target: &str,
        tag: &str,
        at: &str,
        actor: &str,
    ) -> Result<Option<String>, JsError> {
        let (target, tag, at, actor) = (
            target_of(target)?,
            tag_of(tag)?,
            at_of(at)?,
            actor_of(actor)?,
        );
        Ok(self
            .0
            .tag(&target, &tag, at, &actor)
            .map(|event| canonical(&event)))
    }

    /// The `Untagged` events that take `tag` off `target`, printed, as a JSON array.
    pub fn untag(&self, target: &str, tag: &str, at: &str, actor: &str) -> Result<String, JsError> {
        let (target, tag, at, actor) = (
            target_of(target)?,
            tag_of(tag)?,
            at_of(at)?,
            actor_of(actor)?,
        );
        json(
            &self
                .0
                .untag(&target, &tag, at, &actor)
                .iter()
                .map(canonical)
                .collect::<Vec<_>>(),
        )
    }

    /// The alias events that rename `from` to `to`, printed, as a JSON array.
    pub fn rename(&self, from: &str, to: &str, at: &str, actor: &str) -> Result<String, JsError> {
        let (from, to, at, actor) = (tag_of(from)?, tag_of(to)?, at_of(at)?, actor_of(actor)?);
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
        .map_err(|e: suzatary_core::secretary::tags::target::NotATarget| refused(e.to_string()))
}

fn tag_of(text: &str) -> Result<Tag, JsError> {
    Tag::new(text).map_err(|e| refused(e.to_string()))
}

fn at_of(text: &str) -> Result<Datetime, JsError> {
    let at = parse_instant("at", text).map_err(refused)?;
    datetime_of(at).map_err(|e| refused(format!("at: {e}")))
}

fn actor_of(text: &str) -> Result<Actor, JsError> {
    Actor::new(text).map_err(|e| refused(format!("actor: {e}")))
}
