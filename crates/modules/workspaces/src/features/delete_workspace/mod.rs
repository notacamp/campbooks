//! `DELETE /api/v1/w/{workspace_id}` — delete a workspace. Owners only.
//!
//! The workspace disappears at once (soft delete) and `WorkspaceDeleting`
//! tells every module to purge its data. Its own records are hard-deleted
//! by the [`PurgeWorkspace`] job, scheduled in the same transaction for the
//! end of the grace period, which also purges its blobs from the kernel's
//! blob store.

use std::time::Duration;

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Job, JobContext, JobError, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
    tenancy::{Access, Role, Workspace, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        access::{acting_user, lock_workspace},
        security::{self, SecurityKind},
    },
    events::WorkspaceDeleting,
    scopes::WorkspacesAdmin,
};

/// How long a deleted workspace's records are kept before they are purged.
pub const GRACE_PERIOD: TimeDelta = TimeDelta::days(30);

/// A deleted workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorkspaceDeletion {
    /// The workspace.
    pub id: WorkspaceId,
    /// When its records will be purged for good.
    pub purge_after: DateTime<Utc>,
}

/// Delete a workspace.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}",
    operation_id = "workspaces.delete",
    tag = "workspaces",
    security(("bearer" = ["workspaces:admin"])),
    // Deletes everything in the workspace after the grace period.
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["workspaces.workspace"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = ACCEPTED, description = "Deleted; its records are purged after the grace \
            period.", body = WorkspaceDeletion),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not an owner.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<WorkspacesAdmin>,
) -> Result<(StatusCode, Json<WorkspaceDeletion>)> {
    let deletion = execute(&state, &workspace.access).await?;
    Ok((StatusCode::ACCEPTED, Json(deletion)))
}

/// Delete the workspace of `access`.
pub async fn execute(state: &AppState, access: &Access) -> Result<WorkspaceDeletion> {
    let (user, role) = acting_user(access)?;
    if role != Role::Owner {
        return Err(Error::Forbidden);
    }
    let id = access.workspace;
    let mut tx = state.db.begin_for(id).await?;
    lock_workspace(&mut tx, id).await?;
    let purge_after = sqlx::query_scalar!(
        r#"UPDATE workspaces.workspaces
              SET deleted_at = now(), purge_after = now() + make_interval(days => $2)
            WHERE id = $1
        RETURNING purge_after AS "purge_after!: DateTime<Utc>""#,
        id as WorkspaceId,
        i32::try_from(GRACE_PERIOD.num_days()).unwrap_or(i32::MAX),
    )
    .fetch_one(&mut *tx)
    .await?;
    security::record(
        &mut tx,
        Some(id),
        Some(user),
        SecurityKind::WorkspaceDeleted,
        serde_json::json!({ "purge_after": purge_after }),
    )
    .await?;
    state
        .events
        .publish(
            &mut tx,
            &WorkspaceDeleting {
                workspace: id.to_string(),
                purge_after,
                deleted_by: user,
            },
        )
        .await?;
    jobs::enqueue(
        &mut tx,
        &PurgeWorkspace { workspace: id },
        EnqueueOptions::for_workspace(id.to_string())
            .unique(id.to_string())
            .run_at(purge_after),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(workspace = %id, user = %user, %purge_after, "workspace deleted");
    Ok(WorkspaceDeletion { id, purge_after })
}

/// Hard-delete a deleted workspace's records once its grace period is over:
/// its blobs (`BlobStore::purge_workspace`), then the workspace row, and with
/// it (foreign keys cascade) its members, invitations and security events. Runs in the workspace, so row-level
/// security applies; a workspace that is gone already is a no-op.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeWorkspace {
    /// The workspace.
    pub workspace: WorkspaceId,
}

impl Job for PurgeWorkspace {
    const NAME: &'static str = "workspaces.purge_workspace";
    const QUEUE: &'static str = "default";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let id = self.workspace;
        let workspace = id.to_string();
        if ctx.workspace() != Some(workspace.as_str()) {
            return Err(JobError::fatal("purge enqueued outside its workspace"));
        }
        // Is it due? Read in a short transaction of its own (in the
        // workspace, for row-level security), so the blob purge below runs
        // outside the job's transaction.
        let mut check = ctx.pool().begin_for(id).await?;
        let row = sqlx::query!(
            r#"SELECT purge_after, (purge_after <= now()) AS "due"
                 FROM workspaces.workspaces WHERE id = $1"#,
            id as WorkspaceId,
        )
        .fetch_optional(&mut *check)
        .await?;
        check.commit().await?;
        match row.map(|row| (row.purge_after, row.due)) {
            // Purged already, or not deleted (any more): nothing to do.
            None | Some((None, _)) => return Ok(()),
            Some((Some(_), Some(true))) => {}
            Some((Some(purge_after), _)) => {
                let wait = (purge_after - Utc::now())
                    .to_std()
                    .unwrap_or(Duration::from_mins(1));
                return Err(JobError::retry_after("grace period not over", wait));
            }
        }
        // The workspace's files first (idempotent: a failure retries the job).
        let blobs = &ctx.state().blobs;
        if blobs.backend().is_some() {
            blobs
                .purge_workspace(&workspace)
                .await
                .map_err(|e| JobError::retry(format!("purging the workspace's blobs: {e}")))?;
        }
        let tx = ctx.tx().await?;
        sqlx::query!(
            "DELETE FROM workspaces.workspaces WHERE id = $1",
            id as WorkspaceId
        )
        .execute(&mut **tx)
        .await?;
        tracing::info!(workspace = %id, "workspace purged");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::jobs::Worker;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn soft_delete_publishes_then_the_job_purges(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "member").await;
        kit.invite(&ana, ws, "cid@example.com", "member").await;
        kit.state
            .blobs
            .put(&ws.to_string(), b"a receipt".as_slice(), "text/plain")
            .await
            .unwrap();
        assert_eq!(kit.blobs_of(ws).await, 1);

        let response = kit.delete(&ana, &format!("/api/v1/w/{ws}")).await;
        assert_eq!(response.status, StatusCode::ACCEPTED, "{:?}", response.json);
        assert!(response.json["purge_after"].is_string());

        // Gone for everyone at once, its event logged with the change.
        assert_eq!(
            kit.get(&ana, &format!("/api/v1/w/{ws}")).await.status,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            kit.get(&bea, &format!("/api/v1/w/{ws}/members"))
                .await
                .status,
            StatusCode::NOT_FOUND
        );
        assert!(
            kit.events_of(&ws.to_string())
                .await
                .contains(&"workspaces.workspace_deleting".to_owned())
        );

        // The purge is scheduled for the end of the grace period…
        let worker = Worker::new(kit.state.clone());
        worker.run_until_idle().await;
        assert_eq!(kit.rows_of(ws).await, (1, 2, 2));
        assert_eq!(kit.blobs_of(ws).await, 1, "kept through the grace period");
        let delay = sqlx::query_scalar!(
            r#"SELECT (run_at - now() > interval '29 days') AS "later!"
                 FROM kernel.jobs WHERE kind = 'workspaces.purge_workspace' AND workspace_id = $1"#,
            ws.to_string()
        )
        .fetch_one(&kit.db)
        .await
        .unwrap();
        assert!(delay);

        // …and then hard-deletes the workspace and everything it owns.
        sqlx::query!(
            "UPDATE workspaces.workspaces SET purge_after = now() - interval '1 second' WHERE id = $1",
            ws as campbooks_kernel::tenancy::WorkspaceId
        )
        .execute(&kit.db)
        .await
        .unwrap();
        sqlx::query!(
            "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'workspaces.purge_workspace'"
        )
        .execute(&kit.db)
        .await
        .unwrap();
        let stats = worker.run_until_idle().await;
        assert_eq!(stats.succeeded, 1, "{stats:?}");
        assert_eq!(kit.rows_of(ws).await, (0, 0, 0));
        assert_eq!(kit.blobs_of(ws).await, 0);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn only_owners_delete(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "admin").await;
        let uri = format!("/api/v1/w/{ws}");
        assert_eq!(kit.delete(&bea, &uri).await.status, StatusCode::FORBIDDEN);
        let client = kit.client(Some(ws), "workspaces:admin");
        assert_eq!(
            kit.delete(&client, &uri).await.status,
            StatusCode::FORBIDDEN
        );
        assert_eq!(kit.get(&ana, &uri).await.status, StatusCode::OK);
    }
}
