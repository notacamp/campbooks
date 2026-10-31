//! The sweep of scheduled sends that lost their release (campbooks-cloud#392).

use axum::http::StatusCode;
use campbooks_kernel::{db::BeginFor as _, jobs::EnqueueOptions, tenancy::WorkspaceId};
use serde_json::json;
use sqlx::PgPool;

use super::{SweepOverdueSends, SweepPayload};
use crate::testkit::Kit;

/// A draft scheduled the ordinary way: its URI and id.
async fn scheduled(
    kit: &Kit,
    workspace: WorkspaceId,
    inbox: crate::model::InboxId,
    sam: &str,
) -> String {
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let made = kit
        .post(
            sam,
            &uri,
            Some(json!({"kind": "new", "inbox_id": inbox, "subject": "Notes",
                "to": [{"address": "leo@example.com"}], "body": "Hi"})),
        )
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{:?}", made.json);
    let id = made.json["id"].as_str().unwrap().to_owned();
    let uri = format!("{uri}/{id}");
    let shown = kit.get(sam, &uri).await.json;
    let sent = kit
        .post(
            sam,
            &format!("{uri}/send"),
            Some(json!({"version": shown["version"], "content_token": shown["content_token"]})),
        )
        .await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    id
}

/// Make the draft's time `minutes` ago.
async fn due(kit: &Kit, id: &str, minutes: i32) {
    sqlx::query(
        "UPDATE conversations.drafts SET send_at = now() - make_interval(mins => $2)
          WHERE id = $1::uuid",
    )
    .bind(id)
    .bind(minutes)
    .execute(&kit.db)
    .await
    .unwrap();
}

/// Lose the draft's release job, as a restored database might.
async fn lose_job(kit: &Kit, id: &str) {
    sqlx::query(
        "DELETE FROM kernel.jobs WHERE kind = 'conversations.release_send' AND unique_key LIKE $1",
    )
    .bind(format!("release:{id}:%"))
    .execute(&kit.db)
    .await
    .unwrap();
}

/// Run the workspace's sweep.
async fn sweep(kit: &Kit, workspace: WorkspaceId) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    campbooks_kernel::jobs::enqueue_to::<SweepOverdueSends>(
        &mut tx,
        &SweepPayload,
        EnqueueOptions::for_workspace(workspace.to_string()),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

async fn state_of(kit: &Kit, id: &str) -> (String, Option<String>) {
    sqlx::query_as("SELECT state, failure_reason FROM conversations.drafts WHERE id = $1::uuid")
        .bind(id)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_overdue_send_with_no_release_job_is_given_up_and_never_sent(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let id = scheduled(&kit, workspace, inbox, &sam).await;
    lose_job(&kit, &id).await;
    due(&kit, &id, 20).await;
    sweep(&kit, workspace).await;
    assert_eq!(
        state_of(&kit, &id).await,
        ("failed".to_owned(), Some("release_lost".to_owned()))
    );
    // Not queued again, nothing sent, and the stop is tracked.
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.release_send'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(queued, 0);
    assert!(kit.events("conversations.send_requested").await.is_empty());
    let tracked =
        campbooks_kernel::testing::tracked_events(&kit.db, "conversations.draft_unscheduled").await;
    assert_eq!(tracked.len(), 1);
    assert_eq!(tracked[0].body["reason"], "release_lost");
    // The member sees it with the drafts, with its reason.
    let read = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/drafts/{id}"))
        .await;
    assert_eq!(
        read.json["failure_reason"], "release_lost",
        "{:?}",
        read.json
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_send_whose_job_is_still_queued_or_not_yet_late_is_left_alone(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    // Overdue but its job is still there (the workers are behind).
    let queued = scheduled(&kit, workspace, inbox, &sam).await;
    due(&kit, &queued, 60).await;
    // Late by less than the grace, job lost.
    let recent = scheduled(&kit, workspace, inbox, &sam).await;
    lose_job(&kit, &recent).await;
    due(&kit, &recent, 5).await;
    // A future send, job lost.
    let future = scheduled(&kit, workspace, inbox, &sam).await;
    lose_job(&kit, &future).await;
    sqlx::query(
        "UPDATE conversations.drafts SET send_at = now() + interval '2 days' WHERE id = $1::uuid",
    )
    .bind(&future)
    .execute(&kit.db)
    .await
    .unwrap();
    // Sweep without running the release jobs.
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    campbooks_kernel::jobs::enqueue_to::<SweepOverdueSends>(
        &mut tx,
        &SweepPayload,
        EnqueueOptions::for_workspace(workspace.to_string()).unique("sweep"),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("UPDATE kernel.jobs SET run_at = now() + interval '1 day' WHERE kind = 'conversations.release_send'")
        .execute(&kit.db)
        .await
        .unwrap();
    kit.run_jobs().await;
    for id in [&queued, &recent, &future] {
        assert_eq!(
            state_of(&kit, id).await,
            ("scheduled".to_owned(), None),
            "{id}"
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_newer_generation_is_judged_by_its_own_job(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let id = scheduled(&kit, workspace, inbox, &sam).await;
    // Cancelled and sent again: the first generation's job is gone, the
    // second's is queued. The draft is overdue, and its own job is there.
    let uri = format!("/api/v1/w/{workspace}/drafts/{id}");
    let cancelled = kit.post(&sam, &format!("{uri}/cancel_send"), None).await;
    assert_eq!(cancelled.status, StatusCode::OK, "{:?}", cancelled.json);
    let shown = kit.get(&sam, &uri).await.json;
    let again = kit
        .post(
            &sam,
            &format!("{uri}/send"),
            Some(json!({"version": shown["version"], "content_token": shown["content_token"]})),
        )
        .await;
    assert_eq!(again.status, StatusCode::ACCEPTED, "{:?}", again.json);
    due(&kit, &id, 30).await;
    // A job left over from the first generation does not stand in for it.
    sqlx::query("UPDATE kernel.jobs SET run_at = now() + interval '1 day' WHERE kind = 'conversations.release_send'")
        .execute(&kit.db)
        .await
        .unwrap();
    sweep(&kit, workspace).await;
    assert_eq!(state_of(&kit, &id).await, ("scheduled".to_owned(), None));
    // Its own job lost: now it is given up, by the generation it is at.
    lose_job(&kit, &id).await;
    sweep(&kit, workspace).await;
    assert_eq!(
        state_of(&kit, &id).await,
        ("failed".to_owned(), Some("release_lost".to_owned()))
    );
}

/// What the scheduler does every ten minutes, then every job it causes.
async fn tick(kit: &Kit) {
    let mut tx = kit.state.db.begin().await.unwrap();
    campbooks_kernel::jobs::enqueue(
        &mut tx,
        &super::DispatchOverdueSends,
        EnqueueOptions::default(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

/// The dispatcher finds every workspace with an overdue send, whatever else
/// it holds (no `needs_backfills` row: that table was seeded once), wakes
/// only those, and reads no draft beyond its workspace id.
#[sqlx::test(migrations = "../../../migrations")]
async fn the_dispatcher_finds_every_workspace_with_an_overdue_send(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let id = scheduled(&kit, workspace, inbox, &sam).await;
    lose_job(&kit, &id).await;
    due(&kit, &id, 20).await;
    // A workspace created after the backfill was seeded has no row there.
    sqlx::query("DELETE FROM conversations.needs_backfills WHERE workspace_id = $1")
        .bind(workspace.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    // A second workspace with nothing overdue gets no sweep at all.
    let (quiet, _, _) = kit.sams_inbox().await;

    tick(&kit).await;

    assert_eq!(
        state_of(&kit, &id).await,
        ("failed".to_owned(), Some("release_lost".to_owned()))
    );
    let swept: Vec<String> = sqlx::query_scalar(
        "SELECT workspace_id FROM kernel.jobs WHERE kind = 'conversations.sweep_overdue_sends'",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    assert!(swept.contains(&workspace.to_string()), "{swept:?}");
    assert!(!swept.contains(&quiet.to_string()), "{swept:?}");
}

/// Only the runtime role may ask, and the answer is ids: no role reads a
/// draft through the function's owner, and nobody else may call it.
#[sqlx::test(migrations = "../../../migrations")]
async fn the_overdue_question_answers_ids_to_the_runtime_role_only(db: PgPool) {
    let callers: Vec<String> = sqlx::query_scalar(
        "SELECT grantee::text FROM information_schema.routine_privileges
          WHERE routine_schema = 'conversations'
            AND routine_name = 'workspaces_with_overdue_sends'
            AND privilege_type = 'EXECUTE'",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert!(callers.contains(&"campbooks_app".to_owned()), "{callers:?}");
    assert!(!callers.contains(&"PUBLIC".to_owned()), "{callers:?}");
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.column_privileges
          WHERE table_schema = 'conversations' AND table_name = 'drafts'
            AND grantee = 'campbooks_definer' AND privilege_type = 'SELECT'
          ORDER BY column_name",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(columns, ["send_at", "state", "workspace_id"]);
}
