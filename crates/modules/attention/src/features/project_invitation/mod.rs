//! `attention.project_invitation` — keep one Decide item per invitation that
//! awaits its member's answer, from `schedule.invitation_changed`
//! (campbooks-cloud#360).
//!
//! Like the other projections it is idempotent and order-proof: an event
//! applies only when its revision is newer than the item's, and a close that
//! arrives before its open leaves a tombstone holding its revision. The item
//! is about the event (`schedule.event`), belongs to its member's container
//! (`schedule.member:<member>`, seen by that member only), is due on the day
//! the event starts and lapses when it starts (an all-day one when its first
//! day ends): the list leaves it out from then on, without waiting for the
//! close. Who sent it (a person or a machine) is kept for the bands.

use campbooks_kernel::{
    Event, EventHandler, JobContext, JobError, RecordRef, tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::ItemId,
    events::{InvitationChanged, InvitationFacts, ItemChanged},
};

/// Record type of an event.
pub const EVENT: &str = "schedule.event";

/// Container type of a member's calendars.
const MEMBER: &str = "schedule.member";

/// Projects invitations into items.
pub struct ProjectInvitation;

impl EventHandler for ProjectInvitation {
    type Event = InvitationChanged;
    const NAME: &'static str = "attention.project_invitation";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<InvitationChanged>) -> Result<(), JobError> {
        let since = event.occurred_at;
        let changed = event.payload;
        let workspace: WorkspaceId = changed
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("invitation_changed names an invalid workspace"))?;
        let events = ctx.state().events.clone();
        let tx = ctx.tx().await?;
        let current: Option<(ItemId, i64)> = sqlx::query_as(
            "SELECT id, producer_revision FROM attention.items
              WHERE workspace_id = $1 AND record_type = $2 AND record_id = $3 FOR UPDATE",
        )
        .bind(workspace.as_uuid())
        .bind(EVENT)
        .bind(&changed.event)
        .fetch_optional(&mut **tx)
        .await?;
        if current
            .as_ref()
            .is_some_and(|(_, revision)| *revision >= changed.revision)
        {
            // Stale or replayed.
            return Ok(());
        }
        let id = current.as_ref().map_or_else(ItemId::new, |(id, _)| *id);
        let written = match &changed.invitation {
            Some(facts) => open(tx, workspace, id, &changed, facts, since).await?,
            None => clear(tx, workspace, id, &changed, since).await?,
        };
        // Nobody saw a tombstone: announce a close only over an item that
        // was there.
        let Some(item) = written.filter(|_| changed.invitation.is_some() || current.is_some())
        else {
            return Ok(());
        };
        events
            .publish(
                tx,
                &ItemChanged {
                    workspace: changed.workspace.clone(),
                    item,
                    container: RecordRef::new(MEMBER, &changed.owner),
                },
            )
            .await?;
        Ok(())
    }
}

/// Open (or update) item `id` for an invitation that awaits an answer: the
/// item written, if any (an item a newer revision reached first, in a race,
/// is left as it is).
async fn open(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    id: ItemId,
    changed: &InvitationChanged,
    facts: &InvitationFacts,
    since: DateTime<Utc>,
) -> Result<Option<ItemId>, JobError> {
    let written: Option<(ItemId,)> = sqlx::query_as(
        r"
        INSERT INTO attention.items
            (id, workspace_id, record_type, record_id, container_type, container_id,
             cause_type, cause_id, reason, trust, due_on, urgent, since, state,
             producer_revision, lapses_at, from_person)
        VALUES ($1, $2, $3, $4, $5, $6, $3, $4, 'decide', 'sure', $7, false, $8, 'open', $9,
                $10, $11)
        ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
           SET container_type = EXCLUDED.container_type,
               container_id = EXCLUDED.container_id,
               reason = EXCLUDED.reason, trust = EXCLUDED.trust, due_on = EXCLUDED.due_on,
               lapses_at = EXCLUDED.lapses_at, from_person = EXCLUDED.from_person,
               state = 'open', cleared_because = NULL, cleared_by = NULL, cleared_at = NULL,
               producer_revision = EXCLUDED.producer_revision, updated_at = now()
         WHERE attention.items.producer_revision < EXCLUDED.producer_revision
        RETURNING id
        ",
    )
    .bind(id)
    .bind(workspace.as_uuid())
    .bind(EVENT)
    .bind(&changed.event)
    .bind(MEMBER)
    .bind(changed.owner.to_string())
    .bind(facts.on)
    .bind(since)
    .bind(changed.revision)
    .bind(facts.lapses_at)
    .bind(facts.from_person)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(written.map(|(id,)| id))
}

/// Clear item `id`; without one yet, leave a tombstone at this revision.
async fn clear(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    id: ItemId,
    changed: &InvitationChanged,
    since: DateTime<Utc>,
) -> Result<Option<ItemId>, JobError> {
    let because = changed.closed_because.as_deref().unwrap_or("answered");
    let written: Option<(ItemId,)> = sqlx::query_as(
        r"
        INSERT INTO attention.items
            (id, workspace_id, record_type, record_id, container_type, container_id,
             cause_type, cause_id, since, state, cleared_because, cleared_at, producer_revision)
        VALUES ($1, $2, $3, $4, $5, $6, $3, $4, $7, 'cleared', $8, now(), $9)
        ON CONFLICT (workspace_id, record_type, record_id) DO UPDATE
           SET state = 'cleared', cleared_because = EXCLUDED.cleared_because,
               cleared_at = now(), producer_revision = EXCLUDED.producer_revision,
               updated_at = now()
         WHERE attention.items.producer_revision < EXCLUDED.producer_revision
        RETURNING id
        ",
    )
    .bind(id)
    .bind(workspace.as_uuid())
    .bind(EVENT)
    .bind(&changed.event)
    .bind(MEMBER)
    .bind(changed.owner.to_string())
    .bind(since)
    .bind(because)
    .bind(changed.revision)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(written.map(|(id,)| id))
}

#[cfg(test)]
mod tests;
