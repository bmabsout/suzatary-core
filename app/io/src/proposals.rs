//! The agent's replica on disk: a replica of its chain, holding what the agent proposes.
//!
//! [`Proposing`] is a run: it syncs the chain into the replica, then appends each event as the
//! Prodrome's `Change`, decided on what the replica holds, so what it writes is closed over the
//! chain and nothing about the run is kept. `Replica::remove` is the only code that deletes
//! objects, under the store's own cross-process `flock`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use prodrome_core::event::{Hash, parents_of};
use suzatary_core::record::{Envelope, EventStore, Node, TodoEvent};
use suzatary_core::store::{Prodrome, StoreError, WriteLock};

use crate::objects::Objects;
use crate::present::Present as _;

/// How a failure names this store.
const STORE: &str = "proposal store";

/// Why the settling side appends nothing: a proposal is the agent's run, written by [`Proposing`].
const WRITTEN_BY_THE_RUN: &str = "a proposal is written by the agent's run alone";

/// The proposal store for one chain.
pub struct Replica {
    /// The replica, behind the same memo and reentrant lock every chain gets.
    store: Objects<TodoEvent>,
    root: PathBuf,
}

impl std::fmt::Debug for Replica {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Replica")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Replica {
    /// Open the replica at `root` under `policy`, which must be its target's.
    #[must_use]
    pub fn new(root: PathBuf, policy: prodrome_core::policy::Untrusted) -> Self {
        Self {
            store: Objects::chain(EventStore::new(root.clone(), policy)),
            root,
        }
    }

    /// The store underneath, which the agent reads and [`Replica::refresh`] syncs into.
    #[must_use]
    pub fn inner(&self) -> &EventStore {
        self.store.inner()
    }

    /// `sync(target, replica)`: take in every object of the chain the replica lacks, verified,
    /// so a run decides on what the chain holds. Answers how many it took.
    ///
    /// # Errors
    /// The target being unreadable, or the replica unwritable.
    pub fn refresh(&self, target: &EventStore) -> Result<usize, StoreError> {
        prodrome_core::store::sync(target, self.store.inner())
            .map(|taken| taken.len())
            .map_err(|e| StoreError::unwritable(STORE, e.to_string()))
    }

    fn object_path(&self, name: &Hash) -> PathBuf {
        self.root
            .join("objects")
            .join(format!("{}.py", name.as_str()))
    }

    /// Remove these objects, re-checking descendant closure under the store's lock.
    ///
    /// # Errors
    /// The replica unreadable, a surviving object resting on a going one, or a deletion failing.
    fn remove(&self, names: &[Hash]) -> Result<Vec<Hash>, StoreError> {
        let _locked = self.lock()?;
        let going: BTreeSet<&Hash> = names.iter().collect();
        let held = Prodrome::objects(&self.store)?;
        let resting: Vec<&Hash> = held
            .iter()
            .filter(|(name, envelope)| {
                !going.contains(name)
                    && parents_of(envelope)
                        .iter()
                        .any(|parent| going.contains(parent))
            })
            .map(|(name, _)| name)
            .collect();
        if !resting.is_empty() {
            return Err(StoreError::unwritable(
                STORE,
                format!(
                    "{} now rest on what this would remove; nothing was removed",
                    resting
                        .iter()
                        .map(|name| name.as_str().chars().take(12).collect::<String>())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        // Children first, so a deletion that fails halfway leaves every survivor's parents.
        let mut gone: BTreeSet<&Hash> = BTreeSet::new();
        for (name, _) in held.iter().rev().filter(|(name, _)| going.contains(name)) {
            let path = self.object_path(name);
            std::fs::remove_file(&path)
                .map_err(|e| StoreError::unwritable(STORE, format!("{}: {e}", path.display())))?;
            gone.insert(name);
        }
        // A name not held was already gone, which is not a failure.
        Ok(names
            .iter()
            .filter(|name| gone.contains(name))
            .cloned()
            .collect())
    }

    /// The store's own `flock` on `root/.lock`; never hold it across an append from this
    /// process.
    fn lock(&self) -> Result<crate::durable::Held, StoreError> {
        crate::durable::hold(&self.root.join(".lock"))
            .map_err(|e| StoreError::unwritable(STORE, format!("{}: {e}", self.root.display())))
    }
}

/// Read as the chain it replicates is read; only the agent's run writes it.
impl Prodrome<TodoEvent> for Replica {
    fn objects(&self) -> Result<Vec<(Hash, Envelope)>, StoreError> {
        self.store.objects()
    }

    fn nodes(&self) -> Result<Vec<Node>, StoreError> {
        self.store.nodes()
    }

    fn excluded(&self) -> Result<BTreeSet<Hash>, StoreError> {
        self.store.excluded()
    }

    fn tips(&self) -> Result<Vec<Hash>, StoreError> {
        self.store.tips()
    }

    fn verify(&self) -> Vec<String> {
        self.store.verify()
    }

    fn append(&self, _event: TodoEvent) -> Result<Hash, StoreError> {
        Err(StoreError::not_appended(STORE, WRITTEN_BY_THE_RUN))
    }

    fn accept(&self, _objects: &[(Hash, String)]) -> Result<Vec<Hash>, StoreError> {
        Err(StoreError::not_appended(STORE, WRITTEN_BY_THE_RUN))
    }

    fn locked(&self) -> Result<Box<dyn WriteLock + '_>, StoreError> {
        self.store.locked()
    }
}

impl suzatary_core::store::Replica<TodoEvent> for Replica {
    fn discard(&self, names: &[Hash]) -> Result<Vec<Hash>, StoreError> {
        let _held = self.store.locked()?;
        self.remove(names)
    }
}

/// The agent's run: the replica, synced with its chain on opening, and appended to.
pub struct Proposing {
    replica: Replica,
    /// The chain being proposed to, read and never written.
    target: EventStore,
}

impl std::fmt::Debug for Proposing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proposing")
            .field("replica", &self.replica)
            .field("target", &self.target.root())
            .finish_non_exhaustive()
    }
}

impl Proposing {
    /// Bind a replica to its target and sync the target into it up front.
    ///
    /// # Errors
    /// The target being unreadable or the replica unwritable.
    pub fn open(replica: Replica, target: EventStore) -> Result<Self, StoreError> {
        replica.refresh(&target)?;
        Ok(Self { replica, target })
    }

    /// The replica underneath, for a caller that wants to read the batches.
    #[must_use]
    pub fn replica(&self) -> &Replica {
        &self.replica
    }
}

impl Prodrome<TodoEvent> for Proposing {
    fn objects(&self) -> Result<Vec<(Hash, Envelope)>, StoreError> {
        Prodrome::objects(&self.replica.store)
    }

    fn nodes(&self) -> Result<Vec<Node>, StoreError> {
        Prodrome::nodes(&self.replica.store)
    }

    fn excluded(&self) -> Result<BTreeSet<Hash>, StoreError> {
        Prodrome::excluded(&self.replica.store)
    }

    fn tips(&self) -> Result<Vec<Hash>, StoreError> {
        Prodrome::tips(&self.replica.store)
    }

    fn verify(&self) -> Vec<String> {
        self.replica.store.verify()
    }

    /// A `Change` over the todo's heads in the replica, or nothing for a twin: what stands
    /// about that todo grows by one write, and no other todo's batch moves. Never begins a
    /// prodrome: the replica writes into the chain's, so a chain nothing has begun is refused.
    fn append(&self, event: TodoEvent) -> Result<Hash, StoreError> {
        self.replica
            .store
            .inner()
            .append(event)
            .map_err(|e| StoreError::not_appended("chain", e.to_string()))
    }

    /// Forwarded; the agent's bundle, not a refusal here, keeps this away from the agent.
    fn accept(&self, objects: &[(Hash, String)]) -> Result<Vec<Hash>, StoreError> {
        self.replica.store.accept(objects)
    }

    fn locked(&self) -> Result<Box<dyn WriteLock + '_>, StoreError> {
        self.replica.store.locked()
    }

    /// The target's objects. A target holding no `objects/` settles nothing; while the replica
    /// holds nothing either, that is a chain not yet begun and both are empty. A replica holding
    /// objects over a target with none is refused rather than read as empty: the agent's
    /// replica writes only into a chain that was begun, so the target is a chain moved or a path
    /// gone wrong, and an empty set would leave every replica object standing.
    fn settled(&self) -> Result<BTreeSet<Hash>, StoreError> {
        let objects = self.target.root().join("objects");
        let begun = std::fs::metadata(&objects)
            .present()
            .map_err(|e| StoreError::unreadable("chain", e))?
            .is_some();
        if !begun {
            if self.replica.store.objects()?.is_empty() {
                return Ok(BTreeSet::new());
            }
            return Err(StoreError::unreadable(
                "chain",
                format!(
                    "{} holds no objects, and its replica holds some: the chain was never begun there",
                    objects.display()
                ),
            ));
        }
        let dag = self
            .target
            .dag()
            .map_err(|e| StoreError::unreadable("chain", e))?;
        Ok(dag.objects().keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use prodrome_core::event::{Actor, mk_completed, mk_created};
    use prodrome_core::literal::Datetime;
    use prodrome_core::policy::Untrusted;

    use super::*;
    use suzatary_core::store::{Replica as _, append_to};

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "suzatary-proposals-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn at(day: u32) -> Datetime {
        Datetime::new(2026, 9, day, 12, 0, 0, 0).expect("a real instant")
    }

    fn policy() -> Untrusted {
        Untrusted::of([Actor::new("triage").expect("valid")])
    }

    /// Every object's name, as the store holds them.
    fn names(store: &dyn Prodrome<TodoEvent>) -> BTreeSet<Hash> {
        Prodrome::objects(store)
            .expect("reads")
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    /// What a change's writer saw, its deps, beside the genesis every change also rests on.
    fn deps(envelope: &Envelope) -> &[Hash] {
        match envelope {
            Envelope::Change(change) => &change.deps,
            _ => &[],
        }
    }

    /// A target chain begun by a `Genesis`, a todo and its completion, and a run bound to it.
    fn a_pair(name: &str) -> (EventStore, Proposing, PathBuf) {
        let root = scratch(name);
        let target = EventStore::new(root.join("events"), policy());
        append_to(
            &target,
            suzatary_core::record::GENESIS,
            mk_created("alpha", at(1), "ada", "", "").expect("valid"),
        )
        .expect("appends");
        append_to(
            &target,
            suzatary_core::record::GENESIS,
            mk_completed("alpha", at(2), "ada", "").expect("valid"),
        )
        .expect("appends");
        let replica = Replica::new(root.join("events-triage"), policy());
        let proposing = Proposing::open(replica, target.clone()).expect("opens");
        (target, proposing, root)
    }

    /// What the chain holds is settled; a chain that cannot be read settles nothing and says
    /// so, rather than reading as empty and leaving every object standing.
    #[test]
    fn an_unreadable_chain_settles_nothing_and_says_so() {
        let (target, proposing, root) = a_pair("unreadable");
        assert_eq!(
            proposing.settled().expect("reads"),
            names(&Objects::chain(target))
        );
        std::fs::remove_dir_all(root.join("events")).expect("removed");
        std::fs::write(root.join("events"), "not a directory").expect("a file in its place");
        assert!(proposing.settled().is_err());
        std::fs::remove_file(root.join("events")).expect("removed");
        assert!(
            proposing.settled().is_err(),
            "a chain with no objects/ under a replica holding some is refused, never the empty set"
        );
        let unbegun = scratch("unbegun");
        let nothing = Proposing::open(
            Replica::new(unbegun.join("events-triage"), policy()),
            EventStore::new(unbegun.join("events"), policy()),
        )
        .expect("opens");
        assert_eq!(
            nothing.settled().expect("a chain not begun"),
            BTreeSet::new(),
            "a chain not begun under an empty replica settles nothing, and leaves nothing standing"
        );
        let _ = std::fs::remove_dir_all(&unbegun);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A run writes `Change`s, each over its own todo's heads, into the replica and never the
    /// chain.
    #[test]
    fn a_run_writes_changes_over_each_todo_s_heads() {
        let (target, proposing, root) = a_pair("changes");
        let before = target.tips().expect("derives");
        let alpha = Objects::chain(target.clone())
            .objects()
            .expect("reads")
            .into_iter()
            .find(|(_, object)| {
                matches!(
                    object.event(),
                    Some(prodrome_core::event::TodoEvent::Completed(_))
                )
            })
            .expect("the completion")
            .0;

        let added = proposing
            .append(mk_created("beta", at(3), "triage", "", "").expect("valid"))
            .expect("appends");
        let reopened = proposing
            .append(prodrome_core::event::mk_reopened("alpha", at(4), "triage", "").expect("valid"))
            .expect("appends");

        assert_eq!(
            target.tips().expect("derives"),
            before,
            "the chain did not move"
        );
        let by_name: std::collections::BTreeMap<Hash, Envelope> = Prodrome::objects(&proposing)
            .expect("reads")
            .into_iter()
            .collect();
        assert!(matches!(by_name[&added], Envelope::Change(_)));
        assert!(
            deps(&by_name[&added]).is_empty(),
            "beta's first write rests on no write"
        );
        assert_eq!(
            deps(&by_name[&reopened]),
            [alpha],
            "alpha's head on the chain"
        );
        assert!(proposing.verify().is_empty(), "{:?}", proposing.verify());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A second run reads what the first proposed, so its write to the same todo rests on it:
    /// what a writer saw is what it rests on.
    #[test]
    fn a_second_run_rests_on_what_the_first_proposed_about_its_todo() {
        let (target, first_run, root) = a_pair("two-runs");
        let monday = first_run
            .append(mk_created("beta", at(3), "triage", "", "").expect("valid"))
            .expect("appends");
        drop(first_run);

        let replica = Replica::new(root.join("events-triage"), policy());
        let second_run = Proposing::open(replica, target.clone()).expect("opens");
        let tuesday = second_run
            .append(mk_completed("beta", at(4), "triage", "").expect("valid"))
            .expect("appends");

        let by_name: std::collections::BTreeMap<Hash, Envelope> = Prodrome::objects(&second_run)
            .expect("reads")
            .into_iter()
            .collect();
        assert_eq!(deps(&by_name[&tuesday]), [monday]);
        assert!(second_run.verify().is_empty(), "{:?}", second_run.verify());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A removal leaves a store `verify` still passes, holding the chain's objects.
    #[test]
    fn removing_a_batch_leaves_a_store_that_verifies() {
        let (target, proposing, root) = a_pair("remove");
        let first = proposing
            .append(mk_created("beta", at(3), "triage", "", "").expect("valid"))
            .expect("appends");
        let second = proposing
            .append(mk_completed("beta", at(4), "triage", "").expect("valid"))
            .expect("appends");

        let removed = proposing
            .replica()
            .discard(&[first, second])
            .expect("removes");
        assert_eq!(removed.len(), 2);
        assert_eq!(names(&proposing), names(&Objects::chain(target.clone())));
        assert!(proposing.verify().is_empty(), "{:?}", proposing.verify());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Syncing twice is syncing once, and a moved target is picked up by the next run.
    #[test]
    fn refreshing_takes_what_the_target_gained_and_nothing_else() {
        let (target, proposing, root) = a_pair("refresh");
        assert_eq!(Prodrome::objects(&proposing).expect("reads").len(), 3);
        assert_eq!(
            proposing.replica().refresh(&target).expect("again is free"),
            0
        );
        assert_eq!(Prodrome::objects(&proposing).expect("reads").len(), 3);

        append_to(
            &target,
            suzatary_core::record::GENESIS,
            mk_created("gamma", at(5), "ada", "", "").expect("valid"),
        )
        .expect("appends");
        proposing.replica().refresh(&target).expect("takes the new");
        assert_eq!(names(&proposing), names(&Objects::chain(target.clone())));
        assert!(proposing.verify().is_empty(), "{:?}", proposing.verify());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A removal waits on the store's lock and deletes nothing while it is held.
    #[test]
    fn a_removal_waits_for_the_store_s_cross_process_lock() {
        let (_target, proposing, root) = a_pair("remove-locked");
        let batch = proposing
            .append(mk_created("beta", at(3), "triage", "", "").expect("valid"))
            .expect("appends");
        let replica = Replica::new(root.join("events-triage"), policy());
        let lock = std::fs::File::create(root.join("events-triage").join(".lock")).expect("opens");
        lock.lock().expect("locks");
        let removing = std::thread::spawn(move || replica.remove(&[batch]));
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(Prodrome::objects(&proposing).expect("reads").len(), 4);
        drop(lock);
        let removed = removing.join().expect("joins").expect("removes");
        assert_eq!(removed.len(), 1);
        assert_eq!(Prodrome::objects(&proposing).expect("reads").len(), 3);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A removal that something now rests on removes nothing.
    #[test]
    fn a_removal_refuses_what_a_later_append_rests_on() {
        let (_target, proposing, root) = a_pair("remove-rested-on");
        let first = proposing
            .append(mk_created("beta", at(3), "triage", "", "").expect("valid"))
            .expect("appends");
        proposing
            .append(mk_completed("beta", at(4), "triage", "").expect("valid"))
            .expect("appends");
        let refused = proposing
            .replica()
            .discard(&[first])
            .expect_err("the second rests on the first");
        assert!(
            refused.to_string().contains("nothing was removed"),
            "{refused}"
        );
        assert_eq!(Prodrome::objects(&proposing).expect("reads").len(), 5);
        assert!(proposing.verify().is_empty(), "{:?}", proposing.verify());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The directory the race test hands its child.
    const RACE_ROOT: &str = "SUZATARY_IO_RACE_ROOT";
    const RACE_WRITES: u32 = 40;

    /// The child: a triage append in its own process, inert without its variable.
    #[test]
    #[ignore = "the child of an_append_from_another_process_survives_a_removal"]
    fn race_child_appends_a_batch() {
        let Some(root) = std::env::var_os(RACE_ROOT).map(PathBuf::from) else {
            return;
        };
        let target = EventStore::new(root.join("events"), policy());
        let run = Proposing::open(Replica::new(root.join("events-triage"), policy()), target)
            .expect("opens");
        run.append(mk_created("gamma", at(5), "triage", "", "").expect("valid"))
            .expect("appends");
        for minute in 0..RACE_WRITES {
            let at = Datetime::new(2026, 9, 5, 12, minute, 0, 0).expect("a real instant");
            run.append(
                prodrome_core::event::mk_reopened("gamma", at, "triage", "").expect("valid"),
            )
            .expect("appends");
        }
    }

    /// A reject beside a concurrent append leaves every appended object and the store verified.
    #[test]
    fn an_append_from_another_process_survives_a_removal() {
        let (_target, proposing, root) = a_pair("race");
        let mut going: Vec<Hash> = Vec::new();
        for day in 3..9 {
            going.push(
                proposing
                    .append(
                        mk_created(&format!("beta{day}"), at(day), "triage", "", "")
                            .expect("valid"),
                    )
                    .expect("appends"),
            );
        }
        let child = std::process::Command::new(std::env::current_exe().expect("the test binary"))
            .args([
                "proposals::tests::race_child_appends_a_batch",
                "--exact",
                "--ignored",
                "--quiet",
            ])
            .env(RACE_ROOT, &root)
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("spawns");
        let removed = proposing.replica().discard(&going).expect("removes");
        assert!(child.wait_with_output().expect("waits").status.success());
        assert_eq!(removed.len(), going.len());

        let objects = Prodrome::objects(&proposing).expect("reads");
        let appended = objects
            .iter()
            .filter(|(_, envelope)| {
                envelope
                    .event()
                    .is_some_and(|event| event.todo().as_str() == "gamma")
            })
            .count();
        assert_eq!(
            appended,
            usize::try_from(RACE_WRITES + 1).expect("a small count"),
            "the other process's batch is all there"
        );
        assert!(objects.iter().all(|(name, _)| !going.contains(name)));
        assert!(proposing.verify().is_empty(), "{:?}", proposing.verify());
        let _ = std::fs::remove_dir_all(&root);
    }
}
