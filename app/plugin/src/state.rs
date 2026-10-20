//! One call's store data: the manifest, the effects lent to it, and what the host records.

use std::sync::Arc;
use std::time::Instant;

use suzatary_core::plugin::{Manifest, Outgoing, Refusal, Response, Said, Verb};
use suzatary_io::http::{HttpError, Req};

use crate::budget::Limits;
use crate::world::{self, clock, fetch, log};
use crate::{Effects, Fetch, Level, Log};

/// The most of an answer's body read.
const MAX_RESPONSE: u64 = 8 * 1024 * 1024;

/// What the imports of one call act on. Built fresh per call and dropped with its store.
pub(crate) struct State {
    pub(crate) manifest: Arc<Manifest>,
    pub(crate) effects: Effects,
    /// When the call's time is up, for a fetch's own timeout.
    pub(crate) deadline: Instant,
    pub(crate) limits: Limits,
    pub(crate) refused: Vec<Refusal>,
    pub(crate) fetches: Vec<Fetch>,
    pub(crate) log: Log,
}

impl State {
    fn send(&mut self, request: fetch::Request) -> Result<Response, Refusal> {
        let reason = format!("plugin {}: a declared credential", self.manifest.name());
        let secrets = &self.effects.secrets;
        let outgoing = world::request(request)
            .admit(&self.manifest, |name| secrets.get(name.as_str(), &reason))?;
        let left = self
            .deadline
            .checked_duration_since(Instant::now())
            .ok_or(Refusal::TimedOut)?;
        let answered = self
            .effects
            .http
            .send(req(&outgoing).timeout(left).limit(MAX_RESPONSE))
            .map_err(|failed| match failed {
                HttpError::NotSent(_) | HttpError::Private { .. } => Refusal::Unreachable,
                HttpError::TimedOut(_) => Refusal::TimedOut,
                HttpError::Lost(_) => Refusal::Lost,
            })?;
        outgoing.answer(Response {
            status: answered.status,
            headers: answered.headers,
            body: answered.body,
        })
    }
}

/// The transport's request, built from what was admitted and nothing else.
fn req(outgoing: &Outgoing) -> Req {
    let url = outgoing.url().to_string();
    let req = match outgoing.verb() {
        Verb::Get => Req::get(url),
        Verb::Post(body) => Req::post(url, body.clone()),
    };
    outgoing.headers().iter().fold(req, |req, (name, value)| {
        req.header(name.as_str(), value.expose())
    })
}

impl fetch::Host for State {
    fn fetch(&mut self, request: fetch::Request) -> Result<fetch::Response, fetch::Refusal> {
        // The ledger keys a link by this name, so only a name the manifest declares is kept;
        // an undeclared one is refused, and its refusal says it, clipped.
        let credential = match &request.target {
            fetch::Target::SecretUrl(name) => Some(name),
            fetch::Target::Url(_) => request.credential.as_ref(),
        }
        .and_then(|name| self.manifest.placement(name))
        .map(|(declared, _)| declared.clone());
        let sent = self.send(request);
        self.fetches.push(Fetch {
            credential,
            outcome: sent
                .as_ref()
                .map(|response| response.status)
                .map_err(Clone::clone),
        });
        sent.map(world::response).map_err(|refused| {
            self.refused.push(refused.clone());
            world::refusal(refused)
        })
    }
}

impl clock::Host for State {
    fn now(&mut self) -> clock::Datetime {
        world::datetime(self.effects.clock.now())
    }
}

impl log::Host for State {
    fn log(&mut self, level: log::Level, message: String) {
        let level = match level {
            log::Level::Debug => Level::Debug,
            log::Level::Info => Level::Info,
            log::Level::Warn => Level::Warn,
        };
        self.log.push(level, Said::clip(message));
    }
}
