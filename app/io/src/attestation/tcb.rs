//! Intel's verdict on a TCB, and the join that combines a platform's, its TDX module's
//! and its quoting enclave's into one.
//!
//! A status is what the platform's owner must still do: nothing, harden software,
//! change configuration, update, or nothing ever again (revoked). Those are a set of
//! obligations, so a status is a set and the join is union. Two quirks of Intel's
//! names are the whole of the structure: an update subsumes software hardening (Intel
//! has no "out of date and hardening needed"), and revocation is the top.

use std::collections::BTreeSet;

use serde::Deserialize;

/// One of Intel's seven TCB statuses, named as Intel's collateral spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum TcbStatus {
    /// Nothing to do.
    UpToDate,
    /// Software mitigations are needed.
    SWHardeningNeeded,
    /// The platform's configuration must change.
    ConfigurationNeeded,
    /// Both of the above.
    ConfigurationAndSWHardeningNeeded,
    /// A TCB recovery has happened and this platform has not taken it.
    OutOfDate,
    /// Out of date, and its configuration must change too.
    OutOfDateConfigurationNeeded,
    /// The platform's keys are compromised.
    Revoked,
}

/// A status's obligations: `None` is revoked, else (update, configure, harden).
type Obligations = Option<(bool, bool, bool)>;

impl TcbStatus {
    /// The status no obligation is owed under, the join's identity.
    pub const BOTTOM: Self = Self::UpToDate;

    fn obligations(self) -> Obligations {
        match self {
            Self::UpToDate => Some((false, false, false)),
            Self::SWHardeningNeeded => Some((false, false, true)),
            Self::ConfigurationNeeded => Some((false, true, false)),
            Self::ConfigurationAndSWHardeningNeeded => Some((false, true, true)),
            Self::OutOfDate => Some((true, false, true)),
            Self::OutOfDateConfigurationNeeded => Some((true, true, true)),
            Self::Revoked => None,
        }
    }

    fn of(obligations: Obligations) -> Self {
        match obligations {
            None => Self::Revoked,
            Some((true, configure, _)) => {
                if configure {
                    Self::OutOfDateConfigurationNeeded
                } else {
                    Self::OutOfDate
                }
            }
            Some((false, false, false)) => Self::UpToDate,
            Some((false, false, true)) => Self::SWHardeningNeeded,
            Some((false, true, false)) => Self::ConfigurationNeeded,
            Some((false, true, true)) => Self::ConfigurationAndSWHardeningNeeded,
        }
    }

    /// The least status owing everything either does.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        Self::of(self.obligations().zip(other.obligations()).map(
            |((update, configure, harden), (update2, configure2, harden2))| {
                (
                    update || update2,
                    configure || configure2,
                    harden || harden2,
                )
            },
        ))
    }

    /// Whether `self` owes nothing `bound` does not.
    #[must_use]
    pub fn at_most(self, bound: Self) -> bool {
        self.join(bound) == bound
    }
}

/// A status and the advisories that explain it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tcb {
    /// The status.
    pub status: TcbStatus,
    /// Intel's advisory ids, `INTEL-SA-…`.
    pub advisories: BTreeSet<String>,
}

impl Tcb {
    /// Nothing owed and nothing advised, the join's identity.
    #[must_use]
    pub fn bottom() -> Self {
        Self {
            status: TcbStatus::BOTTOM,
            advisories: BTreeSet::new(),
        }
    }

    /// Both statuses' obligations and both sets of advisories.
    #[must_use]
    pub fn join(mut self, other: Self) -> Self {
        self.status = self.status.join(other.status);
        self.advisories.extend(other.advisories);
        self
    }
}

impl FromIterator<Tcb> for Tcb {
    fn from_iter<I: IntoIterator<Item = Tcb>>(tcbs: I) -> Self {
        tcbs.into_iter().fold(Self::bottom(), Self::join)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::TcbStatus::{
        ConfigurationAndSWHardeningNeeded, ConfigurationNeeded, OutOfDate,
        OutOfDateConfigurationNeeded, Revoked, SWHardeningNeeded, UpToDate,
    };
    use super::*;

    const ALL: [TcbStatus; 7] = [
        UpToDate,
        SWHardeningNeeded,
        ConfigurationNeeded,
        ConfigurationAndSWHardeningNeeded,
        OutOfDate,
        OutOfDateConfigurationNeeded,
        Revoked,
    ];

    fn status() -> impl Strategy<Value = TcbStatus> {
        prop::sample::select(&ALL[..])
    }

    fn tcb() -> impl Strategy<Value = Tcb> {
        (
            status(),
            prop::collection::btree_set("INTEL-SA-0[0-9]{4}", 0..3),
        )
            .prop_map(|(status, advisories)| Tcb { status, advisories })
    }

    proptest! {
        #[test]
        fn the_status_join_is_a_semilattice_with_up_to_date_as_identity(a in status(), b in status(), c in status()) {
            prop_assert_eq!(a.join(b).join(c), a.join(b.join(c)));
            prop_assert_eq!(a.join(b), b.join(a));
            prop_assert_eq!(a.join(a), a);
            prop_assert_eq!(a.join(TcbStatus::BOTTOM), a);
            prop_assert_eq!(a.join(Revoked), Revoked);
        }

        #[test]
        fn at_most_is_the_joins_order(a in status(), b in status()) {
            prop_assert!(a.at_most(a.join(b)));
            prop_assert_eq!(a.at_most(b) && b.at_most(a), a == b);
        }

        #[test]
        fn a_tcb_join_is_a_semilattice_and_collecting_is_folding(a in tcb(), b in tcb(), c in tcb()) {
            prop_assert_eq!(a.clone().join(b.clone()).join(c.clone()), a.clone().join(b.clone().join(c.clone())));
            prop_assert_eq!(a.clone().join(b.clone()), b.clone().join(a.clone()));
            prop_assert_eq!(a.clone().join(a.clone()), a.clone());
            prop_assert_eq!(a.clone().join(Tcb::bottom()), a.clone());
            let collected: Tcb = [a.clone(), b.clone(), c.clone()].into_iter().collect();
            prop_assert_eq!(collected, a.join(b).join(c));
        }
    }

    /// Intel's `convergeTcbStatusWithQeTcbStatus`, for the statuses a QE identity or a
    /// TDX module identity can carry: the join agrees with it on every platform status.
    #[test]
    fn the_join_is_intels_convergence() {
        let intel = |platform: TcbStatus, component: TcbStatus| match component {
            Revoked => Revoked,
            OutOfDate => match platform {
                UpToDate | SWHardeningNeeded => OutOfDate,
                ConfigurationNeeded | ConfigurationAndSWHardeningNeeded => {
                    OutOfDateConfigurationNeeded
                }
                other => other,
            },
            _ => platform,
        };
        for platform in ALL {
            for component in [UpToDate, OutOfDate, Revoked] {
                assert_eq!(
                    platform.join(component),
                    intel(platform, component),
                    "{platform:?} with {component:?}"
                );
            }
        }
    }
}
