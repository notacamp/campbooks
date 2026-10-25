//! "Later", per member.

use axum::http::StatusCode;
use campbooks_kernel::{tenancy::WorkspaceId, testing};
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
        latest: None,
        lapses_at: None,
        warning: false,
        from_person: None,
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
    // A client acting for no member does not see the item at all.
    let denied = kit.post(&client, &uri, tomorrow.clone()).await;
    assert_eq!(denied.status, StatusCode::NOT_FOUND);
    let reader = kit.client(workspace, "attention:read");
    assert_eq!(
        kit.post(&reader, &uri, tomorrow).await.status,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_sender_writing_again_ends_later_even_without_a_new_need(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    kit.let_see(INBOX, sam_id);
    kit.need(workspace, &need(1, FIRST)).await;
    let item = kit.get(&sam, &list_uri(workspace)).await.json["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let monday = Utc::now() + TimeDelta::days(4);
    kit.post(
        &sam,
        &format!("{}/{item}/snooze", list_uri(workspace)),
        Some(json!({"until": monday})),
    )
    .await;
    assert_eq!(count(&kit, &sam, workspace).await, 0);
    // Same need (same cause), a newer message from the sender.
    let mut again = need(2, FIRST);
    again.latest = Some(FOLLOW_UP.into());
    kit.need(workspace, &again).await;
    assert_eq!(count(&kit, &sam, workspace).await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_removed_inbox_takes_its_items_and_their_later_with_it(db: PgPool) {
    use campbooks_kernel::db::BeginFor as _;

    let kit = Kit::new(db.clone()).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    kit.let_see(INBOX, sam_id);
    kit.need(workspace, &need(1, FIRST)).await;
    let item = kit.get(&sam, &list_uri(workspace)).await.json["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    kit.post(
        &sam,
        &format!("{}/{item}/snooze", list_uri(workspace)),
        Some(json!({"until": Utc::now() + TimeDelta::days(1)})),
    )
    .await;
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &crate::events::InboxRemoved {
                workspace: workspace.to_string(),
                inbox: INBOX.into(),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM attention.items) + (SELECT count(*) FROM attention.snoozes)",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(left, 0);
}

/// A time `days` from now, in whole seconds (as the database keeps it).
fn in_days(days: i64) -> chrono::DateTime<Utc> {
    chrono::DateTime::from_timestamp(Utc::now().timestamp() + days * 86_400, 0).unwrap()
}

fn at(value: &serde_json::Value) -> chrono::DateTime<Utc> {
    value.as_str().unwrap().parse().unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn later_is_tracked_with_what_it_replaced_and_a_no_op_is_not(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    kit.let_see(INBOX, sam_id);
    kit.need(workspace, &need(1, FIRST)).await;
    let item = kit.get(&sam, &list_uri(workspace)).await.json["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let base = format!("{}/{item}", list_uri(workspace));
    let (monday, tuesday) = (in_days(4), in_days(5));

    // Nothing to bring back: no event.
    let back = kit.post(&sam, &format!("{base}/unsnooze"), None).await;
    assert_eq!(back.status, StatusCode::OK);
    for until in [monday, tuesday, tuesday] {
        let snoozed = kit
            .post(
                &sam,
                &format!("{base}/snooze"),
                Some(json!({"until": until})),
            )
            .await;
        assert_eq!(snoozed.status, StatusCode::OK, "{:?}", snoozed.json);
    }
    let back = kit.post(&sam, &format!("{base}/unsnooze"), None).await;
    assert_eq!(back.status, StatusCode::OK);

    let snoozed = testing::tracked_events(&kit.db, "attention.item_snoozed").await;
    assert_eq!(snoozed.len(), 2, "the same Later again tracks nothing");
    for event in &snoozed {
        assert_eq!(event.version, 1);
        assert_eq!(event.workspace_id, Some(*workspace.as_uuid()));
        assert_eq!(
            event.envelope["actor"],
            json!({"type": "user", "id": "sam"})
        );
        assert_eq!(event.envelope["operation"], "attention.snooze");
        let snapshot = &event.body["item"];
        assert_eq!(snapshot["id"], item.as_str());
        assert_eq!(
            snapshot["record"],
            json!({"type": "conversations.conversation", "id": need(1, FIRST).conversation})
        );
        assert_eq!(snapshot["reason"], "reply");
        assert_eq!(snapshot["trust"], "sure");
        assert_eq!(snapshot["state"], "open");
        assert_eq!(snapshot["urgent"], false);
    }
    assert!(snoozed[0].body.get("before").is_none());
    assert_eq!(at(&snoozed[0].body["item"]["snoozed_until"]), monday);
    assert_eq!(at(&snoozed[1].body["before"]), monday);
    assert_eq!(at(&snoozed[1].body["item"]["snoozed_until"]), tuesday);

    let unsnoozed = testing::tracked_events(&kit.db, "attention.item_unsnoozed").await;
    assert_eq!(unsnoozed.len(), 1, "nothing to end tracks nothing");
    let event = &unsnoozed[0];
    assert_eq!(
        event.envelope["actor"],
        json!({"type": "user", "id": "sam"})
    );
    assert_eq!(event.envelope["operation"], "attention.unsnooze");
    assert_eq!(at(&event.body["before"]), tuesday);
    assert!(event.body["item"].get("snoozed_until").is_none());
}
