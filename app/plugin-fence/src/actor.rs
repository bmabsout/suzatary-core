//! An actor that tries to break the host's fences on proposals, one mode per call.
//!
//! The mode is the `line` of the first act it is handed, so `app/plugin/tests/fence.rs` picks one
//! per test by the store it hands over. Built with `escapee` too, it targets a world that also
//! imports `fetch`, which the host must refuse to load at all.

#[allow(clippy::same_length_and_capacity)]
mod bindings {
    #[cfg(not(feature = "escapee"))]
    wit_bindgen::generate!({
        path: "../wit",
        world: "suzatary:plugin/actor@0.3.0",
    });

    #[cfg(feature = "escapee")]
    wit_bindgen::generate!({
        path: ["../wit", "wit"],
        world: "suzatary:fence/actor-escapee",
        generate_all,
    });
}

use bindings::exports::suzatary::plugin::decide::{Act, DecideError, Guest, Proposal};
use bindings::suzatary::plugin::log;

/// What every honest proposal rests on.
const GROUND: &str = "briefing:2026-10-05";

/// The act the honest modes ask for about one line: closing the todo it names.
fn closing(line: &str) -> String {
    format!("Changes(chain='life', acts=(Complete(todo='{line}', note=''),))")
}

fn proposal(act: String, why: &str, sources: &[&str]) -> Proposal {
    Proposal {
        act,
        why: why.into(),
        sources: sources.iter().map(|&source| source.into()).collect(),
    }
}

/// The `line` an act of the fence's schema carries.
fn line(act: &Act) -> Result<String, DecideError> {
    let text = String::from_utf8(act.bytes.clone())
        .map_err(|_| DecideError::Malformed("an act that is not text".into()))?;
    Ok(text
        .split_once("line='")
        .and_then(|(_, rest)| rest.split_once('\''))
        .map_or("", |(line, _)| line)
        .to_owned())
}

struct FenceActor;

impl Guest for FenceActor {
    fn decide(acts: Vec<Act>) -> Result<Vec<Proposal>, DecideError> {
        let Some(first) = acts.first() else {
            return Ok(Vec::new());
        };
        let mode = line(first)?;
        log::log(
            log::Level::Info,
            &format!("{} acts, mode {mode}", acts.len()),
        );
        match mode.as_str() {
            // The honest path: every line's todo closed, with a reason and a ground.
            "echo" => acts
                .iter()
                .map(|act| Ok(proposal(closing(&line(act)?), "echoed", &[GROUND])))
                .collect(),
            // The same act twice, for two reasons: one offer, the first reason's.
            "twice" => Ok(vec![
                proposal(closing("twice"), "second", &[GROUND]),
                proposal(closing("twice"), "first", &[GROUND]),
            ]),
            // An act that claims an actor, which no act of the Inbox has a field for.
            "forge" => Ok(vec![proposal(
                closing("forge").replace("acts=", "actor='ada', acts="),
                "forged",
                &[GROUND],
            )]),
            // The Inbox's own response, as if the act had already been performed.
            "elsewhere" => Ok(vec![proposal(
                "Acted(proposal='p0', at=datetime(2026, 10, 5, 9, 0, 0), actor='ada', what='sent')"
                    .into(),
                "elsewhere",
                &[GROUND],
            )]),
            // A sign-in, an act for the box, which no plugin may ask for.
            "recur" => Ok(vec![proposal(
                "Auth(account='ada@example.com')".into(),
                "recurring",
                &[GROUND],
            )]),
            // Ten thousand distinct acts in one decision.
            "flood" => Ok((0..10_000)
                .map(|n| proposal(closing(&format!("flood{n}")), "flooded", &[GROUND]))
                .collect()),
            // A new act at every call, minted from the clock, so no tick asks for the last one.
            "clock" => {
                let now = bindings::suzatary::plugin::clock::now();
                Ok(vec![proposal(
                    closing(&format!("x{}{}", now.second, now.microsecond)),
                    "the time has come",
                    &[GROUND],
                )])
            }
            // One act resting on a hundred thousand sources, megabytes to store.
            "sprawl" => Ok(vec![Proposal {
                act: closing("sprawl"),
                why: "sprawled".into(),
                sources: (0..100_000).map(|_| GROUND.into()).collect(),
            }]),
            // No reason given.
            "mute" => Ok(vec![proposal(closing("mute"), "", &[GROUND])]),
            // Nothing it rests on.
            "groundless" => Ok(vec![proposal(closing("groundless"), "groundless", &[])]),
            // A megabyte of words in a log line and in the error it answers with.
            "megabyte" => {
                let words = "é".repeat(1 << 19);
                log::log(log::Level::Warn, &words);
                Err(DecideError::Malformed(words))
            }
            "loop" => spin(),
            "grow" => hoard(),
            #[cfg(feature = "escapee")]
            "escape" => {
                let _ = bindings::suzatary::plugin::fetch::fetch(
                    &bindings::suzatary::plugin::fetch::Request {
                        verb: bindings::suzatary::plugin::fetch::Verb::Get,
                        target: bindings::suzatary::plugin::fetch::Target::Url(
                            "https://example.test/".into(),
                        ),
                        headers: Vec::new(),
                        credential: None,
                    },
                );
                Ok(Vec::new())
            }
            _ => Err(DecideError::Malformed(format!("no mode {mode:?}"))),
        }
    }
}

bindings::export!(FenceActor with_types_in bindings);

/// Never returns: only the host's deadline ends it.
fn spin() -> ! {
    let mut turns: u64 = 0;
    loop {
        turns = std::hint::black_box(turns.wrapping_add(1));
    }
}

/// Never returns: only the host's memory cap ends it.
fn hoard() -> ! {
    let mut hoard: Vec<Vec<u8>> = Vec::new();
    loop {
        hoard.push(std::hint::black_box(vec![7; 1 << 20]));
    }
}
