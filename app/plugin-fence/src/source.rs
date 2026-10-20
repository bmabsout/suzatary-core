//! The fence's source: every mode tries one fence of the `source` world.

// The generated lifting rebuilds each list from the pointer and length its own allocator
// handed out, which this lint cannot see.
#[allow(clippy::same_length_and_capacity)]
mod bindings {
    #[cfg(not(feature = "escapee"))]
    wit_bindgen::generate!({
        path: "../wit",
        world: "suzatary:plugin/source@0.3.0",
    });

    #[cfg(feature = "escapee")]
    wit_bindgen::generate!({
        path: ["../wit", "wit"],
        world: "suzatary:fence/escapee",
        generate_all,
    });
}

use bindings::exports::suzatary::plugin::sync::{Credential, Guest, Item, SyncError, Synced};
use bindings::suzatary::plugin::fetch::{self, Request, Response, Target, Verb};
use bindings::suzatary::plugin::{clock, log};

/// The schema every item this plugin emits is in: one line of a body.
const SCHEMA: &str = "fence.line";

struct Fence;

impl Guest for Fence {
    fn sync(cursor: Option<Vec<u8>>, credentials: Vec<Credential>) -> Result<Synced, SyncError> {
        let cursor = cursor.ok_or_else(|| SyncError::BadCursor("a mode is required".into()))?;
        let cursor =
            String::from_utf8(cursor).map_err(|_| SyncError::BadCursor("not text".into()))?;
        let (mode, argument) = cursor.split_once(' ').unwrap_or((&cursor, ""));
        match mode {
            // The honest path: fetch an allowed origin and emit its lines.
            "items" => lines(&get(Target::Url(format!("{argument}/items")), None)?),
            // A URL whose origin the manifest does not allow.
            "host" => lines(&get(Target::Url(argument.to_owned()), None)?),
            // A credential named in a header, whatever the manifest says of it.
            "credential" => {
                let (origin, name) = argument.split_once(' ').unwrap_or((argument, ""));
                lines(&get(
                    Target::Url(format!("{origin}/items")),
                    Some(name.to_owned()),
                )?)
            }
            // A credential named as the URL itself.
            "secret-url" => lines(&get(Target::SecretUrl(argument.to_owned()), None)?),
            // A declared credential sent to a server that echoes what it received.
            "echo" => lines(&get(
                Target::Url(format!("{argument}/echo")),
                Some("PLUGIN_FENCE_KEY".to_owned()),
            )?),
            // A megabyte of words at every door a source has: a log line, a credential's name, and
            // the error it answers with (`malformed`, `cursor`, or a refusal of its own making).
            "megabyte" => {
                let words = megabyte();
                log::log(log::Level::Warn, &words);
                let _ = get(Target::SecretUrl(words.clone()), Some(words.clone()));
                Err(match argument {
                    "cursor" => SyncError::BadCursor(words),
                    "refused" => SyncError::Refused(fetch::Refusal::BadUrl(words)),
                    _ => SyncError::Malformed(words),
                })
            }
            "loop" => spin(),
            "grow" => hoard(),
            "panic" => panic!("the fence plugin panics on purpose"),
            // The names it was lent, one item each: what it may designate, never a value.
            "lent" => Ok(Synced {
                items: credentials
                    .into_iter()
                    .map(|credential| Item {
                        schema: SCHEMA.into(),
                        bytes: credential.name.into_bytes(),
                    })
                    .collect(),
                cursor: b"done".to_vec(),
            }),
            #[cfg(feature = "escapee")]
            "escape" => escape(),
            _ => Err(SyncError::BadCursor(format!("no mode {mode:?}"))),
        }
    }
}

bindings::export!(Fence with_types_in bindings);

fn get(target: Target, credential: Option<String>) -> Result<Response, SyncError> {
    fetch::fetch(&Request {
        verb: Verb::Get,
        target,
        headers: Vec::new(),
        credential,
    })
    .map_err(SyncError::Refused)
}

/// One item per line of the body, and a cursor saying it is done.
fn lines(response: &Response) -> Result<Synced, SyncError> {
    let body = std::str::from_utf8(&response.body)
        .map_err(|_| SyncError::Malformed("the body is not text".into()))?;
    let items: Vec<Item> = body
        .lines()
        .map(|line| Item {
            schema: SCHEMA.into(),
            bytes: line.as_bytes().to_vec(),
        })
        .collect();
    let now = clock::now();
    log::log(
        log::Level::Info,
        &format!(
            "{} lines at {}-{:02}-{:02}",
            items.len(),
            now.year,
            now.month,
            now.day
        ),
    );
    Ok(Synced {
        items,
        cursor: b"done".to_vec(),
    })
}

/// A mebibyte of text, one multi-byte character after another, so a cut must find a boundary.
fn megabyte() -> String {
    "é".repeat(1 << 19)
}

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

#[cfg(feature = "escapee")]
fn escape() -> Result<Synced, SyncError> {
    let directories = bindings::wasi::filesystem::preopens::get_directories();
    let network = bindings::wasi::sockets::instance_network::instance_network();
    Err(SyncError::Malformed(format!(
        "{} directories and network {network}",
        directories.len()
    )))
}
