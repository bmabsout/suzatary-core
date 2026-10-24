//! The rounds and notes of one conversation, as targets a topic is put on.

use crate::secretary::SecretaryVerb;
use crate::secretary::outcome::Outcome;
use crate::secretary::store::{ConversationId, Said, Stored};
use crate::secretary::topics::target::Target;
use crate::secretary::turn_shape::{TurnShape, shape};

/// The most topics one round or note holds.
pub const MAX_TOPICS: usize = 3;

/// One live round: the turn that opened it, and its turns with their indices, oldest first.
#[derive(Debug, Clone)]
pub struct Round<'a> {
    pub conversation: &'a ConversationId,
    pub turn: usize,
    pub turns: Vec<(usize, &'a Said)>,
}

/// The live rounds, each named by the person's or act's turn that opened it (or the first live
/// turn), each holding the turns up to the next round's.
#[must_use]
pub fn rounds(stored: &Stored) -> Vec<Round<'_>> {
    let live = stored.live();
    let Some((&first, rest)) = live.split_first() else {
        return Vec::new();
    };
    let open = |(turn, said)| Round {
        conversation: &stored.id,
        turn,
        turns: vec![(turn, said)],
    };
    let mut rounds = vec![open(first)];
    for &(turn, said) in rest {
        if matches!(
            shape(&said.turn),
            TurnShape::Person { .. } | TurnShape::Act { .. }
        ) {
            rounds.push(open((turn, said)));
        } else {
            let last = rounds.len() - 1;
            rounds[last].turns.push((turn, said));
        }
    }
    rounds
}

impl Round<'_> {
    #[must_use]
    pub fn target(&self) -> Target {
        Target::Round {
            conversation: self.conversation.clone(),
            turn: self.turn,
        }
    }

    /// Its notes.
    #[must_use]
    pub fn notes(&self) -> Vec<Target> {
        notes_in(self.conversation, &self.turns)
    }
}

/// The notes among `turns` of `conversation`: a `draft_note` call writes the note and then its
/// own answer, both answered turns of that verb, so a note is the first of each such pair.
#[must_use]
pub fn notes_in(conversation: &ConversationId, turns: &[(usize, &Said)]) -> Vec<Target> {
    let drafted = |said: &Said| {
        matches!(shape(&said.turn), TurnShape::Tool { verb, outcome: Outcome::Answered, .. }
            if verb == SecretaryVerb::DraftNote.into())
    };
    let mut notes = Vec::new();
    let mut at = 0;
    while at < turns.len() {
        let (turn, said) = turns[at];
        if drafted(said) && turns.get(at + 1).is_some_and(|(_, next)| drafted(next)) {
            notes.push(Target::Note {
                conversation: conversation.clone(),
                turn,
            });
            at += 2;
        } else {
            at += 1;
        }
    }
    notes
}
