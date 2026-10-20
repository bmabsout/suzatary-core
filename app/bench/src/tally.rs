//! Passes out of the cases a check applied to: the one monoid every rate in a report folds.
//!
//! A tally is a pair of counts, not a fraction, so tallies add (a stratum is the sum of its
//! rows, a corpus the sum of its strata) and a check that applied nowhere stays visibly empty
//! instead of reading as 0% or 100%.

use std::collections::BTreeMap;
use std::fmt;
use std::iter::Sum;
use std::ops::Add;

use serde::Serialize;

/// Passes out of applications; `passed <= applied`, kept by the constructors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
pub struct Tally {
    passed: u32,
    applied: u32,
}

impl Tally {
    /// One case's result: `None` where the check did not apply.
    #[must_use]
    pub fn of(result: Option<bool>) -> Self {
        result.map_or_else(Self::default, |passed| Self {
            passed: u32::from(passed),
            applied: 1,
        })
    }

    /// How many applications passed.
    #[must_use]
    pub fn passed(self) -> u32 {
        self.passed
    }

    /// How many cases the check applied to.
    #[must_use]
    pub fn applied(self) -> u32 {
        self.applied
    }

    /// The pass rate, `None` where nothing applied.
    #[must_use]
    pub fn rate(self) -> Option<f64> {
        (self.applied > 0).then(|| f64::from(self.passed) / f64::from(self.applied))
    }
}

/// `passed/applied (rate)`, or `n/a` where nothing applied.
impl fmt::Display for Tally {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.rate() {
            Some(rate) => write!(f, "{}/{} ({:.0}%)", self.passed, self.applied, rate * 100.0),
            None => f.write_str("n/a"),
        }
    }
}

impl Add for Tally {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            passed: self.passed + other.passed,
            applied: self.applied + other.applied,
        }
    }
}

impl Sum for Tally {
    fn sum<I: Iterator<Item = Self>>(tallies: I) -> Self {
        tallies.fold(Self::default(), Add::add)
    }
}

/// One tally per key, added pointwise: the product of [`Tally`]'s monoid over a key set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Tallies<K: Ord>(BTreeMap<K, Tally>);

impl<K: Ord> Default for Tallies<K> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<K: Ord + Copy> Tallies<K> {
    /// One case's results, a key per check.
    pub fn of(results: impl IntoIterator<Item = (K, Option<bool>)>) -> Self {
        Self(
            results
                .into_iter()
                .map(|(key, result)| (key, Tally::of(result)))
                .collect(),
        )
    }

    /// One key's tally; empty where the key was never seen.
    #[must_use]
    pub fn get(&self, key: K) -> Tally {
        self.0.get(&key).copied().unwrap_or_default()
    }

    /// Every key and its tally, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (K, Tally)> + '_ {
        self.0.iter().map(|(key, tally)| (*key, *tally))
    }
}

impl<K: Ord + Copy> Add for Tallies<K> {
    type Output = Self;
    fn add(mut self, other: Self) -> Self {
        for (key, tally) in other.0 {
            let mine = self.0.entry(key).or_default();
            *mine = *mine + tally;
        }
        self
    }
}

impl<K: Ord + Copy> Sum for Tallies<K> {
    fn sum<I: Iterator<Item = Self>>(tallies: I) -> Self {
        tallies.fold(Self::default(), Add::add)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn tally() -> impl Strategy<Value = Tally> {
        prop::collection::vec(prop::option::of(any::<bool>()), 0..20)
            .prop_map(|results| results.into_iter().map(Tally::of).sum())
    }

    fn tallies() -> impl Strategy<Value = Tallies<u8>> {
        prop::collection::vec((0u8..4, prop::option::of(any::<bool>())), 0..20)
            .prop_map(|results| results.into_iter().map(|r| Tallies::of([r])).sum())
    }

    proptest! {
        #[test]
        fn a_tally_is_a_commutative_monoid(a in tally(), b in tally(), c in tally()) {
            prop_assert_eq!(a + Tally::default(), a);
            prop_assert_eq!(a + b, b + a);
            prop_assert_eq!((a + b) + c, a + (b + c));
            prop_assert!(a.passed() <= a.applied());
        }

        #[test]
        fn tallies_are_a_commutative_monoid(a in tallies(), b in tallies(), c in tallies()) {
            prop_assert_eq!(a.clone() + Tallies::default(), a.clone());
            prop_assert_eq!(a.clone() + b.clone(), b.clone() + a.clone());
            prop_assert_eq!((a.clone() + b.clone()) + c.clone(), a + (b + c));
        }

        /// A summary is a function of the set of results: any order sums alike.
        #[test]
        fn a_sum_ignores_order(results in prop::collection::vec(prop::option::of(any::<bool>()), 0..40)) {
            let forward: Tally = results.iter().copied().map(Tally::of).sum();
            let backward: Tally = results.iter().rev().copied().map(Tally::of).sum();
            prop_assert_eq!(forward, backward);
        }
    }

    #[test]
    fn a_check_that_never_applied_has_no_rate() {
        assert_eq!(Tally::of(None).rate(), None);
        assert_eq!(
            (Tally::of(Some(true)) + Tally::of(Some(false))).rate(),
            Some(0.5)
        );
    }
}
