//! `/api/briefings` and `/api/briefings/{date}` over the real `BriefingFiles` in a temp
//! directory, which pins the on-disk shape.

mod common;

use common::Fakes;

fn start() -> (String, std::path::PathBuf) {
    let fakes = Fakes::new("briefings");
    let dir = fakes.root.join("briefings");
    let deployment = common::deployment([], []);
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment,
        common::loopback_is_this_box(),
    );
    (base, dir)
}

fn write(dir: &std::path::Path, day: &str, text: &str) {
    std::fs::create_dir_all(dir).expect("dir exists");
    std::fs::write(dir.join(format!("{day}.typ")), text).expect("writes");
}

/// The list is newest first.
#[test]
fn listing_is_newest_first() {
    let (base, dir) = start();
    write(&dir, "2026-09-01", "= Day one\n\nsome words");
    write(&dir, "2026-09-03", "= Day three\n\nmore words");
    write(&dir, "2026-09-02", "= Day two\n\nwords again");
    let mut response = common::get(&format!("{base}/api/briefings"));
    assert_eq!(common::status(&response), 200);
    let body = common::body_json(&mut response);
    let dates: Vec<&str> = body["briefings"]
        .as_array()
        .expect("briefings")
        .iter()
        .map(|b| b["date"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(dates, vec!["2026-09-03", "2026-09-02", "2026-09-01"]);
}

/// The title is the first heading, or the date when there is none.
#[test]
fn the_title_is_the_first_heading_or_the_date() {
    let (base, dir) = start();
    write(&dir, "2026-09-01", "= Morning briefing\n\nbody text here");
    write(&dir, "2026-09-02", "no heading here, just prose");
    let mut response = common::get(&format!("{base}/api/briefings"));
    let body = common::body_json(&mut response);
    let by_date = |date: &str| -> String {
        body["briefings"]
            .as_array()
            .expect("briefings")
            .iter()
            .find(|b| b["date"] == date)
            .expect("that date")["title"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    assert_eq!(by_date("2026-09-01"), "Morning briefing");
    assert_eq!(by_date("2026-09-02"), "2026-09-02");
}

/// The word count is the source split on whitespace.
#[test]
fn the_word_count_is_the_source_split_on_whitespace() {
    let (base, dir) = start();
    let text = "= Heading\n\none two three four";
    write(&dir, "2026-09-01", text);
    let mut response = common::get(&format!("{base}/api/briefings"));
    let body = common::body_json(&mut response);
    let row = &body["briefings"].as_array().expect("briefings")[0];
    assert_eq!(row["words"], text.split_whitespace().count());
}

/// A good date returns its source beside the date.
#[test]
fn a_good_date_returns_its_source() {
    let (base, dir) = start();
    write(&dir, "2026-09-01", "= Morning\n\nHello there.");
    let mut response = common::get(&format!("{base}/api/briefings/2026-09-01"));
    assert_eq!(common::status(&response), 200);
    let body = common::body_json(&mut response);
    assert_eq!(body["date"], "2026-09-01");
    assert_eq!(body["day"]["kind"], "saved");
    assert_eq!(body["day"]["source"], "= Morning\n\nHello there.");
}

/// A day the secretary briefed is its brief, block by block, whatever the saved files say.
#[test]
fn a_day_the_secretary_briefed_is_its_brief_block_by_block() {
    use suzatary_core::secretary::briefs::BRIEF;
    use suzatary_core::secretary::store::DEFAULT;
    use suzatary_core::secretary::{converse, read_or_open_default};

    let fakes = Fakes::new("briefings-brief").model(common::FakeModel::saying([
        "{\"say\": \"Nothing urgent.\\n\\nThe permit filing moved.\"}",
    ]));
    let deployment = common::deployment([], []);
    read_or_open_default(
        fakes.conversations.conversations(),
        &fakes.model,
        &DEFAULT,
        common::now(),
    )
    .expect("opens");
    converse(
        &fakes.secretary(),
        &deployment,
        &suzatary_core::settings::Settables::default(),
        &[],
        &DEFAULT,
        suzatary_core::secretary::Saying {
            text: BRIEF,
            references: &[],
            by: None,
        },
        "system prompt",
    )
    .expect("the scripted model answers")
    .expect("a round ran");
    let day = common::now().isoformat();
    let day = &day[..10];
    write(
        &fakes.root.join("briefings"),
        day,
        "= Saved\n\nOlder words.",
    );
    let base = common::start(
        std::sync::Arc::new(fakes),
        deployment,
        common::loopback_is_this_box(),
    );
    let mut response = common::get(&format!("{base}/api/briefings/{day}"));
    assert_eq!(common::status(&response), 200);
    let body = common::body_json(&mut response);
    assert_eq!(body["day"]["kind"], "brief", "{body}");
    assert_eq!(body["day"]["conversation"], DEFAULT.as_str());
    let blocks = body["day"]["blocks"].as_array().expect("blocks");
    assert_eq!(blocks.len(), 2, "{body}");
    assert_eq!(blocks[0]["plain"], "Nothing urgent.");
}

/// A bad date is 400.
#[test]
fn a_bad_date_is_400() {
    let (base, _dir) = start();
    for bad in ["not-a-date", "2026-13-40", "hello"] {
        let response = common::get(&format!("{base}/api/briefings/{bad}"));
        assert_eq!(common::status(&response), 400, "{bad}");
    }
}

/// A percent-encoded path traversal is refused as a bad date before it becomes a filename.
#[test]
fn a_path_traversal_attempt_is_400() {
    let (base, _dir) = start();
    let response = common::get(&format!("{base}/api/briefings/%2e%2e"));
    assert_eq!(common::status(&response), 400);
}

/// A missing date is 404.
#[test]
fn a_missing_date_is_404() {
    let (base, _dir) = start();
    let response = common::get(&format!("{base}/api/briefings/2099-01-01"));
    assert_eq!(common::status(&response), 404);
}
