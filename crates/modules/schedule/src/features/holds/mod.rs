//! The kernel's `Holds` port (campbooks-cloud#362): the time a member holds
//! for a task, as a busy event of kind `hold` on one of their writable
//! calendars, made and taken off in the tasks module's own transaction.
//!
//! - **Hold**: the same insert as `schedule.create_event`, made by the
//!   member and linked to the task (`source = tasks.task:<id>`), in their
//!   time zone. A provider calendar is asked for it like any event the
//!   member makes (stored at once, marked pending); the Campbooks calendar
//!   keeps it to itself. `schedule.event_created` is published.
//! - **Release**: the block is deleted as `schedule.delete_event` deletes,
//!   without its undo window (the task's undo makes a new block), and
//!   `schedule.event_cancelled` is published. A block still on its way to
//!   the provider is deleted there once it lands
//!   ([`features::apply_outcome`](crate::features::apply_outcome)).
//!
//! Events and jobs carry ids, kinds and times only: the title stays on the
//! row (and the provider's copy).

use std::sync::Arc;

use campbooks_kernel::{
    AppState, Error, Result,
    authz::Action,
    jobs::EnqueueOptions,
    schedule::{Held, HoldFuture, HoldRequest, Holds, HoldsPort, ReleaseFuture},
    tenancy::{Access, UserId, WorkspaceId},
};
use sqlx::{Postgres, Transaction, types::Uuid};

use crate::{
    domain::{
        moment::{Moment, Span},
        ops, outbound, publish,
        rows::{calendar_of, ensure_campbooks_calendar, event_of},
        settings, tracked,
        write::{MAX_TITLE, NewRow, insert},
    },
    features::{create_event::ensure_writable, delete_event::ReleaseDelete},
    model::{CalendarChange, CalendarId, EventId, EventKind, EventOrigin},
    tracking,
};

/// The record type a hold's source must be.
pub const TASK_RECORD: &str = "tasks.task";

/// The port, answered from the member's calendars.
#[derive(Debug, Clone, Copy)]
struct Calendars;

/// The port, for the server to register.
pub fn holds() -> HoldsPort {
    Arc::new(Calendars)
}

impl Holds for Calendars {
    fn hold<'a>(
        &'a self,
        state: &'a AppState,
        tx: &'a mut Transaction<'static, Postgres>,
        workspace: WorkspaceId,
        request: HoldRequest<'a>,
    ) -> HoldFuture<'a> {
        Box::pin(async move { hold(state, tx, workspace, &request).await })
    }

    fn release<'a>(
        &'a self,
        state: &'a AppState,
        tx: &'a mut Transaction<'static, Postgres>,
        workspace: WorkspaceId,
        member: UserId,
        event: Uuid,
    ) -> ReleaseFuture<'a> {
        Box::pin(release(
            state,
            tx,
            workspace,
            member,
            EventId::from_uuid(event),
        ))
    }
}

/// Make the block of `request` on the member's calendar.
async fn hold(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    request: &HoldRequest<'_>,
) -> Result<Held> {
    let member = request.member;
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_when"));
    if request.task.record_type != TASK_RECORD {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_source")));
    }
    let span = Span::new(Moment::At(request.starts), Some(Moment::At(request.ends)))
        .filter(|_| request.ends > request.starts)
        .ok_or_else(invalid)?;
    let title: String = request.title.trim().chars().take(MAX_TITLE).collect();
    if title.is_empty() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_text")));
    }
    let access = Access {
        workspace,
        user: Some(member),
        role: None,
    };
    let calendar = match request.calendar.map(CalendarId::from_uuid) {
        // 404 for a calendar that is not the member's.
        Some(id) => {
            ops::require_calendar(state, tx, &access, Action::Write, id).await?;
            calendar_of(tx, member, id).await?
        }
        None => ensure_campbooks_calendar(tx, workspace, member).await?,
    };
    ensure_writable(&calendar)?;
    let settings = settings::load(tx, member).await?;
    let id = EventId::new();
    insert(
        tx,
        &NewRow {
            id,
            workspace,
            calendar: calendar.id,
            kind: EventKind::Hold,
            title: &title,
            span,
            time_zone: settings.time_zone.as_deref(),
            place: None,
            notes: None,
            origin: EventOrigin::Member,
            found_by: None,
            source: Some(&request.task),
            item_key: None,
            ical_uid: None,
            busy: true,
            pushed_from: None,
            series_id: None,
            original_start: None,
            source_conversation: None,
            quote: None,
        },
    )
    .await?;
    let row = event_of(tx, member, id).await?;
    let at_provider = !calendar.is_campbooks();
    if at_provider {
        let document = outbound::document(CalendarChange::Create, &calendar, &row, None, None)?;
        outbound::request(
            state,
            tx,
            outbound::Asked {
                workspace,
                member,
                calendar: &calendar,
                event: &row,
                change: CalendarChange::Create,
                document,
            },
        )
        .await?;
    }
    publish::created(state, tx, workspace, member, &row).await?;
    tracked::created(state, tx, workspace, id).await?;
    Ok(Held {
        event: *id.as_uuid(),
        calendar: *calendar.id.as_uuid(),
        at_provider,
    })
}

/// Take `member`'s hold `id` off their calendar; nothing for an event that
/// is gone or is not their hold.
async fn release(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    member: UserId,
    id: EventId,
) -> Result<()> {
    let row = match event_of(tx, member, id).await {
        Ok(row) => row,
        Err(Error::NotFound) => return Ok(()),
        Err(error) => return Err(error),
    };
    if row.kind != EventKind::Hold.as_str() || row.deleted_at.is_some() || row.pushed_to.is_some() {
        return Ok(());
    }
    // A block on its way to the provider is deleted there once it lands; any
    // other change still waiting is the member's own, made on the calendar.
    let creating = row.outbound_pending.as_deref() == Some(CalendarChange::Create.as_str());
    if row.is_pending() && !creating {
        return Err(Error::Conflict(crate::MESSAGES.reason("change_pending")));
    }
    let calendar = calendar_of(tx, member, row.calendar_id).await?;
    sqlx::query(
        "UPDATE schedule.events SET deleted_at = now(), undo_token = NULL, updated_at = now()
          WHERE id = $1",
    )
    .bind(row.id)
    .execute(&mut **tx)
    .await?;
    if !calendar.is_campbooks() && !creating && row.provider_event_id.is_some() {
        ask_delete(tx, workspace, row.id).await?;
    }
    publish::cancelled(state, tx, workspace, member, &row).await?;
    if let Some(calendar_event) = tracked::calendar_event(tx, row.id).await? {
        state
            .tracker
            .track(
                tx,
                &tracking::CalendarEventDeleted {
                    workspace,
                    calendar_event,
                },
            )
            .await?;
    }
    Ok(())
}

/// Mark deleted event `id` as waiting for its delete at the provider, and ask
/// for it now (`schedule.release_delete`).
pub async fn ask_delete(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    id: EventId,
) -> Result<()> {
    sqlx::query("UPDATE schedule.events SET outbound_pending = 'delete' WHERE id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    let options = EnqueueOptions::for_workspace(workspace.to_string())
        .unique(format!("schedule.release_delete:{id}"));
    campbooks_kernel::jobs::enqueue(tx, &ReleaseDelete { event: id }, options).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
