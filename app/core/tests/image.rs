//! A history's image (`store::image`) over generated memory histories written by two replicas of
//! one prodrome and joined: the identity map under the history's own genesis is the history,
//! byte for byte; an involution's image of its image is the history; images compose as their
//! maps do; every object's image rests on exactly the images of what it rested on; and the
//! memory's reading of an image is the map of the history's reading.

use std::collections::{BTreeMap, BTreeSet};

use prodrome_core::event::{Actor, Envelope, Hash, canonical_envelope, parents_of};
use prodrome_core::genesis::{Genesis, mk_genesis};
use prodrome_core::literal::{Datetime, Value};
use proptest::prelude::*;
use suzatary_core::secretary::memory::{FactId, Kind, Origin};
use suzatary_core::secretary::memory_event::{MemoryEvent, MemoryVocabulary};
use suzatary_core::secretary::salted_text::{SALT_BYTES, Text};
use suzatary_core::secretary::store::ConversationId;
use suzatary_core::secretary::two_phase::TwoPhase;
use suzatary_core::store::{Image, InMemory, Prodrome, image};

#[derive(Debug, Clone)]
enum Step {
    /// Replica `b` (else `a`) remembers its own fact `n`.
    Remember { b: bool, n: u64, kind: Kind },
    /// Replica `b` forgets fact `n`.
    Forget { b: bool, n: u64 },
    /// `a` takes what `b` holds, or `b` what `a` holds.
    Sync { into_b: bool },
}

fn kind() -> impl Strategy<Value = Kind> {
    proptest::sample::select(Kind::ALL.to_vec())
}

fn steps(shared: bool) -> impl Strategy<Value = Vec<Step>> {
    proptest::collection::vec(
        prop_oneof![
            (any::<bool>(), 1u64..12, kind()).prop_map(|(b, n, kind)| Step::Remember {
                b,
                n,
                kind
            }),
            (any::<bool>(), 1u64..12).prop_map(|(b, n)| Step::Forget { b, n }),
            any::<bool>().prop_map(move |into_b| Step::Sync {
                into_b: into_b && shared
            }),
        ],
        0..40,
    )
}

fn at(n: u64) -> Datetime {
    Datetime::new(2026, 10, 5, 9, 0, u32::try_from(n % 60).expect("small"), 0).expect("an instant")
}

/// A fact's number owned by one replica: odd for `a`, even for `b`, so no two replicas write
/// one fact unless `shared`.
fn id(b: bool, n: u64, shared: bool) -> FactId {
    FactId(if shared { n } else { 2 * n + u64::from(!b) })
}

/// The objects of a store, each beside its bytes.
fn prints(store: &InMemory<MemoryEvent>) -> BTreeMap<Hash, String> {
    store
        .objects()
        .expect("reads")
        .iter()
        .map(|(name, object)| (name.clone(), canonical_envelope(object)))
        .collect()
}

/// Every step written, and the two replicas joined.
fn history(script: &[Step], shared: bool) -> BTreeMap<Hash, String> {
    let (a, b) = (
        InMemory::<MemoryEvent>::default(),
        InMemory::<MemoryEvent>::default(),
    );
    let said = Origin::Turn {
        conversation: ConversationId::new("default").expect("a slug"),
        turn: 0,
    };
    let actor = Actor::new("ada").expect("an actor");
    // A first remembering, so even an empty script's history is begun by its genesis.
    let begun = Step::Remember {
        b: false,
        n: 20,
        kind: Kind::Person,
    };
    for (step, n) in std::iter::once(&begun).chain(script).zip(1u64..) {
        let (into, event) = match step {
            Step::Remember {
                b: on_b,
                n: fact,
                kind,
            } => (
                *on_b,
                MemoryEvent::remembered(
                    id(*on_b, *fact, shared),
                    at(n),
                    actor.clone(),
                    Text::new(
                        suzatary_core::secretary::memory::Fact::new(format!("fact {fact}"))
                            .expect("a fact"),
                        [0; SALT_BYTES],
                    )
                    .name(),
                    *kind,
                    said.clone(),
                ),
            ),
            Step::Forget { b: on_b, n: fact } => {
                let Ok(event) =
                    MemoryEvent::forgotten(id(*on_b, *fact, shared), at(n), actor.clone(), 100 + n)
                else {
                    continue;
                };
                (*on_b, event)
            }
            Step::Sync { into_b } => {
                let (from, to) = if *into_b { (&a, &b) } else { (&b, &a) };
                let objects: Vec<_> = prints(from).into_iter().collect();
                to.accept(&objects)
                    .expect("a replica takes its sibling's objects");
                continue;
            }
        };
        // The store refuses what the register's order forbids (a fact remembered once
        // forgotten); a refusal writes nothing.
        let _ = if into { &b } else { &a }.append(event);
    }
    let objects: Vec<_> = prints(&b).into_iter().collect();
    a.accept(&objects).expect("joins");
    prints(&a)
}

/// The history's own genesis.
fn genesis_of(history: &BTreeMap<Hash, String>) -> Genesis {
    history
        .values()
        .find_map(|text| {
            match prodrome_core::event::parse_envelope::<MemoryEvent>(&MemoryVocabulary, text) {
                Ok(Envelope::Genesis(genesis)) => Some(genesis),
                _ => None,
            }
        })
        .expect("a history holds its genesis")
}

fn other(nonce: char) -> Genesis {
    mk_genesis("memory", &nonce.to_string().repeat(32)).expect("a genesis")
}

/// A kind's word swapped for another's, an involution on events.
fn swapped(from: Kind, to: Kind) -> impl Fn(&Value) -> Result<Value, String> {
    move |event| {
        let Value::Call(call) = event else {
            return Err("not a call".to_owned());
        };
        let mut call = call.clone();
        for (name, value) in &mut call.fields {
            if name == "kind" {
                match value.as_str() {
                    Some(word) if word == from.as_str() => *value = Value::str(to.as_str()),
                    Some(word) if word == to.as_str() => *value = Value::str(from.as_str()),
                    _ => {}
                }
            }
        }
        Ok(Value::Call(call))
    }
}

/// The identity map.
fn identity() -> impl Fn(&Value) -> Result<Value, String> {
    |event| Ok(event.clone())
}

fn imaged(
    history: &BTreeMap<Hash, String>,
    genesis: &Genesis,
    map: impl Fn(&Value) -> Result<Value, String>,
) -> Image {
    image::<MemoryEvent>(
        &MemoryVocabulary,
        history
            .iter()
            .map(|(name, text)| (name.clone(), text.clone())),
        genesis,
        map,
    )
    .expect("an image")
}

fn objects(image: &Image) -> BTreeMap<Hash, String> {
    image.objects.iter().cloned().collect()
}

fn reading(history: &BTreeMap<Hash, String>) -> TwoPhase {
    let store = InMemory::<MemoryEvent>::default();
    let objects: Vec<_> = history
        .iter()
        .map(|(name, text)| (name.clone(), text.clone()))
        .collect();
    store.accept(&objects).expect("a store takes the history");
    TwoPhase::of(&store.nodes().expect("reads"))
}

proptest! {
    #[test]
    fn the_identity_under_its_own_genesis_is_the_history(script in steps(true)) {
        let history = history(&script, true);
        prop_assert_eq!(objects(&imaged(&history, &genesis_of(&history), identity())), history);
    }

    #[test]
    fn an_involutions_image_of_its_image_is_the_history(script in steps(true)) {
        let history = history(&script, true);
        let swap = || swapped(Kind::Person, Kind::Standing);
        let there = objects(&imaged(&history, &other('1'), swap()));
        prop_assert_ne!(&there, &history, "another genesis renames every object");
        prop_assert_eq!(objects(&imaged(&there, &genesis_of(&history), swap())), history);
    }

    #[test]
    fn images_compose_as_their_maps_do(script in steps(true)) {
        let history = history(&script, true);
        let (f, g) = (
            swapped(Kind::Person, Kind::Standing),
            swapped(Kind::Preference, Kind::Standing),
        );
        let twice = objects(&imaged(&objects(&imaged(&history, &other('1'), &f)), &other('2'), &g));
        let once = objects(&imaged(&history, &other('2'), |event| g(&f(event)?)));
        prop_assert_eq!(twice, once);
    }

    #[test]
    fn every_image_rests_on_the_images_of_what_it_rested_on(script in steps(true)) {
        let history = history(&script, true);
        let image = imaged(&history, &other('1'), swapped(Kind::Person, Kind::Standing));
        prop_assert_eq!(image.names.len(), history.len());
        prop_assert_eq!(
            image.names.values().collect::<BTreeSet<_>>().len(),
            history.len(),
            "no two objects share an image"
        );
        let images = objects(&image);
        let parents = |text: &str| -> BTreeSet<Hash> {
            parents_of(
                &prodrome_core::event::parse_envelope::<MemoryEvent>(&MemoryVocabulary, text)
                    .expect("an object"),
            )
            .into_iter()
            .collect()
        };
        for (name, text) in &history {
            let mapped: BTreeSet<Hash> =
                parents(text).iter().map(|parent| image.names[parent].clone()).collect();
            prop_assert_eq!(parents(&images[&image.names[name]]), mapped);
        }
    }

    #[test]
    fn the_reading_of_an_image_is_the_map_of_the_reading(script in steps(false)) {
        let history = history(&script, false);
        let image = objects(&imaged(&history, &other('1'), swapped(Kind::Person, Kind::Standing)));
        let swap = |kind| match kind {
            Kind::Person => Kind::Standing,
            Kind::Standing => Kind::Person,
            other => other,
        };
        let (before, after) = (reading(&history), reading(&image));
        prop_assert_eq!(before.forgotten(), after.forgotten());
        let mapped: BTreeMap<_, _> = before
            .remembered()
            .iter()
            .map(|(id, kept)| {
                let mut kept = kept.clone();
                kept.kind = swap(kept.kind);
                (*id, kept)
            })
            .collect();
        prop_assert_eq!(&mapped, after.remembered());
    }
}

#[test]
fn a_history_of_two_prodromes_has_no_image() {
    let (a, b) = (
        history(&[], true),
        imaged(&history(&[], true), &other('1'), identity()),
    );
    let both: BTreeMap<_, _> = a.into_iter().chain(objects(&b)).collect();
    assert_eq!(
        image::<MemoryEvent>(&MemoryVocabulary, both, &other('2'), identity()),
        Err(suzatary_core::store::NotAnImage::Geneses(2))
    );
}
