//! A Notion stream's run, against the fake Notion: the first read, an
//! unchanged page, Notion's edits, a page that stops being shared, trouble
//! and refusals, and what the events carry (campbooks-cloud#376).

use campbooks_kernel::{
    blobs::BlobRef,
    clock::Clock as _,
    tenancy::{Role, WorkspaceId},
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::testkit::{Kit, assert_contract};

const SECRET_TEXT: &str = "The launch is on the 14th, tell nobody";

struct Linked {
    kit: Kit,
    ws: WorkspaceId,
    connection: Value,
    page: Uuid,
    notion_page: Uuid,
    stream: Uuid,
}

async fn linked(db: PgPool) -> Linked {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let notion_page = kit.notion.add_page("Product roadmap", None);
    kit.notion.paragraph(notion_page, SECRET_TEXT);
    let page = Uuid::now_v7();
    let stream = kit.link_notion(ws, &connection, page, notion_page).await;
    Linked {
        kit,
        ws,
        connection,
        page,
        notion_page,
        stream,
    }
}

async fn reads(l: &Linked) -> Vec<Value> {
    l.kit
        .payloads(l.ws, "connectors.notion_page_read")
        .await
}

async fn health(l: &Linked) -> String {
    sqlx::query_scalar("SELECT health FROM connectors.connections WHERE provider = 'notion'")
        .fetch_one(&l.kit.db)
        .await
        .unwrap()
}

async fn run_again(l: &Linked) {
    let due = l.kit.notion_due(l.stream).await.expect("scheduled");
    l.kit
        .tick((due - l.kit.clock.now()).max(TimeDelta::zero()) + TimeDelta::seconds(1))
        .await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn linking_a_page_reads_it_and_announces_the_snapshot_by_reference(db: PgPool) {
    let l = linked(db).await;
    let events = reads(&l).await;
    assert_eq!(events.len(), 1, "{events:?}");
    let event = &events[0];
    assert_contract("connectors.notion_page_read.v1.schema.json", event);
    assert_eq!(event["notion_page"], l.notion_page.to_string());
    assert_eq!(event["page"], l.page.to_string());
    assert_eq!(event["connection"], l.connection["id"]);
    // The event carries the reference only: the text is in the blob.
    assert!(!event.to_string().contains("launch"));
    let blob: BlobRef = serde_json::from_value(event["snapshot"].clone()).unwrap();
    let snapshot: Value = serde_json::from_slice(&l.kit.blob(l.ws, &blob).await).unwrap();
    assert_contract("notion-page-snapshot.v1.schema.json", &snapshot);
    assert_eq!(snapshot["title"], "Product roadmap");
    assert_eq!(snapshot["complete"], true);
    assert_eq!(snapshot["blocks"][0]["runs"][0]["text"], SECRET_TEXT);
    assert!(snapshot.get("hash").is_none(), "the snapshot carries no hash");
    // No event, anywhere, carries the page's words.
    let everything: Vec<String> =
        sqlx::query_scalar("SELECT payload::text FROM kernel.events")
            .fetch_all(&l.kit.db)
            .await
            .unwrap();
    assert!(everything.iter().all(|p| !p.contains("launch")));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unchanged_page_costs_one_call_and_announces_nothing(db: PgPool) {
    let l = linked(db).await;
    l.kit.notion.forget_calls();
    run_again(&l).await;
    assert_eq!(reads(&l).await.len(), 1);
    let calls = l.kit.notion.calls();
    assert!(
        calls.iter().all(|c| !c.contains("/children")),
        "the blocks were read again: {calls:?}"
    );
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(health(&l).await, "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_edit_in_notion_is_read_at_the_next_run(db: PgPool) {
    let l = linked(db).await;
    let block = l.kit.notion.children_of(l.notion_page)[0];
    l.kit.notion.set_text(block, "Moved to the 21st");
    run_again(&l).await;
    let events = reads(&l).await;
    assert_eq!(events.len(), 2);
    let (first, second) = (
        events[0]["read_at"].as_str().unwrap(),
        events[1]["read_at"].as_str().unwrap(),
    );
    assert!(second > first, "a later read has a later time");
    let blob: BlobRef = serde_json::from_value(events[1]["snapshot"].clone()).unwrap();
    let snapshot: Value = serde_json::from_slice(&l.kit.blob(l.ws, &blob).await).unwrap();
    assert_eq!(snapshot["blocks"][0]["runs"][0]["text"], "Moved to the 21st");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_webhook_event_makes_the_run_read_even_if_the_edit_time_did_not_move(db: PgPool) {
    let l = linked(db).await;
    let body = serde_json::to_vec(&json!({
        "type": "page.content_updated",
        "entity": {"id": l.notion_page.to_string(), "type": "page"},
    }))
    .unwrap();
    let posted = l
        .kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(posted.status.as_u16(), 202);
    l.kit.tick(TimeDelta::seconds(1)).await;
    assert_eq!(reads(&l).await.len(), 2);
    let cursor: Value = sqlx::query_scalar("SELECT cursor FROM connectors.streams WHERE id = $1")
        .bind(l.stream)
        .fetch_one(&l.kit.db)
        .await
        .unwrap();
    assert_eq!(cursor["force"], json!(false));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_that_is_not_shared_any_more_ends_the_link_and_deletes_nothing(db: PgPool) {
    let l = linked(db).await;
    l.kit.notion.unshare(l.notion_page);
    l.kit.notion.forget_calls();
    run_again(&l).await;
    let stopped = l.kit.payloads(l.ws, "connectors.notion_link_stopped").await;
    assert_eq!(stopped.len(), 1, "{stopped:?}");
    assert_contract("connectors.notion_link_stopped.v1.schema.json", &stopped[0]);
    assert_eq!(stopped[0]["reason"], "not_shared");
    assert_eq!(stopped[0]["page"], l.page.to_string());
    assert!(l.kit.notion_stream_opt(l.notion_page).await.is_none());
    let streams: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.streams")
        .fetch_one(&l.kit.db)
        .await
        .unwrap();
    assert_eq!(streams, 0);
    // Notion was only ever read from.
    assert!(
        l.kit.notion.calls().iter().all(|c| c.starts_with("GET")),
        "{:?}",
        l.kit.notion.calls()
    );
    assert!(l.kit.notion.deleted().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_in_the_trash_ends_the_link_with_its_own_reason(db: PgPool) {
    let l = linked(db).await;
    l.kit.notion.trash_page(l.notion_page);
    run_again(&l).await;
    let stopped = l.kit.payloads(l.ws, "connectors.notion_link_stopped").await;
    assert_eq!(stopped.len(), 1);
    assert_eq!(stopped[0]["reason"], "trashed_in_notion");
    assert!(l.kit.notion.deleted().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn throttling_backs_the_run_off_without_losing_the_page(db: PgPool) {
    let l = linked(db).await;
    l.kit.notion.set_text(l.kit.notion.children_of(l.notion_page)[0], "Edited");
    l.kit.notion.throttle_next(120);
    run_again(&l).await;
    assert_eq!(reads(&l).await.len(), 1, "nothing was read while throttled");
    assert!(l.kit.notion_stream_opt(l.notion_page).await.is_some());
    // Later, the page is read.
    l.kit.tick(TimeDelta::minutes(10)).await;
    assert_eq!(reads(&l).await.len(), 2);
    assert_eq!(health(&l).await, "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refresh_rotates_both_tokens_in_one_step(db: PgPool) {
    let l = linked(db).await;
    let id: Uuid = l.connection["id"].as_str().unwrap().parse().unwrap();
    let sealed = |db: &PgPool| {
        let db = db.clone();
        async move {
            sqlx::query_as::<_, (Vec<u8>, Vec<u8>)>(
                "SELECT refresh_token, access_token FROM connectors.credentials
                  WHERE connection_id = $1",
            )
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    let before = sealed(&l.kit.db).await;
    // The access token has expired: the next run refreshes.
    l.kit.clock.advance(TimeDelta::hours(2));
    l.kit.notion.set_text(l.kit.notion.children_of(l.notion_page)[0], "Edited");
    run_again(&l).await;
    let after = sealed(&l.kit.db).await;
    assert_ne!(before.0, after.0, "the refresh token was not rotated");
    assert_ne!(before.1, after.1, "the access token was not renewed");
    assert_eq!(reads(&l).await.len(), 2, "the run went on with the new token");
    assert_eq!(health(&l).await, "ok");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_grant_asks_the_member_to_connect_again_and_keeps_the_link(db: PgPool) {
    let l = linked(db).await;
    l.kit.clock.advance(TimeDelta::hours(2));
    l.kit
        .notion
        .refuse_refresh(400, json!({"error": "invalid_grant"}));
    run_again(&l).await;
    assert_eq!(health(&l).await, "needs_reauth");
    // The page is still linked: it runs again once the member is back.
    assert!(l.kit.notion_stream_opt(l.notion_page).await.is_some());
    assert!(
        l.kit
            .payloads(l.ws, "connectors.notion_link_stopped")
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_notion_a_run_changes_nothing(db: PgPool) {
    let l = linked(db).await;
    // The same database, with Notion not offered any more.
    let off = Kit::new(l.kit.db.clone()).await;
    off.tick(TimeDelta::days(1)).await;
    assert!(l.kit.notion_stream_opt(l.notion_page).await.is_some());
    assert_eq!(reads(&l).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn ending_the_link_in_pages_removes_the_stream_and_what_it_recorded(db: PgPool) {
    let l = linked(db).await;
    l.kit
        .unlink_notion(l.ws, &l.connection, l.page, l.notion_page)
        .await;
    assert!(l.kit.notion_stream_opt(l.notion_page).await.is_none());
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM connectors.streams)
              + (SELECT count(*) FROM connectors.notion_links)
              + (SELECT count(*) FROM connectors.notion_changes)
              + (SELECT count(*) FROM connectors.batches)",
    )
    .fetch_one(&l.kit.db)
    .await
    .unwrap();
    assert_eq!(left, 0);
    assert!(l.kit.notion.deleted().is_empty());
}
