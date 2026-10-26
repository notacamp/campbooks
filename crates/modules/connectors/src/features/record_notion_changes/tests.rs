//! Block changes members' edits asked for: only a request the pages module
//! confirms is recorded, each change is bound to where it is made, a block
//! Notion changed first is held back, and a page is never removed.

use campbooks_kernel::{
    blobs::BlobRef,
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::model::{ConnectionId, StreamId},
    notion::changes::{self, At, Request},
    testkit::{Kit, assert_contract},
    testkit_notion::{CHANGE_TYPE, append_item, delete_item, paragraphs, update_item},
};

struct Linked {
    kit: Kit,
    ws: WorkspaceId,
    connection: Value,
    page: Uuid,
    notion_page: Uuid,
    first: Uuid,
    second: Uuid,
}

async fn linked(db: PgPool) -> Linked {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let notion_page = kit.notion.add_page("Product roadmap", None);
    let first = kit.notion.paragraph(notion_page, "Plan the launch");
    let second = kit.notion.paragraph(notion_page, "Write the notes");
    let page = Uuid::now_v7();
    kit.link_notion(ws, &connection, page, notion_page).await;
    Linked {
        kit,
        ws,
        connection,
        page,
        notion_page,
        first,
        second,
    }
}

async fn completed(l: &Linked) -> Vec<Value> {
    l.kit
        .payloads(l.ws, "connectors.notion_changes_completed")
        .await
}

async fn states(l: &Linked) -> Vec<(String, Option<String>)> {
    sqlx::query_as("SELECT state, reason FROM connectors.notion_changes ORDER BY seq")
        .fetch_all(&l.kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_confirmed_update_is_written_and_reported_with_the_new_edit_time(db: PgPool) {
    let l = linked(db).await;
    let mine = Uuid::now_v7();
    let base = l.kit.notion.edited(l.first);
    l.kit
        .ask_notion_changes(
            l.ws,
            &l.connection,
            (l.page, l.notion_page),
            vec![update_item(mine, l.first, base)],
            &paragraphs(&[(0, mine, "Plan the launch for Friday")]),
            true,
        )
        .await;
    assert_eq!(
        l.kit.notion.text(l.first).as_deref(),
        Some("Plan the launch for Friday")
    );
    let done = completed(&l).await;
    assert_eq!(done.len(), 1, "{done:?}");
    assert_contract("connectors.notion_changes_completed.v1.schema.json", &done[0]);
    let result = &done[0]["results"][0];
    assert_eq!(result["outcome"], "applied");
    assert_eq!(result["block"], mine.to_string());
    assert_eq!(result["notion_block"], l.first.to_string());
    assert!(result["edited_at"].is_string());
    // The text is not in the report.
    assert!(!done[0].to_string().contains("Friday"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_block_notion_changed_first_is_held_back_not_overwritten(db: PgPool) {
    let l = linked(db).await;
    let mine = Uuid::now_v7();
    let base = l.kit.notion.edited(l.first);
    l.kit.notion.set_text(l.first, "Their version");
    l.kit
        .ask_notion_changes(
            l.ws,
            &l.connection,
            (l.page, l.notion_page),
            vec![update_item(mine, l.first, base)],
            &paragraphs(&[(0, mine, "My version")]),
            true,
        )
        .await;
    assert_eq!(l.kit.notion.text(l.first).as_deref(), Some("Their version"));
    assert!(l.kit.notion.patches().is_empty());
    let done = completed(&l).await;
    assert_eq!(done[0]["results"][0]["outcome"], "skipped");
    assert_eq!(done[0]["results"][0]["reason"], "changed_in_notion");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_request_the_pages_module_does_not_confirm_records_nothing(db: PgPool) {
    let l = linked(db).await;
    let mine = Uuid::now_v7();
    l.kit
        .ask_notion_changes(
            l.ws,
            &l.connection,
            (l.page, l.notion_page),
            vec![update_item(mine, l.first, l.kit.notion.edited(l.first))],
            &paragraphs(&[(0, mine, "Planted")]),
            false,
        )
        .await;
    assert!(states(&l).await.is_empty());
    assert_eq!(l.kit.notion.text(l.first).as_deref(), Some("Plan the launch"));
    assert!(l.kit.notion.patches().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_event_naming_another_page_than_the_stored_request_is_ignored(db: PgPool) {
    let l = linked(db).await;
    let mine = Uuid::now_v7();
    // The stored request is for this page; the event claims another one.
    let request = Uuid::now_v7();
    let body: BlobRef = l
        .kit
        .state
        .blobs
        .put(
            &l.ws.to_string(),
            serde_json::to_vec(&paragraphs(&[(0, mine, "Planted")])).unwrap(),
            CHANGE_TYPE,
        )
        .await
        .unwrap();
    let id: ConnectionId = l.connection["id"].as_str().unwrap().parse().unwrap();
    l.kit.pages.release(
        request,
        campbooks_kernel::outbound::StoredPageChanges {
            page: l.page,
            connection: *id.as_uuid(),
            external_page: l.notion_page,
            items: vec![update_item(mine, l.first, l.kit.notion.edited(l.first))],
            body: body.clone(),
        },
    );
    let mut tx = l.kit.state.db.begin_for(l.ws).await.unwrap();
    l.kit
        .state
        .events
        .publish(
            &mut tx,
            &crate::events::NotionChangesRequested {
                workspace: l.ws.to_string(),
                request,
                page: Uuid::now_v7(),
                connection: id,
                notion_page: l.notion_page,
                body,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    l.kit.run_jobs().await;
    assert!(states(&l).await.is_empty());
    assert!(l.kit.notion.patches().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_delivery_twice_writes_once(db: PgPool) {
    let l = linked(db).await;
    let mine = Uuid::now_v7();
    let base = l.kit.notion.edited(l.first);
    let request = l
        .kit
        .ask_notion_changes(
            l.ws,
            &l.connection,
            (l.page, l.notion_page),
            vec![update_item(mine, l.first, base)],
            &paragraphs(&[(0, mine, "Once")]),
            true,
        )
        .await;
    // The same event again.
    let id: ConnectionId = l.connection["id"].as_str().unwrap().parse().unwrap();
    let body: BlobRef = l
        .kit
        .state
        .blobs
        .put(
            &l.ws.to_string(),
            serde_json::to_vec(&paragraphs(&[(0, mine, "Once")])).unwrap(),
            CHANGE_TYPE,
        )
        .await
        .unwrap();
    let mut tx = l.kit.state.db.begin_for(l.ws).await.unwrap();
    l.kit
        .state
        .events
        .publish(
            &mut tx,
            &crate::events::NotionChangesRequested {
                workspace: l.ws.to_string(),
                request,
                page: l.page,
                connection: id,
                notion_page: l.notion_page,
                body,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    l.kit.run_jobs().await;
    assert_eq!(l.kit.notion.patches().len(), 1);
    assert_eq!(states(&l).await.len(), 1);
}

/// Record `request`'s changes for `stream` directly, as the consumer does,
/// to tamper with the rows before the stream runs.
async fn record_for(l: &Linked, stream: StreamId, notion_page: Uuid, request: Uuid, kinds: Vec<(&str, Uuid, Option<Uuid>)>) {
    let id: ConnectionId = l.connection["id"].as_str().unwrap().parse().unwrap();
    let body = l
        .kit
        .state
        .blobs
        .put(
            &l.ws.to_string(),
            serde_json::to_vec(&paragraphs(&[(0, Uuid::now_v7(), "Planted")])).unwrap(),
            CHANGE_TYPE,
        )
        .await
        .unwrap();
    let items: Vec<_> = kinds
        .into_iter()
        .map(|(kind, block, external)| match kind {
            "append" => append_item(block, external),
            "delete" => delete_item(block, external.unwrap(), None),
            _ => update_item(block, external.unwrap(), None),
        })
        .collect();
    let mut tx = l.kit.state.db.begin_for(l.ws).await.unwrap();
    changes::record(
        &mut tx,
        &l.kit.state.binding,
        l.ws,
        stream,
        At {
            connection: id,
            notion_page,
        },
        &Request {
            request,
            page: l.page,
            items: &items,
            body: &body,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_row_edited_after_it_was_bound_is_never_made(db: PgPool) {
    let l = linked(db).await;
    let stream = StreamId::from_uuid(l.kit.notion_stream(l.notion_page).await);
    let request = Uuid::now_v7();
    record_for(
        &l,
        stream,
        l.notion_page,
        request,
        vec![("update", Uuid::now_v7(), Some(l.first))],
    )
    .await;
    // Someone with SQL points the change at another block of the page.
    sqlx::query("UPDATE connectors.notion_changes SET notion_block_id = $1")
        .bind(l.second)
        .execute(&l.kit.db)
        .await
        .unwrap();
    l.kit.tick(TimeDelta::minutes(10)).await;
    assert!(l.kit.notion.patches().is_empty());
    assert_eq!(l.kit.notion.text(l.second).as_deref(), Some("Write the notes"));
    assert_eq!(
        states(&l).await,
        vec![("skipped".to_owned(), Some("unverified".to_owned()))]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_row_without_a_binding_is_never_made(db: PgPool) {
    let l = linked(db).await;
    let stream = l.kit.notion_stream(l.notion_page).await;
    let body = l
        .kit
        .state
        .blobs
        .put(
            &l.ws.to_string(),
            serde_json::to_vec(&paragraphs(&[(0, Uuid::now_v7(), "Planted")])).unwrap(),
            CHANGE_TYPE,
        )
        .await
        .unwrap();
    let mut tx = l.kit.state.db.begin_for(l.ws).await.unwrap();
    sqlx::query(
        "INSERT INTO connectors.notion_changes
             (id, workspace_id, stream_id, request_id, page_id, seq, kind, block_id,
              notion_block_id, body, mac)
         VALUES ($1, $2, $3, $4, $5, 0, 'update', $6, $7, $8, NULL)",
    )
    .bind(Uuid::now_v7())
    .bind(l.ws)
    .bind(stream)
    .bind(Uuid::now_v7())
    .bind(l.page)
    .bind(Uuid::now_v7())
    .bind(l.first)
    .bind(sqlx::types::Json(&body))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    l.kit.tick(TimeDelta::minutes(10)).await;
    assert!(l.kit.notion.patches().is_empty());
    assert_eq!(
        states(&l).await,
        vec![("skipped".to_owned(), Some("unverified".to_owned()))]
    );
    let done = completed(&l).await;
    assert_eq!(done[0]["results"][0]["reason"], "unverified");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_moved_to_another_pages_stream_is_refused(db: PgPool) {
    let l = linked(db).await;
    let other_page = l.kit.notion.add_page("Another page", None);
    let victim = l.kit.notion.paragraph(other_page, "Not for this request");
    let other_stream = l
        .kit
        .link_notion(l.ws, &l.connection, Uuid::now_v7(), other_page)
        .await;
    let stream = StreamId::from_uuid(l.kit.notion_stream(l.notion_page).await);
    record_for(
        &l,
        stream,
        l.notion_page,
        Uuid::now_v7(),
        vec![("update", Uuid::now_v7(), Some(victim))],
    )
    .await;
    // Moved to the stream that reads the other page.
    sqlx::query("UPDATE connectors.notion_changes SET stream_id = $1")
        .bind(other_stream)
        .execute(&l.kit.db)
        .await
        .unwrap();
    l.kit.tick(TimeDelta::minutes(10)).await;
    assert!(l.kit.notion.patches().is_empty());
    assert_eq!(l.kit.notion.text(victim).as_deref(), Some("Not for this request"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_request_naming_another_workspaces_connection_writes_nothing(db: PgPool) {
    let l = linked(db).await;
    let ws_b = WorkspaceId::new();
    let rui = l.kit.user("rui");
    l.kit.join(&rui, ws_b, Role::Member);
    let mine = Uuid::now_v7();
    // Asked in workspace B, naming workspace A's connection and Notion page.
    l.kit
        .ask_notion_changes(
            ws_b,
            &l.connection,
            (l.page, l.notion_page),
            vec![update_item(mine, l.first, l.kit.notion.edited(l.first))],
            &paragraphs(&[(0, mine, "Across")]),
            true,
        )
        .await;
    assert!(l.kit.notion.patches().is_empty());
    assert_eq!(l.kit.notion.text(l.first).as_deref(), Some("Plan the launch"));
    assert!(states(&l).await.is_empty());
    // B is told the request cannot be made.
    let done = l
        .kit
        .payloads(ws_b, "connectors.notion_changes_completed")
        .await;
    assert_eq!(done[0]["results"][0]["reason"], "not_connected");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_is_never_removed_even_when_the_request_says_so(db: PgPool) {
    let l = linked(db).await;
    let sub = l.kit.notion.add_page("A sub-page", Some(l.notion_page));
    // Read again so the sub-page is known, then ask to delete it.
    l.kit.tick(TimeDelta::minutes(10)).await;
    l.kit.notion.forget_calls();
    l.kit
        .ask_notion_changes(
            l.ws,
            &l.connection,
            (l.page, l.notion_page),
            vec![delete_item(Uuid::now_v7(), sub, l.kit.notion.edited(sub))],
            &json!({"version": 1, "items": [{"seq": 0}]}),
            true,
        )
        .await;
    assert!(l.kit.notion.deleted().is_empty());
    assert!(
        l.kit.notion.calls().iter().all(|c| !c.starts_with("DELETE")),
        "{:?}",
        l.kit.notion.calls()
    );
    let done = completed(&l).await;
    assert_eq!(done[0]["results"][0]["outcome"], "skipped");
    assert_eq!(done[0]["results"][0]["reason"], "unsupported");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn blocks_added_here_land_after_the_ones_they_follow_and_a_delete_goes(db: PgPool) {
    let l = linked(db).await;
    let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
    let mut second = append_item(b, None);
    second.after_block = Some(a);
    l.kit
        .ask_notion_changes(
            l.ws,
            &l.connection,
            (l.page, l.notion_page),
            vec![
                append_item(a, Some(l.first)),
                second,
                delete_item(Uuid::now_v7(), l.second, l.kit.notion.edited(l.second)),
            ],
            &json!({"version": 1, "items": [
                {"seq": 0, "block": {"id": a, "kind": "paragraph", "runs": [{"text": "Added A"}]}},
                {"seq": 1, "block": {"id": b, "kind": "paragraph", "runs": [{"text": "Added B"}]}},
                {"seq": 2},
            ]}),
            true,
        )
        .await;
    let order: Vec<String> = l
        .kit
        .notion
        .children_of(l.notion_page)
        .into_iter()
        .map(|id| l.kit.notion.text(id).unwrap())
        .collect();
    assert_eq!(order, ["Plan the launch", "Added A", "Added B"]);
    let done = completed(&l).await;
    let results = done[0]["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r["outcome"] == "applied"));
    assert!(results[0]["notion_block"].is_string());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn throttling_leaves_the_change_pending_for_the_next_run(db: PgPool) {
    let l = linked(db).await;
    let mine = Uuid::now_v7();
    let base = l.kit.notion.edited(l.first);
    l.kit.notion.throttle_next(60);
    l.kit
        .ask_notion_changes(
            l.ws,
            &l.connection,
            (l.page, l.notion_page),
            vec![update_item(mine, l.first, base)],
            &paragraphs(&[(0, mine, "After the wait")]),
            true,
        )
        .await;
    assert_eq!(l.kit.notion.text(l.first).as_deref(), Some("Plan the launch"));
    assert!(completed(&l).await.is_empty());
    l.kit.tick(TimeDelta::minutes(10)).await;
    assert_eq!(l.kit.notion.text(l.first).as_deref(), Some("After the wait"));
    assert_eq!(completed(&l).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_who_must_connect_again_has_nothing_written(db: PgPool) {
    let l = linked(db).await;
    sqlx::query("UPDATE connectors.connections SET health = 'needs_reauth'")
        .execute(&l.kit.db)
        .await
        .unwrap();
    let mine = Uuid::now_v7();
    l.kit
        .ask_notion_changes(
            l.ws,
            &l.connection,
            (l.page, l.notion_page),
            vec![update_item(mine, l.first, l.kit.notion.edited(l.first))],
            &paragraphs(&[(0, mine, "Never")]),
            true,
        )
        .await;
    assert!(l.kit.notion.patches().is_empty());
    let done = completed(&l).await;
    assert_eq!(done[0]["results"][0]["reason"], "needs_reauth");
}
