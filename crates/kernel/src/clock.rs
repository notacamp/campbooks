//! Time as a dependency, so logic that depends on "now" is testable.

use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, TimeDelta, Utc};

/// A source of the current time.
pub trait Clock: Send + Sync + 'static {
    /// The current instant, in UTC.
    fn now(&self) -> DateTime<Utc>;
}

/// The real wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// A clock frozen at a given instant that only moves when told to. For tests.
#[derive(Debug)]
pub struct FixedClock(Mutex<DateTime<Utc>>);

impl FixedClock {
    /// A clock stopped at `at`.
    pub fn new(at: DateTime<Utc>) -> Self {
        Self(Mutex::new(at))
    }

    /// Jump to `at`.
    pub fn set(&self, at: DateTime<Utc>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = at;
    }

    /// Move forward (or backward, with a negative delta) by `by`.
    pub fn advance(&self, by: TimeDelta) {
        let mut now = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        *now += by;
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_clock_stays_put_until_advanced() {
        let start = DateTime::parse_from_rfc3339("2026-01-01T09:00:00Z")
            .unwrap()
            .to_utc();
        let clock = FixedClock::new(start);
        assert_eq!(clock.now(), start);
        clock.advance(TimeDelta::hours(2));
        assert_eq!(clock.now(), start + TimeDelta::hours(2));
        clock.set(start);
        assert_eq!(clock.now(), start);
    }

    #[test]
    fn system_clock_moves() {
        let clock: &dyn Clock = &SystemClock;
        assert!(clock.now() <= Utc::now());
    }
}
