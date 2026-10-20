//! A proposal filed: asked once in the inbox, by its content, and drawn as a card in one
//! conversation. The secretary's verbs and a plugin's offers file through the one function here,
//! so a plugin's proposal is the inbox's proposal, tapped by the same route.

use crate::capability::Datetime;
use crate::plugin::{Offer, PluginName};
use crate::reference::ProposalRef;
use crate::refusal::Refusal;
use crate::secretary::inbox::{self, ProposalId};
use crate::secretary::inbox_event::InboxEvent;
use crate::secretary::proposer::Proposer;
use crate::secretary::store::{ConversationId, Entry, Kind, Proposal, Reasons};
use crate::secretary::transcript;
use crate::store::{Prodrome, StoreError};

/// Where a proposal is drawn: its card's index in the conversation it was filed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filed {
    /// Drawn by this filing.
    Drawn(usize),
    /// Already drawn there: the same act asked again is the card already here, and nothing was
    /// written.
    Already(usize),
}

impl Filed {
    /// The card's index, which a tap names.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Filed::Drawn(index) | Filed::Already(index) => index,
        }
    }
}

/// What is filed, by whom, in which conversation: everything but the instant and the stores.
#[derive(Debug, Clone)]
pub struct Filing<'a> {
    /// The conversation it is drawn in.
    pub conversation: &'a ConversationId,
    /// Who asks for it.
    pub proposer: Proposer,
    /// The act.
    pub proposal: Proposal,
    /// Why, and on what.
    pub reasons: Reasons,
    /// The todo it serves.
    pub serves: Option<crate::reference::TodoRef>,
    /// The card it replaces, recorded on the line.
    pub revises: Option<usize>,
}

/// File `filing` at `at`: the inbox asked first ([`inbox::ask`], which writes nothing for an act
/// it holds), then a line naming the object, unless the conversation already draws it.
///
/// Under the inbox's one hold, taken before the conversations' as the tap takes them, so the
/// card's index is read and its line written with no other filing between.
///
/// # Errors
/// A conversation that cannot be read, and the stores' failures to append.
pub fn file(
    conversations: crate::secretary::conversations::Conversations<'_>,
    inbox: &dyn Prodrome<InboxEvent>,
    at: Datetime,
    filing: Filing<'_>,
) -> Result<(Filed, ProposalId), Refusal> {
    let _held = inbox.locked()?;
    file_held(conversations, inbox, at, filing)
}

/// [`file()`], under the inbox's hold the caller has taken.
fn file_held(
    conversations: crate::secretary::conversations::Conversations<'_>,
    inbox: &dyn Prodrome<InboxEvent>,
    at: Datetime,
    filing: Filing<'_>,
) -> Result<(Filed, ProposalId), Refusal> {
    let Filing {
        conversation,
        proposer,
        proposal,
        reasons,
        serves,
        revises,
    } = filing;
    let stored = super::read(conversations, conversation)?;
    let index = stored.proposals.len();
    let asked = inbox::Asked {
        proposer,
        proposal,
        reasons,
        asked: ProposalRef {
            conversation: conversation.clone(),
            index,
        },
        price: serves.map_or_else(inbox::Price::unpriced, inbox::Price::Serves),
    };
    let object = inbox::ask_held(inbox, asked, &super::writes_as(), at)?;
    if let Some(already) = stored
        .proposals
        .iter()
        .position(|asking| asking.object == object)
    {
        return Ok((Filed::Already(already), object));
    }
    transcript::push(
        conversations,
        conversation,
        &Entry::Proposed {
            at,
            object: object.clone(),
            revises,
        },
    )?;
    Ok((Filed::Drawn(index), object))
}

/// The conversation a plugin's offers are drawn in: one per plugin, the box's own.
///
/// # Panics
/// Never: `plugin-` and a plugin's name, at most 32 of `a-z0-9`, is a conversation id.
#[must_use]
pub fn conversation_of(plugin: &PluginName) -> ConversationId {
    ConversationId::new(format!("plugin-{plugin}")).expect("a plugin's name makes a slug")
}

/// The most proposals a plugin may have waiting for the owner: past it, an offer of a new act
/// files nothing until he answers one, however often the plugin asks.
pub const MAX_STANDING: usize = 5;

/// What filing an actor's offer came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offered {
    /// Filed as the plugin's, drawn in its conversation by this filing or an earlier one.
    Filed(Filed, ProposalId),
    /// The Inbox already holds the act, asked by someone else, named: nothing is drawn, so no
    /// card says a plugin asked for what another did, and the sync says who did.
    Elsewhere(ProposalId, Proposer),
    /// A new act, while the plugin has [`MAX_STANDING`] waiting: nothing is filed.
    Waiting,
}

/// File an actor's admitted offer as its plugin's, in [`conversation_of`] that plugin, opened
/// the first time. The same act offered again files nothing: the inbox's identity is the act.
///
/// # Errors
/// As [`file()`].
pub fn offer(
    conversations: crate::secretary::conversations::Conversations<'_>,
    inbox: &dyn Prodrome<InboxEvent>,
    at: Datetime,
    offer: &Offer,
) -> Result<Offered, Refusal> {
    let plugin = offer.plugin();
    let conversation = conversation_of(plugin);
    let opened = Entry::Opened {
        at,
        title: crate::secretary::words::Words::kept(format!("Proposed by the {plugin} plugin")),
        model: format!("the {plugin} plugin"),
        kind: Kind::Review,
    };
    match transcript::start(conversations, &conversation, &opened) {
        Ok(()) | Err(StoreError::Taken { .. }) => {}
        Err(failed) => return Err(failed.into()),
    }
    let _held = inbox.locked()?;
    let fold = inbox::fold(inbox)?;
    // A plugin may offer only acts that are their content alone, so the act is the identity.
    let proposal = offer.proposal();
    match fold
        .objects()
        .find(|object| object.asked.proposal == proposal)
    {
        Some(held) if held.asked.proposer != offer.proposer() => {
            return Ok(Offered::Elsewhere(
                held.id.clone(),
                held.asked.proposer.clone(),
            ));
        }
        Some(_) => {}
        None => {
            let waiting = fold
                .objects()
                .filter(|object| object.asked.proposer == offer.proposer() && object.reading.open())
                .count();
            if waiting >= MAX_STANDING {
                return Ok(Offered::Waiting);
            }
        }
    }
    file_held(
        conversations,
        inbox,
        at,
        Filing {
            conversation: &conversation,
            proposer: offer.proposer(),
            proposal,
            reasons: offer.reasons().clone(),
            serves: None,
            revises: None,
        },
    )
    .map(|(filed, object)| Offered::Filed(filed, object))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemory;

    fn at(minute: u32) -> Datetime {
        Datetime::new(2026, 10, 5, 9, minute, 0, 0).expect("an instant")
    }

    fn offered(plugin: &str, why: &str) -> Offer {
        crate::plugin::propose(
            &PluginName::parse(plugin).expect("a name"),
            "Changes(chain='life', acts=(Complete(todo='dentist', note=''),))",
            why,
            &["briefing:2026-10-05".to_owned()],
        )
        .expect("admitted")
    }

    /// An offer is the inbox's proposal, filed as its plugin's and drawn in its plugin's
    /// conversation; the same act offered again, at another instant and for another reason,
    /// writes nothing, and another plugin's offer of it is the same proposal, drawn nowhere else.
    #[test]
    fn an_offer_is_filed_once_as_its_plugins() {
        let conversations = crate::secretary::conversations::InMemory::default();
        let inbox_store = InMemory::<InboxEvent>::default();
        let first = offer(
            conversations.conversations(),
            &inbox_store,
            at(0),
            &offered("dueday", "on the calendar"),
        )
        .expect("filed");
        let Offered::Filed(filed, object) = first else {
            panic!("filed: {first:?}");
        };
        assert_eq!(filed, Filed::Drawn(0));
        let fold = inbox::fold(&inbox_store).expect("reads");
        let held = fold.get(&object).expect("asked");
        assert_eq!(
            held.asked.proposer,
            Proposer::Plugin(PluginName::parse("dueday").expect("a name"))
        );
        assert_eq!(
            held.asked.asked.to_string(),
            "proposal:plugin-dueday/0",
            "drawn as a card a tap names"
        );
        let stored = super::super::read(
            conversations.conversations(),
            &conversation_of(&PluginName::parse("dueday").expect("a name")),
        )
        .expect("opened");
        assert_eq!(stored.kind, Kind::Review);
        assert_eq!(stored.proposals.len(), 1);

        let written = (
            conversations.lines.objects().expect("reads").len(),
            inbox_store.objects().expect("reads").len(),
        );
        assert_eq!(
            offer(
                conversations.conversations(),
                &inbox_store,
                at(15),
                &offered("dueday", "again")
            ),
            Ok(Offered::Filed(Filed::Already(0), object.clone()))
        );
        assert_eq!(
            (
                conversations.lines.objects().expect("reads").len(),
                inbox_store.objects().expect("reads").len(),
            ),
            written,
            "nothing written"
        );
        assert_eq!(
            offer(
                conversations.conversations(),
                &inbox_store,
                at(20),
                &offered("other", "mine too")
            ),
            Ok(Offered::Elsewhere(
                object,
                Proposer::Plugin(PluginName::parse("dueday").expect("a name"))
            )),
            "one act, one proposal, and no card says another plugin asked for it"
        );
        assert!(
            super::super::read(
                conversations.conversations(),
                &conversation_of(&PluginName::parse("other").expect("a name")),
            )
            .expect("opened")
            .proposals
            .is_empty()
        );
    }

    /// A plugin with [`MAX_STANDING`] proposals waiting files no new act, however many it
    /// mints, until the owner answers one; an act it already filed still answers its card.
    #[test]
    fn a_plugin_with_its_proposals_waiting_files_nothing_new() {
        let conversations = crate::secretary::conversations::InMemory::default();
        let inbox_store = InMemory::<InboxEvent>::default();
        let minted = |n: usize| {
            crate::plugin::propose(
                &PluginName::parse("clock").expect("a name"),
                &format!("Changes(chain='life', acts=(Complete(todo='x{n}', note=''),))"),
                "minted",
                &["briefing:2026-10-05".to_owned()],
            )
            .expect("admitted")
        };
        let filed: Vec<Offered> = (0..MAX_STANDING + 3)
            .map(|n| {
                offer(
                    conversations.conversations(),
                    &inbox_store,
                    at(0),
                    &minted(n),
                )
                .expect("files")
            })
            .collect();
        assert!(
            filed[..MAX_STANDING]
                .iter()
                .all(|one| matches!(one, Offered::Filed(Filed::Drawn(_), _)))
        );
        assert!(
            filed[MAX_STANDING..]
                .iter()
                .all(|one| matches!(one, Offered::Waiting))
        );
        assert!(matches!(
            offer(
                conversations.conversations(),
                &inbox_store,
                at(1),
                &minted(0)
            ),
            Ok(Offered::Filed(Filed::Already(0), _))
        ));
        let Offered::Filed(_, first) = &filed[0] else {
            unreachable!("filed above");
        };
        let owner = prodrome_core::event::Actor::new("ada").expect("an actor");
        inbox::dismiss(&inbox_store, first, &owner, at(2), "not now").expect("dismissed");
        assert!(matches!(
            offer(
                conversations.conversations(),
                &inbox_store,
                at(3),
                &minted(MAX_STANDING)
            ),
            Ok(Offered::Filed(Filed::Drawn(_), _))
        ));
        assert!(matches!(
            offer(
                conversations.conversations(),
                &inbox_store,
                at(4),
                &minted(MAX_STANDING + 1)
            ),
            Ok(Offered::Waiting)
        ));
    }
}
