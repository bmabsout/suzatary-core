//! The reading is a function of the two stores' object sets alone, so every device holding the
//! same objects reads the same tags, offline included. Where concurrent renames disagree, it
//! still chooses one tag by a rule every device applies alike, and lists the [`Dispute`].

use std::collections::{BTreeMap, BTreeSet};

use prodrome_core::event::Actor;
use prodrome_core::literal::Datetime;
use prodrome_core::policy::Everything;
use prodrome_core::registers::{Node, Stamp, fold};

use crate::secretary::tags::alias_event::{Alias, AliasEvent};
use crate::secretary::tags::alias_registers::AliasRegisters;
use crate::secretary::tags::dispute::Dispute;
use crate::secretary::tags::membership::Presence;
use crate::secretary::tags::membership_event::MembershipEvent;
use crate::secretary::tags::membership_registers::MembershipRegisters;
use crate::secretary::tags::tag::Tag;
use crate::secretary::tags::target::Target;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reading {
    on: BTreeMap<Target, BTreeSet<Tag>>,
    aliases: BTreeMap<Tag, Candidates>,
    spelled: BTreeMap<Tag, Tag>,
    rings: BTreeMap<Tag, BTreeSet<Tag>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidates {
    chosen: Alias,
    others: Vec<Alias>,
}

impl Reading {
    #[must_use]
    pub fn of(memberships: &[Node<MembershipEvent>], aliases: &[Node<AliasEvent>]) -> Self {
        let mut on: BTreeMap<Target, BTreeSet<Tag>> = BTreeMap::new();
        for (membership, stream) in fold(memberships).entities() {
            let registers: MembershipRegisters<'_> =
                prodrome_core::fold::read(stream, None, &Everything);
            if registers.presence() == Presence::On {
                on.entry(membership.target.clone())
                    .or_default()
                    .insert(membership.tag.clone());
            }
        }
        let folded = fold(aliases);
        let mut written: BTreeMap<&Tag, Vec<&Stamp<AliasEvent>>> = BTreeMap::new();
        for (tag, stream) in folded.entities() {
            let registers: AliasRegisters<'_> =
                prodrome_core::fold::read(stream, None, &Everything);
            written.entry(tag).or_default().extend(registers.reading());
        }
        let aliases: BTreeMap<Tag, Candidates> = written
            .into_iter()
            .filter_map(|(tag, mut stamps)| {
                stamps.sort_by(|one, two| one.name.cmp(&two.name));
                let mut values: Vec<Alias> = Vec::with_capacity(stamps.len());
                for stamp in stamps {
                    if !values.contains(stamp.event.alias()) {
                        values.push(stamp.event.alias().clone());
                    }
                }
                let mut values = values.into_iter();
                let chosen = values.next()?;
                Some((
                    tag.clone(),
                    Candidates {
                        chosen,
                        others: values.collect(),
                    },
                ))
            })
            .collect();
        let known: BTreeSet<&Tag> = on
            .values()
            .flatten()
            .chain(aliases.keys())
            .chain(
                aliases
                    .values()
                    .filter_map(|candidates| match &candidates.chosen {
                        Alias::As(to) => Some(to),
                        Alias::Itself => None,
                    }),
            )
            .collect();
        let mut spelled = BTreeMap::new();
        let mut rings = BTreeMap::new();
        for name in known {
            let (tag, ring) = follow(&aliases, name);
            if let Some(ring) = ring {
                rings.insert(tag.clone(), ring);
            }
            spelled.insert(name.clone(), tag);
        }
        Self {
            on,
            aliases,
            spelled,
            rings,
        }
    }

    /// The end of `name`'s alias chain, or the least name of a ring.
    #[must_use]
    pub fn spelling(&self, name: &Tag) -> Tag {
        self.spelled.get(name).unwrap_or(name).clone()
    }

    #[must_use]
    pub fn names(&self, name: &Tag) -> BTreeSet<Tag> {
        let tag = self.spelling(name);
        self.spelled
            .iter()
            .filter(|(_, spelled)| **spelled == tag)
            .map(|(name, _)| name.clone())
            .chain([tag.clone()])
            .collect()
    }

    #[must_use]
    pub fn tags(&self, target: &Target) -> BTreeSet<Tag> {
        self.on
            .get(target)
            .into_iter()
            .flatten()
            .map(|name| self.spelling(name))
            .collect()
    }

    #[must_use]
    pub fn tagged(&self) -> BTreeMap<Target, BTreeSet<Tag>> {
        self.on
            .keys()
            .map(|target| (target.clone(), self.tags(target)))
            .filter(|(_, tags)| !tags.is_empty())
            .collect()
    }

    #[must_use]
    pub fn disputes(&self) -> Vec<Dispute> {
        let aliases = self
            .aliases
            .iter()
            .filter(|(_, candidates)| !candidates.others.is_empty())
            .map(|(tag, candidates)| Dispute::Aliases {
                tag: tag.clone(),
                chosen: candidates.chosen.clone(),
                others: candidates.others.clone(),
            });
        let rings = self.rings.iter().map(|(spelled, names)| Dispute::Ring {
            names: names.clone(),
            spelled: spelled.clone(),
        });
        aliases.chain(rings).collect()
    }

    #[must_use]
    pub fn tag(
        &self,
        target: &Target,
        tag: &Tag,
        at: Datetime,
        actor: &Actor,
    ) -> Option<MembershipEvent> {
        let spelled = self.spelling(tag);
        (!self.tags(target).contains(&spelled))
            .then(|| MembershipEvent::tagged(target.clone(), spelled, at, actor.clone()))
    }

    /// Takes off every name of the tag this reading sees on `target`, and no add it has not
    /// seen.
    #[must_use]
    pub fn untag(
        &self,
        target: &Target,
        tag: &Tag,
        at: Datetime,
        actor: &Actor,
    ) -> Vec<MembershipEvent> {
        let spelled = self.spelling(tag);
        self.on
            .get(target)
            .into_iter()
            .flatten()
            .filter(|name| self.spelling(name) == spelled)
            .map(|name| MembershipEvent::untagged(target.clone(), name.clone(), at, actor.clone()))
            .collect()
    }

    /// Afterwards every name of `from`'s tag and of `to`'s tag stands for `to`: a rename to a
    /// name of another tag merges the two, and renaming back undoes a rename. Each write
    /// supersedes what this reading holds for its name, so it settles disputes on those names.
    #[must_use]
    pub fn rename(&self, from: &Tag, to: &Tag, at: Datetime, actor: &Actor) -> Vec<AliasEvent> {
        let itself = (!self.settled(to, &Alias::Itself))
            .then(|| AliasEvent::unaliased(to.clone(), at, actor.clone()));
        let pointed = Alias::As(to.clone());
        let mut names = self.names(from);
        names.extend(self.names(to));
        let aliased = names
            .into_iter()
            .filter(|name| !self.settled(name, &pointed))
            // `aliased` refuses `to` itself, the one name that is no alias afterwards.
            .filter_map(|name| AliasEvent::aliased(name, to.clone(), at, actor.clone()).ok());
        itself.into_iter().chain(aliased).collect()
    }

    /// An unwritten register reads `Itself`.
    fn settled(&self, name: &Tag, alias: &Alias) -> bool {
        match self.aliases.get(name) {
            None => *alias == Alias::Itself,
            Some(candidates) => candidates.chosen == *alias && candidates.others.is_empty(),
        }
    }
}

fn follow(aliases: &BTreeMap<Tag, Candidates>, name: &Tag) -> (Tag, Option<BTreeSet<Tag>>) {
    let mut path: Vec<&Tag> = Vec::new();
    let mut at = name;
    loop {
        if let Some(start) = path.iter().position(|seen| *seen == at) {
            let ring = &path[start..];
            let least = ring.iter().fold(at, |least, name| least.min(*name));
            return (
                least.clone(),
                Some(ring.iter().map(|name| (*name).clone()).collect()),
            );
        }
        path.push(at);
        match aliases.get(at).map(|candidates| &candidates.chosen) {
            Some(Alias::As(to)) => at = to,
            Some(Alias::Itself) | None => return (at.clone(), None),
        }
    }
}
