//! The blind sheet: every model's answer to a case under a shuffled anonymous [`Label`], and
//! a separate [`Key`] the scorer does not open until scoring is done.
//!
//! The incumbent is an ordinary [`Candidate`], with no arm of its own, so nothing can tell it
//! apart by shape. The shuffle is seeded, so anyone can re-derive the same sheet.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cases::CaseId;

/// An anonymous label, spreadsheet-style (`A` … `Z`, `AA`, …), so it never runs out.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Label(String);

impl Label {
    /// The label at this position in a case's shuffled order; `0` is `A`.
    #[must_use]
    pub fn of(mut index: usize) -> Self {
        let mut letters = Vec::new();
        loop {
            let letter = u8::try_from(index % 26).unwrap_or(0);
            letters.push(b'A' + letter);
            if index < 26 {
                break;
            }
            index = index / 26 - 1;
        }
        letters.reverse();
        Self(String::from_utf8(letters).unwrap_or_default())
    }

    /// The label's own text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One model's answer to one case before anonymising; `model` reaches the [`Key`] and never
/// the [`Sheet`].
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The producing model, the incumbent's name included.
    pub model: String,
    /// The answer as text a judge reads, never a raw transcript that would announce itself.
    pub text: String,
}

/// One case, anonymised: the input a judge is shown, and every answer under a shuffled label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetCase {
    /// Which case.
    pub case: CaseId,
    /// The case's kind word, owned so the case can be deserialised back.
    pub kind: String,
    /// What the judge is shown as the input, never a model's own words.
    pub input: String,
    /// Every answer in shuffled order; a `Vec`, since sorting by label would give a repeat
    /// judge a stable column order to learn.
    pub answers: Vec<(Label, String)>,
}

/// The blind sheet: every case, anonymised.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sheet {
    /// One entry per case.
    pub cases: Vec<SheetCase>,
}

/// The key: each case's labels back to the models, unopened until scoring is done.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Key {
    /// `case -> label text -> model`.
    pub entries: BTreeMap<CaseId, BTreeMap<String, String>>,
}

impl Key {
    /// The model a label named for this case, if the key has both.
    #[must_use]
    pub fn model_of(&self, case: &CaseId, label: &str) -> Option<&str> {
        self.entries.get(case)?.get(label).map(String::as_str)
    }
}

/// A candidate's rank for one seed and case, independent of the order the caller listed them.
fn rank(seed: u64, case: &CaseId, model: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(case.as_str().as_bytes());
    hasher.update(model.as_bytes());
    hasher.finalize().into()
}

/// Build the sheet and the key together: the one function that knows the mapping.
#[must_use]
pub fn build(
    seed: u64,
    cases: Vec<(CaseId, &'static str, String, Vec<Candidate>)>,
) -> (Sheet, Key) {
    let mut sheet_cases = Vec::with_capacity(cases.len());
    let mut key = Key::default();
    for (case, kind, input, mut candidates) in cases {
        candidates.sort_by_cached_key(|candidate| rank(seed, &case, &candidate.model));
        let mut answers = Vec::with_capacity(candidates.len());
        let mut labelled = BTreeMap::new();
        for (index, candidate) in candidates.into_iter().enumerate() {
            let label = Label::of(index);
            labelled.insert(label.as_str().to_owned(), candidate.model);
            answers.push((label, candidate.text));
        }
        key.entries.insert(case.clone(), labelled);
        sheet_cases.push(SheetCase {
            case,
            kind: kind.to_owned(),
            input,
            answers,
        });
    }
    (Sheet { cases: sheet_cases }, key)
}

/// The model name the box's own recorded answer is pooled under, which only the key holds.
pub const INCUMBENT: &str = "incumbent";

/// What the judge answers for every label, in `bench join`'s scores file: written before any
/// answer is read, and the same for every case, so no answer is judged by a rule of its own.
pub const RUBRIC: &str = "# Rubric\n\n\
Score every label of every case, blind, against its input alone; this box's own answer is one \
of the labels and is judged like the rest. Mechanical failures are already counted by \
`bench score` and no judgement here lifts one. Write `{case: {label: {...}}}` with:\n\n\
- `right_todo` (true/false): is this the right thing to have done with the message at all, \
a todo where one is owed and nothing where none is?\n\
- `wording` (1-5): is the todo usable in the owner's register, 1 unusable, 5 as he would write \
it; a decline scores 5 when declining was right.\n\
- `priority_defensible` (true/false): is the price defensible against the anchor table in \
OPERATOR.md (10 life this hour, 20 an irreversible blow tomorrow, 35-45 weeks, 50-65 this \
month, 70-85 errands, 90+ parked)? A decline is true.\n\
- `would_embarrass` (true/false): would this embarrass the owner if it were acted on?\n\
- `note`: anything else, or \"\".\n\n---\n\n";

/// Render the sheet as the text a judge opens: the rubric, then inputs and labelled answers,
/// never a model name.
#[must_use]
pub fn render_markdown(sheet: &Sheet) -> String {
    let mut text = String::from(RUBRIC);
    for case in &sheet.cases {
        // `write!` into a `String` never fails.
        let _ = writeln!(text, "## {} ({})\n", case.case, case.kind);
        let _ = writeln!(text, "### Input\n\n{}\n", case.input);
        for (label, answer) in &case.answers {
            let _ = writeln!(text, "### {}\n\n{answer}\n", label.as_str());
        }
        text.push_str("---\n\n");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(id: &str) -> CaseId {
        CaseId::new(id).expect("valid")
    }

    /// The same seed gives the same labelling, whatever the input order.
    #[test]
    fn the_same_seed_orders_candidates_the_same_way_regardless_of_input_order() {
        let forward = vec![
            Candidate {
                model: "incumbent".to_owned(),
                text: "wrote t1".to_owned(),
            },
            Candidate {
                model: "candidate/one".to_owned(),
                text: "wrote t1 differently".to_owned(),
            },
        ];
        let mut backward = forward.clone();
        backward.reverse();
        let (sheet_a, key_a) = build(
            7,
            vec![(case("triage:a"), "triage", "input".to_owned(), forward)],
        );
        let (sheet_b, key_b) = build(
            7,
            vec![(case("triage:a"), "triage", "input".to_owned(), backward)],
        );
        assert_eq!(sheet_a.cases[0].answers, sheet_b.cases[0].answers);
        assert_eq!(key_a.entries, key_b.entries);
    }

    /// The rendered sheet carries no model name; the key carries them all.
    #[test]
    fn the_rendered_sheet_names_no_model() {
        let candidates = vec![
            Candidate {
                model: "incumbent".to_owned(),
                text: "wrote a todo about the permit".to_owned(),
            },
            Candidate {
                model: "deepseek/v4.1-flash".to_owned(),
                text: "declined, nothing actionable".to_owned(),
            },
        ];
        let (sheet, key) = build(
            0,
            vec![(
                case("triage:a"),
                "triage",
                "a message about a permit deadline".to_owned(),
                candidates,
            )],
        );
        let text = render_markdown(&sheet);
        assert!(!text.contains("incumbent"), "{text}");
        assert!(!text.contains("deepseek"), "{text}");
        // The key DOES carry both, under the two labels the sheet used.
        let names: std::collections::BTreeSet<&str> = key
            .entries
            .get(&case("triage:a"))
            .expect("this case is in the key")
            .values()
            .map(String::as_str)
            .collect();
        assert_eq!(
            names,
            std::collections::BTreeSet::from(["incumbent", "deepseek/v4.1-flash"])
        );
    }
}
