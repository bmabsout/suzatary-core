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

    /// A run answering 3 of 5 cases yields 3; the other 2 are counted, never pooled with only
    /// the incumbent's answer.
    #[test]
    fn a_run_answering_three_of_five_yields_a_three_case_intersection() {
        let ids: Vec<CaseId> = (1..=5).map(|n| case(&format!("triage:{n}"))).collect();
        let mut entries = BTreeMap::new();
        for id in &ids[..3] {
            entries.insert(id.clone(), answered(id));
        }
        // ids[3] and ids[4] have no entry at all — missing from the run.
        let runs = vec![entries];
        let (kept, left_out) = answered_by_all(&ids, &runs);
        assert_eq!(kept, ids[..3].to_vec());
        assert_eq!(left_out, 2);
    }

    /// A `Failed` entry excludes its case exactly like a missing one.
    #[test]
    fn a_failed_entry_excludes_its_case() {
        let a = case("triage:a");
        let b = case("triage:b");
        let mut entries = BTreeMap::new();
        entries.insert(a.clone(), answered(&a));
        entries.insert(b.clone(), failed(&b));
        let runs = vec![entries];
        let (kept, left_out) = answered_by_all([&a, &b], &runs);
        assert_eq!(kept, vec![a]);
        assert_eq!(left_out, 1);
    }

    /// One run missing a case leaves it out, even when every other run answered it.
    #[test]
    fn a_case_missing_from_any_one_run_is_left_out() {
        let a = case("triage:a");
        let mut run_one = BTreeMap::new();
        run_one.insert(a.clone(), answered(&a));
        let run_two = BTreeMap::new(); // never answered `a`
        let runs = vec![run_one, run_two];
        let (kept, left_out) = answered_by_all([&a], &runs);
        assert!(kept.is_empty());
        assert_eq!(left_out, 1);
    }

    /// With no runs every case stays, vacuously: the incumbent alone still answers each.
    #[test]
    fn no_runs_leaves_every_selected_case_in() {
        let a = case("triage:a");
        let (kept, left_out) = answered_by_all([&a], &[]);
        assert_eq!(kept, vec![a]);
        assert_eq!(left_out, 0);
    }
}
