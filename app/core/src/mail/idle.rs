//! The IDLE policy of `suzatary mail idle`, with no I/O.
//!
//! - [`Debounce`] folds a burst of wakes into one trigger at the end of a fixed
//!   window.
//! - [`backoff`] is the reconnect schedule after a dropped connection. It is not
//!   RFC 2177's 29-minute IDLE renewal ([`Wake::TimedOut`]); confusing the two
//!   would hammer a server or be slow to notice a disconnect.
//! - [`after`] is the state machine: a change triggers, a timeout re-issues
//!   IDLE, a drop reconnects with backoff ([`after_failure`], which a failed
//!   connect takes alone).

use std::time::{Duration, Instant};

/// What one blocking wait on an already-selected mailbox answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    /// The server reported `EXISTS` or `RECENT` — new mail.
    Changed,
    /// Nothing happened before the renewal deadline; routine, since a server may
    /// drop an IDLE past 30 minutes (RFC 2177), and never an error.
    TimedOut,
}

/// What the driver does next, after one watch attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Re-issue IDLE at once — the periodic renewal, with nothing to report.
    Reidle,
    /// The mailbox changed: hand the wake to the debounce window.
    Trigger,
    /// The connection is gone: wait this long, then reconnect.
    Reconnect(Duration),
}

/// Decide the next [`Step`] from one watch attempt's outcome.
///
/// `attempt` is the caller's reconnect counter, reset to 0 by any successful
/// watch, so a recovered connection starts [`backoff`]'s schedule over.
#[must_use]
pub fn after(outcome: Result<Wake, ()>, attempt: u32) -> (Step, u32) {
    match outcome {
        Ok(Wake::Changed) => (Step::Trigger, 0),
        Ok(Wake::TimedOut) => (Step::Reidle, 0),
        Err(()) => {
            let (delay, next) = after_failure(attempt);
            (Step::Reconnect(delay), next)
        }
    }
}

/// How long to wait after a failure, and the next attempt's counter.
#[must_use]
pub fn after_failure(attempt: u32) -> (Duration, u32) {
    (backoff(attempt), attempt.saturating_add(1))
}

/// Reconnect backoff: five seconds, doubling, never past five minutes.
///
/// The shift is capped at 6 (already past the ceiling) so it cannot overflow.
#[must_use]
pub fn backoff(attempt: u32) -> Duration {
    let seconds = 5u64.saturating_mul(1u64 << attempt.min(6));
    Duration::from_secs(seconds.min(300))
}

/// A debounce window: a wake inside an open window neither extends it nor opens
/// another, and its end fires exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Debounce {
    window: Duration,
    open: Option<Instant>,
}

impl Debounce {
    /// A debounce that fires `window` after its first wake.
    #[must_use]
    pub fn new(window: Duration) -> Self {
        Self { window, open: None }
    }

    /// A wake arrived at `now`; `true` when it opened a new window (the cue to
    /// [`Debounce::poll`] at [`Debounce::deadline`]), `false` when absorbed.
    pub fn wake(&mut self, now: Instant) -> bool {
        if self.open.is_none() {
            self.open = Some(now);
            true
        } else {
            false
        }
    }

    /// When the open window's deadline is, if one is open.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        self.open.map(|opened| opened + self.window)
    }

    /// Has the open window's deadline passed as of `now`? Firing closes the
    /// window, so polling on any schedule sees one `true` per window.
    pub fn poll(&mut self, now: Instant) -> bool {
        match self.deadline() {
            Some(deadline) if now >= deadline => {
                self.open = None;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // --- Debounce --------------------------------------------------------

    #[test]
    fn a_first_wake_opens_a_window() {
        let mut debounce = Debounce::new(Duration::from_secs(30));
        let now = Instant::now();
        assert!(debounce.wake(now), "the first wake opens a window");
        assert_eq!(debounce.deadline(), Some(now + Duration::from_secs(30)));
    }

    /// Wakes inside an open window are absorbed; the deadline does not move.
    #[test]
    fn wakes_inside_an_open_window_are_absorbed_and_extend_nothing() {
        let mut debounce = Debounce::new(Duration::from_secs(30));
        let first = Instant::now();
        assert!(debounce.wake(first));
        let original_deadline = debounce.deadline();

        let later = first + Duration::from_secs(20);
        assert!(!debounce.wake(later), "absorbed, not a new window");
        assert_eq!(
            debounce.deadline(),
            original_deadline,
            "the deadline did not move"
        );
    }

    /// The window's end fires once: not before the deadline, once at it, then
    /// silent.
    #[test]
    fn the_window_fires_once_at_its_end() {
        let mut debounce = Debounce::new(Duration::from_secs(30));
        let opened = Instant::now();
        debounce.wake(opened);

        assert!(
            !debounce.poll(opened + Duration::from_secs(29)),
            "not yet due"
        );
        assert!(
            debounce.poll(opened + Duration::from_secs(30)),
            "due at the deadline"
        );
        assert!(
            !debounce.poll(opened + Duration::from_secs(31)),
            "already fired — the window is closed"
        );
        assert_eq!(debounce.deadline(), None);
    }

    /// A wake after firing opens a fresh window, on its own new deadline.
    #[test]
    fn a_wake_after_firing_opens_a_fresh_window() {
        let mut debounce = Debounce::new(Duration::from_secs(30));
        let opened = Instant::now();
        debounce.wake(opened);
        assert!(debounce.poll(opened + Duration::from_secs(30)));

        let second = opened + Duration::from_secs(90);
        assert!(debounce.wake(second), "a fresh window, not absorbed");
        assert_eq!(debounce.deadline(), Some(second + Duration::from_secs(30)));
    }

    // --- backoff -----------------------------------------------------------

    #[test]
    fn backoff_starts_at_five_seconds_and_doubles() {
        assert_eq!(backoff(0), Duration::from_secs(5));
        assert_eq!(backoff(1), Duration::from_secs(10));
        assert_eq!(backoff(2), Duration::from_secs(20));
        assert_eq!(backoff(3), Duration::from_secs(40));
        assert_eq!(backoff(4), Duration::from_secs(80));
        assert_eq!(backoff(5), Duration::from_secs(160));
    }

    #[test]
    fn backoff_is_capped_at_five_minutes() {
        assert_eq!(backoff(6), Duration::from_mins(5));
        assert_eq!(backoff(7), Duration::from_mins(5));
        assert_eq!(backoff(1000), Duration::from_mins(5));
    }

    // --- after: the IDLE state machine, sans I/O ----------------------------

    /// A change triggers and resets the reconnect counter.
    #[test]
    fn a_change_triggers_and_resets_the_attempt_counter() {
        assert_eq!(after(Ok(Wake::Changed), 3), (Step::Trigger, 0));
        assert_eq!(after(Ok(Wake::Changed), 0), (Step::Trigger, 0));
    }

    /// The 29-minute renewal re-issues IDLE and is not a failure.
    #[test]
    fn a_timeout_reidles_and_is_not_an_error() {
        assert_eq!(after(Ok(Wake::TimedOut), 0), (Step::Reidle, 0));
    }

    /// A dropped connection reconnects with climbing backoff that resets on
    /// recovery.
    #[test]
    fn a_dropped_connection_reconnects_with_climbing_backoff_that_resets_on_recovery() {
        let (step, attempt) = after(Err(()), 0);
        assert_eq!(step, Step::Reconnect(Duration::from_secs(5)));
        assert_eq!(attempt, 1);

        let (step, attempt) = after(Err(()), attempt);
        assert_eq!(step, Step::Reconnect(Duration::from_secs(10)));
        assert_eq!(attempt, 2);

        // A wake of either kind resets the counter.
        let (step, attempt) = after(Ok(Wake::TimedOut), attempt);
        assert_eq!(step, Step::Reidle);
        assert_eq!(attempt, 0);

        let (step, attempt) = after(Err(()), attempt);
        assert_eq!(step, Step::Reconnect(Duration::from_secs(5)));
        assert_eq!(attempt, 1);
    }

    fn outcome() -> impl Strategy<Value = Result<Wake, ()>> {
        prop_oneof![
            Just(Ok(Wake::Changed)),
            Just(Ok(Wake::TimedOut)),
            Just(Err(())),
        ]
    }

    proptest! {
        /// Over any run of outcomes, the wait after the k-th failure in a row is `backoff(k -
        /// 1)`: it never shrinks along the run, stays within five seconds and five minutes,
        /// and any answer from the server starts it over.
        #[test]
        fn a_reconnect_waits_by_its_run_of_failures(
            outcomes in proptest::collection::vec(outcome(), 0..64),
            start in prop_oneof![Just(0_u32), Just(u32::MAX - 1), Just(u32::MAX)],
        ) {
            let mut attempt = start;
            // Whether the counter has started from zero, so the run is the whole count.
            let mut counted = start == 0;
            let mut run = 0_u32;
            let mut last = Duration::ZERO;
            for outcome in outcomes {
                let (step, next) = after(outcome, attempt);
                if let Step::Reconnect(delay) = step {
                    prop_assert_eq!((delay, next), after_failure(attempt));
                    prop_assert!(delay >= Duration::from_secs(5) && delay <= Duration::from_mins(5));
                    if counted {
                        prop_assert_eq!(delay, backoff(run));
                    }
                    prop_assert!(run == 0 || delay >= last);
                    run += 1;
                    last = delay;
                } else {
                    prop_assert_eq!(next, 0);
                    counted = true;
                    run = 0;
                }
                attempt = next;
            }
        }
    }
}
