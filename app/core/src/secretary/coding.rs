//! A coding session's lines in its conversation: each change is an [`Entry::Coding`] line
//! and a [`Turn::Act`], so the secretary reads what became of the session it proposed.

use crate::capability::{Datetime, Turn};
use crate::coding::{Fired, Seen, Session, Step};
use crate::reference::{ProposalRef, Reference};
use crate::refusal::Refusal;
use crate::secretary::store::{ConversationId, Entry, stamp};

/// One session a poll moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advanced {
    /// Its conversation.
    pub conversation: ConversationId,
    /// The proposal that started it.
    pub index: usize,
    /// Where it now stands.
    pub session: Session,
}

/// Every session still waiting on a branch: fired or reported.
///
/// # Errors
/// The store's own failures.
pub fn waiting(
    conversations: crate::secretary::conversations::Conversations<'_>,
) -> Result<Vec<Advanced>, Refusal> {
    Ok(super::list(conversations)?
        .into_iter()
        .flat_map(|stored| {
            stored
                .coding
                .into_iter()
                .filter(|(_, session)| {
                    matches!(session, Session::Fired { .. } | Session::Reported { .. })
                })
                .map(move |(index, session)| Advanced {
                    conversation: stored.id.clone(),
                    index,
                    session,
                })
        })
        .collect())
}

/// The tap fired a session: its first line, and the act the secretary reads.
///
/// # Errors
/// The store's own failures.
pub fn fired(
    conversations: crate::secretary::conversations::Conversations<'_>,
    id: &ConversationId,
    index: usize,
    fired: Fired,
    at: Datetime,
) -> Result<Session, Refusal> {
    let text = format!(
        "the coding session of proposal {index} started: {}",
        fired.url
    );
    let session = Session::Fired {
        id: fired.id,
        url: fired.url,
        at: stamp(at),
    };
    written(conversations, id, index, &session, at, text)?;
    Ok(session)
}

/// Steps every waiting session by what `seen` shows and answers the sessions it moved.
///
/// # Errors
/// The store's own failures.
pub fn advance(
    conversations: crate::secretary::conversations::Conversations<'_>,
    seen: &[Seen],
    now: Datetime,
) -> Result<Vec<Advanced>, Refusal> {
    let mut moved = Vec::new();
    for Advanced {
        conversation,
        index,
        session,
    } in waiting(conversations)?
    {
        let Some(step) = crate::coding::next(&session, seen) else {
            continue;
        };
        let text = match &step {
            Step::Report { branch, report } => format!(
                "the coding session of proposal {index} reported on {branch}: {}",
                report.path.as_str()
            ),
            Step::Merge { sha } => format!(
                "the coding session of proposal {index} is merged at {}",
                sha.short()
            ),
            Step::Drop { reason } => {
                format!("the coding session of proposal {index} was dropped: {reason}")
            }
        };
        let next = session
            .step(step)
            .map_err(|refused| Refusal::internal(refused.to_string()))?;
        written(conversations, &conversation, index, &next, now, text)?;
        moved.push(Advanced {
            conversation,
            index,
            session: next,
        });
    }
    Ok(moved)
}

/// The owner let a session go, with his reason, as `by` (the gate's actor).
///
/// # Errors
/// An empty reason, no session for that proposal, one already ended, or the store's failures.
pub fn dropped(
    conversations: crate::secretary::conversations::Conversations<'_>,
    id: &ConversationId,
    index: usize,
    by: &prodrome_core::event::Actor,
    reason: &str,
    at: Datetime,
) -> Result<Session, Refusal> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(Refusal::bad_request(
            "reason says why the session is dropped",
        ));
    }
    let session = started(conversations, id, index)?
        .step(Step::Drop {
            reason: reason.to_owned(),
        })
        .map_err(|refused| Refusal::conflict(refused.to_string()))?;
    written(
        conversations,
        id,
        index,
        &session,
        at,
        format!(
            "{} dropped the coding session of proposal {index}: {reason}",
            crate::secretary::acts::named(by.as_str())
        ),
    )?;
    Ok(session)
}

/// The session proposal `index` started, as it stands.
///
/// # Errors
/// [`Refusal::not_found`] when that proposal started none.
pub fn started(
    conversations: crate::secretary::conversations::Conversations<'_>,
    id: &ConversationId,
    index: usize,
) -> Result<Session, Refusal> {
    super::read(conversations, id)?
        .coding
        .remove(&index)
        .ok_or_else(|| {
            Refusal::not_found(format!(
                "proposal {index} of {id} started no coding session"
            ))
        })
}

/// One session line and the act turn beside it.
fn written(
    conversations: crate::secretary::conversations::Conversations<'_>,
    id: &ConversationId,
    index: usize,
    session: &Session,
    at: Datetime,
    text: String,
) -> Result<(), Refusal> {
    super::append(
        conversations,
        id,
        &Entry::Coding {
            at: stamp(at),
            proposal: index,
            session: session.clone(),
        },
    )?;
    super::append(
        conversations,
        id,
        &Entry::Said {
            at: stamp(at),
            turn: Turn::Act {
                text,
                references: vec![Reference::Proposal(ProposalRef {
                    conversation: id.clone(),
                    index,
                })],
            },
            by: None,
        },
    )
}
