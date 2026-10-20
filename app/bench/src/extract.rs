//! The pure half of extraction: builds [`crate::cases::Case`]s from values the capabilities
//! already read (chain objects, triage state, conversation lines); nothing here opens a file.
//!
//! The settled grade is filled only where the corpus records the owner's own hand on the same
//! case: a later event on the todo by a different actor, or a tapped or dismissed proposal.

use prodrome_core::event::{Actor, Hash, TodoId};
use suzatary_core::mail::known::Known;
use suzatary_core::mail::sourced_from;
use suzatary_core::record::{Envelope, TodoEvent};
use suzatary_core::secretary::responded::{Done, Responded};
use suzatary_core::secretary::store::{Entry, Standing};

use crate::cases::{
    AdversarialCase, CaseId, ChainSnapshot, JudgementAnswer, JudgementCase, NotACaseId,
    ReviewAnswer, ReviewCase, SnapshotBasis, TriageAnswer, TriageCase, Truths,
};

/// An object that authored a todo from a message: where `state.json`'s hash joins the chain.
struct Anchor<'a> {
    /// Position in the linearisation.
    index: usize,
    object: &'a Hash,
    /// The exact snapshot boundary.
    parent: Option<Hash>,
    /// A prefix of the message hash, as `source.hash` carries it.
    source_prefix: &'a str,
    todo: &'a TodoId,
    actor: &'a Actor,
}

/// Every object that authored a todo from a message, in linearisation order.
fn anchors(objects: &[(Hash, Envelope)]) -> Vec<Anchor<'_>> {
    objects
        .iter()
        .enumerate()
        .filter_map(|(index, (object, envelope))| {
            let Envelope::Sealed { prev, event } = envelope else {
                return None;
            };
            let TodoEvent::Authored(authored) = event else {
                return None;
            };
            let source = authored.payload.source.as_ref()?;
            Some(Anchor {
                index,
                object,
                parent: prev.clone(),
                source_prefix: source.hash.as_str(),
                todo: &authored.todo,
                actor: &authored.actor,
            })
        })
        .collect()
}

/// The first later event on the anchor's todo by a different actor: `Cancelled` reads as
/// "should not have existed", anything else as "kept", naming the object that kept it.
fn settled_triage_truth(
    objects: &[(Hash, Envelope)],
    anchor: &Anchor<'_>,
    chain_name: &str,
) -> Option<TriageAnswer> {
    objects[anchor.index + 1..]
        .iter()
        .find_map(|(object, envelope)| {
            let Envelope::Sealed { event, .. } = envelope else {
                return None;
            };
            if event.todo() != anchor.todo || event.actor() == anchor.actor {
                return None;
            }
            Some(match event {
                TodoEvent::Cancelled(_) => TriageAnswer::Declined,
                _ => TriageAnswer::Wrote {
                    chain: chain_name.to_owned(),
                    todo: anchor.todo.as_str().to_owned(),
                    object: object.as_str().to_owned(),
                },
            })
        })
}

/// One chain's objects and tip. Chains are checked in the order given (`instance.toml`'s),
/// and the first that anchors a message wins.
pub struct ChainObjects<'a> {
    /// `instance.toml`'s `[[chains]]` name.
    pub name: &'a str,
    /// Its objects, in linearisation order (`Prodrome::objects`).
    pub objects: &'a [(Hash, Envelope)],
    /// Its tip (`Prodrome::tips`).
    pub tip: Option<&'a Hash>,
}

/// One message's snapshot and truths, checked against every chain.
///
/// A message produces one case, never one per chain, or a single decline would count once
/// per chain: unanchored reads as declined against the first chain, anchored as written
/// there alone, since a todo lives in exactly one chain.
fn triage_truths_across(
    chains: &[ChainObjects<'_>],
    per_chain_anchors: &[Vec<Anchor<'_>>],
    message_hash: &str,
) -> (ChainSnapshot, Truths<TriageAnswer>) {
    for (chain, anchor_list) in chains.iter().zip(per_chain_anchors) {
        let Some(anchor) = anchor_list
            .iter()
            .find(|anchor| sourced_from(anchor.source_prefix, message_hash))
        else {
            continue;
        };
        return (
            ChainSnapshot {
                chain: chain.name.to_owned(),
                tip: anchor.parent.as_ref().map(Hash::as_str).map(str::to_owned),
                basis: SnapshotBasis::Anchored,
            },
            Truths {
                incumbent: TriageAnswer::Wrote {
                    chain: chain.name.to_owned(),
                    todo: anchor.todo.as_str().to_owned(),
                    object: anchor.object.as_str().to_owned(),
                },
                settled: settled_triage_truth(chain.objects, anchor, chain.name),
            },
        );
    }
    let first = chains.first();
    (
        ChainSnapshot {
            chain: first.map_or_else(String::new, |chain| chain.name.to_owned()),
            tip: first
                .and_then(|chain| chain.tip)
                .map(Hash::as_str)
                .map(str::to_owned),
            basis: SnapshotBasis::Extraction,
        },
        Truths {
            incumbent: TriageAnswer::Declined,
            settled: None,
        },
    )
}

/// One case per message `state.json` records a decision about, against every chain.
///
/// # Errors
/// [`NotACaseId`], which a well-formed (lowercase hex) hash cannot produce.
pub fn triage_cases(
    chains: &[ChainObjects<'_>],
    state: &Known,
) -> Result<Vec<TriageCase>, NotACaseId> {
    let per_chain_anchors: Vec<Vec<Anchor<'_>>> =
        chains.iter().map(|chain| anchors(chain.objects)).collect();
    state
        .decisions()
        .map(|(message_hash, _)| {
            let id = CaseId::new(format!("triage:{message_hash}"))?;
            let (chain_as_stood, truths) =
                triage_truths_across(chains, &per_chain_anchors, message_hash);
            Ok(TriageCase {
                id,
                message_hash: message_hash.clone(),
                chain_as_stood,
                truths,
            })
        })
        .collect()
}

/// Every flagged message `state.json` also records a decision about, checked like
/// [`triage_cases`].
///
/// `flagged` is `(message hash, why)`, real matches from the corpus. A flagged hash never
/// triaged is skipped rather than given a hollow `Declined` with no incumbent behind it.
///
/// # Errors
/// See [`triage_cases`].
pub fn adversarial_cases(
    chains: &[ChainObjects<'_>],
    state: &Known,
    flagged: &[(String, String)],
) -> Result<Vec<AdversarialCase>, NotACaseId> {
    let per_chain_anchors: Vec<Vec<Anchor<'_>>> =
        chains.iter().map(|chain| anchors(chain.objects)).collect();
    flagged
        .iter()
        .filter(|(message_hash, _)| state.decided(message_hash).is_some())
        .map(|(message_hash, signal)| {
            let id = CaseId::new(format!("adversarial:{message_hash}"))?;
            let (chain_as_stood, truths) =
                triage_truths_across(chains, &per_chain_anchors, message_hash);
            Ok(AdversarialCase {
                id,
                message_hash: message_hash.clone(),
                signal: signal.clone(),
                chain_as_stood,
                truths,
            })
        })
        .collect()
}

/// Every batch the secretary settled in one conversation; `conversation_id` is stamped into
/// each case's id.
///
/// # Errors
/// See [`triage_cases`].
pub fn review_cases(
    conversation_id: &str,
    entries: &[Entry],
) -> Result<Vec<ReviewCase>, NotACaseId> {
    entries
        .iter()
        .enumerate()
        .filter_map(|(line, entry)| {
            let Entry::Reviewed {
                chain,
                batch,
                decision,
                why,
                ..
            } = entry
            else {
                return None;
            };
            // A review whose words are gone has no incumbent answer to score.
            Some((line, chain, batch, decision, why.get()?))
        })
        .map(|(line, chain, batch, decision, why)| {
            Ok(ReviewCase {
                id: CaseId::new(format!("review:{conversation_id}:{line}"))?,
                conversation: conversation_id.to_owned(),
                chain: chain.clone(),
                batch_head: batch.clone(),
                truths: Truths {
                    incumbent: ReviewAnswer {
                        decision: decision.clone(),
                        why: why.clone(),
                    },
                    // Nothing records the owner's review of a review.
                    settled: None,
                },
            })
        })
        .collect()
}

/// Every proposal the secretary made in one conversation and what became of it, in the order
/// made, so a case's index is the tap's.
///
/// # Errors
/// See [`triage_cases`].
pub fn judgement_cases(
    conversation_id: &str,
    standings: &[Standing],
) -> Result<Vec<JudgementCase>, NotACaseId> {
    standings
        .iter()
        .map(|standing| {
            let index = standing.asking.index;
            let (proposal, reasons) = (
                &standing.object.asked.proposal,
                &standing.object.asked.reasons,
            );
            let incumbent = JudgementAnswer::Propose {
                proposal: Box::new(proposal.clone()),
                why: reasons.why().to_owned(),
            };
            // The owner never edits a draft, only taps or dismisses it, so a tap settles on
            // the proposal exactly as offered.
            let shown = standing.object.reading.shown();
            let settled = shown.and_then(|answer| match &answer.response {
                Responded::Done(Done::Sent { .. } | Done::Applied { .. } | Done::Acted { .. }) => {
                    Some(JudgementAnswer::Propose {
                        proposal: Box::new(proposal.clone()),
                        why: "The owner tapped it as proposed".to_owned(),
                    })
                }
                Responded::Dismissed { reason } => Some(JudgementAnswer::Decline {
                    why: reason.clone(),
                }),
                // A try, a send that may have gone or the owner's check of one judges nothing,
                // and neither does a card the box or the secretary took back unanswered.
                Responded::Tried { .. }
                | Responded::Done(Done::MayHaveGone { .. })
                | Responded::Checked(_)
                | Responded::Withdrawn(_) => None,
            });
            Ok(JudgementCase {
                id: CaseId::new(format!("judgement:{conversation_id}:{index}"))?,
                conversation: conversation_id.to_owned(),
                proposal_index: index,
                context: reasons.sources().iter().map(ToString::to_string).collect(),
                truths: Truths { incumbent, settled },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use prodrome_core::event::{Authored, Lifecycle};
    use prodrome_core::literal::Datetime;
    use suzatary_core::record::Record;
    use suzatary_core::secretary::store::{Draft, Proposal, Reasons};

    use super::*;

    fn known() -> Known {
        Known {
            pushed: suzatary_core::mail::state::TriageState::empty(),
            local: suzatary_core::mail::local_state::LocalState::default(),
            key: suzatary_core::mail::dedup_key::DedupKey::of(
                &suzatary_core::capability::Secret::new("0f".repeat(32)),
            )
            .expect("a key"),
        }
    }
    use crate::cases::SnapshotBasis;

    fn at(day: u32) -> Datetime {
        Datetime::new(2026, 9, day, 8, 0, 0, 0).expect("a real instant")
    }

    fn actor(name: &str) -> Actor {
        Actor::new(name).expect("a valid actor")
    }

    fn record_with_source(hash_prefix: &str) -> Record {
        let mut record = suzatary_core::record::mk_record(
            "todo",
            at(1),
            suzatary_core::record::MarkupSource::escaped("body"),
            prodrome_core::fpl::mk_absent(),
            vec![],
            "",
            "",
            suzatary_core::record::MarkupSource::empty(),
            None,
            vec![],
            vec![],
            "",
        )
        .expect("a minimal record");
        record.source = Some(suzatary_core::record::Source {
            sender: suzatary_core::record::StringSource::new("mail@example.test"),
            subject: suzatary_core::record::StringSource::new("subject"),
            date: at(1),
            hash: suzatary_core::record::StringSource::new(hash_prefix),
            thread_id: suzatary_core::record::StringSource::new(""),
            message_id: suzatary_core::record::StringSource::new(""),
        });
        record
    }

    fn hash(byte: u8) -> Hash {
        Hash::new(format!("{byte:02x}").repeat(32)).expect("a well-formed hash")
    }

    /// A message that became a todo is anchored at the object's parent and names the todo.
    #[test]
    fn a_written_todo_anchors_the_snapshot_at_its_own_parent() {
        let genesis = hash(1);
        let written = hash(2);
        let objects = vec![
            (
                genesis.clone(),
                Envelope::Sealed {
                    prev: None,
                    event: TodoEvent::Created(prodrome_core::event::Created {
                        todo: TodoId::new("seed").expect("valid"),
                        at: at(1),
                        actor: actor("triage"),
                        text: "seed".to_owned(),
                        note: String::new(),
                    }),
                },
            ),
            (
                written.clone(),
                Envelope::Sealed {
                    prev: Some(genesis.clone()),
                    event: TodoEvent::Authored(Box::new(Authored {
                        todo: TodoId::new("t1").expect("valid"),
                        at: at(2),
                        actor: actor("triage"),
                        payload: record_with_source("abcdef0123456789"),
                    })),
                },
            ),
        ];
        let state = known().mark(
            "abcdef0123456789fullhash",
            suzatary_core::mail::state::Decision::parse("todo", Some("t1"))
                .expect("a todo decision"),
            suzatary_core::mail::local_state::Kept::default(),
        );
        let chains = [ChainObjects {
            name: "life",
            objects: &objects,
            tip: Some(&written),
        }];
        let cases = triage_cases(&chains, &state).expect("builds");
        assert_eq!(cases.len(), 1);
        let case = &cases[0];
        assert_eq!(case.chain_as_stood.basis, SnapshotBasis::Anchored);
        assert_eq!(case.chain_as_stood.tip.as_deref(), Some(genesis.as_str()));
        assert!(matches!(case.truths.incumbent, TriageAnswer::Wrote { .. }));
        assert!(case.truths.settled.is_none());
    }

    /// A message nothing wrote gets the extraction-time tip and `Declined`.
    #[test]
    fn a_declined_message_falls_back_to_the_extraction_tip() {
        let tip = hash(9);
        let state = known().mark(
            "deadbeefcafefeed",
            suzatary_core::mail::state::Decision::Skip,
            suzatary_core::mail::local_state::Kept::default(),
        );
        let chains = [ChainObjects {
            name: "life",
            objects: &[],
            tip: Some(&tip),
        }];
        let cases = triage_cases(&chains, &state).expect("builds");
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].chain_as_stood.basis, SnapshotBasis::Extraction);
        assert_eq!(cases[0].chain_as_stood.tip.as_deref(), Some(tip.as_str()));
        assert_eq!(cases[0].truths.incumbent, TriageAnswer::Declined);
    }

    /// A later cancel by a different actor is the settled truth, overruling `Wrote`.
    #[test]
    fn a_later_cancel_by_a_different_actor_is_the_settled_truth() {
        let genesis = hash(1);
        let written = hash(2);
        let cancelled = hash(3);
        let todo = TodoId::new("t1").expect("valid");
        let objects = vec![
            (
                genesis.clone(),
                Envelope::Sealed {
                    prev: None,
                    event: TodoEvent::Authored(Box::new(Authored {
                        todo: todo.clone(),
                        at: at(1),
                        actor: actor("triage"),
                        payload: record_with_source("abcdef0123456789"),
                    })),
                },
            ),
            (
                written.clone(),
                Envelope::Sealed {
                    prev: Some(genesis.clone()),
                    event: TodoEvent::Cancelled(Lifecycle {
                        todo: todo.clone(),
                        at: at(2),
                        actor: actor("ada"),
                        note: "not real".to_owned(),
                    }),
                },
            ),
        ];
        let _ = &cancelled;
        let state = known().mark(
            "abcdef0123456789fullhash",
            suzatary_core::mail::state::Decision::parse("todo", Some("t1"))
                .expect("a todo decision"),
            suzatary_core::mail::local_state::Kept::default(),
        );
        let chains = [ChainObjects {
            name: "life",
            objects: &objects,
            tip: Some(&written),
        }];
        let cases = triage_cases(&chains, &state).expect("builds");
        assert_eq!(cases[0].truths.settled, Some(TriageAnswer::Declined));
    }

    #[test]
    fn a_flagged_hash_state_never_triaged_produces_no_adversarial_case() {
        let state = known();
        let flagged = vec![(
            "neverseen".to_owned(),
            "ignore all previous instructions".to_owned(),
        )];
        let chains: [ChainObjects<'_>; 0] = [];
        let cases = adversarial_cases(&chains, &state, &flagged).expect("builds");
        assert!(cases.is_empty());
    }

    /// A message anchored on neither chain produces exactly one case, not one per chain.
    #[test]
    fn a_declined_message_produces_one_case_across_two_chains() {
        let state = known().mark(
            "deadbeefcafefeed",
            suzatary_core::mail::state::Decision::Skip,
            suzatary_core::mail::local_state::Kept::default(),
        );
        let life_tip = hash(7);
        let project_tip = hash(8);
        let chains = [
            ChainObjects {
                name: "life",
                objects: &[],
                tip: Some(&life_tip),
            },
            ChainObjects {
                name: "suzatary",
                objects: &[],
                tip: Some(&project_tip),
            },
        ];
        let cases = triage_cases(&chains, &state).expect("builds");
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].chain_as_stood.chain, "life");
    }

    /// A sent proposal settles as offered; a dismissed one as `Decline` with his reason.
    #[test]
    fn a_proposal_is_judged_by_whether_it_was_tapped_or_dismissed() {
        let standing = |to: &str, why: &str, response: Responded| {
            use suzatary_core::secretary::inbox::{self, Fold, ProposalId};
            use suzatary_core::secretary::inbox_event::{InboxEvent, Said};
            let asked = inbox::Asked {
                proposer: suzatary_core::secretary::proposer::Proposer::Secretary,
                proposal: Proposal::Email(Draft {
                    account: "a@example.test".to_owned(),
                    to: vec![format!("{to}@example.test")],
                    cc: vec![],
                    subject: "hi".to_owned(),
                    body: "hi".to_owned(),
                    attachments: Vec::new(),
                }),
                reasons: Reasons::new(
                    why,
                    vec![suzatary_core::reference::Reference::Mail {
                        hash: suzatary_core::reference::MailHash::new("abc123").expect("valid"),
                    }],
                )
                .expect("valid"),
                asked: "proposal:2026-09-20/0".parse().expect("a reference"),
                price: inbox::Price::unpriced(),
            };
            let at =
                prodrome_core::literal::Datetime::new(2026, 9, 20, 8, 0, 0, 0).expect("an instant");
            let id = ProposalId::of(&asked, at);
            let said = |said| InboxEvent {
                proposal: id.clone(),
                at,
                actor: prodrome_core::event::Actor::new("ada").expect("an actor"),
                said,
            };
            let events = [
                said(Said::Asked(Box::new(asked))),
                said(Said::Responded(response)),
            ];
            let object = Fold::of_chain(&events).get(&id).expect("held").clone();
            Standing {
                asking: suzatary_core::secretary::store::Asking {
                    index: 0,
                    at: suzatary_core::capability::Datetime::new(2026, 9, 20, 8, 0, 0, 0)
                        .expect("an instant"),
                    object: id,
                    after_turn: 0,
                },
                object,
            }
        };
        let standings = [
            standing(
                "b",
                "a reply was owed",
                Responded::Done(Done::Sent {
                    recipients: vec!["b@example.test".to_owned()],
                    message_id: "<1@example.test>".to_owned(),
                }),
            ),
            standing(
                "c",
                "a follow-up seemed due",
                Responded::Dismissed {
                    reason: "already handled this myself".to_owned(),
                },
            ),
            standing(
                "d",
                "a reminder seemed due",
                Responded::Withdrawn(
                    suzatary_core::secretary::responded::Withdrawal::NoLongerNeeded {
                        why: "she replied before it was sent".to_owned(),
                    },
                ),
            ),
        ];
        let cases = judgement_cases("2026-09-20", &standings).expect("builds");
        assert_eq!(cases.len(), 3);
        assert_eq!(
            cases[2].truths.settled, None,
            "a card withdrawn unanswered is no decline of the owner's"
        );
        assert!(matches!(
            cases[0].truths.settled,
            Some(JudgementAnswer::Propose { .. })
        ));
        assert_eq!(
            cases[1].truths.settled,
            Some(JudgementAnswer::Decline {
                why: "already handled this myself".to_owned()
            })
        );
    }

    #[test]
    fn a_review_line_becomes_a_review_case_with_no_settled_grade() {
        let entries = vec![Entry::Reviewed {
            at: suzatary_core::capability::Datetime::new(2026, 9, 20, 8, 0, 0, 0)
                .expect("an instant"),
            chain: "life".to_owned(),
            batch: "b".repeat(64),
            decision: suzatary_core::secretary::store::Decision::Settled {
                how: suzatary_core::secretary::store::Settle::Accepted,
                objects: suzatary_core::nonempty::NonEmpty::of(vec!["o".repeat(64)])
                    .expect("one object"),
            },
            why: suzatary_core::secretary::words::Words::kept(
                "a duplicate mark, routine".to_owned(),
            ),
        }];
        let cases = review_cases("2026-09-20-review", &entries).expect("builds");
        assert_eq!(cases.len(), 1);
        assert!(cases[0].truths.settled.is_none());
        assert_eq!(cases[0].truths.incumbent.decision.objects().len(), 1);
    }
}
