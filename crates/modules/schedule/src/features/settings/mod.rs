//! `GET` and `PATCH …/schedule/settings` — the member's time zone, working
//! hours, the kinds Scout adds and, per kind, the provider calendar an event
//! is also added to.

use std::collections::BTreeMap;

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        ops,
        rows::calendars_of,
        settings::{self, Settings, WorkingHours},
    },
    model::{CalendarId, EventKind},
    scopes::{ScheduleRead, ScheduleWrite},
};

/// A member's settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SettingsView {
    /// Their IANA time zone; empty until they or their device set it.
    pub time_zone: Option<String>,
    /// The zone times are shown in: theirs, or UTC.
    pub effective_time_zone: String,
    /// Working hours by weekday (`mon` … `sun`), a missing day being a day
    /// off. Monday to Friday 09:00–18:00 until they set some.
    pub working_hours: WorkingHours,
    /// The kinds Scout adds to their Campbooks calendar.
    pub scout_kinds: Vec<EventKind>,
    /// Per kind, the provider calendar an event is also added to.
    pub also_add_to: BTreeMap<String, CalendarId>,
    /// The provider calendar they last added an event to, offered first.
    pub last_push_calendar: Option<CalendarId>,
}

impl SettingsView {
    fn of(settings: &Settings) -> Self {
        Self {
            time_zone: settings.time_zone.clone(),
            effective_time_zone: settings.zone().name().to_owned(),
            working_hours: settings.hours(),
            scout_kinds: settings.scout_kinds.clone(),
            also_add_to: settings.also_add_to.clone(),
            last_push_calendar: settings.last_push_calendar,
        }
    }
}

/// Your calendar settings.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/schedule/settings",
    operation_id = "schedule.get_settings",
    tag = "schedule",
    security(("bearer" = ["schedule:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["schedule.calendar"]).access("schedule.calendar", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "Your settings.", body = SettingsView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleRead>,
) -> Result<Json<SettingsView>> {
    let access = &workspace.access;
    let member = ops::require_calendars_type(&state, access, Action::Read)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let settings = settings::load(&mut tx, member).await?;
    tx.commit().await?;
    Ok(Json(SettingsView::of(&settings)))
}

/// What to change of the settings. Leave out what stays.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SettingsPatch {
    /// A new time zone (an IANA name).
    pub time_zone: Option<String>,
    /// The time zone of the device, sent once: it is taken only when the
    /// member has no time zone yet, and ignored after.
    pub device_time_zone: Option<String>,
    /// New working hours, replacing the old; an empty map is a week off.
    pub working_hours: Option<WorkingHours>,
    /// The kinds Scout adds.
    pub scout_kinds: Option<Vec<EventKind>>,
    /// Per kind, the provider calendar an event is also added to, replacing
    /// the old map: a kind left out is taken off.
    pub also_add_to: Option<BTreeMap<String, CalendarId>>,
}

/// Change your calendar settings.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/schedule/settings",
    operation_id = "schedule.update_settings",
    tag = "schedule",
    security(("bearer" = ["schedule:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["schedule.calendar"]).access("schedule.calendar", campbooks_kernel::authz::Action::Write)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = SettingsPatch,
    responses(
        (status = OK, description = "Your settings afterwards.", body = SettingsView),
        (status = BAD_REQUEST, description = "A time zone that is not an IANA name \
            (`invalid_time_zone`), working hours that end before they start (`invalid_hours`), a \
            kind Scout does not add (`invalid_kind`), or a calendar that is not one of yours \
            or cannot be written (`invalid_calendar`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleWrite>,
    extract::Json(patch): extract::Json<SettingsPatch>,
) -> Result<Json<SettingsView>> {
    let access = &workspace.access;
    let member = ops::require_calendars_type(&state, access, Action::Write)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let mut current = settings::load(&mut tx, member).await?;
    if let Some(zone) = &patch.time_zone {
        settings::check_zone(zone)?;
        current.time_zone = Some(zone.clone());
    } else if let Some(zone) = &patch.device_time_zone
        && current.time_zone.is_none()
    {
        // A device that reports a zone Campbooks does not know sets nothing.
        if settings::check_zone(zone).is_ok() {
            current.time_zone = Some(zone.clone());
        }
    }
    if let Some(hours) = patch.working_hours {
        settings::check_hours(&hours)?;
        current.working_hours = hours;
    }
    if let Some(kinds) = patch.scout_kinds {
        if kinds.iter().any(|kind| !settings::scout_can_add(*kind)) {
            return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_kind")));
        }
        current.scout_kinds = kinds;
    }
    if let Some(also) = patch.also_add_to {
        let calendars = calendars_of(&mut tx, access.workspace, member).await?;
        for (kind, id) in &also {
            let kind_ok = kind.parse::<EventKind>().is_ok_and(settings::scout_can_add);
            if !kind_ok {
                return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_kind")));
            }
            let usable = calendars
                .iter()
                .find(|calendar| calendar.id == *id)
                .is_some_and(|calendar| {
                    !calendar.is_campbooks() && calendar.writable && calendar.syncing
                });
            if !usable {
                return Err(Error::BadRequest(
                    crate::MESSAGES.reason("invalid_calendar"),
                ));
            }
        }
        current.also_add_to = also;
    }
    settings::save(&mut tx, access.workspace, member, &current).await?;
    tx.commit().await?;
    Ok(Json(SettingsView::of(&current)))
}
