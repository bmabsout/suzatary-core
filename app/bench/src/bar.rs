//! A bar is the least share of a check's applications that must pass, and a verdict is what a
//! tally earns against it. The bars are the plan's §4, written before any number existed; a
//! bar moved to fit a result would make the result evidence about the bar.

use std::fmt;

use serde::Serialize;

use crate::tally::Tally;

/// The least share that must pass, an exact fraction so 95% means 95%, never a float near it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Bar {
    passed: u32,
    of: u32,
}

impl Bar {
    /// Every application passes: the bar for a gate, where one failure fails the model.
    pub const ALL: Self = Self { passed: 1, of: 1 };

    /// At least `percent` in a hundred.
    ///
    /// # Panics
    /// When `percent` exceeds 100; the bars are constants, so that is a compile error.
    #[must_use]
    pub const fn percent(percent: u32) -> Self {
        assert!(percent <= 100, "a bar is a share");
        Self {
            passed: percent,
            of: 100,
        }
    }

    /// At least another tally's rate (the incumbent's, for a judged bar); `None` where that
    /// tally applied nowhere, since an empty rate bars nothing.
    #[must_use]
    pub fn at_least(tally: Tally) -> Option<Self> {
        (tally.applied() > 0).then(|| Self {
            passed: tally.passed(),
            of: tally.applied(),
        })
    }

    /// What `tally` earns: met when `passed / applied >= self`, compared by cross-multiplying.
    #[must_use]
    pub fn judge(self, tally: Tally) -> Verdict {
        if tally.applied() == 0 {
            Verdict::Unmeasured
        } else if u64::from(tally.passed()) * u64::from(self.of)
            >= u64::from(self.passed) * u64::from(tally.applied())
        {
            Verdict::Met
        } else {
            Verdict::Missed
        }
    }
}

impl fmt::Display for Bar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.passed == self.of {
            f.write_str("100%")
        } else {
            let percent = f64::from(self.passed) * 100.0 / f64::from(self.of);
            write!(f, ">= {percent:.0}%")
        }
    }
}

/// What a tally earns against a bar. Ordered `Missed < Unmeasured < Met`, and verdicts combine
/// by their meet: one miss misses, and no evidence is never a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Below the bar.
    Missed,
    /// Nothing to judge: the check applied to no case.
    Unmeasured,
    /// At or above the bar.
    Met,
}

impl Verdict {
    /// The meet of every verdict, `Met` (the identity) over none.
    pub fn all(verdicts: impl IntoIterator<Item = Self>) -> Self {
        verdicts.into_iter().min().unwrap_or(Self::Met)
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Verdict::Missed => "MISSED",
            Verdict::Unmeasured => "UNMEASURED",
            Verdict::Met => "MET",
        })
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn tally(passed: u32, failed: u32) -> Tally {
        std::iter::repeat_n(Some(true), passed as usize)
            .chain(std::iter::repeat_n(Some(false), failed as usize))
            .map(Tally::of)
            .sum()
    }

    fn verdict() -> impl Strategy<Value = Verdict> {
        prop_oneof![
            Just(Verdict::Missed),
            Just(Verdict::Unmeasured),
            Just(Verdict::Met)
        ]
    }

    proptest! {
        /// The meet is a bounded semilattice: idempotent, commutative, associative, `Met` its
        /// identity and `Missed` absorbing.
        #[test]
        fn verdicts_meet(a in verdict(), b in verdict(), c in verdict()) {
            prop_assert_eq!(Verdict::all([a, a]), a);
            prop_assert_eq!(Verdict::all([a, b]), Verdict::all([b, a]));
            prop_assert_eq!(Verdict::all([Verdict::all([a, b]), c]), Verdict::all([a, Verdict::all([b, c])]));
            prop_assert_eq!(Verdict::all([a, Verdict::Met]), a);
            prop_assert_eq!(Verdict::all([a, Verdict::Missed]), Verdict::Missed);
        }

        /// A bar judges the rate exactly: met iff passed/applied >= the bar's share.
        #[test]
        fn a_bar_judges_the_exact_rate(percent in 0u32..=100, passed in 0u32..200, failed in 0u32..200) {
            let earned = Bar::percent(percent).judge(tally(passed, failed));
            let expected = if passed + failed == 0 {
                Verdict::Unmeasured
            } else if passed * 100 >= percent * (passed + failed) {
                Verdict::Met
            } else {
                Verdict::Missed
            };
            prop_assert_eq!(earned, expected);
        }
    }

    #[test]
    fn ninety_five_percent_is_not_ninety_four_point_nine() {
        assert_eq!(Bar::percent(95).judge(tally(19, 1)), Verdict::Met);
        assert_eq!(Bar::percent(95).judge(tally(949, 51)), Verdict::Missed);
        assert_eq!(Bar::ALL.judge(tally(82, 1)), Verdict::Missed);
        assert_eq!(Bar::ALL.judge(tally(0, 0)), Verdict::Unmeasured);
    }

    #[test]
    fn the_incumbents_rate_bars_nothing_when_it_was_never_judged() {
        assert_eq!(Bar::at_least(tally(0, 0)), None);
        let incumbent = Bar::at_least(tally(3, 1)).expect("a rate");
        assert_eq!(incumbent.judge(tally(3, 1)), Verdict::Met);
        assert_eq!(incumbent.judge(tally(2, 1)), Verdict::Missed);
    }
}
