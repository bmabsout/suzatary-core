//! The sheet's pool intersection: a case enters only when every pooled run answered it.
//!
//! An unequal pool breaks blindness: a case only the incumbent answered unmasks it outright,
//! and a thin case unmasks it by style. The incumbent always answers, so it is not a run here.

use std::collections::BTreeMap;

use crate::cases::CaseId;
use crate::run::{Outcome, RunEntry};

/// Filter a selected sample to the cases every run answered (a `Failed` or missing entry
/// excludes one); returns the survivors in `selected`'s order and how many were left out.
#[must_use]
pub fn answered_by_all<'a>(
    selected: impl IntoIterator<Item = &'a CaseId>,
    runs: &[BTreeMap<CaseId, RunEntry>],
) -> (Vec<CaseId>, usize) {
    let mut kept = Vec::new();
    let mut left_out = 0usize;
    for case in selected {
        let answered = runs.iter().all(|entries| {
            matches!(
                entries.get(case),
                Some(RunEntry {
                    outcome: Outcome::Answered(_),
                    ..
                })
            )
        });
        if answered {
            kept.push(case.clone());
        } else {
            left_out += 1;
        }
    }
    (kept, left_out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Answer;

    fn case(id: &str) -> CaseId {
        CaseId::new(id).expect("valid")
    }

    fn answered(case: &CaseId) -> RunEntry {
        RunEntry {
            case: case.clone(),
            recorded_at: "2026-09-21T00:00:00Z".to_owned(),
            outcome: Outcome::Answered(Answer {
                model: "candidate/one".to_owned(),
                rounds: Vec::new(),
                objects: Vec::new(),
                usage: None,
            }),
        }
    }

    fn failed(case: &CaseId) -> RunEntry {
        RunEntry {
            case: case.clone(),
            recorded_at: "2026-09-21T00:00:00Z".to_owned(),
            outcome: Outcome::Failed {
                error: "no route to host".to_owned(),
            },
        }
    }

    /// A case enters the pool only when every run answered it: a case missing from any one run,
    /// or `Failed` there, is left out and counted, never pooled with only the incumbent's answer;
    /// with no runs every case stays, vacuously, since the incumbent alone still answers each.
    #[test]
    fn a_case_is_pooled_only_when_every_run_answered_it() {
        let ids: Vec<CaseId> = (1..=5).map(|n| case(&format!("triage:{n}"))).collect();
        let run = |answers: &[usize], failures: &[usize]| -> BTreeMap<CaseId, RunEntry> {
            answers
                .iter()
                .map(|&n| (ids[n].clone(), answered(&ids[n])))
                .chain(failures.iter().map(|&n| (ids[n].clone(), failed(&ids[n]))))
                .collect()
        };
        for (runs, pooled, left_out) in [
            (vec![run(&[0, 1, 2], &[])], vec![0, 1, 2], 2),
            (vec![run(&[0], &[1, 2, 3, 4])], vec![0], 4),
            (
                vec![run(&[0, 1, 2, 3, 4], &[]), run(&[1, 2, 3, 4], &[])],
                vec![1, 2, 3, 4],
                1,
            ),
            (Vec::new(), vec![0, 1, 2, 3, 4], 0),
        ] {
            let (kept, out) = answered_by_all(&ids, &runs);
            let pooled: Vec<CaseId> = pooled.iter().map(|&n| ids[n].clone()).collect();
            assert_eq!(kept, pooled);
            assert_eq!(out, left_out);
        }
    }
}
