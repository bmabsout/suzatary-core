//! The conversation about a thing: a query over the one transcript (the rounds that touch it),
//! not a second chat.
//!
//! A person's turn or an act opens a round, the same partition `web/src/chat/rounds.ts` draws;
//! `app/core/tests/about-fixture.json` is read by both sides' tests so the two cannot drift.

use crate::capability::Turn;
use crate::reference::{self, Reference};
use crate::secretary::store::Said;

/// Inclusive `[start, end]` turn-index spans partitioning the whole transcript with no gaps.
///
/// An empty transcript has no spans; the last span may be unfinished (no answer yet).
#[must_use]
pub fn round_spans(turns: &[Said]) -> Vec<(usize, usize)> {
    if turns.is_empty() {
        return Vec::new();
    }
    let mut spans = Vec::new();
    let mut start = 0usize;
    for (index, said) in turns.iter().enumerate().skip(1) {
        if matches!(&said.turn, Turn::Person { .. } | Turn::Act { .. }) {
            spans.push((start, index - 1));
            start = index;
        }
    }
    spans.push((start, turns.len() - 1));
    spans
}

/// Whether the round at `span` touches `reference`: its opening turn carries it, or a model
/// turn in it names it in prose (so a round with no opening, like the brief, can still touch).
#[must_use]
pub fn round_touches(turns: &[Said], span: (usize, usize), reference: &Reference) -> bool {
    let (start, end) = span;
    let opens_on_it = matches!(
        &turns[start].turn,
        Turn::Person { references, .. } | Turn::Act { references, .. }
            if references.contains(reference)
    );
    opens_on_it
        || turns[start..=end].iter().any(|said| match &said.turn {
            Turn::Model { text, .. } => reference::mentions(text).contains(reference),
            _ => false,
        })
}

/// The span owning turn index `target`, clamped to the nearest end; mirrors the page's proposal
/// placement, so a proposal kept for `about` is the one the page draws in a touched round.
#[must_use]
pub fn owning_span(spans: &[(usize, usize)], target: usize) -> usize {
    match spans
        .iter()
        .position(|&(start, end)| target >= start && target <= end)
    {
        Some(index) => index,
        None if spans.is_empty() => 0,
        None => usize::from(target >= spans[0].0) * (spans.len() - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::Reference;
    use serde::Deserialize;

    /// One row of `app/core/tests/about-fixture.json`, the file `web/test/rounds.test.ts` also
    /// reads.
    #[derive(Deserialize)]
    struct FixtureTurn {
        who: String,
        text: String,
        references: Vec<String>,
        at: String,
    }

    fn said(row: &FixtureTurn) -> Said {
        let references: Vec<Reference> = row
            .references
            .iter()
            .map(|text| text.parse().expect("a reference in the fixture"))
            .collect();
        let turn = match row.who.as_str() {
            "person" => Turn::Person {
                text: row.text.clone(),
                references,
                attachments: Vec::new(),
            },
            "act" => Turn::Act {
                text: row.text.clone(),
                references,
            },
            "model" => Turn::Model {
                text: row.text.clone(),
                calls: Vec::new(),
            },
            "verb" => Turn::Tool {
                verb: "todo_show".into(),
                answer: row.text.clone(),
                refused: false,
                files: Vec::new(),
            },
            other => panic!("the fixture named an unknown turn kind {other:?}"),
        };
        Said {
            at: row.at.clone(),
            turn,
            by: None,
        }
    }

    /// Three rounds: the first opens on `todo:life/x`, the second names `mail:a1b2c3d4e5f6` only in
    /// prose, the third touches neither.
    fn fixture() -> Vec<Said> {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/about-fixture.json"),
        )
        .expect("app/core/tests/about-fixture.json reads");
        let rows: Vec<FixtureTurn> = serde_json::from_str(&text).expect("the fixture parses");
        rows.iter().map(said).collect()
    }

    #[test]
    fn round_spans_partitions_on_person_and_act_openings() {
        let spans = round_spans(&fixture());
        assert_eq!(spans, vec![(0, 3), (4, 5), (6, 7)]);
    }

    #[test]
    fn an_empty_transcript_has_no_spans() {
        assert_eq!(round_spans(&[]), Vec::new());
    }

    #[test]
    fn a_leading_model_turn_still_gets_a_span() {
        let turns = vec![Said {
            at: "2026-09-15T10:03:00".to_owned(),
            turn: Turn::Model {
                text: "Your brief is ready.".to_owned(),
                calls: Vec::new(),
            },
            by: None,
        }];
        assert_eq!(round_spans(&turns), vec![(0, 0)]);
    }

    #[test]
    fn a_round_touches_when_its_opening_carries_the_reference() {
        let turns = fixture();
        let spans = round_spans(&turns);
        let reference: Reference = "todo:life/x".parse().expect("a reference");
        assert!(round_touches(&turns, spans[0], &reference));
        assert!(!round_touches(&turns, spans[1], &reference));
        assert!(!round_touches(&turns, spans[2], &reference));
    }

    #[test]
    fn a_round_touches_when_a_model_turn_names_it_in_prose() {
        let turns = fixture();
        let spans = round_spans(&turns);
        let reference: Reference = "mail:a1b2c3d4e5f6".parse().expect("a reference");
        assert!(!round_touches(&turns, spans[0], &reference));
        assert!(round_touches(&turns, spans[1], &reference));
        assert!(!round_touches(&turns, spans[2], &reference));
    }

    #[test]
    fn an_untouched_round_touches_nothing_it_was_not_asked_about() {
        let turns = fixture();
        let spans = round_spans(&turns);
        let reference: Reference = "todo:life/somethingelse".parse().expect("a reference");
        assert!(!round_touches(&turns, spans[0], &reference));
        assert!(!round_touches(&turns, spans[1], &reference));
        assert!(!round_touches(&turns, spans[2], &reference));
    }

    #[test]
    fn owning_span_finds_the_span_holding_the_target() {
        let spans = vec![(0, 3), (4, 5), (6, 7)];
        assert_eq!(owning_span(&spans, 0), 0);
        assert_eq!(owning_span(&spans, 5), 1);
        assert_eq!(owning_span(&spans, 7), 2);
    }

    #[test]
    fn owning_span_clamps_to_the_nearest_end() {
        let spans = vec![(2, 3), (4, 5)];
        assert_eq!(owning_span(&spans, 0), 0, "before the first span");
        assert_eq!(owning_span(&spans, 99), 1, "past the last span");
    }
}
