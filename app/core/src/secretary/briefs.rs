//! The morning briefs, derived from the transcripts: a brief is the question [`BRIEF`] asked
//! on a schedule and the answer to it, so nothing is stored twice.
//!
//! The answer is what the model said after its last read, up to the owner's next turn or tap;
//! a turn that reads, or a one-paragraph turn beside a proposal with more words after it, is a
//! status line.

use chrono::NaiveDate;

use super::store::{ConversationId, Said, Stored};
use super::surface::Effect;
use crate::capability::Turn;

/// The question the morning brief asks: a person turn, not a system instruction, since the
/// secretary answers questions somebody asked.
pub const BRIEF: &str = "What changed since the last briefing? Call briefing_delta first. \
                     Tell me what moved, what became urgent, and any good news — not the \
                     priced list, which I can already see. Then propose the two or three \
                     moves that would make the most progress, with the drafts written out.";

/// One morning's brief, where it was said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Brief {
    /// The conversation it is in.
    pub conversation: ConversationId,
    /// The day it was asked for.
    pub date: NaiveDate,
    /// The index of the answering turn.
    pub turn: usize,
    /// The answer, as the secretary wrote it.
    pub text: String,
}

/// Every brief in one conversation, oldest first. A brief whose round never
/// produced an answer (the model was unreachable) is not listed.
#[must_use]
pub fn briefs(stored: &Stored) -> Vec<Brief> {
    let mut found = Vec::new();
    // An answer a regenerate set aside is not the brief.
    let live = stored.live();
    for (at, (_, said)) in live.iter().enumerate() {
        let Turn::Person { text, .. } = &said.turn else {
            continue;
        };
        if text != BRIEF {
            continue;
        }
        let date = prodrome_core::fpl::instant_of(said.at).date();
        let round: Vec<&(usize, &Said)> = live[at + 1..]
            .iter()
            .take_while(|(_, later)| !matches!(later.turn, Turn::Person { .. } | Turn::Act { .. }))
            .collect();
        let after_reads = round
            .iter()
            .rposition(|(_, later)| reads(&later.turn))
            .map_or(0, |last| last + 1);
        let mut words: Vec<(usize, &str, bool)> = round[after_reads..]
            .iter()
            .filter_map(|(index, later)| match &later.turn {
                Turn::Model { text, calls } if !text.trim().is_empty() => {
                    let status = !calls.is_empty() && super::answer::blocks(text).len() == 1;
                    Some((*index, text.trim(), status))
                }
                _ => None,
            })
            .collect();
        let leading = words.iter().take_while(|(_, _, status)| *status).count();
        words.drain(..leading.min(words.len().saturating_sub(1)));
        if let Some(&(turn, _, _)) = words.first() {
            found.push(Brief {
                conversation: stored.id.clone(),
                date,
                turn,
                text: words
                    .iter()
                    .map(|(_, text, _)| *text)
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            });
        }
    }
    found
}

/// Whether a turn is a status line: the model asked for something read. A
/// verb this build no longer names reads, since what it did is unknown.
fn reads(turn: &Turn) -> bool {
    match super::turn_shape::shape(turn) {
        super::turn_shape::TurnShape::Model { calls } => calls.iter().any(|verb| {
            verb.verb()
                .is_none_or(|verb| verb.class().effect == Effect::Reads)
        }),
        super::turn_shape::TurnShape::Person { .. }
        | super::turn_shape::TurnShape::Tool { .. }
        | super::turn_shape::TurnShape::Act { .. } => false,
    }
}

/// A brief's line in a list: the first line of its words as Typst parses
/// them, so a link reads as its label and never as `#link(…)[…]`.
#[must_use]
pub fn headline(text: &str) -> String {
    super::answer::blocks(text)
        .iter()
        .flat_map(|block| block.plain.lines())
        .map(|line| line.trim_start_matches(['•', ' ']).trim())
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_owned()
}

/// How many words a brief holds, as Typst parses them.
#[must_use]
pub fn words(text: &str) -> usize {
    super::answer::blocks(text)
        .iter()
        .map(|block| block.plain.split_whitespace().count())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{ToolCall, Turn};
    use crate::secretary::store::Said;

    fn said(at: &str, turn: Turn) -> Said {
        Said {
            at: crate::secretary::store::instant(at),
            turn,
            by: None,
        }
    }

    fn person(text: &str) -> Turn {
        Turn::Person {
            text: text.to_owned(),
            references: Vec::new(),
            attachments: Vec::new(),
        }
    }

    fn model(text: &str, verbs: &[&str]) -> Turn {
        Turn::Model {
            text: text.to_owned(),
            calls: verbs
                .iter()
                .map(|verb| ToolCall {
                    verb: (*verb).into(),
                    args: std::collections::BTreeMap::new(),
                })
                .collect(),
        }
    }

    fn tool(verb: &str) -> Turn {
        Turn::Tool {
            verb: verb.into(),
            answer: "{}".to_owned(),
            outcome: crate::secretary::outcome::Outcome::Answered,
            files: Vec::new(),
        }
    }

    fn stored(turns: Vec<Said>) -> Stored {
        Stored {
            opened: crate::capability::Datetime::new(2026, 9, 14, 11, 5, 41, 0)
                .expect("an instant"),
            title: crate::secretary::words::Words::kept("Secretary".to_owned()),
            model: "m".to_owned(),
            turns,
            ..Stored::empty(
                &crate::secretary::store::DEFAULT,
                crate::secretary::store::Kind::Default,
                crate::capability::Datetime::new(2026, 9, 14, 11, 5, 41, 0).expect("an instant"),
            )
        }
    }

    #[test]
    fn the_answer_is_what_the_model_said_after_its_last_read() {
        let found = briefs(&stored(vec![
            said("2026-09-23T10:00:00", person("hello")),
            said("2026-09-23T10:00:05", model("hi", &[])),
            said("2026-09-24T11:03:00", person(BRIEF)),
            said(
                "2026-09-24T11:03:05",
                model("Pulling the delta.", &["weather"]),
            ),
            said("2026-09-24T11:03:06", tool("weather")),
            said(
                "2026-09-24T11:04:08",
                model("= What moved\nThe permit landed.", &[]),
            ),
            said("2026-09-24T12:00:00", person("thanks")),
            said("2026-09-24T12:00:05", model("You're welcome.", &[])),
        ]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].turn, 5);
        assert_eq!(found[0].date.to_string(), "2026-09-24");
        assert_eq!(found[0].text, "= What moved\nThe permit landed.");
        assert_eq!(headline(&found[0].text), "What moved");
    }

    /// A brief said beside its `propose_email` is the brief, with the note after it appended.
    #[test]
    fn a_brief_said_beside_a_proposal_is_the_brief_with_what_follows() {
        let found = briefs(&stored(vec![
            said("2026-09-28T07:03:00", person(BRIEF)),
            said(
                "2026-09-28T07:03:05",
                model("Reading what moved.", &["briefing_delta"]),
            ),
            said("2026-09-28T07:03:06", tool("briefing_delta")),
            said(
                "2026-09-28T07:04:00",
                model(
                    "Waiting on you: Mira's question.\n\n== Moves",
                    &["propose_email"],
                ),
            ),
            said("2026-09-28T07:04:01", tool("propose_email")),
            said(
                "2026-09-28T07:04:30",
                model(
                    r#"The reply is #link("proposal:default/41")[proposal 41]."#,
                    &[],
                ),
            ),
        ]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].turn, 3);
        assert_eq!(
            found[0].text,
            "Waiting on you: Mira's question.\n\n== Moves\n\n\
             The reply is #link(\"proposal:default/41\")[proposal 41]."
        );
        assert_eq!(headline(&found[0].text), "Waiting on you: Mira's question.");
        assert_eq!(words(&found[0].text), 11);
    }

    /// A card's status line is not the brief, and the owner's dismissal starts the next round.
    #[test]
    fn a_card_s_status_line_and_the_owner_s_tap_are_not_the_brief() {
        let found = briefs(&stored(vec![
            said("2026-09-27T07:03:00", person(BRIEF)),
            said(
                "2026-09-27T07:03:05",
                model("Reading what moved.", &["briefing_delta"]),
            ),
            said("2026-09-27T07:03:06", tool("briefing_delta")),
            said(
                "2026-09-27T07:04:00",
                model("Putting up a follow-up to Mira.", &["propose_email"]),
            ),
            said("2026-09-27T07:04:01", tool("propose_email")),
            said(
                "2026-09-27T07:04:30",
                model("Rain later.\n\n== What moved", &[]),
            ),
            said(
                "2026-09-27T09:00:00",
                Turn::Act {
                    text: "Ada dismissed proposal 35".to_owned(),
                    references: Vec::new(),
                },
            ),
            said(
                "2026-09-27T09:00:05",
                model("Fair, that was too early.", &[]),
            ),
        ]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].turn, 5);
        assert_eq!(found[0].text, "Rain later.\n\n== What moved");
    }

    #[test]
    fn a_short_brief_beside_its_card_is_still_the_brief() {
        let found = briefs(&stored(vec![
            said("2026-09-29T07:03:00", person(BRIEF)),
            said(
                "2026-09-29T07:03:05",
                model("Nothing moved; the reply is up.", &["propose_email"]),
            ),
            said("2026-09-29T07:03:06", tool("propose_email")),
        ]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "Nothing moved; the reply is up.");
    }

    #[test]
    fn a_headline_reads_a_link_as_its_label() {
        assert_eq!(
            headline(r#"The reply is #link("proposal:default/41")[proposal 41]."#),
            "The reply is proposal 41."
        );
    }

    #[test]
    fn a_brief_nobody_answered_is_not_listed() {
        let found = briefs(&stored(vec![
            said("2026-09-24T11:03:00", person(BRIEF)),
            said(
                "2026-09-24T11:03:05",
                model("Pulling the delta.", &["weather"]),
            ),
        ]));
        assert!(found.is_empty());
    }
}
