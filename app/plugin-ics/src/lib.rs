//! The app's reference calendar plugin: a `source` that reads each calendar's secret iCal link
//! and emits one item per event occurrence in a window around today, each an event of its
//! declared schema, `ics.schema`. Built with the `due` feature it is the calendar's reference
//! `actor` instead (`due`), which reads the same schema's store and proposes a todo for each
//! `Due:` entry.
//!
//! It names no person and no provider: a calendar is a credential of use `url` its manifest
//! declares (`PLUGIN_ICS_*`), fetched through the host, so the link is never in this plugin's
//! memory, only the feed it answers. Google, Microsoft and Proton each publish such a link.
//!
//! Recurrence is RFC 5545's, written here over chrono and chrono-tz: the maintained RRULE crates
//! (`rrule` and its forks, `icalendar`'s `recurrence`) take chrono with its `wasmbind` feature,
//! which links `js-sys` imports a host that gives only the `source` world refuses to load. The
//! calendar logic is plain Rust tested natively; `guest` (or, for the actor, `actor_guest`) is
//! the one module that touches the component model.

#![deny(unsafe_code)]

pub mod content;
pub mod due;
pub mod event;
pub mod expand;
pub mod item;
pub mod literal;
pub mod read;
pub mod rule;
pub mod sync;
pub mod value;
pub mod zone;

/// The component's side: the world's imports lent to [`sync::sync`].
///
/// wit-bindgen's generated lifting and its `export!` expand to `unsafe` where they are written,
/// so this module, and only this one, allows it; everything it calls is safe Rust.
#[cfg(not(feature = "due"))]
#[allow(unsafe_code)]
mod guest {
    // The generated lifting rebuilds each list from the pointer and length its own allocator
    // handed out, which this lint cannot see.
    #[allow(clippy::same_length_and_capacity)]
    mod bindings {
        wit_bindgen::generate!({
            path: "../wit",
            world: "suzatary:plugin/source@0.3.0",
        });
    }

    use bindings::exports::suzatary::plugin::sync::{
        Credential, Guest, Item, Placement, SyncError, Synced,
    };
    use bindings::suzatary::plugin::fetch::{self, Header, Request, Target, Verb};
    use bindings::suzatary::plugin::{clock, log};
    use chrono::{NaiveDate, NaiveDateTime};

    use crate::item::SCHEMA;
    use crate::sync::{self, Fetched, Level};

    struct Ics;

    impl Guest for Ics {
        fn sync(
            cursor: Option<Vec<u8>>,
            credentials: Vec<Credential>,
        ) -> Result<Synced, SyncError> {
            let calendars: Vec<String> = credentials
                .into_iter()
                .filter(|credential| matches!(credential.into, Placement::Url))
                .map(|credential| credential.name)
                .collect();
            let now = now()?;
            let learned = sync::sync(
                cursor.as_deref(),
                &calendars,
                now,
                &mut get,
                &mut |level, line| {
                    let level = match level {
                        Level::Info => log::Level::Info,
                        Level::Warn => log::Level::Warn,
                    };
                    log::log(level, &line);
                },
            )
            .map_err(|bad| SyncError::BadCursor(bad.0))?;
            Ok(Synced {
                items: learned
                    .items
                    .iter()
                    .map(|item| Item {
                        schema: SCHEMA.to_owned(),
                        bytes: item.event(now),
                    })
                    .collect(),
                cursor: learned.cursor,
            })
        }
    }

    bindings::export!(Ics with_types_in bindings);

    /// One calendar's feed, by the credential that holds its link.
    fn get(name: &str) -> Fetched {
        let request = Request {
            verb: Verb::Get,
            target: Target::SecretUrl(name.to_owned()),
            headers: vec![Header {
                name: "accept".to_owned(),
                value: "text/calendar".to_owned(),
            }],
            credential: None,
        };
        match fetch::fetch(&request) {
            Ok(response) => Fetched::Answered {
                status: response.status,
                body: response.body,
            },
            Err(refusal) => Fetched::Refused(format!("{refusal:?}")),
        }
    }

    fn now() -> Result<NaiveDateTime, SyncError> {
        let now = clock::now();
        NaiveDate::from_ymd_opt(now.year, u32::from(now.month), u32::from(now.day))
            .and_then(|day| {
                day.and_hms_micro_opt(
                    u32::from(now.hour),
                    u32::from(now.minute),
                    u32::from(now.second),
                    now.microsecond,
                )
            })
            .ok_or_else(|| SyncError::Malformed("the host's clock gave no real instant".to_owned()))
    }
}

/// The actor's side: its store's events handed to [`due::decide`] on the host's day.
///
/// As `guest`, the one module that allows the generated `unsafe`.
#[cfg(feature = "due")]
#[allow(unsafe_code)]
mod actor_guest {
    #[allow(clippy::same_length_and_capacity)]
    mod bindings {
        wit_bindgen::generate!({
            path: "../wit",
            world: "suzatary:plugin/actor@0.3.0",
        });
    }

    use bindings::exports::suzatary::plugin::decide::{Act, DecideError, Guest, Proposal};
    use bindings::suzatary::plugin::clock;
    use chrono::NaiveDate;

    use crate::due;

    struct Due;

    impl Guest for Due {
        fn decide(acts: Vec<Act>) -> Result<Vec<Proposal>, DecideError> {
            let texts = acts
                .into_iter()
                .map(|act| String::from_utf8(act.bytes))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| DecideError::Malformed("an act that is not text".to_owned()))?;
            let now = clock::now();
            let today = NaiveDate::from_ymd_opt(now.year, u32::from(now.month), u32::from(now.day))
                .ok_or_else(|| DecideError::Malformed("the host's clock gave no day".to_owned()))?;
            let events: Vec<&str> = texts.iter().map(String::as_str).collect();
            Ok(due::decide(&events, today)
                .map_err(DecideError::Malformed)?
                .into_iter()
                .map(|proposed| Proposal {
                    act: proposed.act,
                    why: proposed.why,
                    sources: proposed.sources,
                })
                .collect())
        }
    }

    bindings::export!(Due with_types_in bindings);
}
