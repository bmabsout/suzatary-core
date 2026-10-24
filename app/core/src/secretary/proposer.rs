//! Who asked for a proposal: a field of what the inbox stores, never read off the actor who wrote
//! it, since the box writes every proposal and a plugin is no actor of any store.

use crate::plugin::PluginName;
use crate::reference::ProposalRef;
use crate::secretary::acts::Act;
use crate::secretary::noticed::CONVERSATION;
use crate::secretary::store::Proposal;

/// Who proposed an act the owner may tap.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Proposer {
    /// The secretary, in a round of one of its conversations: the box's own proposals are its
    /// rounds too.
    Secretary,
    /// An `actor` plugin, from what its store holds: it proposes and never performs.
    Plugin(PluginName),
    /// The box, with no model asked: what a reading of the deployment calls for (a declared
    /// secret that does not resolve). A function of that reading, so the same reading asks for
    /// the same proposal, whenever it is read.
    Box,
}

impl Proposer {
    /// Who asked, as a card and the Inbox's row carry it: the box's Secret card in the setup
    /// conversation is a setup card, its every other card the box's own.
    #[must_use]
    pub fn on_wire(&self, proposal: &Proposal, asked: &ProposalRef) -> crate::wire::Asker {
        match self {
            Proposer::Secretary => crate::wire::Asker::Secretary {},
            Proposer::Plugin(name) => crate::wire::Asker::Plugin {
                name: name.to_string(),
            },
            Proposer::Box => match proposal {
                Proposal::Act(Act::Secret { .. } | Act::GoogleClient { .. }) => {
                    if asked.conversation == *CONVERSATION {
                        crate::wire::Asker::Setup {}
                    } else {
                        crate::wire::Asker::Box {}
                    }
                }
                // Every other act the box asks for is its own, and each is named here, so a new
                // act decides which it is rather than falling to one.
                Proposal::Act(
                    Act::Auth { .. }
                    | Act::AuthSend { .. }
                    | Act::Web { .. }
                    | Act::Patch { .. }
                    | Act::DriveRead { .. }
                    | Act::CodingSession { .. }
                    | Act::Device(_)
                    | Act::ClaudeSignIn { .. }
                    | Act::SignInCode { .. }
                    | Act::UnlockStore { .. }
                    | Act::Connect { .. }
                    | Act::Disconnect { .. }
                    | Act::Grant { .. }
                    | Act::Erase { .. }
                    | Act::GoogleSignIn { .. }
                    | Act::CalendarEvent { .. }
                    | Act::Configure { .. },
                )
                | Proposal::Email(_)
                | Proposal::Forward(_)
                | Proposal::Changes(_)
                | Proposal::Remember { .. }
                | Proposal::Decide(_) => crate::wire::Asker::Box {},
            },
        }
    }

    /// Who asked, as a sentence names them: what a trailer on a tapped act says.
    #[must_use]
    pub fn named(&self) -> String {
        match self {
            Proposer::Secretary => "the Suzatary secretary".to_owned(),
            Proposer::Plugin(name) => format!("the {name} plugin"),
            Proposer::Box => "the box".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tapped act names who asked: the secretary by its name, a plugin by its own.
    #[test]
    fn who_asked_is_named_from_the_proposer() {
        let plugin = Proposer::Plugin(PluginName::parse("due").expect("a name"));
        assert_eq!(Proposer::Secretary.named(), "the Suzatary secretary");
        assert_eq!(plugin.named(), "the due plugin");
        assert_eq!(Proposer::Box.named(), "the box");
    }

    /// Only the box's Secret card in the setup conversation is a setup card: the box's other
    /// cards, and a Secret card asked anywhere else, are its own and stay among the work.
    #[test]
    fn only_a_secret_asked_in_setup_is_a_setup_card() {
        let secret = Proposal::Act(Act::Secret {
            name: "NTFY_TOPIC".to_owned(),
            why: "the ntfy notification channel reads it".to_owned(),
        });
        let unlock = Proposal::Act(Act::UnlockStore { after: None });
        let at = |conversation: &str| ProposalRef {
            conversation: crate::secretary::store::ConversationId::new(conversation)
                .expect("a slug"),
            index: 0,
        };
        assert_eq!(
            Proposer::Box.on_wire(&secret, &at("setup")),
            crate::wire::Asker::Setup {}
        );
        assert_eq!(
            Proposer::Box.on_wire(&secret, &at("2026-10-13-1")),
            crate::wire::Asker::Box {}
        );
        assert_eq!(
            Proposer::Box.on_wire(&unlock, &at("setup")),
            crate::wire::Asker::Box {}
        );
        assert_eq!(
            Proposer::Plugin(PluginName::parse("due").expect("a name"))
                .on_wire(&secret, &at("setup")),
            crate::wire::Asker::Plugin {
                name: "due".to_owned()
            }
        );
    }
}
