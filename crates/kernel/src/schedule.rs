//! What the module that keeps calendars tells the others: a member's time
//! zone and when they are free, and the time they hold for a task.
//!
//! Reading an email's dates needs the zone its reader lives in (a relative
//! "Thursday at 10" means 10:00 where they are), and holding time for a task
//! needs a free slot and a block on the member's calendar. The `schedule`
//! module implements [`TimeZones`], [`Availability`] and [`Holds`]; the
//! server registers them with
//! [`AppState::with_time_zones`](crate::AppState::with_time_zones),
//! [`AppState::with_availability`](crate::AppState::with_availability) and
//! [`AppState::with_holds`](crate::AppState::with_holds). Until then no
//! member has a zone (readers use UTC), nobody is free and no time can be
//! held.

use std::{future::Future, pin::Pin, sync::Arc};

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    AppState, Error,
    records::RecordRef,
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

/// Time a member holds on their calendar for a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldRequest<'a> {
    /// The member whose calendar it goes on.
    pub member: UserId,
    /// The task it is held for (`tasks.task:<id>`): the event's source, so
    /// opening the block leads back to it and its changes come back to it.
    pub task: RecordRef,
    /// What the block is called: the task's title. It stays in the module
    /// that keeps calendars (and the provider's), never in an event or a job.
    pub title: &'a str,
    /// When it starts.
    pub starts: DateTime<Utc>,
    /// When it ends.
    pub ends: DateTime<Utc>,
    /// One of the member's writable calendars; `None`: their Campbooks
    /// calendar.
    pub calendar: Option<Uuid>,
}

/// The block a [`Holds::hold`] made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    /// The event (an id of the module that keeps calendars).
    pub event: Uuid,
    /// The calendar it is on.
    pub calendar: Uuid,
    /// Whether that calendar is a provider's (the block is asked of it) rather
    /// than the member's Campbooks calendar.
    pub at_provider: bool,
}

/// A future returned by [`Holds::hold`].
pub type HoldFuture<'a> = Pin<Box<dyn Future<Output = Result<Held, Error>> + Send + 'a>>;

/// A future returned by [`Holds::release`].
pub type ReleaseFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;

/// Puts time a member holds for a task on their calendar, and takes it off.
pub trait Holds: std::fmt::Debug + Send + Sync + 'static {
    /// Make a busy event of kind `hold` for `request` on the member's
    /// calendar, in `tx` (a transaction working in `workspace`), so the
    /// block exists exactly when the caller's change commits. A calendar that
    /// is not the member's is [`Error::NotFound`]; one that cannot be written
    /// is refused (`calendar_read_only`). A provider calendar is asked for it
    /// like any event the member makes.
    fn hold<'a>(
        &'a self,
        state: &'a AppState,
        tx: &'a mut Transaction<'static, Postgres>,
        workspace: WorkspaceId,
        request: HoldRequest<'a>,
    ) -> HoldFuture<'a>;

    /// Take `member`'s hold `event` off their calendar, in `tx`. An event
    /// that is gone, or that is not a hold of that member, is left alone.
    fn release<'a>(
        &'a self,
        state: &'a AppState,
        tx: &'a mut Transaction<'static, Postgres>,
        workspace: WorkspaceId,
        member: UserId,
        event: Uuid,
    ) -> ReleaseFuture<'a>;
}

/// What [`NoHolds`] answers.
fn unsupported() -> Error {
    Error::Unavailable(crate::i18n::MESSAGES.reason("holds_unsupported"))
}

/// The port before a module provides one: no time can be held.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHolds;

impl Holds for NoHolds {
    fn hold<'a>(
        &'a self,
        _state: &'a AppState,
        _tx: &'a mut Transaction<'static, Postgres>,
        _workspace: WorkspaceId,
        _request: HoldRequest<'a>,
    ) -> HoldFuture<'a> {
        Box::pin(async { Err(unsupported()) })
    }

    fn release<'a>(
        &'a self,
        _state: &'a AppState,
        _tx: &'a mut Transaction<'static, Postgres>,
        _workspace: WorkspaceId,
        _member: UserId,
        _event: Uuid,
    ) -> ReleaseFuture<'a> {
        Box::pin(async { Err(unsupported()) })
    }
}

/// Shared handle to the registered [`Holds`] port.
pub type HoldsPort = Arc<dyn Holds>;

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

    #[sqlx::test(migrations = "../../migrations")]
    async fn before_a_module_provides_them_no_time_can_be_held(db: PgPool) {
        let state = crate::testing::state(db.clone());
        let mut tx = db.begin().await.unwrap();
        let (workspace, member) = (WorkspaceId::new(), UserId::new());
        let now = Utc::now();
        let refused = NoHolds
            .hold(
                &state,
                &mut tx,
                workspace,
                HoldRequest {
                    member,
                    task: RecordRef::new("tasks.task", &Uuid::nil()),
                    title: "Send the signed lease",
                    starts: now + TimeDelta::hours(1),
                    ends: now + TimeDelta::hours(2),
                    calendar: None,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(refused.reason().code(), "holds_unsupported");
        let release = NoHolds
            .release(&state, &mut tx, workspace, member, Uuid::nil())
            .await
            .unwrap_err();
        assert_eq!(
            release.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
