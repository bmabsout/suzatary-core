//! The batch laws as proptests over generated worlds shaped like the agent's runs (each sealed
//! on the target's head, one object on the last), plus unit tests of what a batch is.

use std::collections::{BTreeMap, BTreeSet};

use prodrome_core::event::{
    Hash, mk_cancelled, mk_completed, mk_created, mk_sealed, parents_of, parse_envelope, seal_hash,
};
use prodrome_core::literal::Datetime;

use crate::record::Envelope;
use crate::sync::Objects;

use super::{Acceptance, Batch, accept, batches, reject};

fn at(day: u32) -> Datetime {
    Datetime::new(2026, 9, day.clamp(1, 28), 12, 0, 0, 0).expect("a real instant")
}

fn set(objects: &[Envelope]) -> Objects {
    let mut out = Objects::default();
    for object in objects {
        out.insert(seal_hash(object), object.clone());
    }
    out
}

fn merge(into: &Objects, objects: &[Envelope]) -> Objects {
    let mut out = into.clone();
    for object in objects {
        out.insert(seal_hash(object), object.clone());
    }
    out
}

/// A chain of `count` objects for `todo`, each sealed on the last, starting
/// from `from`.
fn run(from: Option<&Hash>, todo: &str, count: u32, who: &str) -> Vec<Envelope> {
    let mut out = Vec::new();
    let mut prev = from.cloned();
    for n in 0..count {
        let event = if n == 0 {
            mk_created(todo, at(n + 1), who, "", "").expect("valid")
        } else if n % 2 == 1 {
            mk_completed(todo, at(n + 1), who, "").expect("valid")
        } else {
            mk_cancelled(todo, at(n + 1), who, "").expect("valid")
        };
        let object = mk_sealed(prev.clone(), event);
        prev = Some(seal_hash(&object));
        out.push(object);
    }
    out
}

/// The head-fold: rows over everything a set holds, under the empty policy.
fn folded(objects: &Objects) -> Vec<crate::record::Entry> {
    let by_name: BTreeMap<Hash, Envelope> = objects
        .names()
        .into_iter()
        .map(|name| {
            let envelope = objects.get(&name).expect("named").clone();
            (name, envelope)
        })
        .collect();
    let ordered: Vec<(Hash, Envelope)> = prodrome_core::dag::Dag::from_iter(by_name.clone())
        .linearise()
        .expect("a closed set linearises")
        .into_iter()
        .map(|name| {
            let envelope = by_name[&name].clone();
            (name, envelope)
        })
        .collect();
    let nodes = prodrome_core::dag::Dag::from_iter(ordered)
        .nodes()
        .expect("a closed set reads");
    prodrome_core::view::entries(&nodes, at(28), &prodrome_core::policy::Untrusted::none())
        .expect("a closed set folds")
}

/// Apply an acceptance the way the binary does: take in exactly what it names.
fn applied(target: &Objects, outcome: &Acceptance) -> Objects {
    let mut after = target.clone();
    for (name, literal) in &outcome.taken {
        let envelope: Envelope = parse_envelope(&crate::record::TodoVocabulary::default(), literal)
            .expect("we printed it");
        assert_eq!(&seal_hash(&envelope), name, "the name is the print's hash");
        after.insert(name.clone(), envelope);
    }
    after
}

/// A store with an object removed, which is what `Replica::discard` does.
fn without(objects: &Objects, going: &[Hash]) -> Objects {
    let mut out = Objects::default();
    for name in objects.names() {
        if !going.contains(&name) {
            out.insert(name.clone(), objects.get(&name).expect("named").clone());
        }
    }
    out
}

/// Every object's parents are held, or are outside the set entirely: the structural half of
/// `verify`.
fn parent_closed(objects: &Objects, alongside: &Objects) -> bool {
    objects.names().into_iter().all(|name| {
        parents_of(objects.get(&name).expect("named"))
            .iter()
            .all(|parent| objects.holds(parent) || alongside.holds(parent))
    })
}

fn names(of: &[Envelope]) -> Vec<Hash> {
    of.iter().map(seal_hash).collect()
}

// --- What a batch is ---------------------------------------------------------

/// A replica head the target holds is the target's own tip, not a batch.
#[test]
fn the_targets_own_tip_is_not_a_proposal() {
    let chain = run(None, "alpha", 3, "ada");
    let target = set(&chain);
    let replica = target.clone();
    assert!(batches(&target, &replica).is_empty());
}

/// One run's changes to two todos are two batches: a change rests on its own todo's heads, so
/// each is closed over the chain alone, and accepting one leaves the other standing whole.
#[test]
fn a_run_of_changes_to_two_todos_is_two_batches_that_commute() {
    use prodrome_core::change::mk_change;
    use prodrome_core::genesis::mk_genesis;

    let genesis = Envelope::Genesis(mk_genesis("todos", &"0".repeat(32)).expect("a genesis"));
    let root = seal_hash(&genesis);
    let alpha = Envelope::Change(
        mk_change(
            root.clone(),
            Vec::new(),
            mk_created("alpha", at(1), "ada", "", "").expect("valid"),
        )
        .expect("a change"),
    );
    let target = set(&[genesis, alpha.clone()]);
    let closes = Envelope::Change(
        mk_change(
            root.clone(),
            vec![seal_hash(&alpha)],
            mk_completed("alpha", at(2), "triage", "").expect("valid"),
        )
        .expect("a change"),
    );
    let adds = Envelope::Change(
        mk_change(
            root,
            Vec::new(),
            mk_created("beta", at(3), "triage", "", "").expect("valid"),
        )
        .expect("a change"),
    );
    let replica = merge(&target, &[closes.clone(), adds.clone()]);

    let standing = batches(&target, &replica);
    assert_eq!(standing.len(), 2, "{standing:?}");
    assert_eq!(standing[0].todo.as_str(), "beta", "newest first");
    assert_eq!(standing[0].objects.len(), 1);
    assert_eq!(standing[1].todo.as_str(), "alpha");
    assert_eq!(standing[1].head, seal_hash(&closes));

    let outcome = accept(&target, &replica, &[seal_hash(&adds)]).expect("one batch alone");
    let after = applied(&target, &outcome);
    let left = batches(&after, &replica);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0], standing[1], "the other batch is untouched");
}

/// A legacy run sealed each object on the last, across todos; a batch takes what its todo's
/// writes rest on, so an earlier todo's object is in each later todo's batch too.
#[test]
fn a_legacy_run_across_todos_puts_what_a_write_rests_on_in_its_batch() {
    let chain = run(None, "alpha", 1, "ada");
    let tip = seal_hash(&chain[0]);
    let first = run(Some(&tip), "beta", 1, "triage");
    let second = run(Some(&seal_hash(&first[0])), "gamma", 1, "triage");
    let target = set(&chain);
    let replica = merge(&merge(&target, &first), &second);

    let standing = batches(&target, &replica);
    let gamma = standing
        .iter()
        .find(|batch| batch.todo.as_str() == "gamma")
        .expect("gamma's batch");
    assert_eq!(
        gamma
            .objects
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>(),
        vec![seal_hash(&first[0]), seal_hash(&second[0])],
        "parents first"
    );
    assert_eq!(gamma.head, seal_hash(&second[0]));
    accept(&target, &replica, &[seal_hash(&second[0])])
        .expect_err("a pick without what it rests on is refused");
}

/// An object resolves to the batch it is in (the first head when two stand), and one the chain
/// holds to nothing: "it exists" and "it is proposed" are different refusals.
#[test]
fn an_object_resolves_to_the_batch_it_is_in_and_a_settled_one_to_none() {
    let chain = run(None, "alpha", 2, "ada");
    let tip = seal_hash(chain.last().expect("two objects"));
    let monday = run(Some(&tip), "beta", 2, "triage");
    let tuesday = run(Some(&tip), "gamma", 1, "triage");

    let target = set(&chain);
    let replica = merge(&merge(&target, &monday), &tuesday);
    let settled: BTreeSet<Hash> = target.names().into_iter().collect();
    let nodes = nodes_of(&replica);

    let monday_head = seal_hash(monday.last().expect("two"));
    let tuesday_head = seal_hash(tuesday.last().expect("one"));
    for object in &monday {
        assert_eq!(
            super::batch_of(&nodes, &settled, &seal_hash(object)),
            Some(monday_head.clone()),
        );
    }
    assert_eq!(
        super::batch_of(&nodes, &settled, &tuesday_head),
        Some(tuesday_head.clone()),
        "a head is in its own batch",
    );
    for object in &chain {
        assert_eq!(
            super::batch_of(&nodes, &settled, &seal_hash(object)),
            None,
            "an object the chain holds is settled, whatever descends from it",
        );
    }
}

/// A pick is grouped by batch before it is settled, since settling removes what batches derive
/// from.
#[test]
fn a_pick_spanning_two_batches_is_recorded_as_two() {
    let chain = run(None, "alpha", 2, "ada");
    let tip = seal_hash(chain.last().expect("two objects"));
    let monday = run(Some(&tip), "beta", 2, "triage");
    let tuesday = run(Some(&tip), "gamma", 1, "triage");
    let target = set(&chain);
    let replica = merge(&merge(&target, &monday), &tuesday);

    let picked: Vec<Hash> = vec![
        seal_hash(&monday[0]),
        seal_hash(&tuesday[0]),
        // A name nothing holds is dropped here and refused by `accept`.
        seal_hash(&run(None, "nowhere", 1, "triage")[0]),
    ];
    let grouped = super::grouped(&target, &replica, &picked);
    assert_eq!(grouped.len(), 2, "{grouped:?}");
    let heads: BTreeSet<Hash> = grouped.iter().map(|(head, _)| head.clone()).collect();
    assert_eq!(
        heads,
        [
            seal_hash(monday.last().expect("two")),
            seal_hash(tuesday.last().expect("one")),
        ]
        .into_iter()
        .collect::<BTreeSet<Hash>>(),
    );
    for (_, objects) in &grouped {
        assert_eq!(objects.len(), 1, "one picked object in each");
    }
}

/// The nodes a reader of a store gets, over a set held in memory.
fn nodes_of(objects: &Objects) -> Vec<crate::record::Node> {
    let by_name: BTreeMap<Hash, Envelope> = objects
        .names()
        .into_iter()
        .map(|name| {
            let envelope = objects.get(&name).expect("named").clone();
            (name, envelope)
        })
        .collect();
    let ordered: Vec<(Hash, Envelope)> = prodrome_core::dag::Dag::from_iter(by_name.clone())
        .linearise()
        .expect("a closed set linearises")
        .into_iter()
        .map(|name| {
            let envelope = by_name[&name].clone();
            (name, envelope)
        })
        .collect();
    prodrome_core::dag::Dag::from_iter(ordered)
        .nodes()
        .expect("a closed set reads")
}

/// One run is one batch, and its objects are exactly the run's, parents first.
#[test]
fn one_run_is_one_batch_of_exactly_its_own_objects() {
    let chain = run(None, "alpha", 2, "ada");
    let target = set(&chain);
    let tip = seal_hash(&chain[1]);
    let batch = run(Some(&tip), "beta", 3, "triage");
    let replica = merge(&target, &batch);

    let found = batches(&target, &replica);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].head, seal_hash(&batch[2]));
    assert_eq!(
        found[0]
            .objects
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>(),
        names(&batch),
        "parents first, and nothing the target already holds"
    );
}

/// Two runs are two batches, because each seals on the target's head, not on the other.
#[test]
fn two_runs_off_one_head_are_two_independent_batches() {
    let chain = run(None, "alpha", 1, "ada");
    let target = set(&chain);
    let tip = seal_hash(&chain[0]);
    let monday = run(Some(&tip), "beta", 2, "triage");
    let tuesday = run(Some(&tip), "gamma", 1, "triage");
    let replica = merge(&merge(&target, &monday), &tuesday);

    let found = batches(&target, &replica);
    assert_eq!(found.len(), 2);
    let heads: BTreeSet<Hash> = found.iter().map(|batch| batch.head.clone()).collect();
    assert_eq!(
        heads,
        [seal_hash(&monday[1]), seal_hash(&tuesday[0])]
            .into_iter()
            .collect()
    );
    for batch in &found {
        assert!(
            batch.objects.iter().all(|(_, envelope)| {
                envelope
                    .event()
                    .is_some_and(|e| e.actor().as_str() == "triage")
            }),
            "a batch holds the run's objects and nothing of the chain's"
        );
    }
}

/// A half-accepted batch needs no bookkeeping: what is left is the batch.
#[test]
fn accepting_a_prefix_leaves_the_rest_as_the_batch() {
    let chain = run(None, "alpha", 1, "ada");
    let target = set(&chain);
    let tip = seal_hash(&chain[0]);
    let batch = run(Some(&tip), "beta", 3, "triage");
    let replica = merge(&target, &batch);

    let taken = accept(&target, &replica, &names(&batch)[..2]).expect("a closed prefix");
    let after = applied(&target, &taken);

    let left = batches(&after, &replica);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].head, seal_hash(&batch[2]));
    assert_eq!(left[0].objects.len(), 1, "only the object still outside");
}

// --- The refusals ------------------------------------------------------------

/// An ancestor-open pick is refused, naming what is missing.
#[test]
fn an_ancestor_open_pick_is_refused_and_names_what_is_missing() {
    let chain = run(None, "alpha", 1, "ada");
    let target = set(&chain);
    let tip = seal_hash(&chain[0]);
    let batch = run(Some(&tip), "beta", 2, "triage");
    let replica = merge(&target, &batch);

    // The completion without the todo it completes.
    let refused = accept(&target, &replica, &[seal_hash(&batch[1])]).expect_err("open");
    assert!(
        refused
            .message()
            .contains(&seal_hash(&batch[0]).as_str()[..12]),
        "the refusal names the missing ancestor: {}",
        refused.message()
    );
    assert!(
        refused
            .message()
            .contains("Picking a completion means taking the todo it completes"),
        "{}",
        refused.message()
    );
}

/// The mirror: a rejection that would orphan something still in the replica is refused.
#[test]
fn a_descendant_open_rejection_is_refused_and_names_what_rests_on_it() {
    let chain = run(None, "alpha", 1, "ada");
    let target = set(&chain);
    let tip = seal_hash(&chain[0]);
    let batch = run(Some(&tip), "beta", 2, "triage");
    let replica = merge(&target, &batch);

    let refused = reject(&target, &replica, &[seal_hash(&batch[0])]).expect_err("open");
    assert!(
        refused
            .message()
            .contains(&seal_hash(&batch[1]).as_str()[..12]),
        "{}",
        refused.message()
    );
    // …and the whole batch is fine.
    assert_eq!(
        reject(&target, &replica, &names(&batch))
            .expect("closed")
            .len(),
        2
    );
}

/// An object on the chain cannot be rejected: rejecting is deletion, and the chain never rewrites.
#[test]
fn rejecting_something_the_chain_already_holds_is_refused() {
    let chain = run(None, "alpha", 2, "ada");
    let target = set(&chain);
    let replica = target.clone();
    let refused = reject(&target, &replica, &[seal_hash(&chain[1])]).expect_err("on the chain");
    assert!(
        refused.message().contains("is on the chain already"),
        "{}",
        refused.message()
    );
}

/// A name nobody proposed is a 404 from either verb.
#[test]
fn a_name_no_batch_holds_is_not_found() {
    let target = set(&run(None, "alpha", 1, "ada"));
    let replica = target.clone();
    let stranger = Hash::new("f".repeat(64)).expect("64 hex");
    assert_eq!(
        accept(&target, &replica, std::slice::from_ref(&stranger))
            .expect_err("unknown")
            .status()
            .code(),
        404
    );
    assert_eq!(
        reject(&target, &replica, &[stranger])
            .expect_err("unknown")
            .status()
            .code(),
        404
    );
}

// --- The laws ----------------------------------------------------------------

use proptest::prelude::*;

/// A generated world: a target chain, then `runs` batches off its head, each a little chain.
fn a_world() -> impl Strategy<Value = (Vec<Envelope>, Vec<Vec<Envelope>>)> {
    (1usize..4, proptest::collection::vec(1u32..4, 1..4)).prop_map(|(depth, runs)| {
        let chain = run(None, "alpha", u32::try_from(depth).unwrap_or(1), "ada");
        let tip = chain.last().map(seal_hash);
        let batches = runs
            .iter()
            .enumerate()
            .map(|(n, count)| run(tip.as_ref(), &format!("todo{n}"), *count, "triage"))
            .collect();
        (chain, batches)
    })
}

/// A world as today's writers make it: a chain begun by a `Genesis` with a todo of `depth`
/// writes, and a replica of it where the agent wrote, as `Change`s over each todo's heads, a
/// todo of its own per run and, where asked, a reopening of the chain's.
fn a_world_of_changes() -> impl Strategy<Value = (Objects, Objects)> {
    (
        1u32..4,
        proptest::collection::vec((1u32..4, proptest::bool::ANY), 1..4),
    )
        .prop_map(|(depth, runs)| {
            let lifecycle = |todo: &str, n: u32, who: &str| {
                if n == 0 {
                    mk_created(todo, at(n + 1), who, "", "").expect("valid")
                } else if n % 2 == 1 {
                    mk_completed(todo, at(n + 1), who, "").expect("valid")
                } else {
                    prodrome_core::event::mk_reopened(todo, at(n + 1), who, "").expect("valid")
                }
            };
            let mut chain = crate::believed::Preview::default();
            for n in 0..depth {
                chain.ahead(lifecycle("alpha", n, "ada")).expect("writes");
            }
            let mut replica = chain.clone();
            for (run, (count, touches)) in runs.iter().enumerate() {
                for n in 0..*count {
                    replica
                        .ahead(lifecycle(&format!("todo{run}"), n, "triage"))
                        .expect("writes");
                }
                if *touches {
                    let at = at(10 + u32::try_from(run).unwrap_or(0));
                    replica
                        .ahead(
                            prodrome_core::event::mk_reopened("alpha", at, "triage", "")
                                .expect("valid"),
                        )
                        .expect("writes");
                }
            }
            (Objects::of(chain.objects()), Objects::of(replica.objects()))
        })
}

proptest! {
    /// Accept is union: after accepting a batch whole, the target IS target ∪ batch, object for
    /// object, and so folds as it.
    #[test]
    fn accepting_a_batch_folds_as_the_target_union_the_batch(
        (chain, runs) in a_world(),
    ) {
        let target = set(&chain);
        let mut replica = target.clone();
        for batch in &runs {
            replica = merge(&replica, batch);
        }
        // Not `runs.len()`: two generated runs can be the same objects, and so one head.
        let standing = batches(&target, &replica);
        prop_assert!(!standing.is_empty());

        for batch in &standing {
            let picked: Vec<Hash> = batch.objects.iter().map(|(name, _)| name.clone()).collect();
            let outcome = accept(&target, &replica, &picked).expect("a whole batch is closed");
            let after = applied(&target, &outcome);

            let mut union = target.clone();
            for (name, envelope) in &batch.objects {
                union.insert(name.clone(), envelope.clone());
            }
            prop_assert_eq!(after.names(), union.names());
            prop_assert_eq!(folded(&after), folded(&union));
        }
    }

    /// Over changes, the batches partition what stands: every standing object is in exactly one,
    /// each about the todo every object in it writes; and they commute, so accepting one leaves
    /// every other batch exactly as it was.
    #[test]
    fn batches_of_changes_partition_what_stands_and_commute(
        (target, replica) in a_world_of_changes(),
    ) {
        let standing = batches(&target, &replica);
        let held: Vec<Hash> = replica
            .names()
            .into_iter()
            .filter(|name| !target.holds(name))
            .collect();
        for name in &held {
            prop_assert_eq!(standing.iter().filter(|batch| batch.holds(name)).count(), 1);
        }
        prop_assert_eq!(
            standing.iter().map(|batch| batch.objects.len()).sum::<usize>(),
            held.len()
        );
        for batch in &standing {
            for (_, envelope) in &batch.objects {
                prop_assert_eq!(envelope.event().map(prodrome_core::event::TodoEvent::todo), Some(&batch.todo));
            }
        }
        for batch in &standing {
            let picked: Vec<Hash> = batch.objects.iter().map(|(name, _)| name.clone()).collect();
            let after = applied(&target, &accept(&target, &replica, &picked).expect("closed"));
            let left: Vec<Batch> = standing.iter().filter(|other| *other != batch).cloned().collect();
            prop_assert_eq!(batches(&after, &replica), left);
        }
    }

    /// Accepting is idempotent: the second pass takes nothing, reports every name as `already`,
    /// and leaves the fold alone.
    #[test]
    fn accepting_twice_is_the_same_store(
        (chain, runs) in a_world(),
    ) {
        let target = set(&chain);
        let mut replica = target.clone();
        for batch in &runs {
            replica = merge(&replica, batch);
        }
        for batch in batches(&target, &replica) {
            let picked: Vec<Hash> = batch.objects.iter().map(|(name, _)| name.clone()).collect();
            let once = applied(&target, &accept(&target, &replica, &picked).expect("closed"));
            let again = accept(&once, &replica, &picked).expect("closed the second time");
            prop_assert!(again.taken.is_empty(), "nothing left to take");
            prop_assert_eq!(again.already.len(), picked.len());
            let twice = applied(&once, &again);
            prop_assert_eq!(folded(&once), folded(&twice));
            prop_assert_eq!(once.names(), twice.names());
        }
    }

    /// Reject is nothing: the target's fold is unchanged and the replica's parents are all still
    /// held.
    #[test]
    fn rejecting_leaves_the_target_untouched_and_the_replica_whole(
        (chain, runs) in a_world(),
    ) {
        let target = set(&chain);
        let mut replica = target.clone();
        for batch in &runs {
            replica = merge(&replica, batch);
        }
        let before = folded(&target);
        for batch in batches(&target, &replica) {
            let picked: Vec<Hash> = batch.objects.iter().map(|(name, _)| name.clone()).collect();
            let going = reject(&target, &replica, &picked).expect("a whole batch is closed");
            prop_assert_eq!(going.len(), picked.len());
            let left = without(&replica, &going);
            // Rejecting removes from the replica, never the target.
            prop_assert_eq!(folded(&target), before.clone());
            prop_assert!(parent_closed(&left, &target), "the replica is still a store");
            for name in &going {
                prop_assert!(!left.holds(name));
            }
        }
    }
}

// --- Signatures travel with what they sign -----------------------------------

/// The agent's replica signs every write; a batch shows the writes, and accepting or rejecting
/// one takes or removes their signatures with them, so an accepted batch keeps the agent's
/// proof and a rejected one leaves nothing resting on a removed object.
#[test]
fn a_batchs_signatures_are_accepted_and_rejected_with_it() {
    let key = prodrome_core::sign::Secret::new(&"4".repeat(64)).expect("a seed");
    let base = run(None, "alpha", 1, "ada");
    let target = set(&base);
    let proposed = run(Some(&seal_hash(&base[0])), "beta", 2, "triage");
    let signatures: Vec<Envelope> = proposed
        .iter()
        .map(|object| Envelope::Signed(key.sign(&seal_hash(object))))
        .collect();
    let replica = merge(&merge(&target, &proposed), &signatures);
    let standing = batches(&target, &replica);
    assert_eq!(standing.len(), 1);
    assert_eq!(
        standing[0].objects.len(),
        2,
        "a batch shows the writes, not their signatures"
    );

    let outcome = accept(&target, &replica, &names(&proposed)).expect("a closed pick");
    let taken: BTreeSet<Hash> = outcome.taken.iter().map(|(name, _)| name.clone()).collect();
    assert!(names(&signatures).iter().all(|name| taken.contains(name)));
    let after = applied(&target, &outcome);
    assert!(
        batches(&after, &replica).is_empty(),
        "nothing is left standing"
    );

    let removed = reject(&target, &replica, &names(&proposed)).expect("a closed pick");
    let left = without(&replica, &removed);
    assert!(
        parent_closed(&left, &target),
        "no signature outlives its object"
    );
    assert!(names(&signatures).iter().all(|name| !left.holds(name)));
}
