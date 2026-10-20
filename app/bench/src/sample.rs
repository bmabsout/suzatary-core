//! The frozen sample a run replays and a sheet pools: a seeded draw from each stratum of the
//! triage cases, written once beside the case file, so every model answers the same cases.
//!
//! The draw is a function of the SET of cases: each case ranks by a digest of the seed and its
//! id, never by its place in the file, so a duplicate or a reordering draws the same sample.
//! Every adversarial case is in it, since a gate that one failure fails is never sampled.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cases::{Case, CaseId};
use crate::stratum::{Stratum, measure_of};

/// How many triage cases each stratum contributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shape {
    /// Where the incumbent wrote a todo.
    pub acted: usize,
    /// Where the owner decided otherwise.
    pub overturned: usize,
    /// Where the incumbent correctly wrote nothing.
    pub quiet: usize,
}

impl Shape {
    /// The standard: 83 cases, the size of the first real run, with the rare overturned stratum
    /// given far more than its share of the corpus.
    pub const STANDARD: Self = Self {
        acted: 30,
        overturned: 23,
        quiet: 30,
    };

    /// One stratum's quota.
    #[must_use]
    pub fn quota(self, stratum: Stratum) -> usize {
        match stratum {
            Stratum::Acted => self.acted,
            Stratum::Overturned => self.overturned,
            Stratum::Quiet => self.quiet,
        }
    }
}

/// The sample, with what it was drawn from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// The seed every rank digests.
    pub seed: u64,
    /// The quotas asked for.
    pub shape: Shape,
    /// How many triage cases each stratum holds in the whole case file.
    pub population: BTreeMap<Stratum, usize>,
    /// Each stratum's draw, best rank first; shorter than its quota only when the stratum is.
    pub strata: BTreeMap<Stratum, Vec<CaseId>>,
    /// Every adversarial case.
    pub adversarial: Vec<CaseId>,
}

/// A case's rank for one seed.
fn rank(seed: u64, id: &CaseId) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(id.as_str().as_bytes());
    hasher.finalize().into()
}

impl Sample {
    /// Draw the sample.
    #[must_use]
    pub fn draw(cases: &[Case], seed: u64, shape: Shape) -> Self {
        let mut triage: BTreeMap<Stratum, BTreeSet<([u8; 32], &CaseId)>> = BTreeMap::new();
        let mut adversarial = BTreeSet::new();
        for case in cases {
            match (case, measure_of(case).stratum()) {
                (Case::Triage(_), Some(stratum)) => {
                    triage
                        .entry(stratum)
                        .or_default()
                        .insert((rank(seed, case.id()), case.id()));
                }
                (Case::Adversarial(_), _) => {
                    adversarial.insert(case.id().clone());
                }
                _ => {}
            }
        }
        let population = Stratum::ALL
            .into_iter()
            .map(|stratum| (stratum, triage.get(&stratum).map_or(0, BTreeSet::len)))
            .collect();
        let strata = Stratum::ALL
            .into_iter()
            .map(|stratum| {
                let drawn = triage
                    .get(&stratum)
                    .into_iter()
                    .flatten()
                    .take(shape.quota(stratum))
                    .map(|(_, id)| (*id).clone())
                    .collect();
                (stratum, drawn)
            })
            .collect();
        Self {
            seed,
            shape,
            population,
            strata,
            adversarial: adversarial.into_iter().collect(),
        }
    }

    /// Every case the sample names.
    #[must_use]
    pub fn ids(&self) -> BTreeSet<&CaseId> {
        self.strata
            .values()
            .flatten()
            .chain(&self.adversarial)
            .collect()
    }

    /// The first case the sample names that `cases` does not hold: a sample is read only
    /// against the case file it was drawn from.
    #[must_use]
    pub fn stranger<'a>(&'a self, cases: &[Case]) -> Option<&'a CaseId> {
        let held: BTreeSet<&CaseId> = cases.iter().map(Case::id).collect();
        self.ids().into_iter().find(|id| !held.contains(id))
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::cases::{
        AdversarialCase, ChainSnapshot, SnapshotBasis, TriageAnswer, TriageCase, Truths,
    };

    fn snapshot() -> ChainSnapshot {
        ChainSnapshot {
            chain: "life".to_owned(),
            tip: None,
            basis: SnapshotBasis::Extraction,
        }
    }

    fn wrote() -> TriageAnswer {
        TriageAnswer::Wrote {
            chain: "life".to_owned(),
            todo: "t".to_owned(),
            object: "d".repeat(64),
        }
    }

    /// A synthetic case: `kind` 0 quiet, 1 acted, 2 overturned, 3 adversarial.
    fn synthetic(n: u16, kind: u8) -> Case {
        let truths = match kind {
            0 => Truths {
                incumbent: TriageAnswer::Declined,
                settled: None,
            },
            1 => Truths {
                incumbent: wrote(),
                settled: None,
            },
            _ => Truths {
                incumbent: wrote(),
                settled: Some(TriageAnswer::Declined),
            },
        };
        if kind == 3 {
            return Case::Adversarial(AdversarialCase {
                id: CaseId::new(format!("adversarial:{n}")).expect("valid"),
                message_hash: "abc".to_owned(),
                signal: "\"wire transfer\"".to_owned(),
                chain_as_stood: snapshot(),
                truths,
            });
        }
        Case::Triage(TriageCase {
            id: CaseId::new(format!("triage:{n}")).expect("valid"),
            message_hash: "abc".to_owned(),
            chain_as_stood: snapshot(),
            truths,
        })
    }

    fn corpus() -> impl Strategy<Value = Vec<Case>> {
        prop::collection::btree_map(any::<u16>(), 0u8..4, 0..120)
            .prop_map(|kinds| kinds.into_iter().map(|(n, k)| synthetic(n, k)).collect())
    }

    proptest! {
        /// Reordering and duplicating the cases draws the same sample.
        #[test]
        fn the_draw_is_a_function_of_the_set(cases in corpus(), seed in any::<u64>(), cut in any::<prop::sample::Index>()) {
            let drawn = Sample::draw(&cases, seed, Shape::STANDARD);
            let mut shuffled = cases.clone();
            shuffled.reverse();
            let half = cut.index(cases.len().max(1));
            shuffled.extend(cases.iter().take(half).cloned());
            prop_assert_eq!(Sample::draw(&shuffled, seed, Shape::STANDARD), drawn);
        }

        /// Each stratum gives its quota or all it has, each drawn case is of its stratum, and
        /// every adversarial case is in.
        #[test]
        fn each_stratum_fills_its_quota_from_itself(cases in corpus(), seed in any::<u64>()) {
            let sample = Sample::draw(&cases, seed, Shape::STANDARD);
            for stratum in Stratum::ALL {
                let drawn = &sample.strata[&stratum];
                prop_assert_eq!(drawn.len(), sample.population[&stratum].min(Shape::STANDARD.quota(stratum)));
                for id in drawn {
                    let case = cases.iter().find(|case| case.id() == id).expect("drawn from the file");
                    prop_assert_eq!(measure_of(case).stratum(), Some(stratum));
                }
            }
            let adversarial = cases.iter().filter(|case| matches!(case, Case::Adversarial(_))).count();
            prop_assert_eq!(sample.adversarial.len(), adversarial);
            prop_assert_eq!(sample.stranger(&cases), None);
        }
    }

    #[test]
    fn a_sample_read_against_another_file_names_its_stranger() {
        let cases: Vec<Case> = (0..5).map(|n| synthetic(n, 1)).collect();
        let sample = Sample::draw(&cases, 0, Shape::STANDARD);
        assert_eq!(sample.stranger(&cases[1..]), Some(cases[0].id()));
    }

    /// The file a run and a sheet both read survives its own serialisation.
    #[test]
    fn a_sample_round_trips_through_json() {
        let cases: Vec<Case> = (0..40)
            .map(|n| synthetic(n, u8::try_from(n % 4).expect("below four")))
            .collect();
        let sample = Sample::draw(&cases, 7, Shape::STANDARD);
        let text = serde_json::to_string(&sample).expect("serialises");
        assert_eq!(
            serde_json::from_str::<Sample>(&text).expect("parses"),
            sample
        );
    }
}
