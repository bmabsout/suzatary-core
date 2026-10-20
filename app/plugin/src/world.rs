//! The worlds' bindings, generated from `app/wit/plugin.wit`, and the crossings between their
//! types and `suzatary_core::plugin`'s.

use suzatary_core::capability::Datetime;
use suzatary_core::plugin::{
    Item, Manifest, Placement, Refusal, Request, Response, Said, Synced, Target, Verb,
};

use crate::RunError;

wasmtime::component::bindgen!({
    path: "../wit",
    world: "source",
});

pub(crate) use exports::suzatary::plugin::sync as exported;
pub(crate) use suzatary::plugin::{clock, fetch, log};

/// The `actor` world's bindings. Its imports are the source world's clock and log by name and
/// type, so the one linker that holds the source world's imports instantiates it too.
pub(crate) mod actor {
    wasmtime::component::bindgen!({
        path: "../wit",
        world: "actor",
    });

    pub(crate) use exports::suzatary::plugin::decide;
}

/// The names of the credentials `manifest` declares, each with its one use: designations the
/// host honours through `fetch`, never values.
pub(crate) fn credentials(manifest: &Manifest) -> Vec<exported::Credential> {
    manifest
        .credentials()
        .map(|(name, placement)| exported::Credential {
            name: name.as_str().to_owned(),
            into: match placement {
                Placement::Url => exported::Placement::Url,
                Placement::Bearer { .. } => exported::Placement::Bearer,
                Placement::Header { .. } => exported::Placement::Header,
            },
        })
        .collect()
}

/// The request as the plugin wrote it, untrusted, in the core's type.
pub(crate) fn request(request: fetch::Request) -> Request {
    Request {
        verb: match request.verb {
            fetch::Verb::Get => Verb::Get,
            fetch::Verb::Post(body) => Verb::Post(body),
        },
        target: match request.target {
            fetch::Target::Url(url) => Target::Url(url),
            fetch::Target::SecretUrl(name) => Target::SecretUrl(name),
        },
        headers: request
            .headers
            .into_iter()
            .map(|header| (header.name, header.value))
            .collect(),
        credential: request.credential,
    }
}

pub(crate) fn response(response: Response) -> fetch::Response {
    fetch::Response {
        status: response.status,
        headers: response
            .headers
            .into_iter()
            .map(|(name, value)| fetch::Header { name, value })
            .collect(),
        body: response.body,
    }
}

pub(crate) fn refusal(refusal: Refusal) -> fetch::Refusal {
    match refusal {
        Refusal::BadUrl(said) => fetch::Refusal::BadUrl(said.to_string()),
        Refusal::OriginNotAllowed(said) => fetch::Refusal::OriginNotAllowed(said.to_string()),
        Refusal::CredentialNotDeclared(said) => {
            fetch::Refusal::CredentialNotDeclared(said.to_string())
        }
        Refusal::CredentialUnset(said) => fetch::Refusal::CredentialUnset(said.to_string()),
        Refusal::CredentialUnavailable(_) | Refusal::Unreachable => fetch::Refusal::Unreachable,
        Refusal::CredentialNotAUrl(said) => fetch::Refusal::CredentialNotAUrl(said.to_string()),
        Refusal::InTheClear(said) => fetch::Refusal::InTheClear(said.to_string()),
        Refusal::BadHeader(said) => fetch::Refusal::BadHeader(said.to_string()),
        Refusal::Echoed(said) => fetch::Refusal::Echoed(said.to_string()),
        Refusal::TimedOut => fetch::Refusal::TimedOut,
        Refusal::Lost => fetch::Refusal::Lost,
    }
}

/// A refusal as the plugin reported it back, in the core's type: its words whatever it put
/// there, so each is clipped as it crosses.
pub(crate) fn refused(refusal: fetch::Refusal) -> Refusal {
    match refusal {
        fetch::Refusal::BadUrl(said) => Refusal::BadUrl(Said::clip(said)),
        fetch::Refusal::OriginNotAllowed(said) => Refusal::OriginNotAllowed(Said::clip(said)),
        fetch::Refusal::CredentialNotDeclared(said) => {
            Refusal::CredentialNotDeclared(Said::clip(said))
        }
        fetch::Refusal::CredentialUnset(said) => Refusal::CredentialUnset(Said::clip(said)),
        fetch::Refusal::CredentialNotAUrl(said) => Refusal::CredentialNotAUrl(Said::clip(said)),
        fetch::Refusal::InTheClear(said) => Refusal::InTheClear(Said::clip(said)),
        fetch::Refusal::BadHeader(said) => Refusal::BadHeader(Said::clip(said)),
        fetch::Refusal::Echoed(said) => Refusal::Echoed(Said::clip(said)),
        fetch::Refusal::Unreachable => Refusal::Unreachable,
        fetch::Refusal::TimedOut => Refusal::TimedOut,
        fetch::Refusal::Lost => Refusal::Lost,
    }
}

/// What a source learned, in the core's type: items and a cursor, each bounded by its own door
/// (`plugin::admit`, the cursor file), never read here.
pub(crate) fn synced(synced: exported::Synced) -> Synced {
    Synced {
        items: synced
            .items
            .into_iter()
            .map(|item| Item {
                schema: item.schema,
                bytes: item.bytes,
            })
            .collect(),
        cursor: synced.cursor,
    }
}

/// Why a source learned nothing, as it said: its words clipped as they cross.
pub(crate) fn sync_error(error: exported::SyncError) -> RunError {
    match error {
        exported::SyncError::Refused(refusal) => RunError::Refused(refused(refusal)),
        exported::SyncError::Malformed(said) => RunError::Malformed(Said::clip(said)),
        exported::SyncError::BadCursor(said) => RunError::BadCursor(Said::clip(said)),
    }
}

/// Why an actor proposed nothing, as it said: its words clipped as they cross.
pub(crate) fn decide_error(error: actor::decide::DecideError) -> RunError {
    let actor::decide::DecideError::Malformed(said) = error;
    RunError::Malformed(Said::clip(said))
}

/// `literal::Datetime` holds months, days and clock fields in their calendar ranges, so each
/// fits a byte.
pub(crate) fn datetime(now: Datetime) -> clock::Datetime {
    let byte = |field: u32| u8::try_from(field).unwrap_or(u8::MAX);
    clock::Datetime {
        year: now.year(),
        month: byte(now.month()),
        day: byte(now.day()),
        hour: byte(now.hour()),
        minute: byte(now.minute()),
        second: byte(now.second()),
        microsecond: now.microsecond(),
    }
}
