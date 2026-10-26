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
        "api_version": "2026-03-11",
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
async fn with_the_secret_set_a_verification_post_is_none_of_ours(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, _, stream) = linked(&kit, "marta").await;
    let body = serde_json::to_vec(&json!({"verification_token": "secret_abc"})).unwrap();
    // Unsigned, and even signed: a token cannot be armed again.
    assert_eq!(
        kit.notion_hook(&body, None).await.status,
        StatusCode::NOT_FOUND
    );
    let signed = Kit::notion_signature(&body);
    assert_eq!(
        kit.notion_hook(&body, Some(&signed)).await.status,
        StatusCode::NOT_FOUND
    );
    assert!(!due(&kit, stream).await);
    assert_eq!(stored_tokens(&kit).await, 0, "nothing was kept");
    // Signatures stay enforced for everything else.
    let event = serde_json::to_vec(&json!({"type": "page.deleted",
        "entity": {"id": Uuid::now_v7().to_string()}}))
    .unwrap();
    assert_eq!(
        kit.notion_hook(&event, None).await.status,
        StatusCode::UNAUTHORIZED
    );
}

async fn stored_tokens(kit: &Kit) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM connectors.notion_webhook_token")
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

fn verification(token: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({"verification_token": token})).unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn before_the_secret_exists_the_verification_token_is_kept_sealed(db: PgPool) {
    let kit = Kit::with_notion_unsigned(db).await;
    let posted = kit.notion_hook(&verification("secret_first"), None).await;
    assert_eq!(posted.status, StatusCode::OK);
    assert_eq!(stored_tokens(&kit).await, 1);
    let sealed: Vec<u8> = sqlx::query_scalar("SELECT token FROM connectors.notion_webhook_token")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&sealed).contains("secret_first"));
    // It opens with the instance key and the token's own purpose only.
    let key = campbooks_kernel::crypto::SecretKey::from_bytes([3; 32]);
    let stored = crate::notion_webhook_token::read(
        &mut *kit.db.acquire().await.unwrap(),
        &crate::notion_webhook_token::cipher(&key),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(stored.token, "secret_first");
    assert!(!format!("{stored:?}").contains("secret_first"));
    let credentials_cipher =
        campbooks_kernel::crypto::Cipher::new(&key, crate::CREDENTIALS_PURPOSE);
    assert!(
        credentials_cipher
            .open(&sealed, b"connectors.notion_webhook_token")
            .is_err()
    );
    // The token appears in no event.
    let events: Vec<String> = sqlx::query_scalar("SELECT payload::text FROM kernel.events")
        .fetch_all(&kit.db)
        .await
        .unwrap();
    assert!(events.iter().all(|p| !p.contains("secret_first")));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_second_verification_replaces_the_first(db: PgPool) {
    let kit = Kit::with_notion_unsigned(db).await;
    assert_eq!(
        kit.notion_hook(&verification("secret_first"), None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        kit.notion_hook(&verification("secret_second"), None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(stored_tokens(&kit).await, 1);
    let key = campbooks_kernel::crypto::SecretKey::from_bytes([3; 32]);
    let stored = crate::notion_webhook_token::read(
        &mut *kit.db.acquire().await.unwrap(),
        &crate::notion_webhook_token::cipher(&key),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(stored.token, "secret_second");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn before_the_secret_exists_nothing_but_a_bare_verification_token_is_taken(db: PgPool) {
    let kit = Kit::with_notion_unsigned(db).await;
    let long = verification(&"t".repeat(201));
    let padded = {
        let mut body = verification("secret_x");
        body.extend(std::iter::repeat_n(b' ', 2000));
        body
    };
    for body in [
        serde_json::to_vec(&json!({"type": "page.content_updated",
            "entity": {"id": Uuid::now_v7().to_string()}}))
        .unwrap(),
        serde_json::to_vec(&json!({"verification_token": "x", "type": "page.deleted"})).unwrap(),
        serde_json::to_vec(&json!({"verification_token": 7})).unwrap(),
        serde_json::to_vec(&json!({"verification_token": ["x"]})).unwrap(),
        verification(""),
        verification("has a space"),
        verification("new\nline"),
        long,
        padded,
        b"not json".to_vec(),
        b"{}".to_vec(),
        b"[]".to_vec(),
    ] {
        let posted = kit
            .notion_hook(&body, Some(&Kit::notion_signature(&body)))
            .await;
        assert_eq!(
            posted.status,
            StatusCode::NOT_FOUND,
            "{}",
            String::from_utf8_lossy(&body[..body.len().min(80)])
        );
    }
    assert_eq!(stored_tokens(&kit).await, 0);
    // The longest allowed one is kept.
    let edge = verification(&"t".repeat(200));
    assert_eq!(kit.notion_hook(&edge, None).await.status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_runtime_role_can_replace_the_token_but_never_read_it(db: PgPool) {
    let kit = Kit::with_notion_unsigned(db).await;
    kit.notion_hook(&verification("secret_first"), None).await;
    let app = campbooks_kernel::testing::app_role_pool(&kit.db).await;
    let read = sqlx::query("SELECT token FROM connectors.notion_webhook_token")
        .fetch_all(&app)
        .await;
    assert!(read.is_err(), "the runtime role reads the token");
    let count = sqlx::query("SELECT count(*) FROM connectors.notion_webhook_token")
        .fetch_all(&app)
        .await;
    assert!(count.is_err());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_page_events_for_a_known_page_wake_a_stream(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, page, stream) = linked(&kit, "marta").await;
    // Unknown event types and unusable bodies are answered 200 (so Notion
    // does not retry them) and mark nothing due.
    for body in [
        event("comment.created", page),
        event("page.locked", page),
        event("database.schema_updated", page),
        event("page.content_updated", Uuid::now_v7()),
        b"{}".to_vec(),
        serde_json::to_vec(
            &json!({"type": "page.content_updated", "entity": {"id": "not-a-uuid"}}),
        )
        .unwrap(),
    ] {
        let posted = kit
            .notion_hook(&body, Some(&Kit::notion_signature(&body)))
            .await;
        assert_eq!(
            posted.status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&body)
        );
    }
    assert!(!due(&kit, stream).await);
    assert!(!forced(&kit, stream).await);
    let junk = b"not json".to_vec();
    let posted = kit
        .notion_hook(&junk, Some(&Kit::notion_signature(&junk)))
        .await;
    assert_eq!(posted.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn each_of_the_five_page_events_marks_the_stream_due(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, page, stream) = linked(&kit, "marta").await;
    for kind in [
        "page.content_updated",
        "page.properties_updated",
        "page.moved",
        "page.deleted",
        "page.undeleted",
    ] {
        sqlx::query("UPDATE connectors.sync_due SET due_at = now() + interval '1 day'")
            .execute(&kit.db)
            .await
            .unwrap();
        sqlx::query("UPDATE connectors.streams SET cursor = '{}'::jsonb")
            .execute(&kit.db)
            .await
            .unwrap();
        assert!(!due(&kit, stream).await, "{kind}");
        let body = event(kind, page);
        let posted = kit
            .notion_hook(&body, Some(&Kit::notion_signature(&body)))
            .await;
        assert_eq!(posted.status, StatusCode::ACCEPTED, "{kind}");
        assert!(due(&kit, stream).await, "{kind}");
        assert!(forced(&kit, stream).await, "{kind}");
    }
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
async fn the_hook_is_absent_unless_notion_is_on(db: PgPool) {
    let kit = Kit::new(db).await;
    let body = event("page.content_updated", Uuid::now_v7());
    let posted = kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(posted.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_event_of_another_api_version_or_without_one_is_ignored(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let (_, page, stream) = linked(&kit, "marta").await;
    for version in [Some("2022-06-28"), Some("2099-01-01"), None] {
        let mut body = json!({"type": "page.content_updated",
            "entity": {"id": page.to_string(), "type": "page"}});
        if let Some(version) = version {
            body["api_version"] = json!(version);
        }
        let body = serde_json::to_vec(&body).unwrap();
        let posted = kit
            .notion_hook(&body, Some(&Kit::notion_signature(&body)))
            .await;
        assert_eq!(posted.status, StatusCode::OK, "{version:?}");
    }
    assert!(!due(&kit, stream).await);
    // The pinned version still works.
    let body = event("page.content_updated", page);
    let posted = kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(posted.status, StatusCode::ACCEPTED);
    assert!(due(&kit, stream).await);
}
