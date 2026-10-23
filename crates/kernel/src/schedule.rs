//! What the module that keeps calendars tells the others: a member's time
//! zone and when they are free.
//!
//! Reading an email's dates needs the zone its reader lives in (a relative
//! "Thursday at 10" means 10:00 where they are), and holding time for a task
//! needs a free slot. The `schedule` module implements [`TimeZones`] and
//! [`Availability`]; the server registers them with
//! [`AppState::with_time_zones`](crate::AppState::with_time_zones) and
//! [`AppState::with_availability`](crate::AppState::with_availability).
//! Until then no member has a zone (readers use UTC) and nobody is free.

use std::{future::Future, pin::Pin, sync::Arc};

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    Error,
    tenancy::{UserId, WorkspaceId},
};

/// The zone readers use when a member has none (or there is no member).
pub const DEFAULT_TIME_ZONE: &str = "UTC";

/// A future returned by [`TimeZones::time_zone`].
pub type TimeZoneFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<String>, Error>> + Send + 'a>>;

/// Answers a member's time zone.
pub trait TimeZones: std::fmt::Debug + Send + Sync + 'static {
    /// The IANA name of `member`'s time zone in `workspace`, read in `conn`
    /// (a transaction working in that workspace); `None` while they have not
    /// set one, when readers use [`DEFAULT_TIME_ZONE`].
    fn time_zone<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        member: UserId,
    ) -> TimeZoneFuture<'a>;
}

/// The port before a module provides one: nobody has a time zone.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoTimeZones;

impl TimeZones for NoTimeZones {
    fn time_zone<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _member: UserId,
    ) -> TimeZoneFuture<'a> {
        Box::pin(async { Ok(None) })
    }
}

/// Shared handle to the registered [`TimeZones`] port.
pub type TimeZonesPort = Arc<dyn TimeZones>;

/// A stretch of time nothing is booked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeSlot {
    /// When it starts.
    pub starts: DateTime<Utc>,
    /// When it ends.
    pub ends: DateTime<Utc>,
}

/// The free slots found for a window, and the calendars that were counted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FreeSlots {
    /// The slots, earliest first, within the member's working hours.
    pub slots: Vec<FreeSlot>,
    /// The calendars (ids of the module that keeps them) whose events were
    /// counted as busy, so an answer can say what it looked at.
    pub calendars: Vec<Uuid>,
}

/// A future returned by [`Availability::free_slots`].
pub type FreeSlotsFuture<'a> = Pin<Box<dyn Future<Output = Result<FreeSlots, Error>> + Send + 'a>>;

/// Answers when a member is free.
pub trait Availability: std::fmt::Debug + Send + Sync + 'static {
    /// The slots of at least `duration` within `from..to` that fall in
    /// `member`'s working hours (in their time zone) and that no event of a
    /// syncing calendar, or of their Campbooks calendar marked busy,
    /// overlaps. Cancelled and all-day events do not count. Read in `conn`,
    /// a transaction working in `workspace`.
    fn free_slots<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        member: UserId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        duration: TimeDelta,
    ) -> FreeSlotsFuture<'a>;
}

/// The port before a module provides one: nobody is ever free.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAvailability;

impl Availability for NoAvailability {
    fn free_slots<'a>(
        &'a self,
        _conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        _member: UserId,
        _from: DateTime<Utc>,
        _to: DateTime<Utc>,
        _duration: TimeDelta,
    ) -> FreeSlotsFuture<'a> {
        Box::pin(async { Ok(FreeSlots::default()) })
    }
}

/// Shared handle to the registered [`Availability`] port.
pub type AvailabilityPort = Arc<dyn Availability>;

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    #[sqlx::test(migrations = "../../migrations")]
    async fn before_a_module_provides_them_nobody_has_a_zone_and_nobody_is_free(db: PgPool) {
        let mut conn = db.acquire().await.unwrap();
        let (workspace, member) = (WorkspaceId::new(), UserId::new());
        let zone = NoTimeZones
            .time_zone(&mut conn, workspace, member)
            .await
            .unwrap();
        assert_eq!(zone, None);
        let now = Utc::now();
        let free = NoAvailability
            .free_slots(
                &mut conn,
                workspace,
                member,
                now,
                now + TimeDelta::days(1),
                TimeDelta::hours(1),
            )
            .await
            .unwrap();
        assert_eq!(free, FreeSlots::default());
    }
}
