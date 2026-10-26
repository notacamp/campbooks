//! Notion's webhook: a forged post does nothing, an authentic event only
//! makes the streams of that page read early, in their own workspace, and
//! the hook never calls Notion or reads a page.

use axum::http::StatusCode;
use campbooks_kernel::{
    clock::Clock as _,
    tenancy::{Role, WorkspaceId},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::{MAX_BODY, signed};
use crate::testkit::{Kit, NOTION_HOOK_SECRET};

fn event(kind: &str, page: Uuid) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "id": Uuid::now_v7().to_string(),
        "type": kind,
        "entity": {"id": page.to_string(), "type": "page"},
    }))
    .unwrap()
}

async fn forced(kit: &Kit, stream: Uuid) -> bool {
    let cursor: Value = sqlx::query_scalar("SELECT cursor FROM connectors.streams WHERE id = $1")
        .bind(stream)
        .fetch_one(&kit.db)
        .await
        .unwrap();
    cursor["force"] == json!(true)
}

async fn due(kit: &Kit, stream: Uuid) -> bool {
    kit.notion_due(stream)
        .await
        .is_some_and(|at| at <= kit.clock.now())
}

/// A workspace with Marta's Notion connected and one page linked.
async fn linked(kit: &Kit, name: &str) -> (WorkspaceId, Uuid, Uuid) {
    let ws = WorkspaceId::new();
    let marta = kit.user(name);
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let notion_page = kit.notion.add_page(&format!("Roadmap of {name}"), None);
    let stream = kit
        .link_notion(ws, &connection, Uuid::now_v7(), notion_page)
        .await;
    (ws, notion_page, stream)
}

#[test]
fn a_signature_is_the_hmac_of_the_raw_body() {
    let body = br#"{"type":"page.content_updated"}"#;
    let good = Kit::notion_signature(body);
    assert!(signed(NOTION_HOOK_SECRET.as_bytes(), body, &good));
    assert!(!signed(NOTION_HOOK_SECRET.as_bytes(), b"{}", &good));
    assert!(!signed(b"another secret", body, &good));
    assert!(!signed(NOTION_HOOK_SECRET.as_bytes(), body, ""));
    assert!(!signed(NOTION_HOOK_SECRET.as_bytes(), body, "sha256=zz"));
    assert!(!signed(NOTION_HOOK_SECRET.as_bytes(), body, "sha256=abc"));
    assert!(!signed(
        NOTION_HOOK_SECRET.as_bytes(),
        body,
        good.trim_start_matches("sha256=")
    ));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_authentic_event_makes_the_page_read_now(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, page, stream) = linked(&kit, "marta").await;
    assert!(!due(&kit, stream).await);
    let calls = kit.notion.calls().len();
    let body = event("page.content_updated", page);
    let posted = kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(posted.status, StatusCode::ACCEPTED);
    assert!(due(&kit, stream).await);
    assert!(forced(&kit, stream).await);
    // The hook reads nothing and calls nobody: the run does.
    assert_eq!(kit.notion.calls().len(), calls);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_forged_post_does_nothing(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, page, stream) = linked(&kit, "marta").await;
    let body = event("page.content_updated", page);
    // No signature, a wrong one, one for another body, and a malformed one.
    let other = Kit::notion_signature(b"something else");
    for signature in [
        None,
        Some("sha256=00"),
        Some(other.as_str()),
        Some("nonsense"),
    ] {
        let posted = kit.notion_hook(&body, signature).await;
        assert_eq!(posted.status, StatusCode::UNAUTHORIZED, "{signature:?}");
    }
    assert!(!due(&kit, stream).await);
    assert!(!forced(&kit, stream).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_signature_does_not_cover_a_changed_body(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, page, stream) = linked(&kit, "marta").await;
    let other = kit.notion.add_page("Another", None);
    let signature = Kit::notion_signature(&event("page.content_updated", other));
    // The signature of an event for another page is of no use here.
    let posted = kit
        .notion_hook(&event("page.content_updated", page), Some(&signature))
        .await;
    assert_eq!(posted.status, StatusCode::UNAUTHORIZED);
    assert!(!due(&kit, stream).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_oversize_body_is_refused_before_anything_is_checked(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, page, stream) = linked(&kit, "marta").await;
    let mut body = event("page.content_updated", page);
    body.extend(std::iter::repeat_n(b' ', MAX_BODY + 1));
    let posted = kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(posted.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!due(&kit, stream).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_verification_post_is_answered_and_wakes_nothing(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, _, stream) = linked(&kit, "marta").await;
    let body = serde_json::to_vec(&json!({"verification_token": "secret_abc"})).unwrap();
    let posted = kit.notion_hook(&body, None).await;
    assert_eq!(posted.status, StatusCode::OK);
    assert!(!due(&kit, stream).await);
    // A verification-shaped body that also names an event is not one.
    let tricked = serde_json::to_vec(
        &json!({"verification_token": "x", "type": "page.deleted",
                "entity": {"id": Uuid::now_v7().to_string()}}),
    )
    .unwrap();
    assert_eq!(
        kit.notion_hook(&tricked, None).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_page_events_for_a_known_page_wake_a_stream(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, page, stream) = linked(&kit, "marta").await;
    for body in [
        event("comment.created", page),
        event("page.content_updated", Uuid::now_v7()),
        b"{}".to_vec(),
        serde_json::to_vec(&json!({"type": "page.content_updated", "entity": {"id": "not-a-uuid"}}))
            .unwrap(),
    ] {
        let posted = kit
            .notion_hook(&body, Some(&Kit::notion_signature(&body)))
            .await;
        assert_eq!(posted.status, StatusCode::ACCEPTED);
    }
    assert!(!due(&kit, stream).await);
    let junk = b"not json".to_vec();
    let posted = kit
        .notion_hook(&junk, Some(&Kit::notion_signature(&junk)))
        .await;
    assert_eq!(posted.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_event_wakes_only_the_streams_of_that_page_in_their_own_workspaces(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (ws_a, page_a, stream_a) = linked(&kit, "marta").await;
    let (ws_b, page_b, stream_b) = linked(&kit, "rui").await;
    assert_ne!(ws_a, ws_b);
    let body = event("page.content_updated", page_a);
    let posted = kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(posted.status, StatusCode::ACCEPTED);
    assert!(due(&kit, stream_a).await);
    assert!(forced(&kit, stream_a).await);
    // The other workspace's stream, reading another page, is left alone.
    assert!(!due(&kit, stream_b).await);
    assert!(!forced(&kit, stream_b).await);
    let body = event("page.deleted", page_b);
    kit.notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert!(due(&kit, stream_b).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stream_waiting_for_the_member_is_not_woken(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, page, stream) = linked(&kit, "marta").await;
    sqlx::query("UPDATE connectors.connections SET health = 'needs_reauth'")
        .execute(&kit.db)
        .await
        .unwrap();
    let body = event("page.content_updated", page);
    let posted = kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(posted.status, StatusCode::ACCEPTED);
    assert!(!forced(&kit, stream).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_hook_is_absent_without_a_webhook_secret(db: PgPool) {
    let kit = Kit::new(db).await;
    let body = event("page.content_updated", Uuid::now_v7());
    let posted = kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(posted.status, StatusCode::NOT_FOUND);
}
