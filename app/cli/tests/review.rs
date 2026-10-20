//! The secretary reviewing the agent's batches end to end, with only the model scripted.
//!
//! No server or peer: the box starts a review, so no request, gate or `Tap` is involved.

mod common;

use common::{FakeModel, Fakes};
use prodrome_core::event::{Hash, mk_completed};
use suzatary_core::capability::Clock as _;
use suzatary_core::secretary::Bound;
use suzatary_core::secretary::review::{review, reviews};
use suzatary_core::secretary::store::{Decision, Outcome, Settle};
use suzatary_core::store::Prodrome as _;
use suzatary_core::wire::EntryState;

/// Three batches, each through a fresh `Proposing` so they stay independent.
fn three_runs(caps: &Fakes) -> [Hash; 3] {
    let ordinary = caps.agent_run();
    let ordinary = ordinary
        .append(a_todo("parkingpermit", "Renew the parking permit"))
        .expect("appends");
    let duplicate = caps.agent_run();
    let duplicate = duplicate
        .append(a_todo("permitagain", "Renew the parking permit"))
        .expect("appends");
    let hisown = caps.agent_run();
    let hisown = hisown
        .append(
            mk_completed("file-permit", common::now(), "triage", "it looked done").expect("valid"),
        )
        .expect("appends");
    [ordinary, duplicate, hisown]
}

fn a_todo(id: &str, body: &str) -> suzatary_core::record::TodoEvent {
    suzatary_core::authoring::build_todo(
        &suzatary_core::authoring::NewTodo {
            todo: id.to_owned(),
            fields: suzatary_core::todo::TodoFields {
                body: body.to_owned(),
                rationale: "an errand with a recoverable cost".to_owned(),
                kind: "todo".to_owned(),
                category: "home".to_owned(),
                price: suzatary_core::todo::Price::Flat { percent: 75 },
                detail: String::new(),
                waiting_on: String::new(),
            },
            source: None,
            subtodos: Vec::new(),
        },
        suzatary_core::zone::Moment {
            at: common::now(),
            zone: suzatary_core::zone::Zone::UTC,
        },
        "triage",
        &["work", "home", "other", "study"],
        &suzatary_io::typeset::InProcess,
    )
    .expect("a valid todo")
}

/// The fixture deployment, which `context` and the chain lookup read.
fn deployment() -> suzatary_core::deployment::Deployment {
    common::deployment(
        [common::login("ada@example.com")],
        [(common::login("ada@example.com"), common::actor("ada"))],
    )
}

/// What the secretary said it was doing, quoted back by the held line.
const SAID: &str = "I took the parking permit, dropped the duplicate, and I am leaving the \
                    deed completion for you: you wrote that todo yourself.";

/// The script: one turn that settles two batches, then one that explains.
fn script(accept: &Hash, reject: &Hash) -> FakeModel {
    let replies = format!(
        r#"{{"say": "Settling the ordinary ones.", "calls": [
             {{"verb": "proposals_accept", "args": {{
                "chain": "life", "objects": "{}",
                "why": "A new todo with its rationale, and nothing about it is a commitment of yours."}}}},
             {{"verb": "proposals_reject", "args": {{
                "chain": "life", "objects": "{}",
                "why": "A duplicate of the permit todo in the batch before it."}}}}
           ]}}"#,
        accept.as_str(),
        reject.as_str(),
    );
    FakeModel::saying_owned([replies, format!(r#"{{"say": "{SAID}"}}"#)])
}

/// The model accepts one batch, rejects another, and the third is derived as held.
#[test]
fn a_review_accepts_one_rejects_one_and_holds_the_rest() {
    let caps = Fakes::new("review-round");
    let [ordinary, duplicate, hisown] = three_runs(&caps);
    let policy = common::chain();
    let deployment = deployment();
    let caps = caps.model(script(&ordinary, &duplicate));
    let chains = [Bound {
        name: "life",
        caps: caps.secretary(),
        policy: &policy,
    }];

    let done = review(
        &caps.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("the review runs");

    // Accepted: the object is on the chain now.
    let chain: Vec<Hash> = caps
        .chain
        .objects()
        .expect("the chain reads")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(
        chain.contains(&ordinary),
        "the accepted object is on the chain"
    );

    // Rejected: gone from the replica, its name in the receipt that guards against its return.
    let replica: Vec<Hash> = caps
        .proposals
        .objects()
        .expect("the replica reads")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(
        !replica.contains(&duplicate),
        "the rejected object is deleted"
    );

    // Held: the residue, never claimed.
    assert!(replica.contains(&hisown), "the held batch is untouched");

    // One record line per decision.
    let accepted = done.with(Outcome::Settled(Settle::Accepted));
    let rejected = done.with(Outcome::Settled(Settle::Rejected));
    let held = done.with(Outcome::Held);
    assert_eq!(accepted.len(), 1, "{:?}", done.decided);
    assert_eq!(rejected.len(), 1, "{:?}", done.decided);
    assert_eq!(held.len(), 1, "{:?}", done.decided);
    assert_eq!(done.standing, 1, "only the held batch is left standing");
    assert_eq!(accepted[0].batch, ordinary.as_str());
    assert_eq!(rejected[0].batch, duplicate.as_str());
    assert_eq!(held[0].batch, hisown.as_str());
    assert_eq!(held[0].chain, "life");
    // With no hold verb, the hold's reason is what the secretary said.
    assert_eq!(held[0].why.get().map(String::as_str), Some(SAID));
    assert_eq!(held[0].decision, Decision::Held, "a hold settles nothing");
}

/// The record line's shape, which the briefing and the terminal both quote.
#[test]
fn a_review_line_says_what_was_done_to_how_many_and_why() {
    let caps = Fakes::new("review-lines");
    let [ordinary, duplicate, _] = three_runs(&caps);
    let policy = common::chain();
    let deployment = deployment();
    let caps = caps.model(script(&ordinary, &duplicate));
    let chains = [Bound {
        name: "life",
        caps: caps.secretary(),
        policy: &policy,
    }];
    let done = review(
        &caps.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("the review runs");

    let accepted = done.with(Outcome::Settled(Settle::Accepted))[0].to_string();
    assert!(
        accepted.starts_with("accepted 1 object of batch "),
        "{accepted}"
    );
    assert!(accepted.contains("… on life: A new todo"), "{accepted}");
    let held = done.with(Outcome::Held)[0].to_string();
    assert!(held.starts_with("held batch "), "{held}");
    assert!(held.contains("… on life: "), "{held}");

    // Readable back out of the store, as `suzatary secretary reviews --since` does.
    let day = prodrome_core::fpl::instant_of(common::now()).date();
    let found = reviews(caps.conversations.conversations(), day)
        .expect("the store reads")
        .items;
    assert_eq!(found.len(), 3, "{found:?}");
    for (id, _) in &found {
        assert_eq!(id, &today_of(day), "today's review conversation");
    }
    assert!(
        reviews(
            caps.conversations.conversations(),
            day.succ_opt().expect("a tomorrow")
        )
        .expect("the store reads")
        .items
        .is_empty(),
        "a window after the decisions holds none of them"
    );
}

/// Nothing standing opens no conversation and never asks the model.
#[test]
fn a_quiet_pass_asks_no_model_and_opens_no_conversation() {
    let caps = Fakes::new("review-quiet");
    let policy = common::chain();
    let deployment = deployment();
    let chains = [Bound {
        name: "life",
        caps: caps.secretary(),
        policy: &policy,
    }];
    let done = review(
        &caps.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("the review runs");
    assert_eq!(done.rounds, 0);
    assert_eq!(done.standing, 0, "nothing was standing");
    assert!(done.decided.is_empty());
    assert!(
        suzatary_core::secretary::transcript::keys(caps.conversations.conversations())
            .expect("the store reads")
            .is_empty(),
        "no conversation was opened for nothing"
    );
}

/// A second review of a batch still held writes no second line.
#[test]
fn holding_the_same_batch_again_is_not_a_second_line() {
    let caps = Fakes::new("review-twice");
    let [_, _, hisown] = three_runs(&caps);
    let policy = common::chain();
    let deployment = deployment();
    let caps = caps.model(FakeModel::saying([
        r#"{"say": "These all change something of yours; I am holding them."}"#,
        r#"{"say": "Still holding them."}"#,
    ]));
    let chains = [Bound {
        name: "life",
        caps: caps.secretary(),
        policy: &policy,
    }];

    let first = review(
        &caps.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("the review runs");
    assert_eq!(first.with(Outcome::Held).len(), 3, "{:?}", first.decided);
    assert_eq!(
        first.standing, 3,
        "all three are still standing when it ends"
    );
    assert!(
        first
            .with(Outcome::Held)
            .iter()
            .any(|held| held.batch == hisown.as_str())
    );

    let again = review(
        &caps.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("the review runs again");
    assert!(
        again.decided.is_empty(),
        "a batch already held today is already recorded: {:?}",
        again.decided
    );
}

/// A conversation that does not read is named in today's review and paged once for that set;
/// the review of the readable rest goes on.
#[test]
fn an_unreadable_conversation_is_named_and_paged_once() {
    use suzatary_core::secretary::review::note_unreadable;
    use suzatary_core::secretary::store::{ConversationId, Entry, Kind};
    use suzatary_core::secretary::transcript;
    let caps = Fakes::new("review-unreadable");
    let zone = suzatary_core::zone::Zone::UTC;
    assert_eq!(
        note_unreadable(&caps.secretary(), zone).map(|page| page.is_some()),
        Ok(false),
        "every conversation reads"
    );
    let bad = ConversationId::new("2026-01-02-1").expect("a slug");
    let at = caps.clock.now();
    let lines = caps.conversations.conversations();
    transcript::start(
        lines,
        &bad,
        &Entry::Opened {
            at,
            title: suzatary_core::secretary::words::Words::kept("Topic".to_owned()),
            model: "m".to_owned(),
            kind: Kind::Topic,
        },
    )
    .expect("opened");
    transcript::push(
        lines,
        &bad,
        &Entry::Rewound {
            to: 9,
            at,
            by: "ada".to_owned(),
        },
    )
    .expect("written");
    let page = note_unreadable(&caps.secretary(), zone).expect("lists");
    assert!(
        matches!(
            page,
            Some(suzatary_core::capability::Push::Unreadable { conversations, .. })
                if conversations.get() == 1
        ),
        "{page:?}"
    );
    assert_eq!(
        note_unreadable(&caps.secretary(), zone).map(|page| page.is_some()),
        Ok(false),
        "the same set pages once"
    );
    let today = prodrome_core::fpl::instant_of(caps.clock.now()).date();
    let round = suzatary_core::secretary::read(lines, &today_of(today)).expect("reads");
    let named = round
        .turns
        .iter()
        .filter(|said| matches!(&said.turn, suzatary_core::capability::Turn::Act { text, .. } if text.contains("2026-01-02-1")))
        .count();
    assert_eq!(named, 1, "named once in today's review");
}

/// A review that could not run leaves a record saying so, while the caller still exits zero.
#[test]
fn a_review_that_cannot_reach_its_model_writes_the_outage_down() {
    let caps = Fakes::new("review-unreachable");
    let [_, _, _] = three_runs(&caps);
    let policy = common::chain();
    let deployment = deployment();
    // An empty script answers `ModelError::Failed`, as an unreachable model does.
    let caps = caps.model(FakeModel::saying([]));
    let chains = [Bound {
        name: "life",
        caps: caps.secretary(),
        policy: &policy,
    }];

    let refused = review(
        &caps.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    );
    assert!(refused.is_err(), "an unreachable model refuses the review");

    // The record names what nobody answered for.
    let today = prodrome_core::fpl::instant_of(caps.clock.now()).date();
    let stored =
        suzatary_core::secretary::read(caps.conversations.conversations(), &today_of(today))
            .expect("the conversation was opened before the model was asked");
    assert_eq!(
        stored.failures.len(),
        1,
        "one attempt, one line: {:?}",
        stored.failures
    );
    assert_eq!(
        stored.failures[0].standing, 3,
        "all three batches stood unanswered"
    );
    assert!(
        stored.failures[0]
            .reason
            .get()
            .is_some_and(|reason| !reason.is_empty()),
        "the box's own words for why, not an empty string"
    );
    assert!(
        stored.reviews.is_empty(),
        "nothing was decided, so nothing is a decision"
    );

    // It reads back through the sibling of `reviews`.
    let seen =
        suzatary_core::secretary::review::failures(caps.conversations.conversations(), today)
            .expect("the store reads")
            .items;
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].1.standing, 3);
    assert!(
        reviews(caps.conversations.conversations(), today)
            .expect("the store reads")
            .items
            .is_empty(),
        "an outage is not a decision and must not show as one"
    );
}

/// Every `decision` row the fold shows, read well after `common::now()` because `FakeClock` ticks.
fn decisions(caps: &Fakes) -> Vec<suzatary_core::wire::Entry> {
    let well_after =
        prodrome_core::literal::Datetime::new(2026, 9, 10, 0, 0, 0, 0).expect("a real instant");
    let policy = suzatary_core::chains::ChainPolicy {
        name: suzatary_core::chains::ChainName::new("life").expect("a slug"),
        untrusted: prodrome_core::policy::Untrusted::none(),
        roster: std::collections::BTreeSet::new(),
        root: suzatary_core::cast::keys::root(),
    };
    let chain: &dyn suzatary_core::store::Prodrome<suzatary_core::record::TodoEvent> = &caps.chain;
    let read = suzatary_core::chain_set::Read::of_stores([(&policy, chain)]);
    suzatary_core::routes::fold(
        &caps.render,
        &policy.name,
        &policy.untrusted,
        &read.set(well_after),
        None,
    )
    .expect("the fold reads")
    .entries
    .into_iter()
    .filter(|entry| {
        entry
            .authored
            .as_ref()
            .is_some_and(|authored| authored.block == suzatary_core::wire::Block::Decision)
    })
    .collect()
}

/// A hold's reason is the answer's prose, not its Typst markup.
#[test]
fn a_hold_s_reason_is_the_answer_s_words_not_its_markup() {
    let caps = Fakes::new("review-held-prose");
    let _ = three_runs(&caps);
    let policy = common::chain();
    let holding = caps.model(FakeModel::saying([
        r#"{"say": "*Why I held it:* each one #minor[changes] something of yours."}"#,
    ]));
    let chains = [Bound {
        name: "life",
        caps: holding.secretary(),
        policy: &policy,
    }];
    review(
        &holding.secretary(),
        &deployment(),
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("the round runs");

    let held = decisions(&holding);
    assert!(!held.is_empty());
    for entry in &held {
        let authored = entry.authored.as_ref().expect("a decision is authored");
        assert_eq!(
            authored.rationale.join(" "),
            "Why I held it: each one changes something of yours.",
            "{authored:?}"
        );
    }
}

/// A held batch is an open decision, which a later pass completes or cancels in place.
#[test]
fn a_held_batch_is_a_decision_accepting_or_rejecting_it_later_settles_it() {
    let caps = Fakes::new("review-batch-decision");
    let [ordinary, duplicate, hisown] = three_runs(&caps);
    let policy = common::chain();
    let deployment = deployment();

    // Round one: hold everything.
    let holding = caps.model(FakeModel::saying([
        r#"{"say": "These all change something of yours; I am holding them."}"#,
    ]));
    let chains = [Bound {
        name: "life",
        caps: holding.secretary(),
        policy: &policy,
    }];
    let first = review(
        &holding.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("round one runs");
    assert_eq!(first.with(Outcome::Held).len(), 3, "{:?}", first.decided);

    let open = decisions(&holding);
    assert_eq!(
        open.len(),
        3,
        "each held batch authored a decision: {open:?}"
    );
    assert!(
        open.iter().all(|entry| entry.state == EntryState::Open),
        "{open:?}"
    );

    // Round two, a later pass: settle two of the three.
    let settling = holding.model(script(&ordinary, &duplicate));
    let chains = [Bound {
        name: "life",
        caps: settling.secretary(),
        policy: &policy,
    }];
    let second = review(
        &settling.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("round two runs");
    assert_eq!(
        second.with(Outcome::Settled(Settle::Accepted)).len(),
        1,
        "{:?}",
        second.decided
    );
    assert_eq!(
        second.with(Outcome::Settled(Settle::Rejected)).len(),
        1,
        "{:?}",
        second.decided
    );

    let settled = decisions(&settling);
    assert_eq!(
        settled.len(),
        3,
        "settling never authors a SECOND decision for the same batch: {settled:?}"
    );
    let completed = settled
        .iter()
        .filter(|entry| entry.state == EntryState::Completed)
        .count();
    let cancelled = settled
        .iter()
        .filter(|entry| entry.state == EntryState::Cancelled)
        .count();
    let still_open = settled
        .iter()
        .filter(|entry| entry.state == EntryState::Open)
        .count();
    assert_eq!(completed, 1, "the accepted batch's decision: {settled:?}");
    assert_eq!(cancelled, 1, "the rejected batch's decision: {settled:?}");
    assert_eq!(
        still_open,
        1,
        "{}'s batch is still held: {settled:?}",
        hisown.as_str()
    );
}

/// A hold recorded without a decision behind it gets its decision on the next pass.
#[test]
fn a_hold_recorded_without_its_decision_gets_one_on_the_next_pass() {
    let caps = Fakes::new("review-heal-decision");
    let heads = three_runs(&caps);
    let policy = common::chain();
    let deployment = deployment();
    let holding = caps.model(FakeModel::saying([r#"{"say": "Still holding these."}"#]));

    let today = prodrome_core::fpl::instant_of(holding.clock.now()).date();
    let id = today_of(today);
    suzatary_core::secretary::start(
        holding.conversations.conversations(),
        &holding.model,
        &id,
        "Review",
        suzatary_core::secretary::store::Kind::Review,
        holding.clock.now(),
    )
    .expect("the conversation opens");
    for head in &heads {
        suzatary_core::secretary::transcript::push(
            holding.conversations.conversations(),
            &id,
            &suzatary_core::secretary::store::Entry::Reviewed {
                at: suzatary_core::capability::Datetime::new(2026, 9, 22, 0, 0, 0, 0)
                    .expect("an instant"),
                chain: "life".to_owned(),
                batch: head.as_str().to_owned(),
                decision: Decision::Held,
                why: suzatary_core::secretary::words::Words::kept("held earlier today".to_owned()),
            },
        )
        .expect("the hold line is written");
    }
    assert!(decisions(&holding).is_empty(), "no decision stands yet");

    let chains = [Bound {
        name: "life",
        caps: holding.secretary(),
        policy: &policy,
    }];
    let pass = review(
        &holding.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("the review runs");
    assert!(
        pass.decided.is_empty(),
        "the holds were already recorded, so no second line: {:?}",
        pass.decided
    );
    let open = decisions(&holding);
    assert_eq!(
        open.len(),
        3,
        "each standing batch now has its decision: {open:?}"
    );
}

/// The review's round, pinned: its lines and what the model is shown and sent.
#[test]
fn the_reviews_round_is_pinned() {
    let caps = Fakes::new("review-pinned");
    let [ordinary, duplicate, _hisown] = three_runs(&caps);
    let policy = common::chain();
    let deployment = deployment();
    let caps = caps.model(script(&ordinary, &duplicate));
    let chains = [Bound {
        name: "life",
        caps: caps.secretary(),
        policy: &policy,
    }];

    let done = review(
        &caps.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        "a brief",
    )
    .expect("the review runs");

    assert_eq!(done.rounds, 2);
    common::pinned("review", &common::scene(&caps, &done.stored.id));
}

/// Day `day`'s review conversation.
fn today_of(day: chrono::NaiveDate) -> suzatary_core::secretary::store::ConversationId {
    suzatary_core::secretary::store::ConversationId::daily(
        suzatary_core::secretary::daily::Daily::Review,
        day,
    )
}
