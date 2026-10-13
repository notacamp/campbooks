//! The projection of `conversations.need_changed`.

use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use sqlx::PgPool;

use crate::{
    domain::Reason,
    testkit::{Kit, Need},
};

fn need(revision: i64, reason: Reason) -> Need {
    Need {
        conversation: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11".into(),
        inbox: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22".into(),
        message: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c33".into(),
        revision,
        reason,
        due_on: None,
        sent_at: Utc::now() - TimeDelta::hours(2),
    }
}

async fn items(db: &PgPool) -> Vec<(String, String, i64)> {
    sqlx::query_as("SELECT reason, state, producer_revision FROM attention.items")
        .fetch_all(db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn one_item_per_conversation_and_newer_revisions_win(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    kit.need(workspace, &need(1, Reason::Reply)).await;
    kit.need(workspace, &need(3, Reason::Pay)).await;
    // A late delivery of revision 2 changes nothing.
    kit.need(workspace, &need(2, Reason::Do)).await;
    assert_eq!(items(&db).await, [("pay".into(), "open".into(), 3)]);
    // The same event again (a replay) changes nothing either.
    kit.need(workspace, &need(3, Reason::Pay)).await;
    let changed = kit.events("attention.item_changed").await;
    assert_eq!(changed.len(), 2, "{changed:?}");
    assert_eq!(changed[0]["container"]["type"], "conversations.inbox");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn clearing_keeps_the_row_and_says_why(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    kit.need(workspace, &need(1, Reason::Reply)).await;
    kit.cleared(workspace, &need(2, Reason::Reply), "replied", None)
        .await;
    let (state, cause): (String, String) =
        sqlx::query_as("SELECT state, cleared_because FROM attention.items")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((state.as_str(), cause.as_str()), ("cleared", "replied"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_cleared_need_never_seen_creates_nothing(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    kit.cleared(WorkspaceId::new(), &need(1, Reason::Reply), "replied", None)
        .await;
    assert!(items(&db).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn replaying_rebuilds_the_items(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    kit.need(workspace, &need(1, Reason::Reply)).await;
    kit.need(workspace, &need(2, Reason::Do)).await;
    let before = items(&db).await;
    sqlx::query("DELETE FROM attention.items")
        .execute(&db)
        .await
        .unwrap();
    // Replay: deliver every logged need again, in log order.
    let logged = kit.events("conversations.need_changed").await;
    for payload in logged {
        let event: crate::events::ConversationNeedChanged =
            serde_json::from_value(payload).unwrap();
        let mut tx = campbooks_kernel::db::BeginFor::begin_for(&kit.state.db, workspace)
            .await
            .unwrap();
        kit.state.events.publish(&mut tx, &event).await.unwrap();
        tx.commit().await.unwrap();
    }
    kit.run_jobs().await;
    assert_eq!(items(&db).await, before);
}
