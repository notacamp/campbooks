//! `schedule.grant_calendars` ← `connectors.capability_granted`: a calendar
//! grant brings the member one calendar per calendar of the provider
//! account, owned by the member who granted it; the primary one syncs and
//! the others wait until the member turns them on. A repeated grant adds only
//! the calendars the module does not have yet, and records whether the
//! account owns one the module had without knowing. A calendar the account
//! does not own starts as one the member follows.
//!
//! `schedule.apply_calendar_list` ← `connectors.calendar_list_read` applies a
//! calendar list read again (about daily) by the same rule, except that it
//! turns nothing on: a calendar found later waits, off, for the member.

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError,
    tenancy::{UserId, WorkspaceId},
};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::{
        blobs::{MAX_DOCUMENT_BYTES, read_json},
        moment::parse_zone,
        rows::ensure_campbooks_calendar,
        tracked,
    },
    inbound::{CalendarList, CalendarListRead, CapabilityGranted},
    model::{CalendarId, ConnectionId},
    tracking,
};

/// Makes the calendars of a calendar grant.
pub struct GrantCalendars;

/// Applies a calendar list read again.
pub struct ApplyCalendarList;

/// Whether `color` is `#rrggbb`.
fn is_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl EventHandler for GrantCalendars {
    type Event = CapabilityGranted;
    const NAME: &'static str = "schedule.grant_calendars";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<CapabilityGranted>) -> Result<(), JobError> {
        let granted = event.payload;
        if granted.capability != "calendar" {
            return Ok(());
        }
        let Some(listed) = &granted.calendars else {
            tracing::warn!("a calendar grant names no calendar list");
            return Ok(());
        };
        let workspace: WorkspaceId = granted
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("capability_granted names an invalid workspace"))?;
        let state = ctx.state().clone();
        let list: CalendarList =
            read_json(&state.blobs, &granted.workspace, listed, MAX_DOCUMENT_BYTES).await?;
        let tx = ctx.tx().await?;
        ensure_campbooks_calendar(tx, workspace, granted.granted_by).await?;
        let added = apply_list(
            tx,
            &state,
            (workspace, granted.connection, granted.granted_by),
            list,
            true,
        )
        .await?;
        tracing::info!(calendars = added, "calendars made for a calendar grant");
        Ok(())
    }
}

impl EventHandler for ApplyCalendarList {
    type Event = CalendarListRead;
    const NAME: &'static str = "schedule.apply_calendar_list";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<CalendarListRead>) -> Result<(), JobError> {
        let read = event.payload;
        let workspace: WorkspaceId = read
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("calendar_list_read names an invalid workspace"))?;
        let state = ctx.state().clone();
        let list: CalendarList = read_json(
            &state.blobs,
            &read.workspace,
            &read.calendars,
            MAX_DOCUMENT_BYTES,
        )
        .await?;
        let tx = ctx.tx().await?;
        let added = apply_list(
            tx,
            &state,
            (workspace, read.connection, read.granted_by),
            list,
            false,
        )
        .await?;
        tracing::info!(
            calendars = added,
            "calendars found in a calendar list read again"
        );
        Ok(())
    }
}

/// Apply a `calendar-list` to the calendars `member` has on `connection`:
/// add the calendars the module does not have (following when the account
/// does not own them, mine otherwise), and record whether the account owns
/// one the module has when it did not know. Nothing else of a calendar the
/// module has changes: the member's role, name and colour are theirs. A grant
/// (`primary_on`) turns the provider's primary calendar on; a list read again
/// never turns a calendar on. Returns how many calendars were added.
async fn apply_list(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    (workspace, connection, member): (WorkspaceId, ConnectionId, UserId),
    list: CalendarList,
    primary_on: bool,
) -> Result<u64, JobError> {
    // Only the providers the module names; anything else is left unnamed.
    let provider = list
        .provider
        .as_deref()
        .filter(|name| matches!(*name, "google" | "zoho"));
    let mut added = 0_u64;
    for calendar in list.calendars {
        let time_zone = calendar.time_zone.filter(|zone| parse_zone(zone).is_some());
        let color = calendar
            .color
            .filter(|color| is_color(color))
            .map(|color| color.to_ascii_lowercase());
        let id = CalendarId::new();
        let inserted = sqlx::query(
            "INSERT INTO schedule.calendars
                 (id, workspace_id, member_id, source, connection_id, provider_calendar_id,
                  name, color, time_zone, syncing, writable, is_primary, provider,
                  owned, following)
             VALUES ($1, $2, $3, 'provider', $4, $5, $6, $7, $8, $9 AND $13, $10, $9, $11,
                     $12, $12 IS FALSE)
             ON CONFLICT (workspace_id, connection_id, provider_calendar_id)
                WHERE source = 'provider' DO NOTHING",
        )
        .bind(id)
        .bind(workspace)
        .bind(member)
        .bind(connection)
        .bind(&calendar.provider_id)
        .bind(&calendar.name)
        .bind(&color)
        .bind(&time_zone)
        .bind(calendar.primary)
        .bind(calendar.writable)
        .bind(provider)
        .bind(calendar.owned)
        .bind(primary_on)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        added += inserted;
        // A calendar the module had: learn whether the account owns it,
        // and nothing else (the member's role is theirs).
        if inserted == 0
            && let Some(owned) = calendar.owned
        {
            sqlx::query(
                "UPDATE schedule.calendars SET owned = $4, updated_at = now()
                  WHERE workspace_id = $1 AND connection_id = $2
                    AND provider_calendar_id = $3 AND source = 'provider'
                    AND owned IS NULL",
            )
            .bind(workspace)
            .bind(connection)
            .bind(&calendar.provider_id)
            .bind(owned)
            .execute(&mut **tx)
            .await?;
        }
        // A calendar the module did not have yet.
        if inserted > 0
            && let Some(snapshot) = tracked::calendar(tx, id).await?
        {
            state
                .tracker
                .track(
                    tx,
                    &tracking::CalendarAdded {
                        workspace,
                        calendar: snapshot,
                    },
                )
                .await?;
        }
    }
    Ok(added)
}
