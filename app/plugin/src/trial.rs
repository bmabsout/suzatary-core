//! A sync and its replays (the plugins plan, §5): the live run records every answer the outside world gave it, and the
//! replays are answered from that record alone, at the live run's instant.

use std::sync::{Arc, Mutex, PoisonError};

use suzatary_core::capability::{Clock, Datetime};
use suzatary_io::http::{Http, HttpError, Method, Req, Resp};

use crate::{Effects, Run, Source};

/// One sync and the two replays its laws are checked over (the plugins plan, §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trial {
    /// The sync, live, its every answer recorded.
    pub run: Run,
    /// The same cursor again, answered from the record.
    pub again: Run,
    /// From the live run's own cursor, answered from the record; `None` when the live run gave
    /// no cursor.
    pub onward: Option<Run>,
}

/// What a request asked, as a replay matches it: never its deadline or its limit.
type Asked = (Method, String, Vec<(String, String)>, Vec<u8>);

fn asked(req: &Req) -> Asked {
    (
        req.method,
        req.url.clone(),
        req.headers.clone(),
        req.body.clone(),
    )
}

/// The transport, with every answer it gave kept in memory for the replays. The record holds
/// what a credential put in a request, so it lives as long as the trial and is never written.
struct Recording {
    inner: Arc<dyn Http>,
    answers: Mutex<Vec<(Asked, Result<Resp, HttpError>)>>,
}

impl Http for Recording {
    fn send(&self, req: Req) -> Result<Resp, HttpError> {
        let key = asked(&req);
        let answer = self.inner.send(req);
        self.answers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((key, answer.clone()));
        answer
    }
}

/// The record, answering each request as the live run's was answered; a request the live run
/// never made was never sent.
struct Replay(Vec<(Asked, Result<Resp, HttpError>)>);

impl Http for Replay {
    fn send(&self, req: Req) -> Result<Resp, HttpError> {
        let key = asked(&req);
        self.0.iter().find(|(made, _)| *made == key).map_or_else(
            || {
                Err(HttpError::NotSent(
                    "the live run made no such request".to_owned(),
                ))
            },
            |(_, answer)| answer.clone(),
        )
    }
}

/// One instant, whenever it is read.
struct Frozen(Datetime);

impl Clock for Frozen {
    fn now(&self) -> Datetime {
        self.0
    }
}

impl Source {
    /// Sync from `cursor` with `effects`, at one instant, and replay it: from the same cursor,
    /// then from the cursor it returned, each answered from what the live run was answered.
    #[must_use]
    pub fn trial(&self, effects: &Effects, cursor: Option<&[u8]>) -> Trial {
        let clock: Arc<dyn Clock + Send> = Arc::new(Frozen(effects.clock.now()));
        let recording = Arc::new(Recording {
            inner: Arc::clone(&effects.http),
            answers: Mutex::new(Vec::new()),
        });
        let live = Effects {
            http: Arc::clone(&recording) as Arc<dyn Http>,
            clock: Arc::clone(&clock),
            secrets: Arc::clone(&effects.secrets),
        };
        let run = self.sync(live, cursor);
        let answers = std::mem::take(
            &mut *recording
                .answers
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        let replayed = Effects {
            http: Arc::new(Replay(answers)),
            clock,
            secrets: Arc::clone(&effects.secrets),
        };
        let again = self.sync(replayed.clone(), cursor);
        let onward = run
            .result
            .as_ref()
            .ok()
            .map(|synced| self.sync(replayed, Some(&synced.cursor)));
        Trial { run, again, onward }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU16, Ordering};

    use super::*;

    /// A world that answers differently every time it is asked.
    struct Restless(AtomicU16);

    impl Http for Restless {
        fn send(&self, _: Req) -> Result<Resp, HttpError> {
            Ok(Resp {
                status: 200 + self.0.fetch_add(1, Ordering::SeqCst),
                headers: Vec::new(),
                body: Vec::new(),
            })
        }
    }

    #[test]
    fn a_replay_answers_as_the_live_run_was_answered_and_sends_nothing_new() {
        let recording = Recording {
            inner: Arc::new(Restless(AtomicU16::new(0))),
            answers: Mutex::new(Vec::new()),
        };
        let feed = || Req::get("https://calendar.example.test/feed.ics");
        assert_eq!(recording.send(feed()).map(|resp| resp.status), Ok(200));
        assert_eq!(
            recording
                .send(Req::get("https://calendar.example.test/other.ics"))
                .map(|resp| resp.status),
            Ok(201)
        );
        let replay = Replay(recording.answers.into_inner().expect("a record"));
        for _ in 0..3 {
            assert_eq!(replay.send(feed()).map(|resp| resp.status), Ok(200));
        }
        assert!(matches!(
            replay.send(Req::get("https://calendar.example.test/new.ics")),
            Err(HttpError::NotSent(_))
        ));
        assert_eq!(
            replay
                .send(feed().timeout(std::time::Duration::from_secs(1)))
                .map(|resp| resp.status),
            Ok(200),
            "a deadline is not what was asked"
        );
    }
}
