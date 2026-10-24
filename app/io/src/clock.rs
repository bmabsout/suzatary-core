//! The system clock, as an interpreter of [`Clock`].
//!
//! UTC, whatever zone the machine is set to: an `at` is a point in time, spelt naive because the
//! Prodrome's literal has no zone, and UTC by definition so every writer spells one moment alike.
//! `at` is data: nothing orders by it, and a wrong clock is reported, never clamped here.

use suzatary_core::capability::{Clock, Datetime};

/// The box's own wall clock; a clock that could be pointed elsewhere would be a fake.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Datetime {
        // `literal::Datetime` admits years 1..=9999, which a set system clock cannot leave.
        prodrome_core::fpl::datetime_of(chrono::Utc::now().naive_utc())
            .expect("the system clock reads a year in 1..=9999")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The machine's zone never enters: the clock reads UTC on a box set to any zone.
    #[test]
    fn the_system_clock_reads_utc() {
        let utc = prodrome_core::fpl::datetime_of(chrono::Utc::now().naive_utc())
            .expect("a year in range");
        let read = SystemClock.now();
        let apart = prodrome_core::fpl::instant_of(read) - prodrome_core::fpl::instant_of(utc);
        assert!(
            apart.num_seconds().abs() < 60,
            "{read:?} is not UTC's {utc:?}"
        );
    }
}
