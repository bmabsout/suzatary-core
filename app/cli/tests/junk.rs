//! The junk sweep's two verbs, called as the secretary calls them, over a fake Junk folder.

mod common;

use common::{FakeJunk, Fakes, JunkFolder};
use serde_json::{Value, json};
use suzatary_core::cast::CAST;
use suzatary_core::mail::junk::{ChurnList, JunkHeader, KeepList, Uid};

const CHURN: &str = "offers.example.net";
const SMALL: &str = "ada@example.org";

fn header(uid: u32, name: &str, address: &str, subject: &str) -> JunkHeader {
    JunkHeader::new(
        Uid(uid),
        Some(name),
        Some(address),
        subject,
        "Tue, 01 Sep 2026 09:00:00 +0000",
    )
}

fn fakes(tag: &str) -> Fakes {
    Fakes::new(tag).junk(FakeJunk {
        churn: ChurnList::new([CHURN], KeepList::default()).expect("a churn list"),
        folders: vec![
            (
                CAST.owner.email.to_owned(),
                JunkFolder::Holds(vec![
                    (
                        header(4, "Deals", "deals@offers.example.net", "70% off"),
                        "buy",
                    ),
                    (
                        header(9, CAST.friend.name, CAST.friend.email, "RSVP for Saturday"),
                        "Will you come to the wedding?",
                    ),
                    (
                        header(12, "Deals", "news@mail.offers.example.net", "80% off"),
                        "buy",
                    ),
                ]),
            ),
            (SMALL.to_owned(), JunkFolder::NeedsLogin),
        ],
        asked: std::sync::Mutex::default(),
    })
}

fn call(fakes: &Fakes, verb: &str, args: Value) -> Result<Value, suzatary_core::refusal::Refusal> {
    let deployment = common::deployment([], []);
    let policy = common::chain();
    let chains = [suzatary_core::secretary::Bound {
        name: "life",
        caps: fakes.secretary(),
        policy: &policy,
    }];
    let Value::Object(args) = args else {
        panic!("arguments are an object: {args}");
    };
    suzatary_core::secretary::call(
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &chains,
        &suzatary_core::secretary::store::DEFAULT.clone(),
        verb,
        &args,
    )
    .map(|answer| answer.value)
}

/// Every Microsoft account by default: churn counted not listed, a lapsed sign-in its own row.
#[test]
fn a_digest_reads_every_account_and_lists_only_what_is_outside_churn() {
    let fakes = fakes("junk-digest");
    let answer = call(&fakes, "junk_digest", json!({})).expect("a digest");
    assert_eq!(answer["window"]["since"], "2026-08-03");
    assert_eq!(answer["window"]["days"], 30);
    assert_eq!(
        answer["churn"],
        json!({"ours": [CHURN], "keep": [], "seed": "none"})
    );

    let owner = &answer["accounts"][0];
    assert_eq!(owner["account"], CAST.owner.email);
    assert_eq!(owner["total"], 3);
    let listed = owner["outside_churn"].as_array().expect("a list");
    assert_eq!(listed.len(), 1, "{owner}");
    assert_eq!(listed[0]["uid"], 9);
    assert_eq!(listed[0]["subject"], "RSVP for Saturday");
    assert_eq!(owner["listed_left_out"], 0);
    assert_eq!(owner["domains_left_out"], 0);
    assert_eq!(
        owner["by_domain"],
        json!([
            ["example.test", 1, null],
            ["mail.offers.example.net", 1, "ours"],
            ["offers.example.net", 1, "ours"]
        ])
    );

    let small = &answer["accounts"][1];
    assert_eq!(small["account"], SMALL);
    let sentence = small["needs_login"].as_str().expect("a sentence");
    assert!(
        sentence.contains(&format!("suzatary auth {SMALL} --scope imap")),
        "{sentence}"
    );

    let asked = fakes.junk.asked.lock().expect("the log").clone();
    let since = chrono::NaiveDate::from_ymd_opt(2026, 8, 3).expect("a date");
    assert_eq!(
        asked,
        vec![
            (CAST.owner.email.to_owned(), since),
            (SMALL.to_owned(), since)
        ]
    );
}

#[test]
fn a_digest_can_name_one_account_and_a_window() {
    let fakes = fakes("junk-one");
    let answer = call(
        &fakes,
        "junk_digest",
        json!({"account": CAST.owner.email, "days": 7}),
    )
    .expect("a digest");
    assert_eq!(answer["window"]["since"], "2026-08-26");
    assert_eq!(answer["accounts"].as_array().expect("rows").len(), 1);
}

#[test]
fn a_digest_refuses_an_account_it_does_not_read_and_a_window_out_of_range() {
    let fakes = fakes("junk-refused");
    let refused = call(
        &fakes,
        "junk_digest",
        json!({"account": CAST.counsel.email}),
    )
    .expect_err("not a Junk account");
    assert!(refused.to_string().contains(CAST.owner.email), "{refused}");
    call(&fakes, "junk_digest", json!({"days": 0})).expect_err("no days");
    call(&fakes, "junk_digest", json!({"days": 400})).expect_err("more than a year");
    assert!(fakes.junk.asked.lock().expect("the log").is_empty());
}

/// A row's handle opens its message as text.
#[test]
fn show_opens_the_message_a_digest_row_names() {
    let fakes = fakes("junk-show");
    let shown = call(
        &fakes,
        "junk_show",
        json!({"account": CAST.owner.email, "uid": 9}),
    )
    .expect("a message");
    assert_eq!(shown["uid"], 9);
    assert_eq!(shown["body"], "Will you come to the wedding?");
    assert_eq!(shown["truncated"], false);
    assert_eq!(
        shown["from"],
        format!("{} <{}>", CAST.friend.name, CAST.friend.email)
    );

    let missing = call(
        &fakes,
        "junk_show",
        json!({"account": CAST.owner.email, "uid": 5}),
    )
    .expect_err("no such UID");
    assert!(missing.to_string().contains("UID 5"), "{missing}");

    let lapsed = call(&fakes, "junk_show", json!({"account": SMALL, "uid": 1})).expect("an answer");
    assert!(lapsed["needs_login"].is_string(), "{lapsed}");
}

/// Reading a Junk message taints the round, so a later `remember` becomes a card.
#[test]
fn the_junk_verbs_read_and_are_tainted() {
    use suzatary_core::secretary::surface::{Effect, SecretaryVerb};
    use suzatary_core::secretary::taint::Taint;
    for verb in [SecretaryVerb::JunkDigest, SecretaryVerb::JunkShow] {
        assert_eq!(verb.class().effect, Effect::Reads, "{verb}");
        assert_eq!(verb.class().taint, Taint::Tainted, "{verb}");
    }
}
