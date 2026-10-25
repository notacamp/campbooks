//! `schedule.apply_outcome` ← `connectors.calendar_change_completed`: how a
//! change this module asked for ended at the provider.
//!
//! - **Applied**: the provider's id and etag are stored and the pending mark
//!   is cleared; what the member changed is now the provider's version. A
//!   hold released before its block reached the provider is deleted there
//!   now.
//! - **Superseded** (412, the event changed there meanwhile): the provider's
//!   version is kept, the mark is cleared and the event says the member's
//!   change was not kept because it changed at the provider.
//! - **Skipped** or **failed**: the mark is cleared, the event goes back to
//!   the last provider version and says the change was not saved. A failure
//!   is recorded by its closed reason only.

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    domain::{
        provider::Snapshot,
        publish,
        rows::{calendar_by_id, event_by_id},
    },
    features::holds,
    inbound::{CalendarChangeCompleted, Outcome},
};

/// Applies the outcome of a calendar change.
pub struct ApplyOutcome;

impl EventHandler for ApplyOutcome {
    type Event = CalendarChangeCompleted;
    const NAME: &'static str = "schedule.apply_outcome";
    const QUEUE: &'static str = "default";

    #[allow(clippy::too_many_lines)]
    async fn handle(
        ctx: &mut JobContext,
        event: Event<CalendarChangeCompleted>,
    ) -> Result<(), JobError> {
        let done = event.payload;
        let workspace: WorkspaceId = done
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("calendar_change_completed names an invalid workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        // Once: a request that is no longer waiting was handled.
        let change: Option<String> = sqlx::query_scalar(
            "UPDATE schedule.change_requests
                SET state = $2, finished_at = now()
              WHERE id = $1 AND state = 'waiting'
          RETURNING change",
        )
        .bind(done.request)
        .bind(match done.outcome {
            Outcome::Applied => "applied",
            Outcome::Superseded => "superseded",
            Outcome::Skipped => "skipped",
            Outcome::Failed => "failed",
        })
        .fetch_optional(&mut **tx)
        .await?;
        let Some(change) = change else {
            return Ok(());
        };
        let Some(row) = event_by_id(tx, done.event).await? else {
            return Ok(());
        };
        let calendar = calendar_by_id(tx, row.calendar_id).await?;
        let owner = calendar.member_id;
        match done.outcome {
            Outcome::Applied => {
                if change == "delete" {
                    // Gone at the provider; the row stays for an undo.
                    sqlx::query(
                        "UPDATE schedule.events
                            SET provider_event_id = NULL, etag = NULL, outbound_pending = NULL,
                                updated_at = now()
                          WHERE id = $1",
                    )
                    .bind(row.id)
                    .execute(&mut **tx)
                    .await?;
                } else {
                    sqlx::query(
                        "UPDATE schedule.events
                            SET provider_event_id = coalesce($2, provider_event_id),
                                etag = coalesce($3, etag), outbound_pending = NULL,
                                notice = NULL, updated_at = now()
                          WHERE id = $1",
                    )
                    .bind(row.id)
                    .bind(&done.provider_event_id)
                    .bind(&done.etag)
                    .execute(&mut **tx)
                    .await?;
                    // What the member asked for is the provider's version now.
                    if let Some(now) = event_by_id(tx, row.id).await? {
                        sqlx::query(
                            "UPDATE schedule.events SET provider_version = $2 WHERE id = $1",
                        )
                        .bind(row.id)
                        .bind(Snapshot::of(&now).to_json())
                        .execute(&mut **tx)
                        .await?;
                        // A hold released while it was on its way there
                        // (`features::holds`): it goes at the provider now.
                        if now.deleted_at.is_some() && change == "create" {
                            holds::ask_delete(tx, workspace, now.id).await?;
                        }
                    }
                }
            }
            Outcome::Superseded | Outcome::Skipped | Outcome::Failed => {
                let notice = if done.outcome == Outcome::Superseded {
                    "kept_theirs"
                } else {
                    "not_saved"
                };
                tracing::info!(
                    outcome = ?done.outcome,
                    reason = done.reason.as_deref().unwrap_or("unknown"),
                    change = change.as_str(),
                    "a calendar change did not apply"
                );
                let snapshot = row
                    .provider_version
                    .clone()
                    .and_then(|json| serde_json::from_value::<Snapshot>(json).ok());
                if change == "create" && snapshot.is_none() {
                    undo_create(&state, tx, workspace, owner, &row, notice).await?;
                    return Ok(());
                }
                // An exception the provider did not take: the occurrence is the
                // series' again.
                if change == "update_occurrence" && snapshot.is_none() && row.series_id.is_some() {
                    publish::cancelled(&state, tx, workspace, owner, &row).await?;
                    sqlx::query("DELETE FROM schedule.events WHERE id = $1")
                        .bind(row.id)
                        .execute(&mut **tx)
                        .await?;
                    return Ok(());
                }
                // A delete that did not happen is undone, an update reverted.
                sqlx::query(
                    "UPDATE schedule.events
                        SET outbound_pending = NULL, notice = $2, deleted_at = NULL,
                            undo_token = NULL, updated_at = now()
                      WHERE id = $1 OR (series_id = $1 AND $3)",
                )
                .bind(row.id)
                .bind(notice)
                .bind(change == "delete")
                .execute(&mut **tx)
                .await?;
                if let Some(snapshot) = &snapshot {
                    snapshot.restore(tx, row.id).await?;
                }
                if let Some(now) = event_by_id(tx, row.id).await? {
                    if change == "delete" {
                        publish::created(&state, tx, workspace, owner, &now).await?;
                    } else {
                        publish::changed(&state, tx, workspace, owner, &now).await?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// A create the provider did not take: a copy made by "Add to my calendar"
/// goes away and the Campbooks event is back; an event the member made there
/// stays, saying it was not saved.
async fn undo_create(
    state: &campbooks_kernel::AppState,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    workspace: WorkspaceId,
    owner: campbooks_kernel::tenancy::UserId,
    row: &crate::domain::rows::EventRow,
    notice: &str,
) -> Result<(), JobError> {
    if let Some(original) = row.pushed_from {
        sqlx::query(
            "UPDATE schedule.events SET pushed_to = NULL, notice = $2, updated_at = now()
              WHERE id = $1",
        )
        .bind(original)
        .bind(notice)
        .execute(&mut **tx)
        .await?;
        publish::cancelled(state, tx, workspace, owner, row).await?;
        sqlx::query("DELETE FROM schedule.events WHERE id = $1")
            .bind(row.id)
            .execute(&mut **tx)
            .await?;
        if let Some(back) = event_by_id(tx, original).await? {
            publish::created(state, tx, workspace, owner, &back).await?;
        }
    } else {
        sqlx::query(
            "UPDATE schedule.events SET outbound_pending = NULL, notice = $2, updated_at = now()
              WHERE id = $1",
        )
        .bind(row.id)
        .bind(notice)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
