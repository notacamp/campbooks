//! `GET …/schedule/calendars` and `PATCH …/schedule/calendars/{calendar_id}`
//! — the member's calendars (their Campbooks calendar, made the first time
//! it is needed, and the provider calendars a calendar grant brought), and
//! turning one on or off, or colouring it. Another member's calendar answers
//! 404.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    blobs::BlobOwner,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        blobs::{CALENDAR_REF_CONTENT_TYPE, CALENDAR_REF_OWNER},
        ops::{self, CalendarPath},
        rows::{calendar_of, calendars_of},
        views::CalendarView,
    },
    events::CalendarSyncChanged,
    model::CalendarId,
    scopes::{ScheduleRead, ScheduleWrite},
};

/// The member's calendars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CalendarList {
    /// Their Campbooks calendar first, then the others by name.
    pub calendars: Vec<CalendarView>,
}

/// List your calendars.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/schedule/calendars",
    operation_id = "schedule.list_calendars",
    tag = "schedule",
    security(("bearer" = ["schedule:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["schedule.calendar"]).access("schedule.calendar", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "Your calendars.", body = CalendarList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleRead>,
) -> Result<Json<CalendarList>> {
    let access = &workspace.access;
    let member = ops::require_calendars_type(&state, access, Action::Read)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let rows = calendars_of(&mut tx, access.workspace, member).await?;
    tx.commit().await?;
    Ok(Json(CalendarList {
        calendars: rows.iter().map(CalendarView::from).collect(),
    }))
}

/// What to change of a calendar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CalendarPatch {
    /// Show its events (a provider calendar only). Turning it on asks
    /// connectors to read it again from the start.
    pub syncing: Option<bool>,
    /// Its colour as `#rrggbb`; empty goes back to the provider's.
    pub color: Option<String>,
}

/// Whether `color` is `#rrggbb`.
fn is_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Turn a calendar on or off, or colour it.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/schedule/calendars/{calendar_id}",
    operation_id = "schedule.update_calendar",
    tag = "schedule",
    security(("bearer" = ["schedule:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["schedule.calendar"]).access("schedule.calendar", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("calendar_id" = CalendarId, Path, description = "The calendar."),
    ),
    request_body = CalendarPatch,
    responses(
        (status = OK, description = "The calendar afterwards.", body = CalendarView),
        (status = BAD_REQUEST, description = "A colour that is not `#rrggbb` (`invalid_color`), \
            or your Campbooks calendar turned off (`invalid_calendar`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such calendar, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    workspace: Workspace<ScheduleWrite>,
    extract::Path(which): extract::Path<CalendarPath>,
    extract::Json(patch): extract::Json<CalendarPatch>,
) -> Result<Json<CalendarView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let member =
        ops::require_calendar(&state, &mut tx, access, Action::Write, which.calendar_id).await?;
    let calendar = calendar_of(&mut tx, member, which.calendar_id).await?;
    if let Some(color) = patch.color.as_deref().filter(|color| !color.is_empty())
        && !is_color(color)
    {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_color")));
    }
    if calendar.is_campbooks() && patch.syncing == Some(false) {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("invalid_calendar"),
        ));
    }
    let syncing = patch.syncing.unwrap_or(calendar.syncing);
    let color = match patch.color.as_deref() {
        Some("") => None,
        Some(color) => Some(color.to_ascii_lowercase()),
        None => calendar.color.clone(),
    };
    sqlx::query(
        "UPDATE schedule.calendars SET syncing = $2, color = $3, updated_at = now() WHERE id = $1",
    )
    .bind(calendar.id)
    .bind(syncing)
    .bind(&color)
    .execute(&mut *tx)
    .await?;
    // Connectors reads a calendar that was turned on from the start, and
    // stops reading one that was turned off.
    if let Some(connection) = calendar.connection_id
        && syncing != calendar.syncing
    {
        // Which calendar of the provider it is travels in a blob: its id can
        // be an address.
        let workspace = access.workspace.to_string();
        let provider_calendar = match &calendar.provider_calendar_id {
            Some(provider_id) => {
                let document = serde_json::to_vec(&serde_json::json!({
                    "version": 1,
                    "provider_calendar_id": provider_id,
                }))
                .map_err(Error::internal)?;
                let blob = state
                    .blobs
                    .put(&workspace, document, CALENDAR_REF_CONTENT_TYPE)
                    .await?;
                state
                    .blobs
                    .attach(
                        &mut tx,
                        &workspace,
                        &blob,
                        &BlobOwner::new(CALENDAR_REF_OWNER, calendar.id),
                    )
                    .await?;
                Some(blob)
            }
            None => None,
        };
        state
            .events
            .publish(
                &mut tx,
                &CalendarSyncChanged {
                    workspace,
                    calendar: calendar.id,
                    connection,
                    syncing,
                    provider_calendar,
                },
            )
            .await?;
    }
    let rows = calendars_of(&mut tx, access.workspace, member).await?;
    tx.commit().await?;
    rows.iter()
        .find(|row| row.id == calendar.id)
        .map(|row| Json(CalendarView::from(row)))
        .ok_or(Error::NotFound)
}

#[cfg(test)]
mod tests {
    use super::is_color;

    #[test]
    fn a_colour_is_six_hex_digits_after_a_hash() {
        assert!(is_color("#0b8043"));
        assert!(is_color("#0B8043"));
        assert!(!is_color("green"));
        assert!(!is_color("#0b804"));
        assert!(!is_color("0b80430"));
        assert!(!is_color("#0b80zz"));
    }
}
