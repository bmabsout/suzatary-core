//! The one-time carry from conversations to the stream: each conversation's rounds and notes
//! tagged with its title, planned from the conversations and the tags already read, so a second
//! run plans nothing.

use prodrome_core::literal::Datetime;

use crate::secretary::SecretaryVerb;
use crate::secretary::outcome::Outcome;
use crate::secretary::store::{Kind, Stored};
use crate::secretary::tags::membership_event::MembershipEvent;
use crate::secretary::tags::reading::Reading;
use crate::secretary::tags::tag::Tag;
use crate::secretary::tags::target::Target;
use crate::secretary::turn_shape::{TurnShape, shape};
use crate::secretary::words::Words;

/// What the carry does to one conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carrying {
    /// Its rounds and notes get `tag`; the counts are the memberships this run writes.
    Tagged {
        conversation: crate::secretary::store::ConversationId,
        tag: Tag,
        rounds: usize,
        notes: usize,
    },
    /// The default conversation gets no tag; the counts are the rounds and notes it holds.
    Default {
        conversation: crate::secretary::store::ConversationId,
        rounds: usize,
        notes: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carry {
    /// One per conversation, in the order given.
    pub conversations: Vec<Carrying>,
    /// Every membership to write, as the assistant, at one instant.
    pub events: Vec<MembershipEvent>,
}

/// The carry of `conversations` over the tags `reading` already holds: a membership it reads on
/// plans nothing, so running it again plans nothing.
#[must_use]
pub fn plan(conversations: &[Stored], reading: &Reading, at: Datetime) -> Carry {
    let actor = crate::secretary::writes_as();
    let mut events = Vec::new();
    let carried = conversations
        .iter()
        .map(|stored| {
            let (rounds, notes) = targets(stored);
            let conversation = stored.id.clone();
            let tag = match stored.kind {
                Kind::Default => {
                    return Carrying::Default {
                        conversation,
                        rounds: rounds.len(),
                        notes: notes.len(),
                    };
                }
                Kind::Topic => titled(stored),
                Kind::Review => daily(stored),
            };
            let mut written = |targets: Vec<Target>| {
                let planned: Vec<MembershipEvent> = targets
                    .iter()
                    .filter_map(|target| reading.tag(target, &tag, at, &actor))
                    .collect();
                let count = planned.len();
                events.extend(planned);
                count
            };
            let rounds = written(rounds);
            let notes = written(notes);
            Carrying::Tagged {
                conversation,
                tag,
                rounds,
                notes,
            }
        })
        .collect();
    Carry {
        conversations: carried,
        events,
    }
}

/// A topic's title slugged, or its id where the title has no letter or digit or was erased.
fn titled(stored: &Stored) -> Tag {
    match &stored.title {
        Words::Kept { kept } => Tag::slug(kept),
        Words::Gone { .. } => None,
    }
    .unwrap_or_else(|| Tag::of_conversation(&stored.id))
}

/// A review's daily word, `review` or `sweep`; a review conversation whose id names no daily
/// round is tagged by its title, as a topic is.
fn daily(stored: &Stored) -> Tag {
    match stored.id.daily_round() {
        Some(daily) => Tag::new(daily.word()).expect("a daily word is a tag"),
        None => titled(stored),
    }
}

/// The live rounds, each named by the person's or act's turn that opened it (or the first live
/// turn), and the live notes: a `draft_note` call writes the note and then its own answer, both
/// answered turns of that verb, so a note is the first of each such pair.
fn targets(stored: &Stored) -> (Vec<Target>, Vec<Target>) {
    let live = stored.live();
    let shapes: Vec<(usize, TurnShape)> = live
        .iter()
        .map(|(index, said)| (*index, shape(&said.turn)))
        .collect();
    let conversation = &stored.id;
    let rounds = shapes
        .iter()
        .enumerate()
        .filter(|(at, (_, shape))| {
            *at == 0 || matches!(shape, TurnShape::Person { .. } | TurnShape::Act { .. })
        })
        .map(|(_, (turn, _))| Target::Round {
            conversation: conversation.clone(),
            turn: *turn,
        })
        .collect();
    let drafted = |shape: &TurnShape| {
        matches!(shape, TurnShape::Tool { verb, outcome: Outcome::Answered, .. }
            if *verb == SecretaryVerb::DraftNote.into())
    };
    let mut notes = Vec::new();
    let mut at = 0;
    while at < shapes.len() {
        if drafted(&shapes[at].1) && shapes.get(at + 1).is_some_and(|(_, next)| drafted(next)) {
            notes.push(Target::Note {
                conversation: conversation.clone(),
                turn: shapes[at].0,
            });
            at += 2;
        } else {
            at += 1;
        }
    }
    (rounds, notes)
}
