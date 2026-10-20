//! Attach a blind scorer's per-label judgements back to the models that earned them, through
//! the [`crate::sheet::Key`] the scorer never saw.
//!
//! A label the key does not know is refused by name, never dropped, so a hand-edited scores
//! file cannot quietly lose a judgement.

use std::collections::BTreeMap;
use std::ops::Add;

use serde::{Deserialize, Serialize};

use crate::bar::{Bar, Verdict};
use crate::cases::{CaseId, NotACaseId};
use crate::sheet::Key;
use crate::stratum::Stratum;
use crate::tally::Tally;

/// Wording on a 1-5 scale; a type so a malformed scores file cannot say `9`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Wording(u8);

/// A wording score outside `1..=5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("wording must be a whole number 1-5, got {0}")]
pub struct NotAWording(pub u8);

impl Wording {
    /// Build one.
    ///
    /// # Errors
    /// [`NotAWording`] outside `1..=5`.
    pub fn new(value: u8) -> Result<Self, NotAWording> {
        if (1..=5).contains(&value) {
            Ok(Self(value))
        } else {
            Err(NotAWording(value))
        }
    }

    /// The score itself.
    #[must_use]
    pub fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Wording {
    type Error = NotAWording;
    fn try_from(value: u8) -> Result<Self, NotAWording> {
        Self::new(value)
    }
}

impl From<Wording> for u8 {
    fn from(wording: Wording) -> u8 {
        wording.0
    }
}

/// One label's judgement, scored blind against its rendered answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelScore {
    /// Is this the right todo at all.
    pub right_todo: bool,
    /// Is the wording usable in the owner's register, 1 (unusable) to 5 (as written).
    pub wording: Wording,
    /// Is the priority defensible against the anchor table.
    pub priority_defensible: bool,
    /// Would this embarrass him if tapped.
    pub would_embarrass: bool,
    /// The scorer's own note. `""` is a real, if terse, answer.
    #[serde(default)]
    pub note: String,
}

/// The scores file: `{case_id: {label: LabelScore}}`, with nothing wrapping it on disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScoresFile(BTreeMap<String, BTreeMap<String, LabelScore>>);

impl ScoresFile {
    /// Parse one from its JSON text.
    ///
    /// # Errors
    /// The text is not `{case_id: {label: LabelScore}}`, a malformed `wording` included.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

/// A join that could not complete.
#[derive(Debug, Clone, thiserror::Error)]
pub enum JoinError {
    /// A key in the scores file is not a well-formed case id.
    #[error("scores file names case {0:?}, which is not a well-formed case id")]
    BadCaseId(String, #[source] NotACaseId),
    /// The key does not know this case at all.
    #[error("case {0} is scored but the key names no candidates for it")]
    UnknownCase(CaseId),
    /// The key knows this case, but not this label.
    #[error("case {case} has no label {label:?} in the key")]
    UnknownLabel {
        /// Which case.
        case: CaseId,
        /// The label the scores file named.
        label: String,
    },
}

/// One judged row, joined back to its model.
#[derive(Debug, Clone, Serialize)]
pub struct JoinedRow {
    /// Which case.
    pub case: CaseId,
    /// Which model produced the labelled answer this score judged.
    pub model: String,
    /// The case's stratum, when the caller supplied one.
    pub stratum: Option<Stratum>,
    /// The scorer's own judgement.
    pub score: LabelScore,
}

/// Every joined row.
#[derive(Debug, Clone, Default, Serialize)]
pub struct JoinedReport {
    /// One row per (case, label) the scores file named.
    pub rows: Vec<JoinedRow>,
}

/// Attach a scores file's judgements back to the models that earned them.
///
/// `strata` comes from the caller because this module reads no [`crate::cases::Case`].
///
/// # Errors
/// See [`JoinError`].
pub fn join(
    scores: &ScoresFile,
    key: &Key,
    strata: &BTreeMap<CaseId, Stratum>,
) -> Result<JoinedReport, JoinError> {
    let mut rows = Vec::new();
    for (case_text, labels) in &scores.0 {
        let case = CaseId::new(case_text.clone())
            .map_err(|failed| JoinError::BadCaseId(case_text.clone(), failed))?;
        if !key.entries.contains_key(&case) {
            return Err(JoinError::UnknownCase(case));
        }
        for (label, score) in labels {
            let model = key
                .model_of(&case, label)
                .ok_or_else(|| JoinError::UnknownLabel {
                    case: case.clone(),
                    label: label.clone(),
                })?;
            rows.push(JoinedRow {
                case: case.clone(),
                model: model.to_owned(),
                stratum: strata.get(&case).copied(),
                score: score.clone(),
            });
        }
    }
    Ok(JoinedReport { rows })
}

/// One model's judged summary, overall or within one stratum: a sum of its rows.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct JudgedSummary {
    /// Judged the right todo, out of every judged row.
    pub right_todo: Tally,
    /// Priority judged defensible against the anchor table, out of every judged row.
    pub priority_defensible: Tally,
    /// The wording scores' sum, 1-5 each, over the same rows.
    pub wording: u32,
    /// Every case judged as one that would embarrass the owner: a list, never a rate,
    /// because the bar is zero.
    pub would_embarrass: Vec<CaseId>,
}

impl JudgedSummary {
    fn of(row: &JoinedRow) -> Self {
        Self {
            right_todo: Tally::of(Some(row.score.right_todo)),
            priority_defensible: Tally::of(Some(row.score.priority_defensible)),
            wording: u32::from(row.score.wording.value()),
            would_embarrass: if row.score.would_embarrass {
                vec![row.case.clone()]
            } else {
                Vec::new()
            },
        }
    }

    /// How many rows this summarises.
    #[must_use]
    pub fn cases(&self) -> u32 {
        self.right_todo.applied()
    }

    /// The mean wording score, `None` over no rows.
    #[must_use]
    pub fn average_wording(&self) -> Option<f64> {
        (self.cases() > 0).then(|| f64::from(self.wording) / f64::from(self.cases()))
    }
}

impl Add for JudgedSummary {
    type Output = Self;
    fn add(mut self, other: Self) -> Self {
        self.right_todo = self.right_todo + other.right_todo;
        self.priority_defensible = self.priority_defensible + other.priority_defensible;
        self.wording += other.wording;
        self.would_embarrass.extend(other.would_embarrass);
        self
    }
}

impl JoinedReport {
    /// Every model's summary, across every stratum together.
    #[must_use]
    pub fn by_model(&self) -> BTreeMap<String, JudgedSummary> {
        let mut per_model: BTreeMap<String, JudgedSummary> = BTreeMap::new();
        for row in &self.rows {
            let summary = per_model.entry(row.model.clone()).or_default();
            *summary = std::mem::take(summary) + JudgedSummary::of(row);
        }
        per_model
    }

    /// Every model's summary per stratum; a row with no stratum is excluded rather than
    /// pooled into one of the three.
    #[must_use]
    pub fn by_model_and_stratum(&self) -> BTreeMap<(String, Stratum), JudgedSummary> {
        let mut per_group: BTreeMap<(String, Stratum), JudgedSummary> = BTreeMap::new();
        for row in &self.rows {
            let Some(stratum) = row.stratum else { continue };
            let summary = per_group.entry((row.model.clone(), stratum)).or_default();
            *summary = std::mem::take(summary) + JudgedSummary::of(row);
        }
        per_group
    }

    /// The judged half of the triage bar, the plan's §4: in each stratum, a model's
    /// right-todo rate is at least the incumbent's there. The incumbent earns none, and a
    /// stratum where the incumbent was never judged bars nothing.
    #[must_use]
    pub fn right_todo_verdicts(&self, incumbent: &str) -> BTreeMap<(String, Stratum), Verdict> {
        let groups = self.by_model_and_stratum();
        groups
            .iter()
            .filter(|((model, _), _)| model != incumbent)
            .filter_map(|((model, stratum), summary)| {
                let theirs = groups.get(&(incumbent.to_owned(), *stratum))?;
                let bar = Bar::at_least(theirs.right_todo)?;
                Some(((model.clone(), *stratum), bar.judge(summary.right_todo)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::{Candidate, build};

    fn case(id: &str) -> CaseId {
        CaseId::new(id).expect("valid")
    }

    fn score(right: bool, wording: u8, defensible: bool, embarrass: bool) -> LabelScore {
        LabelScore {
            right_todo: right,
            wording: Wording::new(wording).expect("1-5"),
            priority_defensible: defensible,
            would_embarrass: embarrass,
            note: String::new(),
        }
    }

    /// A wording outside 1-5 refuses to parse.
    #[test]
    fn wording_refuses_outside_one_to_five() {
        assert!(Wording::new(0).is_err());
        assert!(Wording::new(6).is_err());
        assert!(Wording::new(3).is_ok());
    }

    /// A scores file refuses malformed wording, through `serde`.
    #[test]
    fn a_malformed_scores_file_refuses_to_parse() {
        let text = r#"{"triage:a": {"A": {"right_todo": true, "wording": 9, "priority_defensible": true, "would_embarrass": false}}}"#;
        assert!(ScoresFile::parse(text).is_err());
    }

    /// Join attaches a score to the model its label named.
    #[test]
    fn join_attaches_a_score_to_the_right_model() {
        let candidates = vec![
            Candidate {
                model: "incumbent".to_owned(),
                text: "wrote t1".to_owned(),
            },
            Candidate {
                model: "candidate/one".to_owned(),
                text: "declined".to_owned(),
            },
        ];
        let (_, key) = build(
            0,
            vec![(case("triage:a"), "triage", "input".to_owned(), candidates)],
        );
        let label_for_incumbent = key
            .entries
            .get(&case("triage:a"))
            .expect("case in key")
            .iter()
            .find(|(_, model)| model.as_str() == "incumbent")
            .map(|(label, _)| label.clone())
            .expect("incumbent has a label");

        let mut inner = BTreeMap::new();
        inner.insert(label_for_incumbent.clone(), score(true, 4, true, false));
        let mut scores = BTreeMap::new();
        scores.insert("triage:a".to_owned(), inner);
        let scores = ScoresFile(scores);

        let strata = BTreeMap::new();
        let joined = join(&scores, &key, &strata).expect("joins");
        assert_eq!(joined.rows.len(), 1);
        assert_eq!(joined.rows[0].model, "incumbent");
        assert!(joined.rows[0].score.right_todo);
    }

    /// A label the key does not know is refused, not silently dropped.
    #[test]
    fn join_refuses_a_label_the_key_does_not_know() {
        let candidates = vec![Candidate {
            model: "incumbent".to_owned(),
            text: "wrote t1".to_owned(),
        }];
        let (_, key) = build(
            0,
            vec![(case("triage:a"), "triage", "input".to_owned(), candidates)],
        );

        let mut inner = BTreeMap::new();
        inner.insert("Z".to_owned(), score(true, 4, true, false));
        let mut scores = BTreeMap::new();
        scores.insert("triage:a".to_owned(), inner);
        let scores = ScoresFile(scores);

        let strata = BTreeMap::new();
        let result = join(&scores, &key, &strata);
        assert!(matches!(result, Err(JoinError::UnknownLabel { .. })));
    }

    /// `by_model` never averages `would_embarrass`; it stays a list.
    #[test]
    fn by_model_keeps_would_embarrass_as_a_list_not_a_rate() {
        let rows = vec![JoinedRow {
            case: case("triage:a"),
            model: "candidate/one".to_owned(),
            stratum: Some(Stratum::Acted),
            score: score(true, 5, true, true),
        }];
        let report = JoinedReport { rows };
        let summary = &report.by_model()["candidate/one"];
        assert_eq!(summary.would_embarrass, vec![case("triage:a")]);
    }

    /// `by_model_and_stratum` drops a row with no stratum rather than pooling it.
    #[test]
    fn by_model_and_stratum_drops_rows_with_no_stratum() {
        let rows = vec![JoinedRow {
            case: case("triage:a"),
            model: "candidate/one".to_owned(),
            stratum: None,
            score: score(true, 5, true, false),
        }];
        let report = JoinedReport { rows };
        assert!(report.by_model_and_stratum().is_empty());
    }

    fn judged(case_id: &str, model: &str, stratum: Stratum, right: bool) -> JoinedRow {
        JoinedRow {
            case: case(case_id),
            model: model.to_owned(),
            stratum: Some(stratum),
            score: score(right, 4, true, false),
        }
    }

    /// A model meets the judged bar in a stratum by matching the incumbent's rate there,
    /// never by an average across strata.
    #[test]
    fn the_judged_bar_is_the_incumbents_rate_in_the_same_stratum() {
        let report = JoinedReport {
            rows: vec![
                judged("triage:a", "incumbent", Stratum::Acted, true),
                judged("triage:a", "candidate/one", Stratum::Acted, false),
                judged("triage:b", "incumbent", Stratum::Quiet, false),
                judged("triage:b", "candidate/one", Stratum::Quiet, true),
                judged("triage:c", "candidate/one", Stratum::Overturned, true),
            ],
        };
        let verdicts = report.right_todo_verdicts("incumbent");
        let of = |stratum| {
            verdicts
                .get(&("candidate/one".to_owned(), stratum))
                .copied()
        };
        assert_eq!(of(Stratum::Acted), Some(Verdict::Missed));
        assert_eq!(of(Stratum::Quiet), Some(Verdict::Met));
        assert_eq!(of(Stratum::Overturned), None);
        assert!(verdicts.keys().all(|(model, _)| model != "incumbent"));
    }
}
