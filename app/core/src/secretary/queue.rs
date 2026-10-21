//! The transcript is the queue: what is still undelivered is derived from it, never kept beside
//! it, so "a queued message is never sent twice" is a property of a derivation, not a lock. For
//! one writer it is the owed register (`transcript::Transcript::owed`), which a law holds equal.

use crate::capability::Turn;
use crate::secretary::stamp::Stamp;
use crate::secretary::store::{Said, Scope, Stored};
use crate::secretary::tool_name::ToolName;

/// How many `say`s one conversation may hold unanswered at once; past it `say` refuses rather
/// than grow the conversation forever.
pub const MAX_PENDING: usize = 16;

/// What the next `resume` sends: every live turn after the last one that closes a round, with its
/// index.
///
/// A round is closed exactly by a turn that [closes](Stored::closes) it: a `Turn::Model` with no
/// calls, or a stop, so a stopped question is never owed again, not even between two writes. A
/// view, never a mark: once a round's own turn is appended the boundary moves past what it read. A
/// turn a rewind set aside is on no path, so a regenerate's question is pending again.
#[must_use]
pub fn delta(stored: &Stored) -> Vec<(usize, &Said)> {
    let live = stored.live();
    let boundary = live.iter().rposition(|(index, _)| stored.closes(*index));
    match boundary {
        Some(at) => live[at + 1..].to_vec(),
        None => live,
    }
}

/// Whether `said` starts a round: anything but an act line the gate did not stamp as the
/// owner's. The box's lines (a card it filed, a restart, a sign-in's link) are sent with the next
/// round, and wake no model alone.
#[must_use]
pub fn wakes(said: &Said) -> bool {
    match said.turn {
        Turn::Act { .. } => Stamp::owners(said.by.as_ref()),
        Turn::Person { .. } | Turn::Model { .. } | Turn::Tool { .. } | Turn::Erased { .. } => true,
    }
}

/// What a round answers: [`delta`], never empty and holding a turn that [`wakes`], so a resumed
/// session is always sent something and a nudge that added no turn asks no model.
#[derive(Debug, Clone)]
pub struct Pending<'a> {
    first: &'a Said,
    rest: Vec<&'a Said>,
}

impl<'a> Pending<'a> {
    /// What waits in `stored`, or `None` when nothing that [`wakes`] does.
    #[must_use]
    pub fn of(stored: &'a Stored) -> Option<Self> {
        let waiting = delta(stored);
        if !waiting.iter().any(|(_, said)| wakes(said)) {
            return None;
        }
        let ((_, first), rest) = waiting.split_first()?;
        Some(Self {
            first,
            rest: rest.iter().map(|(_, said)| *said).collect(),
        })
    }

    /// What a resumed session is sent: the memory's heading, if anything moved, then the waiting
    /// turns.
    #[must_use]
    pub fn delta(&self, heading: Option<Turn>) -> crate::capability::Delta {
        let first = self.first.turn.clone();
        let rest = self.rest.iter().map(|said| said.turn.clone());
        match heading {
            Some(heading) => {
                crate::capability::Delta::new(heading, std::iter::once(first).chain(rest))
            }
            None => crate::capability::Delta::new(first, rest),
        }
    }
}

/// The turn heading a batch with what the model is told it remembers. Never stored: the
/// `MemorySeen` line written after the answer is what makes it once.
#[must_use]
pub fn heading(told: String) -> Turn {
    Turn::Tool {
        verb: ToolName::Memory,
        answer: told,
        outcome: crate::secretary::outcome::Outcome::Answered,
        files: Vec::new(),
    }
}

/// A batch: the heading, if any, then the turns.
#[must_use]
pub fn headed(heading: Option<Turn>, turns: &[(usize, &Said)]) -> Vec<Turn> {
    heading
        .into_iter()
        .chain(turns.iter().map(|(_, said)| said.turn.clone()))
        .collect()
}

/// How many turns a model without a session is sent: the conversation page's
/// own default, so what the owner sees and what the model reads are one window.
pub const MODEL_WINDOW: usize = 60;

/// What a model without a session is sent: the last [`MODEL_WINDOW`] live turns, starting at the
/// first round boundary inside them.
///
/// A window cut mid-round is malformed (a tool answer without its call), so it starts where
/// [`delta`] cuts; a round longer than the window is sent whole, from its own start.
#[must_use]
pub fn for_model(stored: &Stored) -> Vec<(usize, &Said)> {
    window_of(stored, &stored.live(), MODEL_WINDOW).to_vec()
}

/// [`for_model`] over any window size, so a test can use six turns, not sixty.
fn window_of<'a, 'b>(
    stored: &Stored,
    turns: &'a [(usize, &'b Said)],
    limit: usize,
) -> &'a [(usize, &'b Said)] {
    let floor = turns.len().saturating_sub(limit);
    let starts_a_round = |index: usize| stored.opens(turns, index);
    let start = (floor..turns.len())
        .find(|&index| starts_a_round(index))
        .or_else(|| (0..floor).rev().find(|&index| starts_a_round(index)))
        .unwrap_or(0);
    &turns[start..]
}

/// Every turn a model asked as `stored` records ([`Stored::scope`]) holds now ([`sent`]).
#[must_use]
pub fn held(stored: &Stored) -> Vec<(usize, &Said)> {
    sent(stored, stored.scope)
}

/// Every turn a model asked over `scope` holds: for a transcript, every stored turn, which is what
/// it is sent; for a window, what [`for_model`] sends and, when a session is kept, every live turn
/// from the first one its opening sent, since a resumed session is sent only what it has not read
/// and keeps all it has.
#[must_use]
pub fn sent(stored: &Stored, scope: Scope) -> Vec<(usize, &Said)> {
    match scope {
        Scope::Transcript => stored.turns.iter().enumerate().collect(),
        Scope::Window => {
            let window = for_model(stored);
            let Some(kept) = &stored.session else {
                return window;
            };
            let from = window
                .first()
                .map_or(stored.turns.len(), |(at, _)| *at)
                .min(kept.from);
            stored
                .live()
                .into_iter()
                .filter(|(at, _)| *at >= from)
                .collect()
        }
    }
}

/// How many lines are still queued: [`delta`]'s length, for the `MAX_PENDING` refusal and a badge.
#[must_use]
pub fn pending(stored: &Stored) -> usize {
    delta(stored).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secretary::store::{ConversationId, Entry};

    fn id() -> ConversationId {
        ConversationId::new("2026-09-13-1").expect("a slug")
    }

    fn opened() -> Entry {
        Entry::Opened {
            at: crate::capability::Datetime::new(2026, 9, 13, 9, 0, 0, 0).expect("an instant"),
            title: crate::secretary::words::Words::kept("Morning".to_owned()),
            model: "claude-cli/haiku".to_owned(),
            kind: crate::secretary::store::Kind::Topic,
        }
    }

    fn said(at: &str, turn: Turn) -> Entry {
        Entry::Said {
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

    fn finished_model(text: &str) -> Turn {
        Turn::Model {
            text: text.to_owned(),
            calls: Vec::new(),
        }
    }

    fn calling_model(text: &str, verb: &str) -> Turn {
        Turn::Model {
            text: text.to_owned(),
            calls: vec![crate::capability::ToolCall {
                verb: verb.into(),
                args: std::collections::BTreeMap::new(),
            }],
        }
    }

    fn stored(entries: &[Entry]) -> Stored {
        Stored::of(&id(), entries).expect("a conversation")
    }

    /// `delta`'s bare turns, without each `Said`'s `at`.
    fn turns<'a>(delta: &[(usize, &'a Said)]) -> Vec<&'a Turn> {
        delta.iter().map(|(_, said)| &said.turn).collect()
    }

    /// The box's own notes wake no model: a conversation holding only them waits for nothing,
    /// and the owner's next turn is sent with them, in order.
    #[test]
    fn the_boxs_notes_wait_for_the_owners_turn() {
        let noted = |at: &str, text: &str| {
            Entry::noted(
                crate::secretary::store::instant(at),
                text.to_owned(),
                Vec::new(),
            )
        };
        let mut lines = vec![
            opened(),
            noted("2026-09-13T09:01:00", "The box asks: a key."),
            noted("2026-09-13T09:02:00", "The box asks: a link."),
        ];
        assert!(Pending::of(&stored(&lines)).is_none(), "nothing wakes");
        let mut owner = said("2026-09-13T09:03:00", person("add my Work calendar"));
        if let Entry::Said { by, .. } = &mut owner {
            *by = Some(Stamp::Actor("ada".to_owned()));
        }
        lines.push(owner);
        let waiting = stored(&lines);
        let pending = Pending::of(&waiting).expect("the owner's turn wakes");
        assert_eq!(pending.delta(None).turns().len(), 3, "the notes ride along");
        let dismissed = Entry::Said {
            at: crate::capability::Datetime::new(2026, 9, 13, 9, 4, 0, 0).expect("an instant"),
            turn: Turn::Act {
                text: "Ada dismissed it: not now".to_owned(),
                references: Vec::new(),
            },
            by: Some(Stamp::Actor("ada".to_owned())),
        };
        assert!(
            Pending::of(&stored(&[opened(), dismissed])).is_some(),
            "the owner's own act still wakes"
        );
    }

    /// Nothing waiting is no `Pending`, and what waits is sent headed by the memory's change, then
    /// in order.
    #[test]
    fn pending_is_what_waits_and_its_delta_is_never_empty() {
        let answered = [
            opened(),
            said("2026-09-13T09:01:00", person("what changed")),
            said("2026-09-13T09:02:00", finished_model("the permit")),
        ];
        assert!(Pending::of(&stored(&answered[..1])).is_none());
        assert!(Pending::of(&stored(&answered)).is_none());

        let mut asked = answered.to_vec();
        asked.push(said("2026-09-13T09:03:00", person("and the flight?")));
        asked.push(said("2026-09-13T09:04:00", calling_model("", "todo_show")));
        let asked = stored(&asked);
        let waiting = Pending::of(&asked).expect("two turns wait");
        assert_eq!(
            waiting.delta(None).turns(),
            [person("and the flight?"), calling_model("", "todo_show")]
        );
        let heading = Turn::Tool {
            verb: ToolName::Memory,
            answer: "fact 3: she flies on Friday".to_owned(),
            outcome: crate::secretary::outcome::Outcome::Answered,
            files: Vec::new(),
        };
        assert_eq!(
            waiting.delta(Some(heading.clone())).turns(),
            [
                heading,
                person("and the flight?"),
                calling_model("", "todo_show")
            ]
        );
    }

    /// An empty conversation queues nothing.
    #[test]
    fn nothing_said_is_nothing_queued() {
        let stored = stored(&[opened()]);
        assert!(delta(&stored).is_empty());
        assert_eq!(pending(&stored), 0);
    }

    /// Every person turn before the first completed round is queued.
    #[test]
    fn person_turns_with_no_completed_round_are_all_queued() {
        let stored = stored(&[
            opened(),
            said("t1", person("hi")),
            said("t2", person("still there?")),
        ]);
        assert_eq!(
            turns(&delta(&stored)),
            vec![&person("hi"), &person("still there?")]
        );
        assert_eq!(pending(&stored), 2);
    }

    /// A completed round clears the queue.
    #[test]
    fn a_finished_round_empties_the_queue() {
        let stored = stored(&[
            opened(),
            said("t1", person("hi")),
            said("t2", finished_model("hello")),
        ]);
        assert!(delta(&stored).is_empty());
        assert_eq!(pending(&stored), 0);
    }

    /// Only what came after the last finished round is queued.
    #[test]
    fn only_turns_after_the_last_finished_round_are_queued() {
        let stored = stored(&[
            opened(),
            said("t1", person("first")),
            said("t2", finished_model("answered first")),
            said("t3", person("second")),
        ]);
        assert_eq!(turns(&delta(&stored)), vec![&person("second")]);
    }

    /// A round still running (a model turn with calls, and its tool answers) is part of the next
    /// delta: only a model turn with no calls ends a round.
    #[test]
    fn an_in_flight_round_stays_in_the_delta() {
        let stored = stored(&[
            opened(),
            said("t1", person("what changed")),
            said("t2", calling_model("looking", "briefing_delta")),
            said(
                "t3",
                Turn::Tool {
                    verb: "briefing_delta".into(),
                    answer: "{}".to_owned(),
                    outcome: crate::secretary::outcome::Outcome::Answered,
                    files: Vec::new(),
                },
            ),
        ]);
        assert_eq!(delta(&stored).len(), 3);
    }

    /// An act is queued too: a dismissal is a fact the next round reads, like a person's words.
    #[test]
    fn an_act_after_a_finished_round_is_queued() {
        let stored = stored(&[
            opened(),
            said("t1", finished_model("done")),
            said(
                "t2",
                Turn::Act {
                    text: "Ada dismissed proposal 0: wrong person".to_owned(),
                    references: Vec::new(),
                },
            ),
        ]);
        assert_eq!(pending(&stored), 1);
    }

    fn stop(at: &str) -> Entry {
        Entry::Stopped {
            at: crate::secretary::store::instant(at),
            by: "ada".to_owned(),
        }
    }

    /// A stop closes the question it stopped by its own line: nothing is queued after it, with no
    /// empty answer written to say so.
    #[test]
    fn a_stop_closes_the_question_it_stopped() {
        let stopped = stored(&[
            opened(),
            said("t1", person("read every thread")),
            stop("t2"),
        ]);
        assert!(delta(&stopped).is_empty());
        assert!(
            Pending::of(&stopped).is_none(),
            "no round answers a stopped question"
        );
        assert_eq!(
            crate::secretary::store::state(&stopped),
            crate::secretary::store::ConversationState::Idle
        );

        let asked_again = stored(&[
            opened(),
            said("t1", person("read every thread")),
            stop("t2"),
            said("t3", person("just the first one")),
        ]);
        assert_eq!(
            turns(&delta(&asked_again)),
            vec![&person("just the first one")]
        );
    }

    /// A conversation stored before a stop closed by itself holds the stop then an empty end; it
    /// reads exactly as it did: nothing queued, and the empty end opens no round of its own.
    #[test]
    fn a_stored_stop_with_its_empty_end_reads_as_it_did() {
        let lines = [
            opened(),
            said("t1", person("p1")),
            said("t2", finished_model("one")),
            said("t3", person("p2")),
            stop("t4"),
            said("t4", finished_model("")),
            said("t5", person("p3")),
            said("t6", finished_model("three")),
        ];
        let stored = stored(&lines);
        assert!(delta(&stored).is_empty());
        assert!(delta(&Stored::of(&id(), &lines[..6]).expect("a conversation")).is_empty());
        let live = stored.live();
        let window = window_of(&stored, &live, 3);
        assert_eq!(
            turns(window),
            vec![&person("p3"), &finished_model("three")],
            "the empty end belongs to its stop, so the window starts after it"
        );
    }

    /// In one line, the stop opens the next round where its question was closed.
    #[test]
    fn a_stop_ends_a_round_for_the_window() {
        let stored = stored(&[
            opened(),
            said("t1", person("p1")),
            said("t2", finished_model("one")),
            said("t3", person("p2")),
            stop("t4"),
            said("t5", person("p3")),
            said("t6", finished_model("three")),
        ]);
        let live = stored.live();
        assert_eq!(
            turns(window_of(&stored, &live, 2)),
            vec![&person("p3"), &finished_model("three")]
        );
    }

    // --- The property: every person turn is delivered exactly once. -------

    /// One step of a simulated conversation: the owner says something, dismisses a proposal, a
    /// round asks for a verb, a round completes, delivering whatever is queued, or the owner stops
    /// the round, closing whatever is queued unanswered: in one line, or as stored conversations
    /// hold it, the stop then an empty end.
    #[derive(Debug, Clone, Copy)]
    enum Step {
        Person,
        Act,
        Call,
        Complete,
        Stop { stored_shape: bool },
    }

    fn steps() -> impl proptest::strategy::Strategy<Value = Vec<Step>> {
        use proptest::prelude::*;
        proptest::collection::vec(
            prop_oneof![
                Just(Step::Person),
                Just(Step::Act),
                Just(Step::Call),
                Just(Step::Complete),
                any::<bool>().prop_map(|stored_shape| Step::Stop { stored_shape }),
            ],
            0..40,
        )
    }

    /// The lines a script comes to.
    fn script_lines(script: &[Step]) -> Vec<Entry> {
        let mut entries = vec![opened()];
        let mut person_count = 0usize;
        for (at, step) in script.iter().enumerate() {
            let stamp = format!("t{at}");
            match step {
                Step::Person => {
                    entries.push(said(&stamp, person(&format!("p{person_count}"))));
                    person_count += 1;
                }
                Step::Act => entries.push(said(
                    &stamp,
                    Turn::Act {
                        text: format!("act{at}"),
                        references: Vec::new(),
                    },
                )),
                Step::Call => entries.push(said(&stamp, calling_model("", "todo_show"))),
                Step::Complete => entries.push(said(&stamp, finished_model("ok"))),
                Step::Stop { stored_shape } => {
                    entries.push(Entry::Stopped {
                        at: crate::secretary::store::instant(&stamp),
                        by: "ada".to_owned(),
                    });
                    if *stored_shape {
                        entries.push(said(&stamp, finished_model("")));
                    }
                }
            }
        }
        entries
    }

    proptest::proptest! {
        /// Every person turn appears in exactly one closed batch: the batch standing immediately
        /// before the turn that closes it (an answer or a stop), and whatever is still open at the
        /// end is the unclosed tail.
        #[test]
        fn every_person_turn_is_closed_exactly_once(script in steps()) {
            let lines = script_lines(&script);
            let person_count = lines
                .iter()
                .filter(|line| matches!(line, Entry::Said { turn: Turn::Person { .. }, .. }))
                .count();
            let mut delivered: Vec<usize> = Vec::new();
            for (at, line) in lines.iter().enumerate().skip(1) {
                let closing = matches!(line, Entry::Stopped { .. })
                    || matches!(line, Entry::Said { turn: Turn::Model { calls, .. }, .. } if calls.is_empty());
                if closing {
                    // The batch as it stood right before the closing line joined it.
                    let before = Stored::of(&id(), &lines[..at]).expect("a conversation");
                    delivered.extend(
                        delta(&before).iter().filter_map(|(_, said)| person_index(&said.turn)),
                    );
                }
            }
            let entries = lines;

            // Whatever is still queued at the end is the current, not yet answered delta.
            let after = Stored::of(&id(), &entries).expect("a conversation");
            let still_queued: Vec<usize> = delta(&after).iter().filter_map(|(_, said)| person_index(&said.turn)).collect();

            let mut everywhere = delivered.clone();
            everywhere.extend(still_queued.iter().copied());
            everywhere.sort_unstable();

            let expected: Vec<usize> = (0..person_count).collect();
            proptest::prop_assert_eq!(everywhere, expected, "every person turn exactly once");

            // A delivered index never reappears in a later batch, being behind the boundary the
            // next round's own turn moves.
            let mut seen = std::collections::HashSet::new();
            for index in &delivered {
                proptest::prop_assert!(seen.insert(*index), "index {index} delivered twice");
            }
        }
    }

    proptest::proptest! {
        /// THE OWED LAW: a live turn is queued exactly while no live turn at or after it closes a
        /// round, an answer with no calls or a stop. So a stop needs no second write, and no
        /// reading between a stop's line and anything after it calls the stopped question queued.
        #[test]
        fn a_turn_is_owed_until_an_answer_or_a_stop_comes_after_it(script in steps()) {
            let lines = script_lines(&script);
            for upto in 1..=lines.len() {
                let stored = Stored::of(&id(), &lines[..upto]).expect("a conversation");
                let live = stored.live();
                let queued: Vec<usize> = delta(&stored).iter().map(|(index, _)| *index).collect();
                let owed: Vec<usize> = live
                    .iter()
                    .enumerate()
                    .filter(|(at, _)| !live[*at..].iter().any(|(index, _)| stored.closes(*index)))
                    .map(|(_, (index, _))| *index)
                    .collect();
                proptest::prop_assert_eq!(queued, owed);
            }
        }
    }

    /// A `Turn::Person`'s index, read back out of its own text (`"p<n>"`).
    fn person_index(turn: &Turn) -> Option<usize> {
        match turn {
            Turn::Person { text, .. } => text.strip_prefix('p').and_then(|n| n.parse().ok()),
            _ => None,
        }
    }

    fn tool(verb: &str) -> Turn {
        Turn::Tool {
            verb: verb.into(),
            answer: "ok".to_owned(),
            outcome: crate::secretary::outcome::Outcome::Answered,
            files: Vec::new(),
        }
    }

    /// A conversation of bare turns, for the window laws, which read nothing but the turns' shapes.
    fn of_turns(turns: Vec<Turn>) -> Stored {
        let mut entries = vec![opened()];
        entries.extend(turns.into_iter().map(|turn| said("t", turn)));
        stored(&entries)
    }

    /// One whole round with a tool call in it: four turns.
    fn round(n: usize) -> Vec<Turn> {
        vec![
            person(&format!("p{n}")),
            calling_model("looking", "search"),
            tool("search"),
            finished_model("done"),
        ]
    }

    /// A transcript shorter than the window is sent whole.
    #[test]
    fn a_short_transcript_is_sent_whole() {
        let stored = of_turns([round(1), round(2)].concat());
        assert_eq!(window_of(&stored, &stored.live(), 60).len(), 8);
    }

    /// A long one is cut at a round boundary, never mid-round: rounds of four in a window of six
    /// leave the fifth round alone, starting at its person turn.
    #[test]
    fn a_long_transcript_is_cut_at_the_first_round_boundary_inside_the_window() {
        let stored = of_turns((1..=5).flat_map(round).collect());
        let live = stored.live();
        let window = window_of(&stored, &live, 6);
        assert_eq!(window.len(), 4);
        assert_eq!(window[0].1.turn, person("p5"));
    }

    /// A `say` typed mid-round is not a boundary: a person turn between a call and its answer must
    /// not start the window.
    #[test]
    fn a_person_turn_inside_a_round_is_not_a_boundary() {
        let mut turns = round(1);
        turns.extend([
            person("p2"),
            calling_model("looking", "search"),
            person("typed while it ran"),
            tool("search"),
            finished_model("done"),
        ]);
        let stored = of_turns(turns);
        let live = stored.live();
        let window = window_of(&stored, &live, 3);
        assert_eq!(
            window[0].1.turn,
            person("p2"),
            "the whole second round, from its own first turn"
        );
        assert_eq!(window.len(), 5);
    }

    /// One round longer than the window is sent whole, from its start.
    #[test]
    fn a_round_longer_than_the_window_is_sent_whole() {
        let mut turns = vec![person("p1"), finished_model("hi")];
        turns.push(person("p2"));
        for _ in 0..10 {
            turns.push(calling_model("again", "search"));
            turns.push(tool("search"));
        }
        let stored = of_turns(turns);
        let live = stored.live();
        let window = window_of(&stored, &live, 6);
        assert_eq!(window[0].1.turn, person("p2"));
        assert_eq!(window.len(), 21);
    }

    /// The public window is the page's sixty.
    #[test]
    fn the_model_window_is_the_pages_default() {
        let stored = of_turns((1..=40).flat_map(round).collect());
        let live = stored.live();
        let window = window_of(&stored, &live, MODEL_WINDOW);
        assert!(window.len() <= MODEL_WINDOW);
        assert_eq!(window.len() % 4, 0, "whole rounds only");
    }
}
