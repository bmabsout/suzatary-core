//! The identities this process has established, by keyset digest, and Intel's
//! collateral, by platform: tabulations of [`Identity::establish`](super::Identity::establish)
//! and [`Collateral::fetch`](super::collateral::Collateral::fetch), each entry read only
//! before its own validity ends, so reading through them is computing again and nothing
//! else.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use super::collateral::Collateral;
use super::identity::Identity;
use super::pck::Platform;

/// A value a [`Table`] holds under its own key until its own validity ends.
pub(super) trait Expiring: Clone {
    /// The key the value answers for.
    type Key: Ord + Clone;
    /// That key.
    fn key(&self) -> Self::Key;
    /// The instant, Unix seconds, from which the value no longer answers.
    fn valid_until(&self) -> i64;
}

impl Expiring for Identity {
    type Key = String;

    fn key(&self) -> String {
        self.workload_keyset_digest.clone()
    }

    fn valid_until(&self) -> i64 {
        self.valid_until
    }
}

impl Expiring for Collateral {
    type Key = Platform;

    fn key(&self) -> Platform {
        self.platform
    }

    fn valid_until(&self) -> i64 {
        self.valid_until
    }
}

/// A function's answers, each under the key it answers for, read while valid. An
/// expired entry is dropped and computed again, so the table only ever moves down the
/// information order by forgetting; it holds no answer the function would not give.
#[derive(Debug)]
pub(super) struct Table<V: Expiring> {
    entries: Mutex<BTreeMap<V::Key, V>>,
}

impl<V: Expiring> Default for Table<V> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
        }
    }
}

impl<V: Expiring> Table<V> {
    /// The value under `key` while it is valid at `now`, else `compute`'s, kept under
    /// the key it answers for (which need not be `key`: the caller asked for one
    /// keyset and the gateway may serve another).
    pub(super) fn get_or_try<E>(
        &self,
        key: Option<&V::Key>,
        now: i64,
        compute: impl FnOnce() -> Result<V, E>,
    ) -> Result<V, E> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.retain(|_, value| now < value.valid_until());
        if let Some(found) = key.and_then(|key| entries.get(key)) {
            return Ok(found.clone());
        }
        let computed = compute()?;
        entries.insert(computed.key(), computed.clone());
        Ok(computed)
    }
}

/// The identities established, by keyset digest, and for each gateway the keyset it
/// last served, so a turn knows the identity to check before it sends; and Intel's
/// collateral, by platform, which a gateway's keyset rotation does not change.
#[derive(Debug, Default)]
pub struct Identities {
    by_keyset: Table<Identity>,
    served: Mutex<BTreeMap<String, String>>,
    collateral: Table<Collateral>,
}

impl Identities {
    /// Intel's collateral for `platform`: the one held while current, else `fetch`'s.
    ///
    /// # Errors
    /// `fetch`'s.
    pub fn collateral<E>(
        &self,
        platform: Platform,
        now: i64,
        fetch: impl FnOnce() -> Result<Collateral, E>,
    ) -> Result<Collateral, E> {
        self.collateral.get_or_try(Some(&platform), now, fetch)
    }

    /// The identity `base_url` serves: the one it last served while that is valid,
    /// else `establish`'s.
    ///
    /// # Errors
    /// `establish`'s.
    pub fn current<E>(
        &self,
        base_url: &str,
        now: i64,
        establish: impl FnOnce() -> Result<Identity, E>,
    ) -> Result<Identity, E> {
        let last = self.served(base_url);
        self.remember(
            base_url,
            self.by_keyset.get_or_try(last.as_ref(), now, establish),
        )
    }

    /// The identity of keyset `digest`, which `base_url` has just served: the one held
    /// while valid, else `establish`'s, which names the keyset the gateway serves now.
    ///
    /// # Errors
    /// `establish`'s.
    pub fn of<E>(
        &self,
        base_url: &str,
        digest: &str,
        now: i64,
        establish: impl FnOnce() -> Result<Identity, E>,
    ) -> Result<Identity, E> {
        self.remember(
            base_url,
            self.by_keyset
                .get_or_try(Some(&digest.to_owned()), now, establish),
        )
    }

    fn served(&self, base_url: &str) -> Option<String> {
        self.served
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(base_url)
            .cloned()
    }

    fn remember<E>(&self, base_url: &str, found: Result<Identity, E>) -> Result<Identity, E> {
        if let Ok(identity) = &found {
            self.served
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(base_url.to_owned(), identity.workload_keyset_digest.clone());
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use proptest::prelude::*;

    use super::super::testkit::{identity, keyset};
    use super::*;

    /// An answer of `f`, valid until its own instant.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Answer {
        key: u8,
        until: i64,
    }

    impl Expiring for Answer {
        type Key = u8;

        fn key(&self) -> u8 {
            self.key
        }

        fn valid_until(&self) -> i64 {
            self.until
        }
    }

    /// The function tabulated: its answer for `key` at `now` is valid until the end of
    /// the window `now` falls in, a window as long as the key.
    fn f(key: u8, now: i64) -> Answer {
        let window = i64::from(key) + 1;
        Answer {
            key,
            until: (now / window + 1) * window,
        }
    }

    proptest! {
        /// `memo f = f`: reading through the table equals computing, at every instant,
        /// and a hit is exactly a key asked again inside its answer's window.
        #[test]
        fn reading_through_the_table_is_computing(asks in prop::collection::vec((0u8..4, 0i64..40), 1..40)) {
            let mut asks = asks;
            asks.sort_by_key(|(_, now)| *now);
            let table = Table::<Answer>::default();
            let mut held: BTreeMap<u8, Answer> = BTreeMap::new();
            for (key, now) in asks {
                let computed = Cell::new(false);
                let read = table
                    .get_or_try(Some(&key), now, || {
                        computed.set(true);
                        Ok::<_, ()>(f(key, now))
                    })
                    .expect("f answers");
                prop_assert_eq!(read.clone(), f(key, now));
                let hit = held.get(&key).is_some_and(|answer| now < answer.until);
                prop_assert_eq!(computed.get(), !hit);
                held.insert(key, read);
            }
        }
    }

    #[test]
    fn an_answer_is_kept_under_its_own_key() {
        let table = Table::<Answer>::default();
        let other = table
            .get_or_try(Some(&1), 0, || Ok::<_, ()>(f(2, 0)))
            .expect("answers");
        assert_eq!(other.key, 2);
        let again = table
            .get_or_try(Some(&2), 0, || Err("not computed again"))
            .expect("held under its own key");
        assert_eq!(again, other);
        assert_eq!(
            table.get_or_try(Some(&1), 0, || Err("still nothing under 1")),
            Err("still nothing under 1")
        );
    }

    #[test]
    fn a_refusal_is_not_remembered() {
        let table = Table::<Answer>::default();
        assert_eq!(
            table.get_or_try(Some(&1), 0, || Err("refused")),
            Err("refused")
        );
        assert_eq!(
            table.get_or_try(Some(&1), 0, || Ok::<_, &str>(f(1, 0))),
            Ok(f(1, 0))
        );
    }

    /// One fetch of Intel's collateral per platform per window, whatever the keysets.
    #[test]
    fn collateral_is_fetched_once_per_platform_until_its_next_update() {
        use super::super::collateral::fixture;
        let collateral = Collateral::verify(&fixture::fetched(), fixture::NOW).expect("verifies");
        let identities = Identities::default();
        let fetches = Cell::new(0);
        let fetch = || {
            fetches.set(fetches.get() + 1);
            Ok::<_, ()>(collateral.clone())
        };
        for now in [fixture::NOW, fixture::NOW + 1, collateral.valid_until - 1] {
            identities
                .collateral(fixture::PLATFORM, now, fetch)
                .expect("held or fetched");
        }
        assert_eq!(fetches.get(), 1);
        identities
            .collateral(fixture::PLATFORM, collateral.valid_until, fetch)
            .expect("fetched again");
        assert_eq!(fetches.get(), 2);
    }

    /// One report per keyset: the gateway's current identity is established once, the
    /// keyset its replies name is the one held, and a rotation is established anew.
    #[test]
    fn a_gateway_is_established_once_per_keyset() {
        const BASE: &str = "https://inference.example/v1";
        let first = identity(&keyset(&[], 2_000_000_000), None);
        let rotated = identity(&keyset(&[], 2_000_000_001), None);
        let identities = Identities::default();
        let fetches = Cell::new(0);
        let establish = |answer: &Identity| {
            fetches.set(fetches.get() + 1);
            Ok::<_, ()>(answer.clone())
        };

        assert_eq!(
            identities.current(BASE, 0, || establish(&first)),
            Ok(first.clone())
        );
        assert_eq!(
            identities.current(BASE, 1, || establish(&first)),
            Ok(first.clone())
        );
        assert_eq!(
            identities.of(BASE, &first.workload_keyset_digest, 2, || establish(&first)),
            Ok(first.clone())
        );
        assert_eq!(fetches.get(), 1);

        assert_eq!(
            identities.of(BASE, &rotated.workload_keyset_digest, 3, || establish(
                &rotated
            )),
            Ok(rotated.clone())
        );
        assert_eq!(
            identities.current(BASE, 4, || establish(&first)),
            Ok(rotated.clone())
        );
        assert_eq!(fetches.get(), 2);

        assert_eq!(
            identities.current(BASE, first.valid_until.max(rotated.valid_until), || {
                establish(&first)
            }),
            Ok(first),
            "past its validity an identity is established again"
        );
        assert_eq!(fetches.get(), 3);
    }
}
