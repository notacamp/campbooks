//! The profile: defaults, keeping it, its privacy, the port, and the purge.

use axum::http::StatusCode;
use campbooks_kernel::{
    db::BeginFor as _,
    preferences::MemberPreferences as _,
    tenancy::{Role, WorkspaceId},
};
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;

use super::Preferences;
use crate::{
    events::{MemberLeft, WorkspaceDeleting},
    testkit::Kit,
};

fn uri(workspace: WorkspaceId) -> String {
    format!("/api/v1/w/{workspace}/assistant/profile")
}

#[sqlx::test(migrations = "../../../migrations")]
async fn defaults_then_kept(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ines, _) = kit.member("ines", workspace);

    let fresh = kit.get(&ines, &uri(workspace)).await;
    assert_eq!(fresh.status, StatusCode::OK);
    assert_eq!(
        fresh.json,
        json!({ "proactivity": "ask_first", "asked_at": null, "revision": 0 })
    );

    // Not now: only that the questions were put off.
    let put_off = kit
        .put(
            &ines,
            &uri(workspace),
            json!({ "proactivity": "ask_first", "asked": true }),
        )
        .await;
    assert_eq!(put_off.status, StatusCode::OK);
    assert_eq!(put_off.json["proactivity"], "ask_first");
    assert_eq!(put_off.json["revision"], 1);
    let asked_at = put_off.json["asked_at"].clone();
    assert!(asked_at.is_string());

    let answered = kit
        .put(
            &ines,
            &uri(workspace),
            json!({ "proactivity": "act_then_tell", "asked": true }),
        )
        .await;
    assert_eq!(answered.json["proactivity"], "act_then_tell");
    assert_eq!(
        answered.json["asked_at"], asked_at,
        "the first time is kept"
    );
    assert_eq!(answered.json["revision"], 2);

    let bad = kit
        .put(
            &ines,
            &uri(workspace),
            json!({ "proactivity": "always", "asked": true }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn owners_and_admins_get_only_their_own(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (marta, _) = kit.member("marta", workspace);
    let (owner, _) = kit.member_as("olga", workspace, Role::Owner);
    let (admin, _) = kit.member_as("adam", workspace, Role::Admin);
    kit.put(
        &marta,
        &uri(workspace),
        json!({ "proactivity": "act_then_tell", "asked": true }),
    )
    .await;

    for token in [&owner, &admin] {
        let theirs = kit.get(token, &uri(workspace)).await;
        assert_eq!(theirs.status, StatusCode::OK);
        assert_eq!(theirs.json["proactivity"], "ask_first");
        assert_eq!(theirs.json["revision"], 0);
    }
    let client = kit.client(workspace, "assistant:read assistant:write");
    let by_client = kit.get(&client, &uri(workspace)).await;
    assert_eq!(by_client.status, StatusCode::FORBIDDEN);
    assert_eq!(by_client.json["code"], "profile_needs_member");
    let elsewhere = kit.get(&marta, &uri(WorkspaceId::new())).await;
    assert_eq!(elsewhere.status, StatusCode::NOT_FOUND);
    assert_eq!(
        kit.get("", &uri(workspace)).await.status,
        StatusCode::UNAUTHORIZED
    );
    let read_only = kit.token("marta", "assistant:read");
    let refused = kit
        .put(
            &read_only,
            &uri(workspace),
            json!({ "proactivity": "ask_first", "asked": true }),
        )
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn row_level_security_keeps_workspaces_apart(db: PgPool) {
    let kit = Kit::new(db).await;
    let ours = WorkspaceId::new();
    let theirs = WorkspaceId::new();
    let (ines, _) = kit.member("ines", ours);
    let (leo, _) = kit.member("leo", theirs);
    kit.put(
        &ines,
        &uri(ours),
        json!({ "proactivity": "ask_first", "asked": true }),
    )
    .await;
    kit.put(
        &leo,
        &uri(theirs),
        json!({ "proactivity": "ask_first", "asked": true }),
    )
    .await;

    let mut tx = kit.state.db.begin_for(ours).await.unwrap();
    let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM assistant.profiles")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(seen, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_port_answers_per_member(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (marta, marta_id) = kit.member("marta", workspace);
    let (_, leo_id) = kit.member("leo", workspace);
    kit.put(
        &marta,
        &uri(workspace),
        json!({ "proactivity": "act_then_tell", "asked": true }),
    )
    .await;

    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    assert!(
        Preferences
            .acts_without_asking(&mut tx, workspace, marta_id)
            .await
            .unwrap()
    );
    assert!(
        !Preferences
            .acts_without_asking(&mut tx, workspace, leo_id)
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn leaving_and_deleting_purge(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (marta, marta_id) = kit.member("marta", workspace);
    let (leo, _) = kit.member("leo", workspace);
    for token in [&marta, &leo] {
        kit.put(
            token,
            &uri(workspace),
            json!({ "proactivity": "act_then_tell", "asked": true }),
        )
        .await;
    }

    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &MemberLeft {
                workspace: workspace.to_string(),
                user: marta_id,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    assert_eq!(kit.get(&marta, &uri(workspace)).await.json["revision"], 0);
    assert_eq!(kit.get(&leo, &uri(workspace)).await.json["revision"], 1);

    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &WorkspaceDeleting {
                workspace: workspace.to_string(),
                purge_after: Utc::now(),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM assistant.profiles")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
}
