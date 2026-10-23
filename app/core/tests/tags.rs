//! The tag stores' laws, over real replicas of the Prodrome's in-memory store: each device writes
//! what its own reading plans, devices sync by the Prodrome's `sync` (the union of two object
//! sets), and every law is about the reading after that union.
//!
//! - Merging is commutative, associative and idempotent: devices that hold the same objects read
//!   the same tags, whatever order they synced in and however often.
//! - Observed remove: a removal takes off the adds its writer saw, and an add it did not see stays
//!   on, whichever device merges into which.
//! - A rename keeps every membership: each target that carried the renamed tag carries the new
//!   name, on the renaming device and on a device that tagged under the old name meanwhile.
//! - Every event, tag and target prints and parses back to itself.

use std::collections::BTreeSet;

use prodrome_core::event::{
    Actor, Envelope, canonical, canonical_envelope, parse_event, seal_hash,
};
use prodrome_core::genesis::mk_genesis;
use prodrome_core::literal::Datetime;
use prodrome_core::schema::Schema;
use prodrome_core::store::{MemoryStore, Replica, sync};
use proptest::prelude::*;

use suzatary_core::secretary::store::ConversationId;
use suzatary_core::secretary::tags::alias_event::{Alias, AliasEvent, AliasVocabulary};
use suzatary_core::secretary::tags::dispute::Dispute;
use suzatary_core::secretary::tags::membership_event::{MembershipEvent, MembershipVocabulary};
use suzatary_core::secretary::tags::reading::Reading;
use suzatary_core::secretary::tags::tag::{MAX, Tag};
use suzatary_core::secretary::tags::target::Target;

const NAMES: [&str; 4] = ["travel", "trips", "work", "job"];

fn tag(name: &str) -> Tag {
    Tag::new(name).expect("a tag")
}

fn target(n: usize) -> Target {
    let conversation = ConversationId::new("default").expect("a slug");
    if n % 3 == 2 {
        Target::Note {
            conversation,
            turn: n,
        }
    } else {
        Target::Round {
            conversation,
            turn: n,
        }
    }
}

fn at(minute: u32) -> Datetime {
    Datetime::new(2026, 10, 21, 8 + (minute / 60) % 12, minute % 60, 0, 0).expect("an instant")
}

/// A store at schema `E` holding one genesis, the same on every device.
fn begun<E: Schema<Vocabulary: Default>>() -> MemoryStore<E> {
    let store = MemoryStore::default();
    let genesis: Envelope<E> =
        Envelope::Genesis(mk_genesis("tags", &"0".repeat(32)).expect("a genesis"));
    let print = canonical_envelope(&genesis).into_bytes();
    store
        .receive([seal_hash(&genesis)].into(), &|_| Some(print.clone()))
        .expect("begins");
    store
}

/// The names of the objects `store` holds.
fn names<E: Schema>(store: &MemoryStore<E>) -> BTreeSet<String> {
    let held = store.held().expect("held");
    held.dag
        .objects()
        .keys()
        .map(|name| name.as_str().to_owned())
        .collect()
}

/// One device: its replica of each store, and its name as a writer.
struct Device {
    memberships: MemoryStore<MembershipEvent>,
    aliases: MemoryStore<AliasEvent>,
    actor: Actor,
    clock: u32,
}

impl Device {
    fn new(n: usize) -> Self {
        Self {
            memberships: begun(),
            aliases: begun(),
            actor: Actor::new(format!("device{n}")).expect("an actor"),
            clock: 0,
        }
    }

    fn read(&self) -> Reading {
        let memberships = self
            .memberships
            .held()
            .expect("held")
            .dag
            .nodes()
            .expect("nodes");
        let aliases = self
            .aliases
            .held()
            .expect("held")
            .dag
            .nodes()
            .expect("nodes");
        Reading::of(&memberships, &aliases)
    }

    fn objects(&self) -> (BTreeSet<String>, BTreeSet<String>) {
        (names(&self.memberships), names(&self.aliases))
    }

    /// Take in everything `other` holds.
    fn take(&self, other: &Device) {
        sync(&other.memberships, &self.memberships).expect("synced");
        sync(&other.aliases, &self.aliases).expect("synced");
    }

    fn tick(&mut self) -> Datetime {
        self.clock += 1;
        at(self.clock)
    }

    /// Write what this device's reading plans for `op`; answers how many objects it wrote.
    fn apply(&mut self, op: &Op) -> usize {
        let reading = self.read();
        let at = self.tick();
        match op {
            Op::Tag(n, name) => self.write_memberships(
                reading
                    .tag(&target(*n), &tag(NAMES[*name]), at, &self.actor)
                    .into_iter()
                    .collect(),
            ),
            Op::Untag(n, name) => self.write_memberships(reading.untag(
                &target(*n),
                &tag(NAMES[*name]),
                at,
                &self.actor,
            )),
            Op::Rename(from, to) => {
                let events = reading.rename(&tag(NAMES[*from]), &tag(NAMES[*to]), at, &self.actor);
                let written = events.len();
                for event in events {
                    self.aliases.append(event).expect("appended");
                }
                written
            }
        }
    }

    fn write_memberships(&self, events: Vec<MembershipEvent>) -> usize {
        let written = events.len();
        for event in events {
            self.memberships.append(event).expect("appended");
        }
        written
    }
}

#[derive(Debug, Clone)]
enum Op {
    Tag(usize, usize),
    Untag(usize, usize),
    Rename(usize, usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0usize..4, 0usize..4).prop_map(|(n, name)| Op::Tag(n, name)),
        2 => (0usize..4, 0usize..4).prop_map(|(n, name)| Op::Untag(n, name)),
        1 => (0usize..4, 0usize..4).prop_map(|(from, to)| Op::Rename(from, to)),
    ]
}

/// A membership write over two targets and two tags, so two devices often write one pair.
fn membership_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..2, 0usize..2).prop_map(|(n, name)| Op::Tag(n, name)),
        (0usize..2, 0usize..2).prop_map(|(n, name)| Op::Untag(n, name)),
    ]
}

/// A step of a script: a device writes, or a device takes in another's objects.
#[derive(Debug, Clone)]
enum Step {
    Write(usize, Op),
    Sync { from: usize, to: usize },
}

fn steps(devices: usize) -> impl Strategy<Value = Vec<Step>> {
    proptest::collection::vec(
        prop_oneof![
            3 => (0..devices, op()).prop_map(|(device, op)| Step::Write(device, op)),
            1 => (0..devices, 0..devices).prop_map(|(from, to)| Step::Sync { from, to }),
        ],
        0..30,
    )
}

fn run(devices: &mut [Device], script: &[Step]) {
    for step in script {
        match step {
            Step::Write(device, op) => {
                devices[*device].apply(op);
            }
            Step::Sync { from, to } => {
                if from != to {
                    let (from, to) = (&devices[*from], &devices[*to]);
                    to.take(from);
                }
            }
        }
    }
}

fn devices(n: usize) -> Vec<Device> {
    (0..n).map(Device::new).collect()
}

/// A fresh device holding the union of `parts`, taken in that order.
fn union(parts: &[&Device]) -> Device {
    let merged = Device::new(9);
    for part in parts {
        merged.take(part);
    }
    merged
}

fn tags_of(reading: &Reading, n: usize) -> BTreeSet<Tag> {
    reading.tags(&target(n))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Three devices write and sync in any interleaving. However their objects are then merged,
    /// the reading is one value: union commutes, associates and is idempotent, and every device
    /// reads it once each has taken in the others' objects.
    #[test]
    fn merging_is_commutative_associative_and_idempotent(script in steps(3)) {
        let mut all = devices(3);
        run(&mut all, &script);
        let [a, b, c] = [&all[0], &all[1], &all[2]];
        let ab = union(&[a, b]).read();
        prop_assert_eq!(&ab, &union(&[b, a]).read(), "commutative");
        let left = union(&[&union(&[a, b]), c]).read();
        let right = union(&[a, &union(&[b, c])]).read();
        prop_assert_eq!(&left, &right, "associative");
        let twice = union(&[a, a, b, b]);
        prop_assert_eq!(&twice.read(), &ab, "idempotent");
        prop_assert_eq!(twice.objects(), union(&[a, b]).objects(), "idempotent in objects");
        // Each device takes in the others', in a different order, and all read the union.
        a.take(b);
        a.take(c);
        b.take(c);
        b.take(a);
        c.take(a);
        for device in [a, b, c] {
            prop_assert_eq!(&device.read(), &left, "converged");
        }
    }

    /// Two devices share a history, then write memberships apart. After a merge in either
    /// direction a membership reads as the one device that wrote it after the fork left it, and
    /// where both did, it is on if either left it on: a removal takes off only the adds its
    /// writer saw.
    #[test]
    fn an_add_and_a_remove_apart_merge_the_same_way_in_either_order(
        base in proptest::collection::vec(membership_op(), 0..8),
        mine in proptest::collection::vec(membership_op(), 0..6),
        theirs in proptest::collection::vec(membership_op(), 0..6),
    ) {
        let mut all = devices(2);
        for op in &base {
            all[0].apply(op);
        }
        all[1].take(&all[0]);
        let before = all[0].read();
        let mut wrote: [BTreeSet<(usize, usize)>; 2] = Default::default();
        for (device, ops) in [(0, &mine), (1, &theirs)] {
            for op in ops {
                let (n, name) = match op {
                    Op::Tag(n, name) | Op::Untag(n, name) => (*n, *name),
                    Op::Rename(..) => unreachable!("memberships only"),
                };
                if all[device].apply(op) > 0 {
                    wrote[device].insert((n, name));
                }
            }
        }
        let (a, b) = (all[0].read(), all[1].read());
        let one = union(&[&all[0], &all[1]]).read();
        let other = union(&[&all[1], &all[0]]).read();
        prop_assert_eq!(&one, &other);
        all[0].take(&all[1]);
        all[1].take(&all[0]);
        prop_assert_eq!(&all[0].read(), &one);
        prop_assert_eq!(&all[1].read(), &one);
        for n in 0..2 {
            for (name, spelled) in NAMES.iter().enumerate().take(2) {
                let on = |reading: &Reading| tags_of(reading, n).contains(&tag(spelled));
                let expected = match (wrote[0].contains(&(n, name)), wrote[1].contains(&(n, name))) {
                    (false, false) => on(&before),
                    (true, false) => on(&a),
                    (false, true) => on(&b),
                    (true, true) => on(&a) || on(&b),
                };
                prop_assert_eq!(on(&one), expected, "target {} tag {}", n, spelled);
            }
        }
    }

    /// After a rename, every target that carried the renamed tag or the tag `to` stood for
    /// carries `to`, every other tag stays where it was, and the same rename again writes
    /// nothing.
    #[test]
    fn a_rename_keeps_every_membership(
        script in proptest::collection::vec(op(), 0..20),
        from in 0usize..4,
        to in 0usize..4,
    ) {
        let mut device = Device::new(0);
        for op in &script {
            device.apply(op);
        }
        let before = device.read();
        let (from_tag, to_tag) = (tag(NAMES[from]), tag(NAMES[to]));
        let merged = [before.spelling(&from_tag), before.spelling(&to_tag)];
        let memberships = device.objects().0;
        device.apply(&Op::Rename(from, to));
        let after = device.read();
        prop_assert_eq!(device.objects().0, memberships, "a rename writes no membership");
        for n in 0..4 {
            let expected: BTreeSet<Tag> = tags_of(&before, n)
                .into_iter()
                .map(|tag| if merged.contains(&tag) { to_tag.clone() } else { tag })
                .collect();
            prop_assert_eq!(tags_of(&after, n), expected);
        }
        for name in before.names(&from_tag).iter().chain(&before.names(&to_tag)) {
            prop_assert_eq!(after.spelling(name), to_tag.clone());
        }
        prop_assert!(after.disputes().iter().all(|dispute| match dispute {
            Dispute::Aliases { tag, .. } => after.spelling(tag) != to_tag,
            Dispute::Ring { names, .. } => !names.contains(&to_tag),
        }), "the rename settles the names it writes");
        prop_assert_eq!(device.apply(&Op::Rename(from, to)), 0, "a second rename writes nothing");
    }

    /// Tagging twice writes once, and removing twice writes once.
    #[test]
    fn a_second_run_writes_nothing(script in proptest::collection::vec(op(), 0..20), n in 0usize..4, name in 0usize..4) {
        let mut device = Device::new(0);
        for op in &script {
            device.apply(op);
        }
        device.apply(&Op::Tag(n, name));
        prop_assert_eq!(device.apply(&Op::Tag(n, name)), 0);
        prop_assert!(device.apply(&Op::Untag(n, name)) > 0);
        prop_assert_eq!(device.apply(&Op::Untag(n, name)), 0);
        prop_assert!(!tags_of(&device.read(), n).contains(&device.read().spelling(&tag(NAMES[name]))));
    }

    /// Every event a device writes prints and parses back to itself.
    #[test]
    fn every_event_prints_and_parses_back(script in proptest::collection::vec(op(), 0..20)) {
        let mut device = Device::new(0);
        for op in &script {
            device.apply(op);
        }
        for envelope in device.memberships.held().expect("held").dag.objects().values() {
            if let Some(event) = envelope.event() {
                let print = canonical(event);
                prop_assert_eq!(&parse_event::<MembershipEvent>(&MembershipVocabulary, &print).expect("parses"), event);
            }
        }
        for envelope in device.aliases.held().expect("held").dag.objects().values() {
            if let Some(event) = envelope.event() {
                let print = canonical(event);
                prop_assert_eq!(&parse_event::<AliasEvent>(&AliasVocabulary, &print).expect("parses"), event);
            }
        }
    }

    /// Any text slugs to nothing or to a tag, and a tag slugs to itself and parses as itself.
    #[test]
    fn a_slug_is_a_tag_and_a_tag_its_own_slug(text in "\\PC{0,80}") {
        if let Some(slug) = Tag::slug(&text) {
            prop_assert_eq!(Tag::new(slug.as_str()), Ok(slug.clone()));
            prop_assert_eq!(Tag::slug(slug.as_str()), Some(slug.clone()));
            prop_assert!(slug.as_str().chars().count() <= MAX);
        }
    }

    /// A target prints and parses back to itself.
    #[test]
    fn a_target_prints_and_parses_back(n in any::<usize>(), conversation in "[a-z0-9][a-z0-9-]{0,20}") {
        let conversation = ConversationId::new(conversation).expect("a slug");
        for target in [
            Target::Round { conversation: conversation.clone(), turn: n },
            Target::Note { conversation: conversation.clone(), turn: n },
        ] {
            prop_assert_eq!(target.to_string().parse::<Target>(), Ok(target.clone()));
        }
    }
}

/// The two devices of the concrete cases below, sharing what `shared` writes on the first.
fn forked(shared: &[Op]) -> [Device; 2] {
    let [mut a, b] = [Device::new(0), Device::new(1)];
    for op in shared {
        a.apply(op);
    }
    b.take(&a);
    [a, b]
}

fn merged_both_ways(a: &Device, b: &Device) -> Reading {
    let one = union(&[a, b]).read();
    assert_eq!(one, union(&[b, a]).read(), "either order");
    one
}

const TRAVEL: usize = 0;
const TRIPS: usize = 1;
const WORK: usize = 2;
const JOB: usize = 3;

#[test]
fn a_tag_under_the_old_name_lands_in_the_renamed_tag() {
    let [mut a, mut b] = forked(&[Op::Tag(0, WORK)]);
    a.apply(&Op::Rename(WORK, JOB));
    b.apply(&Op::Tag(1, WORK));
    let reading = merged_both_ways(&a, &b);
    for n in [0, 1] {
        assert_eq!(
            tags_of(&reading, n),
            BTreeSet::from([tag("job")]),
            "round {n}"
        );
    }
}

#[test]
fn two_renames_into_each_other_read_as_the_least_name_until_one_settles_them() {
    let [mut a, mut b] = forked(&[Op::Tag(0, TRAVEL), Op::Tag(1, TRIPS)]);
    a.apply(&Op::Rename(TRAVEL, TRIPS));
    b.apply(&Op::Rename(TRIPS, TRAVEL));
    let reading = merged_both_ways(&a, &b);
    assert_eq!(
        reading.disputes(),
        vec![Dispute::Ring {
            names: BTreeSet::from([tag("travel"), tag("trips")]),
            spelled: tag("travel"),
        }]
    );
    for n in [0, 1] {
        assert_eq!(tags_of(&reading, n), BTreeSet::from([tag("travel")]));
    }
    let mut settled = union(&[&a, &b]);
    settled.apply(&Op::Rename(TRIPS, TRIPS));
    let reading = settled.read();
    assert!(reading.disputes().is_empty());
    for n in [0, 1] {
        assert_eq!(tags_of(&reading, n), BTreeSet::from([tag("trips")]));
    }
}

#[test]
fn two_renames_of_one_tag_apart_are_a_dispute_every_device_reads_alike() {
    let [mut a, mut b] = forked(&[Op::Tag(0, TRAVEL)]);
    a.apply(&Op::Rename(TRAVEL, TRIPS));
    b.apply(&Op::Rename(TRAVEL, WORK));
    let reading = merged_both_ways(&a, &b);
    let disputes = reading.disputes();
    let [
        Dispute::Aliases {
            tag: renamed,
            chosen,
            others,
        },
    ] = disputes.as_slice()
    else {
        panic!("one dispute, got {disputes:?}");
    };
    assert_eq!(renamed, &tag("travel"));
    let both: BTreeSet<Alias> = [chosen.clone()]
        .into_iter()
        .chain(others.iter().cloned())
        .collect();
    assert_eq!(
        both,
        BTreeSet::from([Alias::As(tag("trips")), Alias::As(tag("work"))])
    );
    let Alias::As(spelled) = chosen else {
        panic!("an alias");
    };
    assert_eq!(tags_of(&reading, 0), BTreeSet::from([spelled.clone()]));
}
