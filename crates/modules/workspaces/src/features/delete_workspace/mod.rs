//! `DELETE /api/v1/w/{workspace_id}` — delete a workspace. Owners only.
//!
//! The workspace disappears at once (soft delete) and `WorkspaceDeleting`
//! tells every module to purge its data. Its own records are hard-deleted
//! by the [`PurgeWorkspace`] job, scheduled in the same transaction for the
//! end of the grace period, which also purges its blobs from the kernel's
//! blob store.

use std::time::Duration;

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::authz::Action;
use campbooks_kernel::{
    AppState, Job, JobContext, JobError, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
    tenancy::{Access, Workspace, WorkspaceId},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{
        access::{acting_user, authorize, lock_workspace, workspace_resource},
        security::{self, SecurityKind},
        tracked,
    },
    events::WorkspaceDeleting,
    scopes::WorkspacesAdmin,
    tracking,
};

mod purge_again;
pub use purge_again::{LIVE_PURGE_NOTICE, PurgeAgain, PurgeAgainOptions, purge_again};

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
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["workspaces.workspace"]).access("workspaces.workspace", campbooks_kernel::authz::Action::Delete)))),
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
    let (user, _) = acting_user(access)?;
    let id = access.workspace;
    let mut tx = state.db.begin_for(id).await?;
    authorize(
        state,
        &mut tx,
        access,
        Action::Delete,
        &workspace_resource(access),
    )
    .await?;
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
    let workspace_snapshot = tracked::workspace(&mut tx, id).await?;
    state
        .tracker
        .track(
            &mut tx,
            &tracking::WorkspaceDeleted {
                workspace: id,
                workspace_snapshot,
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
/// its blobs (`BlobStore::purge_workspace`), the kernel's change feed, event
/// log, finished jobs and idempotency keys of the workspace (in short
/// batches, `campbooks_kernel::purge::purge_workspace`: no foreign key
/// reaches them), then the workspace row, and with it (foreign keys
/// cascade) its members, invitations and security events, together with
/// the workspace's tombstone (`campbooks_kernel::purge::tombstone`) in the
/// same transaction. What reaches the kernel's tables of the workspace
/// afterwards is deleted by the kernel's hourly sweep of tombstones. Runs
/// in the workspace, so row-level security applies; a workspace that is
/// gone already is a no-op.
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
        // The kernel's rows of the workspace, batch by batch in short
        // transactions of their own (jobs still queued, such as other
        // modules' purges, stay to run; what they leave is swept later).
        campbooks_kernel::purge::purge_workspace(ctx.pool(), &workspace).await?;
        let tx = ctx.tx().await?;
        // As it stood, read before its row (and its members) go.
        let workspace_snapshot = tracked::workspace(tx, id).await?;
        sqlx::query!(
            "DELETE FROM workspaces.workspaces WHERE id = $1",
            id as WorkspaceId
        )
        .execute(&mut **tx)
        .await?;
        campbooks_kernel::purge::tombstone(tx, &workspace).await?;
        ctx.track(&tracking::WorkspacePurged {
            workspace: id,
            workspace_snapshot,
        })
        .await?;
        log_purged(id, Utc::now());
        Ok(())
    }
}

/// The one line a purge logs: the workspace's id and when, nothing else (no
/// name, no member, no count), so the log is safe to keep as the list of
/// what was purged (the restore runbook in `deploy/README.md`, "Backups" reads it).
fn log_purged(id: WorkspaceId, purged_at: DateTime<Utc>) {
    tracing::info!(workspace = %id, purged_at = %purged_at.to_rfc3339(), "workspace purged");
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::{jobs::Worker, tenancy::WorkspaceId};
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
        // Unexpired idempotency keys: one of the workspace's routes, one of
        // another workspace's, one outside any workspace.
        let other = kit.create_workspace(&ana, "Elsewhere").await;
        for (key, workspace) in [
            ("k-1", Some(ws.to_string())),
            ("k-2", Some(other.to_string())),
            ("k-0", None),
        ] {
            sqlx::query(
                "INSERT INTO kernel.idempotency
                     (principal, key, operation_id, request_hash, expires_at, workspace_id)
                 VALUES ('user:3:ana|azp:', $1, 'test.op', '\\x00', now() + interval '1 day', $2)",
            )
            .bind(key)
            .bind(workspace)
            .execute(&kit.db)
            .await
            .unwrap();
        }
        let stats = worker.run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
        let purged: String = sqlx::query_scalar(
            "SELECT state FROM kernel.jobs WHERE kind = 'workspaces.purge_workspace'",
        )
        .fetch_one(&kit.db)
        .await
        .unwrap();
        assert_eq!(purged, "succeeded");
        assert_eq!(kit.rows_of(ws).await, (0, 0, 0));
        assert_eq!(kit.blobs_of(ws).await, 0);
        let changes: i64 =
            sqlx::query_scalar("SELECT count(*) FROM kernel.changes WHERE workspace_id = $1")
                .bind(ws.to_string())
                .fetch_one(&kit.db)
                .await
                .unwrap();
        assert_eq!(changes, 0, "its change feed went with it");
        let history: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM kernel.events WHERE workspace_id = $1),
                    (SELECT count(*) FROM kernel.jobs
                      WHERE workspace_id = $1 AND kind <> 'workspaces.purge_workspace')",
        )
        .bind(ws.to_string())
        .fetch_one(&kit.db)
        .await
        .unwrap();
        assert_eq!(history, (0, 0), "its events and finished jobs went with it");
        let keys: Vec<Option<String>> =
            sqlx::query_scalar("SELECT workspace_id FROM kernel.idempotency ORDER BY key")
                .fetch_all(&kit.db)
                .await
                .unwrap();
        assert_eq!(
            keys,
            [None, Some(other.to_string())],
            "its idempotency keys went with it, unexpired too; others' stay"
        );
        let tombstones: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM kernel.purged_workspaces WHERE workspace_id = $1",
        )
        .bind(ws.to_string())
        .fetch_one(&kit.db)
        .await
        .unwrap();
        assert_eq!(
            tombstones, 1,
            "its tombstone, written with the row's delete"
        );

        // What finished after the purge (the purge job itself) goes with the
        // kernel's next sweep of tombstones.
        let mut tx = kit.db.begin().await.unwrap();
        campbooks_kernel::jobs::enqueue(
            &mut tx,
            &campbooks_kernel::purge::SweepPurgedWorkspaces,
            campbooks_kernel::jobs::EnqueueOptions::default(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        worker.run_until_idle().await;
        let left: i64 =
            sqlx::query_scalar("SELECT count(*) FROM kernel.jobs WHERE workspace_id = $1")
                .bind(ws.to_string())
                .fetch_one(&kit.db)
                .await
                .unwrap();
        assert_eq!(left, 0, "nothing of the workspace is left in the queue");
    }

    /// The workspace's purges still queued, as the database owner.
    async fn queued_purges(kit: &Kit, ws: WorkspaceId) -> i64 {
        sqlx::query_scalar(
            "SELECT count(*) FROM kernel.jobs
              WHERE kind = 'workspaces.purge_workspace' AND workspace_id = $1
                AND state IN ('pending', 'running')",
        )
        .bind(ws.to_string())
        .fetch_one(&kit.db)
        .await
        .unwrap()
    }

    /// What `purge-again` is told: nothing, or that the workspace was
    /// deleted after the backup (and when to purge it).
    fn told(
        not_deleted_in_backup: bool,
        purge_after: Option<chrono::DateTime<chrono::Utc>>,
    ) -> super::PurgeAgainOptions {
        super::PurgeAgainOptions {
            not_deleted_in_backup,
            purge_after,
        }
    }

    /// Whether the workspace is still live and nothing was published or
    /// queued for it.
    async fn untouched(kit: &Kit, ana: &crate::testkit::Caller, ws: WorkspaceId) {
        assert_eq!(
            kit.get(ana, &format!("/api/v1/w/{ws}")).await.status,
            StatusCode::OK
        );
        assert!(
            !kit.events_of(&ws.to_string())
                .await
                .contains(&"workspaces.workspace_deleting".to_owned())
        );
        assert_eq!(queued_purges(kit, ws).await, 0);
    }

    /// A workspace live in the restored database is left alone unless the
    /// operator says it was deleted after the backup: the command answers
    /// the facts only (members, creation date). Told so, it deletes it at
    /// once (deletion published, the first owner standing in for the
    /// operator) and queues its purge an hour later; a second run enqueues
    /// no second purge and reports when the queued one runs; once purged,
    /// the id is a no-op that says so.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn purge_again_deletes_a_live_workspace_only_when_told(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        kit.join(&ana, ws, "bea", "member").await;

        let refused = super::purge_again(&kit.state, ws, told(false, None))
            .await
            .unwrap();
        assert!(
            matches!(refused, super::PurgeAgain::NotDeleted { members: 2, .. }),
            "{refused:?}"
        );
        untouched(&kit, &ana, ws).await;

        let before = chrono::Utc::now();
        let deleted = super::purge_again(&kit.state, ws, told(true, None))
            .await
            .unwrap();
        let super::PurgeAgain::Deleted { purge_after } = deleted else {
            panic!("{deleted:?}");
        };
        assert!(
            purge_after >= before + super::LIVE_PURGE_NOTICE,
            "{purge_after}"
        );
        assert!(purge_after < chrono::Utc::now() + super::LIVE_PURGE_NOTICE * 2);
        assert_eq!(
            kit.get(&ana, &format!("/api/v1/w/{ws}")).await.status,
            StatusCode::NOT_FOUND,
            "hidden from its members at once"
        );
        assert!(
            kit.events_of(&ws.to_string())
                .await
                .contains(&"workspaces.workspace_deleting".to_owned())
        );
        let deleted_by = kit.last_event_payload(&ws.to_string()).await["deleted_by"].clone();
        assert_eq!(
            deleted_by,
            serde_json::json!(kit.user_id("ana").await),
            "the first owner, standing in for the operator"
        );

        let again = super::purge_again(&kit.state, ws, told(true, None))
            .await
            .unwrap();
        let super::PurgeAgain::AlreadyQueued { run_at } = again else {
            panic!("{again:?}");
        };
        assert!(
            (run_at - purge_after).num_seconds().abs() < 1,
            "{run_at} {purge_after}"
        );
        assert_eq!(queued_purges(&kit, ws).await, 1, "no second purge");

        // An hour later, the worker purges it.
        sqlx::query(
            "UPDATE workspaces.workspaces SET purge_after = now() - interval '1 second'
              WHERE id = $1",
        )
        .bind(ws.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'workspaces.purge_workspace'",
        )
        .execute(&kit.db)
        .await
        .unwrap();
        let stats = Worker::new(kit.state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
        assert_eq!(kit.rows_of(ws).await, (0, 0, 0));
        assert_eq!(
            super::purge_again(&kit.state, ws, told(true, None))
                .await
                .unwrap(),
            super::PurgeAgain::NoSuchWorkspace
        );
        assert_eq!(queued_purges(&kit, ws).await, 0);
    }

    /// The purge time of a live workspace deleted again may be given (its
    /// original `purge_after`), but never less than an hour away.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn purge_again_takes_a_purge_time_an_hour_away_at_least(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;

        let soon = chrono::Utc::now() + chrono::TimeDelta::minutes(30);
        let refused = super::purge_again(&kit.state, ws, told(true, Some(soon)))
            .await
            .unwrap();
        assert!(
            matches!(refused, super::PurgeAgain::TooSoon { .. }),
            "{refused:?}"
        );
        untouched(&kit, &ana, ws).await;

        let later = chrono::Utc::now() + chrono::TimeDelta::days(12);
        let deleted = super::purge_again(&kit.state, ws, told(true, Some(later)))
            .await
            .unwrap();
        let super::PurgeAgain::Deleted { purge_after } = deleted else {
            panic!("{deleted:?}");
        };
        assert!(
            (purge_after - later).num_milliseconds().abs() < 1,
            "{purge_after}"
        );
    }

    /// A live workspace with no owner to name is refused, even when told.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn purge_again_refuses_a_live_workspace_with_no_owner(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        sqlx::query("UPDATE workspaces.members SET role = 'admin' WHERE workspace_id = $1")
            .bind(ws.as_uuid())
            .execute(&kit.db)
            .await
            .unwrap();
        assert_eq!(
            super::purge_again(&kit.state, ws, told(true, None))
                .await
                .unwrap(),
            super::PurgeAgain::NoOwner
        );
        let deleted: bool = sqlx::query_scalar(
            "SELECT deleted_at IS NOT NULL FROM workspaces.workspaces WHERE id = $1",
        )
        .bind(ws.as_uuid())
        .fetch_one(&kit.db)
        .await
        .unwrap();
        assert!(!deleted);
        assert_eq!(queued_purges(&kit, ws).await, 0);
    }

    /// A workspace deleted in its grace period keeps its one queued purge
    /// and its purge time: `purge-again` changes nothing and says when the
    /// purge runs.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn purge_again_keeps_the_queued_purge_of_a_deleted_workspace(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let response = kit.delete(&ana, &format!("/api/v1/w/{ws}")).await;
        assert_eq!(response.status, StatusCode::ACCEPTED);
        let deletions = kit.events_of(&ws.to_string()).await.len();

        let outcome = super::purge_again(&kit.state, ws, told(false, None))
            .await
            .unwrap();
        let super::PurgeAgain::AlreadyQueued { run_at } = outcome else {
            panic!("{outcome:?}");
        };
        assert!(run_at > chrono::Utc::now() + chrono::TimeDelta::days(29));
        assert_eq!(queued_purges(&kit, ws).await, 1);
        assert_eq!(
            kit.events_of(&ws.to_string()).await.len(),
            deletions,
            "its deletion is not published twice"
        );
        let later: bool = sqlx::query_scalar(
            "SELECT purge_after > now() + interval '29 days' FROM workspaces.workspaces
              WHERE id = $1",
        )
        .bind(ws.as_uuid())
        .fetch_one(&kit.db)
        .await
        .unwrap();
        assert!(later, "its purge time is untouched");
    }

    /// A deleted workspace with no purge queued (the purge job gave up) is
    /// marked for purge now and its purge queued to run now.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn purge_again_requeues_the_purge_of_a_deleted_workspace(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let response = kit.delete(&ana, &format!("/api/v1/w/{ws}")).await;
        assert_eq!(response.status, StatusCode::ACCEPTED);
        sqlx::query(
            "UPDATE kernel.jobs SET state = 'failed', finished_at = now()
              WHERE kind = 'workspaces.purge_workspace'",
        )
        .execute(&kit.db)
        .await
        .unwrap();

        assert_eq!(
            super::purge_again(&kit.state, ws, told(false, None))
                .await
                .unwrap(),
            super::PurgeAgain::Scheduled
        );
        assert_eq!(queued_purges(&kit, ws).await, 1);
        let stats = Worker::new(kit.state.clone()).run_until_idle().await;
        assert_eq!(stats.failed, 0, "{stats:?}");
        assert_eq!(kit.rows_of(ws).await, (0, 0, 0));
    }

    /// The `workspace purged` line carries the workspace's id and the time,
    /// and nothing else: the restore runbook relies on it being safe to keep.
    #[test]
    fn the_purge_line_carries_the_id_and_the_time_only() {
        use std::sync::{Arc, Mutex};
        use tracing_subscriber::layer::SubscriberExt as _;

        type Seen = Arc<Mutex<Vec<(String, String)>>>;
        struct Fields(Seen);
        struct Visit<'a>(&'a mut Vec<(String, String)>);
        impl tracing::field::Visit for Visit<'_> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0.push((field.name().to_owned(), format!("{value:?}")));
            }
        }
        impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Fields {
            fn on_event(
                &self,
                event: &tracing::Event<'_>,
                _: tracing_subscriber::layer::Context<'_, S>,
            ) {
                event.record(&mut Visit(&mut self.0.lock().unwrap()));
            }
        }

        let seen = Seen::default();
        let subscriber = tracing_subscriber::registry().with(Fields(Arc::clone(&seen)));
        let id = campbooks_kernel::tenancy::WorkspaceId::new();
        let at = chrono::Utc::now();
        tracing::subscriber::with_default(subscriber, || super::log_purged(id, at));

        let mut fields = seen.lock().unwrap().clone();
        fields.sort();
        assert_eq!(
            fields,
            [
                ("message".to_owned(), "workspace purged".to_owned()),
                ("purged_at".to_owned(), at.to_rfc3339()),
                ("workspace".to_owned(), id.to_string()),
            ]
        );
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
