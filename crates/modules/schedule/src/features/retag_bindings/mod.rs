//! Re-tagging the change requests after a rotation of the instance secret
//! key (campbooks-cloud#262).
//!
//! A member's change to an event on a provider calendar waits for the
//! connector bound to the request that made it with a MAC under the kernel's
//! `Binding` (campbooks-cloud#227). While a previous key is set it verifies
//! under it too; once it is removed, a request still waiting (its calendar
//! stream paused, or the provider down for the whole rotation) would no
//! longer stand and the member's change would not be made. So the kernel
//! enqueues this job in every workspace while a previous key is set
//! (`Background::rekey`). It re-tags, with the current key, every waiting
//! request whose MAC verifies under the previous key only: the MAC was just
//! verified, so what was bound stays bound and nothing new is trusted. Each
//! update is a compare-and-set on the old MAC, and the job is idempotent.
//!
//! It logs what it re-tagged and what it left (rows changed while it ran,
//! re-tagged at the next tick), like `conversations.retag_bindings`.

use campbooks_kernel::{
    Job, JobContext, JobError,
    blobs::BlobRef,
    tenancy::{UserId, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;

use crate::{
    domain::binding::ChangeRequest,
    model::{CalendarId, ConnectionId, EventId, RequestId},
};

/// Re-tag, in the job's workspace, the change requests bound under the
/// previous key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetagBindings;

/// A waiting request, as it is bound.
#[derive(sqlx::FromRow)]
struct Row {
    id: RequestId,
    member_id: UserId,
    calendar_id: CalendarId,
    event_id: EventId,
    connection_id: ConnectionId,
    change: String,
    body: Json<BlobRef>,
    mac: Option<Vec<u8>>,
}

impl Job for RetagBindings {
    const NAME: &'static str = "schedule.retag_bindings";
    const QUEUE: &'static str = "maintenance";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("retag_bindings enqueued outside a workspace"))?;
        let binding = ctx.state().binding.clone();
        if !binding.has_previous() {
            return Ok(());
        }
        let tx = ctx.tx().await?;
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT id, member_id, calendar_id, event_id, connection_id, change, body, mac
               FROM schedule.change_requests
              WHERE state = 'waiting' AND mac IS NOT NULL
              ORDER BY id",
        )
        .fetch_all(&mut **tx)
        .await?;
        let (mut retagged, mut left) = (0_u64, 0_u64);
        for row in rows {
            let request = ChangeRequest {
                workspace,
                request: row.id,
                member: row.member_id,
                calendar: row.calendar_id,
                event: row.event_id,
                connection: row.connection_id,
                change: &row.change,
                body: &row.body.0,
            };
            let Some(mac) = request.retag(&binding, row.mac.as_deref()) else {
                continue;
            };
            let done = sqlx::query(
                "UPDATE schedule.change_requests SET mac = $2 WHERE id = $1 AND mac = $3",
            )
            .bind(row.id)
            .bind(mac)
            .bind(row.mac.as_deref())
            .execute(&mut **tx)
            .await?;
            if done.rows_affected() == 0 {
                left += 1;
            } else {
                retagged += 1;
            }
        }
        if left == 0 {
            tracing::info!(
                %workspace,
                retagged,
                left = 0,
                "binding re-tag: no calendar change in this workspace is bound under the \
                 previous key any more"
            );
        } else {
            tracing::warn!(
                %workspace,
                retagged,
                left,
                "binding re-tag: calendar changes bound under the previous key changed while \
                 they were re-tagged; the next tick tries again"
            );
        }
        Ok(())
    }
}
