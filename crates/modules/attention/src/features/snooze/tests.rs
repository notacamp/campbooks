//! "Later", per member.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    domain::Reason,
    testkit::{Kit, Need},
};

const INBOX: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22";

fn need(revision: i64, message: &str) -> Need {
    Need {
        conversation: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11".into(),
        inbox: INBOX.into(),
        message: message.into(),
        revision,
        reason: Reason::Reply,
        due_on: None,
        sent_at: Utc::now() - TimeDelta::hours(1),
    }
}

const FIRST: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c33";
const FOLLOW_UP: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c44";

#[sqlx::test(migrations = "../../../migrations")]
async fn later_hides_it_for_me_only_until_a_follow_up(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let (leo, leo_id) = kit.member("leo", workspace);
    kit.let_see(INBOX, sam_id);
    kit.let_see(INBOX, leo_id);
    kit.need(workspace, &need(1, FIRST)).await;
    let list = format!("/api/v1/w/{workspace}/attention");
    let item = kit.get(&sam, &list).await.json["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let base = format!("{list}/{item}");

    let monday = Utc::now() + TimeDelta::days(4);
    let snoozed = kit
        .post(
            &sam,
            &format!("{base}/snooze"),
            Some(json!({"until": monday})),
        )
        .await;
    assert_eq!(snoozed.status, StatusCode::OK, "{:?}", snoozed.json);
    assert!(snoozed.json["snoozed_until"].is_string());
    assert_eq!(count(&kit, &sam, workspace).await, 0);
    assert_eq!(count(&kit, &leo, workspace).await, 1);

    let back = kit.post(&sam, &format!("{base}/unsnooze"), None).await;
    assert_eq!(back.json["snoozed_until"], serde_json::Value::Null);
    assert_eq!(count(&kit, &sam, workspace).await, 1);

    // Put off again, then the sender writes again: it is back at once.
    kit.post(
        &sam,
        &format!("{base}/snooze"),
        Some(json!({"until": monday})),
    )
    .await;
    kit.need(workspace, &need(2, FOLLOW_UP)).await;
    assert_eq!(count(&kit, &sam, workspace).await, 1);
    assert_eq!(kit.events("attention.item_changed").await.len(), 5);
}

async fn count(kit: &Kit, token: &str, workspace: WorkspaceId) -> usize {
    kit.get(token, &list_uri(workspace)).await.json["items"]
        .as_array()
        .unwrap()
        .len()
}

fn list_uri(workspace: WorkspaceId) -> String {
    format!("/api/v1/w/{workspace}/attention")
}

#[sqlx::test(migrations = "../../../migrations")]
async fn later_needs_a_future_time_a_member_and_an_item_you_can_see(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let (eve, _) = kit.member("eve", workspace);
    kit.let_see(INBOX, sam_id);
    kit.need(workspace, &need(1, FIRST)).await;
    let item = kit.get(&sam, &list_uri(workspace)).await.json["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let uri = format!("{}/{item}/snooze", list_uri(workspace));
    for until in [
        Utc::now() - TimeDelta::hours(1),
        Utc::now() + TimeDelta::days(400),
    ] {
        let bad = kit.post(&sam, &uri, Some(json!({"until": until}))).await;
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);
        assert_eq!(bad.json["code"], "invalid_until");
    }
    let tomorrow = Some(json!({"until": Utc::now() + TimeDelta::days(1)}));
    assert_eq!(
        kit.post(&eve, &uri, tomorrow.clone()).await.status,
        StatusCode::NOT_FOUND
    );
    let client = kit.client(workspace, "attention:read attention:write");
    let denied = kit.post(&client, &uri, tomorrow.clone()).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_eq!(denied.json["code"], "snooze_needs_member");
    let reader = kit.client(workspace, "attention:read");
    assert_eq!(
        kit.post(&reader, &uri, tomorrow).await.status,
        StatusCode::FORBIDDEN
    );
}
