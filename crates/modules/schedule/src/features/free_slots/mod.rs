//! `GET /api/v1/w/{workspace_id}/schedule/free-slots` — when the caller is
//! free: the stretches of at least `minutes` within their working hours, in
//! their time zone, that no event of a calendar they see overlaps. The answer
//! names the calendars it counted. The same answer is the kernel's
//! `Availability` port for other modules.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    schedule::{Availability, AvailabilityPort, FreeSlot, FreeSlots},
    tenancy::{UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, NaiveTime, TimeDelta, TimeZone as _, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        free::{self, Found},
        moment::Moment,
        ops,
        rows::calendars_of,
        settings,
    },
    model::CalendarId,
    scopes::ScheduleRead,
};

/// The longest window, in days.
pub const MAX_WINDOW_DAYS: i64 = 62;
/// The shortest and longest slot asked for, in minutes.
pub const MINUTES: std::ops::RangeInclusive<u32> = 5..=1_440;

/// When to look.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FreeQuery {
    /// Start of the window: an RFC 3339 time with an offset, or a date (the
    /// start of that day in your time zone).
    #[param(example = "2026-11-03")]
    pub from: String,
    /// End of the window, in the same form, after `from`, at most 62 days
    /// later.
    #[param(example = "2026-11-04")]
    pub to: String,
    /// The length of slot to look for, 5 to 1,440 minutes.
    #[param(example = 60)]
    pub minutes: u32,
}

/// A free stretch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SlotView {
    /// When it starts: RFC 3339 with the offset of your time zone.
    pub starts: String,
    /// When it ends.
    pub ends: String,
}

/// A calendar whose events were counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CountedCalendar {
    /// The calendar.
    pub id: CalendarId,
    /// Its name.
    pub name: String,
}

/// When you are free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct FreeSlotList {
    /// Earliest first.
    pub slots: Vec<SlotView>,
    /// The calendars whose events were counted as busy.
    pub calendars: Vec<CountedCalendar>,
    /// The time zone the working hours and the times are in.
    pub time_zone: String,
}

fn edge(text: &str, zone: Tz) -> Option<DateTime<Utc>> {
    match Moment::parse(text)? {
        Moment::At(at) => Some(at),
        Moment::On(day) => zone
            .from_local_datetime(&day.and_time(NaiveTime::MIN))
            .earliest()
            .map(|local| local.with_timezone(&Utc)),
    }
}

/// When you are free.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/schedule/free-slots",
    operation_id = "schedule.free_slots",
    tag = "schedule",
    security(("bearer" = ["schedule:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["schedule.event"]).access("schedule.event", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), FreeQuery),
    responses(
        (status = OK, description = "The free slots.", body = FreeSlotList),
        (status = BAD_REQUEST, description = "A window that is not valid or longer than 62 days \
            (`invalid_window`), or minutes out of range (`invalid_minutes`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleRead>,
    extract::Query(query): extract::Query<FreeQuery>,
) -> Result<Json<FreeSlotList>> {
    let access = &workspace.access;
    let member = ops::require_events_type(&state, access, Action::Read)?;
    if !MINUTES.contains(&query.minutes) {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_minutes")));
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let settings = settings::load(&mut tx, member).await?;
    let zone = settings.zone();
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_window"));
    let from = edge(&query.from, zone).ok_or_else(invalid)?;
    let to = edge(&query.to, zone).ok_or_else(invalid)?;
    if to <= from || to - from > TimeDelta::days(MAX_WINDOW_DAYS) {
        return Err(invalid());
    }
    let calendars = calendars_of(&mut tx, access.workspace, member).await?;
    let found = free::free_slots(
        &mut tx,
        member,
        &settings,
        &calendars,
        from,
        to,
        TimeDelta::minutes(i64::from(query.minutes)),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(FreeSlotList {
        slots: found
            .slots
            .iter()
            .map(|slot| SlotView {
                starts: slot.starts.with_timezone(&zone).to_rfc3339(),
                ends: slot.ends.with_timezone(&zone).to_rfc3339(),
            })
            .collect(),
        calendars: found
            .calendars
            .iter()
            .filter_map(|id| calendars.iter().find(|calendar| calendar.id == *id))
            .map(|calendar| CountedCalendar {
                id: calendar.id,
                name: calendar.name.clone(),
            })
            .collect(),
        time_zone: zone.name().to_owned(),
    }))
}

/// The kernel's `Availability` port, answered from the member's calendars.
#[derive(Debug, Clone, Copy)]
struct Calendars;

/// The port, for the server to register.
pub fn availability() -> AvailabilityPort {
    Arc::new(Calendars)
}

impl Availability for Calendars {
    fn free_slots<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        member: UserId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        duration: TimeDelta,
    ) -> campbooks_kernel::schedule::FreeSlotsFuture<'a> {
        Box::pin(async move {
            let settings = settings::load(conn, member).await?;
            let calendars = calendars_of(conn, workspace, member).await?;
            let Found { slots, calendars } =
                free::free_slots(conn, member, &settings, &calendars, from, to, duration).await?;
            Ok(FreeSlots {
                slots: slots
                    .into_iter()
                    .map(|slot| FreeSlot {
                        starts: slot.starts,
                        ends: slot.ends,
                    })
                    .collect(),
                calendars: calendars.into_iter().map(|id| *id.as_uuid()).collect(),
            })
        })
    }
}
