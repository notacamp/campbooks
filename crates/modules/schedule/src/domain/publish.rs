//! Publishing what happened to an event, in the transaction of the change.
//! Ids, enums and times only: never a title, place, notes or attendees. An
//! event that is or was an invitation awaiting the member's answer is
//! announced again with it (`invitations::announce`, campbooks-cloud#360).

use campbooks_kernel::{
    AppState, Result,
    tenancy::{UserId, WorkspaceId},
};
use sqlx::{Postgres, Transaction};

use super::{invitations, rows::EventRow};
use crate::{
    events::{EventCancelled, EventChanged, EventCreated, EventFacts},
    model::{EventKind, EventOrigin},
};

/// What the events say about `row`, an event of `owner`'s calendar. `None`
/// when the row holds no time (it cannot be shown either).
pub fn facts(workspace: WorkspaceId, owner: UserId, row: &EventRow) -> Option<EventFacts> {
    let span = row.span()?;
    Some(EventFacts {
        workspace: workspace.to_string(),
        event: row.id,
        calendar: row.calendar_id,
        owner,
        kind: row.kind.parse().unwrap_or(EventKind::None),
        starts: span.start.render_utc(),
        ends: Some(span.end.render_utc()),
        all_day: row.all_day,
        origin: row.origin.parse().unwrap_or(EventOrigin::Member),
        source: row.source(),
    })
}

/// `schedule.event_created` for `row`.
pub async fn created(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    owner: UserId,
    row: &EventRow,
) -> Result<()> {
    if let Some(facts) = facts(workspace, owner, row) {
        state
            .events
            .publish(tx, &EventCreated::from_facts(facts))
            .await?;
    }
    if invitations::concerns(row) {
        invitations::announce(state, tx, workspace, row.id).await?;
    }
    Ok(())
}

/// `schedule.event_changed` for `row`.
pub async fn changed(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    owner: UserId,
    row: &EventRow,
) -> Result<()> {
    if let Some(facts) = facts(workspace, owner, row) {
        state
            .events
            .publish(tx, &EventChanged::from_facts(facts))
            .await?;
    }
    if invitations::concerns(row) {
        invitations::announce(state, tx, workspace, row.id).await?;
    }
    Ok(())
}

/// `schedule.event_cancelled` for `row`.
pub async fn cancelled(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    owner: UserId,
    row: &EventRow,
) -> Result<()> {
    if let Some(facts) = facts(workspace, owner, row) {
        state
            .events
            .publish(tx, &EventCancelled::from_facts(facts))
            .await?;
    }
    if invitations::concerns(row) {
        invitations::announce(state, tx, workspace, row.id).await?;
    }
    Ok(())
}
