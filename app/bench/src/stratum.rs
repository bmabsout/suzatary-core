//! What a case can measure: a stratum of real truth, or imitation alone.
//!
//! Read off a case's truths, never off anything a candidate is shown. Two answers agree when
//! they DECIDE alike: a todo the owner later kept names a later object and a tapped proposal
//! a different `why`, and neither is the owner overturning anything.

use serde::{Deserialize, Serialize};

use crate::cases::{Case, JudgementAnswer, TriageAnswer, Truths};

/// The three strata, scored apart and never averaged into one number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stratum {
    /// The incumbent wrote a todo or a proposal, and the owner did not decide otherwise.
    Acted,
    /// The owner's settled decision differs from the incumbent's: rare, and worth the most.
    Overturned,
    /// The incumbent correctly did nothing: the trivial majority.
    Quiet,
}

impl Stratum {
    /// Every stratum, in the order a report prints them.
    pub const ALL: [Self; 3] = [Self::Acted, Self::Overturned, Self::Quiet];
}

/// What a case can measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "measures", content = "stratum", rename_all = "lowercase")]
pub enum Measure {
    /// A case with a truth to score against, in its stratum.
    Truth(Stratum),
    /// A review case: nothing records the owner's review of a review, so agreeing with the
    /// incumbent is all it can show, and it is never held to a bar.
    Imitation,
}

impl Measure {
    /// The stratum, when the case has one.
    #[must_use]
    pub fn stratum(self) -> Option<Stratum> {
        match self {
            Measure::Truth(stratum) => Some(stratum),
            Measure::Imitation => None,
        }
    }
}

/// An answer's decision: the quotient two answers are compared by.
trait Decides {
    /// Did it act at all?
    fn acted(&self) -> bool;
    /// Do the two decide alike?
    fn agrees(&self, other: &Self) -> bool;
}

impl Decides for TriageAnswer {
    fn acted(&self) -> bool {
        matches!(self, TriageAnswer::Wrote { .. })
    }

    /// Writing and declining disagree, and so do writes to two chains; the todo's id and
    /// object do not decide anything.
    fn agrees(&self, other: &Self) -> bool {
        match (self, other) {
            (TriageAnswer::Declined, TriageAnswer::Declined) => true,
            (TriageAnswer::Wrote { chain: a, .. }, TriageAnswer::Wrote { chain: b, .. }) => a == b,
            _ => false,
        }
    }
}

impl Decides for JudgementAnswer {
    fn acted(&self) -> bool {
        matches!(self, JudgementAnswer::Propose { .. })
    }

    fn agrees(&self, other: &Self) -> bool {
        self.acted() == other.acted()
    }
}

/// A case's stratum from its truths: overturned outranks acted, being the rarer fact.
fn stratum<A: Decides>(truths: &Truths<A>) -> Stratum {
    let overturned = truths
        .settled
        .as_ref()
        .is_some_and(|settled| !settled.agrees(&truths.incumbent));
    if overturned {
        Stratum::Overturned
    } else if truths.incumbent.acted() {
        Stratum::Acted
    } else {
        Stratum::Quiet
    }
}

/// What one case can measure.
#[must_use]
pub fn measure_of(case: &Case) -> Measure {
    match case {
        Case::Triage(case) => Measure::Truth(stratum(&case.truths)),
        Case::Adversarial(case) => Measure::Truth(stratum(&case.truths)),
        Case::Judgement(case) => Measure::Truth(stratum(&case.truths)),
        Case::Review(_) => Measure::Imitation,
    }
}

#[cfg(test)]
mod tests {
    use suzatary_core::secretary::store::{Decision, Draft, Proposal, Settle};

    use super::*;
    use crate::cases::{
        CaseId, ChainSnapshot, JudgementCase, ReviewAnswer, ReviewCase, SnapshotBasis, TriageCase,
    };

    fn triage(incumbent: TriageAnswer, settled: Option<TriageAnswer>) -> Case {
        Case::Triage(TriageCase {
            id: CaseId::new("triage:synthetic").expect("valid"),
            message_hash: "abc123".to_owned(),
            chain_as_stood: ChainSnapshot {
                chain: "life".to_owned(),
                tip: None,
                basis: SnapshotBasis::Extraction,
            },
            truths: Truths { incumbent, settled },
        })
    }

    fn wrote(chain: &str, object: char) -> TriageAnswer {
        TriageAnswer::Wrote {
            chain: chain.to_owned(),
            todo: "t1".to_owned(),
            object: object.to_string().repeat(64),
        }
    }

    fn proposal(why: &str) -> JudgementAnswer {
        JudgementAnswer::Propose {
            proposal: Box::new(Proposal::Email(Draft {
                account: "a@example.test".to_owned(),
                to: vec!["b@example.test".to_owned()],
                cc: vec![],
                subject: "synthetic".to_owned(),
                body: "synthetic".to_owned(),
                attachments: Vec::new(),
            })),
            why: why.to_owned(),
        }
    }

    fn judgement(settled: Option<JudgementAnswer>) -> Case {
        Case::Judgement(JudgementCase {
            id: CaseId::new("judgement:synthetic:0").expect("valid"),
            conversation: "synthetic".to_owned(),
            proposal_index: 0,
            context: vec![],
            truths: Truths {
                incumbent: proposal("a reply was owed"),
                settled,
            },
        })
    }

    /// A decline is quiet and a write acted; the owner keeping a write (a later object of the
    /// same chain) leaves it acted, and cancelling or moving it overturns it; a tapped proposal is
    /// acted and a dismissed one overturned; and a review measures imitation alone, whatever it
    /// decided.
    #[test]
    fn each_case_measures_the_stratum_its_truths_name() {
        let settled = |how| Decision::Settled {
            how,
            objects: suzatary_core::nonempty::NonEmpty::of(vec!["o".repeat(64)])
                .expect("one object"),
        };
        let review = |decision| {
            Case::Review(ReviewCase {
                id: CaseId::new("review:synthetic:0").expect("valid"),
                conversation: "synthetic".to_owned(),
                chain: "life".to_owned(),
                batch_head: "b".repeat(64),
                truths: Truths {
                    incumbent: ReviewAnswer {
                        decision,
                        why: "routine".to_owned(),
                    },
                    settled: None,
                },
            })
        };
        for (case, measure) in [
            (
                triage(TriageAnswer::Declined, None),
                Measure::Truth(Stratum::Quiet),
            ),
            (
                triage(wrote("life", 'a'), None),
                Measure::Truth(Stratum::Acted),
            ),
            (
                triage(wrote("life", 'a'), Some(wrote("life", 'b'))),
                Measure::Truth(Stratum::Acted),
            ),
            (
                triage(wrote("life", 'a'), Some(TriageAnswer::Declined)),
                Measure::Truth(Stratum::Overturned),
            ),
            (
                triage(wrote("life", 'a'), Some(wrote("suzatary", 'b'))),
                Measure::Truth(Stratum::Overturned),
            ),
            (
                judgement(Some(proposal("The owner tapped it as proposed"))),
                Measure::Truth(Stratum::Acted),
            ),
            (
                judgement(Some(JudgementAnswer::Decline {
                    why: "handled it myself".to_owned(),
                })),
                Measure::Truth(Stratum::Overturned),
            ),
            (review(settled(Settle::Accepted)), Measure::Imitation),
            (review(settled(Settle::Rejected)), Measure::Imitation),
            (review(Decision::Held), Measure::Imitation),
        ] {
            assert_eq!(measure_of(&case), measure, "{case:?}");
        }
        assert_eq!(Measure::Imitation.stratum(), None);
    }
}
