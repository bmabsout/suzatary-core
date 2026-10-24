//! The door an actor's proposal comes through: a change to a todo chain, written in the Inbox's
//! closed vocabulary, why, and what it rests on, filed as the plugin's and performed only by the
//! owner's tap.

use prodrome_core::literal::{Table, parse_literal, print_literal};

use super::{PluginName, Said};
use crate::literal::Literal as _;
use crate::reference::Reference;
use crate::secretary::act_vocabulary;
use crate::secretary::changes::{CHANGES, Changeset};
use crate::secretary::proposer::Proposer;
use crate::secretary::store::{NoReasons, Proposal, Reasons};

/// The most bytes a proposal's reason may be.
pub const MAX_WHY: usize = 2 * 1024;

/// The most proposals one decision may make; the rest are refused unread, by their count.
pub const MAX_OFFERS: usize = 8;

/// The most bytes a whole offer may be, its act, its reason and its sources together: a
/// changeset of a few todos and what it rests on.
pub const MAX_OFFER: usize = 24 * 1024;

/// The most sources an offer may rest on: a card lists them all.
pub const MAX_SOURCES: usize = 8;

/// The most bytes one source may be; every reference's text form is far shorter.
pub const MAX_SOURCE: usize = 256;

/// One act an actor asks the owner to tap, admitted: a change to a todo chain, the one act a
/// plugin may ask for (any other is not expressible past the door), and its reasons. Nothing
/// here performs it; [`Offer::proposer`] is who the Inbox files it as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    plugin: PluginName,
    act: Changeset,
    reasons: Reasons,
}

impl Offer {
    /// The plugin that offers it.
    #[must_use]
    pub fn plugin(&self) -> &PluginName {
        &self.plugin
    }

    /// Who asks for it, as the Inbox files it.
    #[must_use]
    pub fn proposer(&self) -> Proposer {
        Proposer::Plugin(self.plugin.clone())
    }

    /// The change, as the owner's tap would apply it.
    #[must_use]
    pub fn act(&self) -> &Changeset {
        &self.act
    }

    /// The change as the Inbox holds a proposal.
    #[must_use]
    pub fn proposal(&self) -> Proposal {
        Proposal::Changes(self.act.clone())
    }

    /// Why the plugin asks, and what it rests on.
    #[must_use]
    pub fn reasons(&self) -> &Reasons {
        &self.reasons
    }

    /// Its act's print, which is its identity in the Inbox: one act offered twice is one offer.
    #[must_use]
    pub fn key(&self) -> String {
        print_literal(&self.proposal().to_value())
    }
}

/// An actor's proposal that was not admitted, naming the plugin that made it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the {plugin} plugin proposed {refusal}")]
pub struct Refused {
    /// The actor.
    pub plugin: PluginName,
    /// Why.
    pub refusal: ProposalRefusal,
}

/// Why a proposal was not admitted. What it repeats of the plugin's act or of a refusal of it is
/// [`Said`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProposalRefusal {
    /// Not a literal of the Inbox's act vocabulary: another constructor, a field it does not
    /// have, a store's event, or no literal at all.
    #[error("something outside the Inbox's acts: {0}")]
    Vocabulary(Said),
    /// A literal of the vocabulary that is no act the Inbox holds.
    #[error("no act the Inbox holds: {0}")]
    NotAnAct(Said),
    /// An act not printed as itself, so its print is not its name.
    #[error("an act not written as itself: it prints as {0}")]
    NotAsWritten(Said),
    /// An act the Inbox holds that a plugin may not yet ask for, by its constructor: only a
    /// change to a todo chain, checked by the box before it is filed and undone by a later
    /// write. A decision, mail, a pull request, a coding session or a box act from a plugin
    /// waits for a stage that gives it the checks the secretary's verbs run.
    #[error("an act of {0}, which a plugin may not ask for")]
    NotOffered(Said),
    /// Proposals past [`MAX_OFFERS`] in one decision, by their count: not read.
    #[error("{0} more proposals than one decision may make, which were not read")]
    Beyond(usize),
    /// A change its chain would refuse as it stands, in the chain's words: checked by the box
    /// before it is filed, as the secretary's are.
    #[error("a change its chain refuses: {0}")]
    Unbuilt(Said),
    /// An offer, act, reason and sources together, longer than this host reads.
    #[error("an offer of {0} bytes, longer than this host reads")]
    TooLarge(usize),
    /// More sources than a card lists.
    #[error("an act resting on {0} sources, more than a card lists")]
    TooManySources(usize),
    /// A source longer than any reference.
    #[error("an act resting on a source of {0} bytes, longer than any reference")]
    SourceTooLong(usize),
    /// It gave no reason, or rests on nothing: a card always says what it is for and on what.
    #[error("an act without its reasons: {0}")]
    Reasons(NoReasons),
    /// Its reason is longer than a card shows.
    #[error("an act with a reason of {0} bytes, longer than this host reads")]
    WhyTooLong(usize),
    /// A source that is not a reference.
    #[error("an act resting on {0}, which is no reference")]
    Source(Said),
}

/// Admit one proposal `plugin` made: `act` a `Changes(...)` literal of the Inbox's closed act
/// vocabulary printed as itself, `why` a reason that fits a card and `sources` at least one
/// reference.
///
/// # Errors
/// [`Refused`], naming `plugin` and the first rule the proposal broke.
pub fn propose(
    plugin: &PluginName,
    act: &str,
    why: &str,
    sources: &[String],
) -> Result<Offer, Refused> {
    admitted(act, why, sources)
        .map(|(act, reasons)| Offer {
            plugin: plugin.clone(),
            act,
            reasons,
        })
        .map_err(|refusal| Refused {
            plugin: plugin.clone(),
            refusal,
        })
}

/// The offer's sizes, every one checked here before any of it is read: what is stored of an
/// offer is bounded by these, whatever a plugin returns.
fn bounded(act: &str, why: &str, sources: &[String]) -> Result<(), ProposalRefusal> {
    let whole = act.len() + why.len() + sources.iter().map(String::len).sum::<usize>();
    if whole > MAX_OFFER {
        return Err(ProposalRefusal::TooLarge(whole));
    }
    if why.len() > MAX_WHY {
        return Err(ProposalRefusal::WhyTooLong(why.len()));
    }
    if sources.len() > MAX_SOURCES {
        return Err(ProposalRefusal::TooManySources(sources.len()));
    }
    match sources.iter().map(String::len).max() {
        Some(longest) if longest > MAX_SOURCE => Err(ProposalRefusal::SourceTooLong(longest)),
        _ => Ok(()),
    }
}

fn admitted(
    act: &str,
    why: &str,
    sources: &[String],
) -> Result<(Changeset, Reasons), ProposalRefusal> {
    bounded(act, why, sources)?;
    let value = parse_literal(act, &Table(act_vocabulary::SIGNATURES))
        .map_err(|failed| ProposalRefusal::Vocabulary(Said::clip(failed.to_string())))?;
    let printed = print_literal(&value);
    if printed != act {
        return Err(ProposalRefusal::NotAsWritten(Said::clip(printed)));
    }
    let constructor = crate::literal::arm(&value)
        .map_err(|failed| ProposalRefusal::NotAnAct(Said::clip(failed.to_string())))?;
    if constructor != CHANGES.0 {
        return Err(ProposalRefusal::NotOffered(Said::clip(constructor)));
    }
    let act = Changeset::from_value(&value)
        .map_err(|failed| ProposalRefusal::NotAnAct(Said::clip(failed.to_string())))?;
    let sources = sources
        .iter()
        .map(|source| {
            source
                .parse::<Reference>()
                .map_err(|_| ProposalRefusal::Source(Said::clip(source.as_str())))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let reasons = Reasons::new(why, sources).map_err(ProposalRefusal::Reasons)?;
    Ok((act, reasons))
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPLETE: &str = "Changes(chain='life', acts=(Complete(todo='dentist', note=''),))";

    fn proposed(act: &str, why: &str, sources: &[&str]) -> Result<Offer, ProposalRefusal> {
        let plugin = PluginName::parse("dueday").expect("a name");
        let sources: Vec<String> = sources.iter().map(|&source| source.to_owned()).collect();
        propose(&plugin, act, why, &sources).map_err(|refused| {
            assert_eq!(refused.plugin, plugin, "a refusal names the plugin");
            refused.refusal
        })
    }

    #[test]
    fn anything_outside_the_vocabulary_or_not_written_as_itself_is_refused() {
        let why = "why";
        let on = ["briefing:2026-10-05"];
        for outside in [
            "Send(to='mira@example.com')",
            "Acted(proposal='p0', at=datetime(2026, 10, 5, 9, 0, 0), actor='ada', what='sent')",
            &COMPLETE.replace("acts=", "actor='ada', acts="),
            "not a literal",
        ] {
            assert!(
                matches!(
                    proposed(outside, why, &on),
                    Err(ProposalRefusal::Vocabulary(_))
                ),
                "{outside}"
            );
        }
        assert!(matches!(
            proposed(&COMPLETE.replace(", ", ","), why, &on),
            Err(ProposalRefusal::NotAsWritten(_))
        ));
        assert!(matches!(
            proposed("Changes(chain='life', acts=())", why, &on),
            Err(ProposalRefusal::NotAnAct(_))
        ));
    }

    #[test]
    fn only_a_change_to_a_chain_is_offered() {
        let on = ["briefing:2026-10-05"];
        for not_yet in [
            "Decide(id='plugins', title='t', body='b', rationale='r', supersedes=())".to_owned(),
            "Auth(account='ada@example.com')".to_owned(),
            format!("Remember(text='{}', kind='owner')", "a".repeat(64)),
            "Email(account='ada@example.com', to=('mira@example.com',), cc=(), subject='s', \
             body='b', attachments=())"
                .to_owned(),
        ] {
            assert!(
                matches!(
                    proposed(&not_yet, "why", &on),
                    Err(ProposalRefusal::NotOffered(_))
                ),
                "{not_yet}"
            );
        }
    }

    /// Every size of an offer is bounded, and checked before the offer is read.
    #[test]
    fn an_offer_is_bounded_whole_and_by_its_sources() {
        let on = |n: usize| -> Vec<String> {
            (0..n).map(|_| "briefing:2026-10-05".to_owned()).collect()
        };
        let plugin = PluginName::parse("dueday").expect("a name");
        let refused = |act: &str, why: &str, sources: &[String]| {
            propose(&plugin, act, why, sources)
                .map(|_| ())
                .map_err(|refused| refused.refusal)
        };
        let long = format!(
            "Changes(chain='life', acts=(Complete(todo='dentist', note='{}'),))",
            "a".repeat(MAX_OFFER)
        );
        assert_eq!(
            refused(&long, "why", &on(1)),
            Err(ProposalRefusal::TooLarge(long.len() + 3 + 19))
        );
        assert_eq!(
            refused(COMPLETE, "why", &on(MAX_SOURCES + 1)),
            Err(ProposalRefusal::TooManySources(MAX_SOURCES + 1))
        );
        assert_eq!(
            refused(COMPLETE, "why", &["a".repeat(MAX_SOURCE + 1)]),
            Err(ProposalRefusal::SourceTooLong(MAX_SOURCE + 1))
        );
        assert_eq!(
            refused(COMPLETE, "why", &["b".repeat(MAX_OFFER)]),
            Err(ProposalRefusal::TooLarge(COMPLETE.len() + 3 + MAX_OFFER)),
            "the whole is checked first, so no part of a flood is read"
        );
        assert_eq!(refused(COMPLETE, "why", &on(MAX_SOURCES)), Ok(()));
    }

    #[test]
    fn an_offer_without_a_reason_or_a_ground_is_refused() {
        assert_eq!(
            proposed(COMPLETE, "  ", &["briefing:2026-10-05"]),
            Err(ProposalRefusal::Reasons(NoReasons::Silent))
        );
        assert_eq!(
            proposed(COMPLETE, "why", &[]),
            Err(ProposalRefusal::Reasons(NoReasons::Groundless))
        );
        assert_eq!(
            proposed(COMPLETE, "why", &["the calendar"]),
            Err(ProposalRefusal::Source(Said::clip("the calendar")))
        );
        assert_eq!(
            proposed(COMPLETE, &"a".repeat(MAX_WHY + 1), &["briefing:2026-10-05"]),
            Err(ProposalRefusal::WhyTooLong(MAX_WHY + 1))
        );
    }
}
