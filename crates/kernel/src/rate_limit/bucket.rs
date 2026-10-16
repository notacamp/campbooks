//! Token buckets kept in this process's memory, for limits that need not be
//! exact across processes (the API's reads, writes and admin calls).
//!
//! Each bucket is one timestamp (GCRA, the "generic cell rate algorithm"):
//! the instant its budget would be full again, its *theoretical arrival
//! time*. A limit of `max` per `window` lets `max` hits arrive at once, then
//! one every `window / max`. A refused hit is not counted, so the answer's
//! wait is exact.
//!
//! Memory is bounded: a bucket whose time has passed is full and carries no
//! information, so a sweep (at most once a [`SWEEP_EVERY`]) drops those; and
//! when [`Buckets::capacity`] buckets are held and a new one is needed, the
//! sweep runs at once and, if that is not enough, the buckets closest to
//! full are dropped first. Buckets are keyed by a hash under a per-process
//! random key: no subject or client id is kept.

use std::{
    collections::HashMap,
    hash::{BuildHasher as _, Hash, RandomState},
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use chrono::{DateTime, TimeDelta, Utc};

use crate::clock::Clock;

/// How often full buckets are swept.
pub(crate) const SWEEP_EVERY: TimeDelta = TimeDelta::minutes(1);

/// Buckets held at most by default (about 4 MB).
pub(crate) const DEFAULT_CAPACITY: usize = 100_000;

/// The answer to one hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Counted.
    Allowed,
    /// Over the limit: the next hit is allowed in `retry_after` seconds (at
    /// least 1). `first` when the bucket let the previous hit through, so a
    /// caller hammering on is reported once.
    Refused { retry_after: u64, first: bool },
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    /// When the budget is full again.
    tat: DateTime<Utc>,
    /// Whether the last hit was refused.
    refused: bool,
}

struct Inner {
    buckets: HashMap<u64, Bucket>,
    next_sweep: DateTime<Utc>,
}

/// The buckets of one process. Cheap to clone; clones share them.
#[derive(Clone)]
pub(crate) struct Buckets {
    clock: Arc<dyn Clock>,
    hasher: RandomState,
    capacity: usize,
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for Buckets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buckets")
            .field("held", &self.len())
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

impl Buckets {
    /// Buckets timed by `clock`, holding at most `capacity`.
    pub(crate) fn new(clock: Arc<dyn Clock>, capacity: usize) -> Self {
        let next_sweep = clock.now() + SWEEP_EVERY;
        Self {
            clock,
            hasher: RandomState::new(),
            capacity: capacity.max(1),
            inner: Arc::new(Mutex::new(Inner {
                buckets: HashMap::new(),
                next_sweep,
            })),
        }
    }

    /// How many buckets are held.
    pub(crate) fn len(&self) -> usize {
        self.lock().buckets.len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Count one hit of `key` against `max` per `window`. `max` must be at
    /// least 1 (a limit of 0 is "off" and never reaches here).
    pub(crate) fn hit(&self, key: &impl Hash, max: u32, window: Duration) -> Verdict {
        let now = self.clock.now();
        let window = TimeDelta::from_std(window).unwrap_or(TimeDelta::MAX);
        let interval = window / i32::try_from(max.max(1)).unwrap_or(i32::MAX);
        let key = self.hasher.hash_one(key);
        let mut inner = self.lock();
        if now >= inner.next_sweep {
            inner.buckets.retain(|_, bucket| bucket.tat > now);
            inner.next_sweep = now + SWEEP_EVERY;
        }
        // A clock that went back never leaves a bucket further than one
        // window ahead.
        let tat = inner
            .buckets
            .get(&key)
            .map_or(now, |bucket| bucket.tat.clamp(now, now + window));
        let next = tat + interval;
        if next - now > window {
            // Positive: `next` is more than a window ahead.
            let wait = next - window - now;
            let retry_after = wait.num_seconds() + i64::from(wait.subsec_nanos() > 0);
            let first = match inner.buckets.get_mut(&key) {
                Some(bucket) => !std::mem::replace(&mut bucket.refused, true),
                None => true,
            };
            return Verdict::Refused {
                retry_after: retry_after.max(1).unsigned_abs(),
                first,
            };
        }
        if !inner.buckets.contains_key(&key) && inner.buckets.len() >= self.capacity {
            make_room(&mut inner.buckets, self.capacity, now);
        }
        inner.buckets.insert(
            key,
            Bucket {
                tat: next,
                refused: false,
            },
        );
        Verdict::Allowed
    }
}

/// Drop full buckets, then, if still at `capacity`, the ones closest to full
/// until a tenth of the room is free.
fn make_room(buckets: &mut HashMap<u64, Bucket>, capacity: usize, now: DateTime<Utc>) {
    buckets.retain(|_, bucket| bucket.tat > now);
    if buckets.len() < capacity {
        return;
    }
    let keep = capacity - capacity / 10 - 1;
    let mut by_tat: Vec<(DateTime<Utc>, u64)> = buckets
        .iter()
        .map(|(key, bucket)| (bucket.tat, *key))
        .collect();
    let drop = by_tat.len().saturating_sub(keep);
    if drop == 0 {
        return;
    }
    by_tat.select_nth_unstable(drop - 1);
    for (_, key) in &by_tat[..drop] {
        buckets.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;

    fn start() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-16T09:00:00Z")
            .unwrap()
            .to_utc()
    }

    fn buckets(capacity: usize) -> (Arc<FixedClock>, Buckets) {
        let clock = Arc::new(FixedClock::new(start()));
        let buckets = Buckets::new(clock.clone(), capacity);
        (clock, buckets)
    }

    const MINUTE: Duration = Duration::from_mins(1);

    #[test]
    fn a_burst_of_max_then_one_per_interval() {
        let (clock, buckets) = buckets(10);
        for _ in 0..6 {
            assert_eq!(buckets.hit(&"ana", 6, MINUTE), Verdict::Allowed);
        }
        // One every ten seconds from here.
        assert_eq!(
            buckets.hit(&"ana", 6, MINUTE),
            Verdict::Refused {
                retry_after: 10,
                first: true
            }
        );
        clock.advance(TimeDelta::seconds(4));
        assert_eq!(
            buckets.hit(&"ana", 6, MINUTE),
            Verdict::Refused {
                retry_after: 6,
                first: false
            },
            "a refused hit does not count, and is reported once"
        );
        clock.advance(TimeDelta::seconds(6));
        assert_eq!(buckets.hit(&"ana", 6, MINUTE), Verdict::Allowed);
        assert!(matches!(
            buckets.hit(&"ana", 6, MINUTE),
            Verdict::Refused { first: true, .. }
        ));
        // A whole window later the whole burst is back.
        clock.advance(TimeDelta::minutes(1));
        for _ in 0..6 {
            assert_eq!(buckets.hit(&"ana", 6, MINUTE), Verdict::Allowed);
        }
    }

    #[test]
    fn retry_after_rounds_up_to_whole_seconds() {
        let (clock, buckets) = buckets(10);
        // 600 a minute: one every 100 ms.
        for _ in 0..600 {
            assert_eq!(buckets.hit(&"ana", 600, MINUTE), Verdict::Allowed);
        }
        assert_eq!(
            buckets.hit(&"ana", 600, MINUTE),
            Verdict::Refused {
                retry_after: 1,
                first: true
            }
        );
        clock.advance(TimeDelta::milliseconds(100));
        assert_eq!(buckets.hit(&"ana", 600, MINUTE), Verdict::Allowed);
    }

    #[test]
    fn keys_and_limits_do_not_share() {
        let (_clock, buckets) = buckets(10);
        assert_eq!(buckets.hit(&"ana", 1, MINUTE), Verdict::Allowed);
        assert!(matches!(
            buckets.hit(&"ana", 1, MINUTE),
            Verdict::Refused { .. }
        ));
        assert_eq!(buckets.hit(&"bea", 1, MINUTE), Verdict::Allowed);
        assert_eq!(buckets.hit(&("ana", "write"), 1, MINUTE), Verdict::Allowed);
    }

    #[test]
    fn a_clock_going_back_does_not_stretch_a_wait() {
        let (clock, buckets) = buckets(10);
        assert_eq!(buckets.hit(&"ana", 1, MINUTE), Verdict::Allowed);
        clock.advance(TimeDelta::hours(-1));
        assert_eq!(
            buckets.hit(&"ana", 1, MINUTE),
            Verdict::Refused {
                retry_after: 60,
                first: true
            }
        );
    }

    #[test]
    fn full_buckets_are_swept() {
        let (clock, buckets) = buckets(100);
        for who in 0..50 {
            buckets.hit(&who, 10, MINUTE);
        }
        assert_eq!(buckets.len(), 50);
        // Still holding budget a second later: nothing to drop yet.
        clock.advance(TimeDelta::seconds(1));
        buckets.hit(&"x", 10, MINUTE);
        assert_eq!(buckets.len(), 51);
        // Past the sweep: only the bucket that is not full stays.
        clock.advance(SWEEP_EVERY);
        buckets.hit(&"y", 1, MINUTE);
        assert_eq!(buckets.len(), 1);
    }

    #[test]
    fn at_capacity_the_buckets_closest_to_full_go_first() {
        let (_clock, buckets) = buckets(10);
        // `heavy` uses its whole budget; the others one hit each.
        for _ in 0..5 {
            buckets.hit(&"heavy", 5, MINUTE);
        }
        for who in 0..9 {
            buckets.hit(&who, 5, MINUTE);
        }
        assert_eq!(buckets.len(), 10);
        // At capacity, with no full bucket: the nearly full ones are dropped
        // (a free tenth and one more), the limited one is kept.
        buckets.hit(&"new", 5, MINUTE);
        assert!(buckets.len() <= 9, "{}", buckets.len());
        assert!(matches!(
            buckets.hit(&"heavy", 5, MINUTE),
            Verdict::Refused { .. }
        ));
    }
}
