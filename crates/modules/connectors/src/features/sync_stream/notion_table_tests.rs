//! A Notion database's stream, against the fake Notion: the first read, the
//! unchanged database, Notion's edits to columns and entries, what a run
//! writes (only what the pages module asked for and the stream's own MAC
//! verifies), the confirmation a column removal needs, and a database that
//! stops being shared (campbooks-cloud#377).

use campbooks_kernel::{
    blobs::BlobRef,
    clock::Clock as _,
    outbound::TableItemKind,
    tenancy::{Role, WorkspaceId},
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    testkit::{Kit, assert_contract},
    testkit_notion::{data_value, table_item},
};

struct Linked {
    kit: Kit,
    ws: WorkspaceId,
    connection: Value,
    table: Uuid,
    ds: Uuid,
    stage: String,
    stream: Uuid,
}

async fn linked(db: PgPool) -> Linked {
    let kit = Kit::with_notion_databases(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let ds = kit.notion.add_data_source("Clients");
    let stage = kit.notion.add_property(
        ds,
        "Stage",
        "select",
        &json!({"options": [{"name": "Lead", "color": "blue"}]}),
    );
    kit.notion
        .add_property(ds, "Secret plan", "rich_text", &json!({}));
    for (name, stage_name) in [("Acme", "Lead"), ("Globex", "Lead")] {
        kit.notion.add_entry(
            ds,
            name,
            &[(stage.as_str(), data_value("select", stage_name))],
        );
    }
    let table = Uuid::now_v7();
    let stream = kit.link_notion_table(ws, &connection, table, ds).await;
    Linked {
        kit,
        ws,
        connection,
        table,
        ds,
        stage,
        stream,
    }
}

async fn reads(l: &Linked) -> Vec<Value> {
    l.kit
        .payloads(l.ws, "connectors.notion_data_source_read")
        .await
}

async fn snapshot_of(l: &Linked, event: &Value) -> Value {
    let blob: BlobRef = serde_json::from_value(event["snapshot"].clone()).unwrap();
    serde_json::from_slice(&l.kit.blob(l.ws, &blob).await).unwrap()
}

async fn run_again(l: &Linked) {
    let due = l.kit.notion_due(l.stream).await.expect("scheduled");
    l.kit
        .tick((due - l.kit.clock.now()).max(TimeDelta::zero()) + TimeDelta::seconds(1))
        .await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn linking_a_database_reads_it_and_announces_the_snapshot_by_reference(db: PgPool) {
    let l = linked(db).await;
    let events = reads(&l).await;
    assert_eq!(events.len(), 1, "{events:?}");
    let event = &events[0];
    assert_contract("connectors.notion_data_source_read.v1.schema.json", event);
    assert_eq!(event["data_source"], l.ds.to_string());
    assert_eq!(event["table"], l.table.to_string());
    assert_eq!(event["connection"], l.connection["id"]);
    // The event carries the reference only; the names are in the blob.
    let text = event.to_string();
    assert!(!text.contains("Acme") && !text.contains("Secret plan") && !text.contains("Stage"));
    let snapshot = snapshot_of(&l, event).await;
    assert_contract("notion-data-source-snapshot.v1.schema.json", &snapshot);
    assert_eq!(snapshot["title"], "Clients");
    assert_eq!(snapshot["entries"].as_array().unwrap().len(), 2);
    let kinds: Vec<&str> = snapshot["schema"]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["title", "rich_text", "select"]);
    // The first read lists every entry: nothing else is known to be gone.
    assert_eq!(snapshot["sweep"], true);
    assert_eq!(snapshot["present"].as_array().unwrap().len(), 2);
    // Every value carries its hash.
    let cell = &snapshot["entries"][0]["cells"][0];
    assert_eq!(cell["hash"].as_str().unwrap().len(), 64);
    // No event, anywhere, carries a name or a value.
    let everything: Vec<String> = sqlx::query_scalar("SELECT payload::text FROM kernel.events")
        .fetch_all(&l.kit.db)
        .await
        .unwrap();
    assert!(
        everything
            .iter()
            .all(|p| !p.contains("Acme") && !p.contains("Secret plan"))
    );
    // Nothing was written at Notion.
    assert!(l.kit.notion.schema_patches().is_empty() && l.kit.notion.entry_sets().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unchanged_database_costs_two_calls_and_reads_no_entry_again(db: PgPool) {
    let l = linked(db).await;
    l.kit.notion.forget_calls();
    // Soon after: nothing to announce. (The wait here outlasts the sweep's
    // hour, so what is announced is the list of ids alone.)
    run_again(&l).await;
    let events = reads(&l).await;
    assert_eq!(events.len(), 2, "the sweep's list of ids");
    let again = snapshot_of(&l, &events[1]).await;
    assert!(
        again["entries"].as_array().unwrap().is_empty(),
        "no entry again"
    );
    assert!(again["schema"].is_null());
    assert_eq!(again["present"].as_array().unwrap().len(), 2);
    let calls: Vec<String> = l
        .kit
        .notion
        .calls()
        .into_iter()
        .filter(|c| !c.contains("/oauth/token"))
        .collect();
    assert_eq!(
        calls.len(),
        3,
        "the database, the entries since, and the sweep's ids: {calls:?}"
    );
    // Only what changed since the cursor is asked for in full; the ids are
    // asked as titles alone.
    let queries = l.kit.notion.queries();
    let since = &queries[queries.len() - 2];
    assert!(since.1["filter"]["last_edited_time"]["on_or_after"].is_string());
    assert!(
        queries
            .last()
            .unwrap()
            .2
            .contains("filter_properties=title")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn notions_edits_are_read_at_the_next_run_and_a_new_column_reads_every_entry(db: PgPool) {
    let l = linked(db).await;
    let acme = l.kit.notion.entries_of(l.ds)[0];
    l.kit
        .notion
        .edit_entry(acme, &l.stage, data_value("select", "Won"));
    run_again(&l).await;
    let events = reads(&l).await;
    assert_eq!(events.len(), 2);
    let snapshot = snapshot_of(&l, &events[1]).await;
    assert!(snapshot["schema"].is_null(), "the columns did not change");
    assert_eq!(snapshot["entries"].as_array().unwrap().len(), 1);
    // A column added in Notion: the columns are read, and every entry again.
    l.kit
        .notion
        .add_property(l.ds, "Region", "rich_text", &json!({}));
    run_again(&l).await;
    let events = reads(&l).await;
    assert_eq!(events.len(), 3);
    let snapshot = snapshot_of(&l, &events[2]).await;
    assert_eq!(
        snapshot["schema"]["properties"].as_array().unwrap().len(),
        4
    );
    assert_eq!(snapshot["entries"].as_array().unwrap().len(), 2);
    // Nothing was written at Notion.
    assert!(l.kit.notion.schema_patches().is_empty() && l.kit.notion.entry_sets().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_entry_trashed_in_notion_is_absent_from_the_next_sweep(db: PgPool) {
    let l = linked(db).await;
    let acme = l.kit.notion.entries_of(l.ds)[0];
    l.kit.notion.trash_data_entry(acme);
    // The webhook asks for a look at once.
    let body = serde_json::to_vec(&json!({
        "type": "data_source.content_updated", "api_version": "2026-03-11",
        "entity": {"id": l.ds, "type": "data_source"},
    }))
    .unwrap();
    let sent = l
        .kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(sent.status.as_u16(), 202, "{:?}", sent.json);
    l.kit.run_jobs().await;
    let events = reads(&l).await;
    let snapshot = snapshot_of(&l, events.last().unwrap()).await;
    assert_eq!(snapshot["sweep"], true);
    assert_eq!(snapshot["present"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn data_source_events_make_only_that_stream_due_and_only_while_databases_are_on(db: PgPool) {
    let l = linked(db).await;
    let other = l.kit.notion.add_data_source("Elsewhere");
    let body = serde_json::to_vec(&json!({
        "type": "data_source.schema_updated", "api_version": "2026-03-11",
        "entity": {"id": other, "type": "data_source"},
    }))
    .unwrap();
    let sent = l
        .kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(sent.status.as_u16(), 202);
    let reads_before = reads(&l).await.len();
    l.kit.run_jobs().await;
    assert_eq!(
        reads(&l).await.len(),
        reads_before,
        "nobody reads that database"
    );
    assert!(l.kit.notion.queries().iter().all(|(ds, _, _)| *ds == l.ds));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn with_databases_off_data_source_events_and_links_do_nothing(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let ds = kit.notion.add_data_source("Clients");
    let table = Uuid::now_v7();
    kit.pages.link_table(
        table,
        connection["id"].as_str().unwrap().parse().unwrap(),
        ds,
    );
    let mut tx = campbooks_kernel::db::BeginFor::begin_for(&kit.state.db, ws)
        .await
        .unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &crate::events::NotionLinkRequested {
                workspace: ws.to_string(),
                page: table,
                connection: connection["id"].as_str().unwrap().parse().unwrap(),
                notion_page: ds,
                kind: crate::events::LinkKind::DataSource,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    assert!(
        kit.notion_stream_opt(ds).await.is_none(),
        "no stream of a database"
    );
    let body = serde_json::to_vec(&json!({
        "type": "data_source.content_updated", "api_version": "2026-03-11",
        "entity": {"id": ds, "type": "data_source"},
    }))
    .unwrap();
    let sent = kit
        .notion_hook(&body, Some(&Kit::notion_signature(&body)))
        .await;
    assert_eq!(sent.status.as_u16(), 200);
    assert!(
        kit.notion
            .calls()
            .iter()
            .all(|c| !c.contains("data_sources"))
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_link_the_pages_module_does_not_confirm_makes_no_stream(db: PgPool) {
    let kit = Kit::with_notion_databases(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let ds = kit.notion.add_data_source("Clients");
    let mut tx = campbooks_kernel::db::BeginFor::begin_for(&kit.state.db, ws)
        .await
        .unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &crate::events::NotionLinkRequested {
                workspace: ws.to_string(),
                page: Uuid::now_v7(),
                connection: connection["id"].as_str().unwrap().parse().unwrap(),
                notion_page: ds,
                kind: crate::events::LinkKind::DataSource,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    assert!(kit.notion_stream_opt(ds).await.is_none());
    assert!(
        kit.notion
            .calls()
            .iter()
            .all(|c| !c.contains("data_sources"))
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_database_that_is_no_longer_shared_or_is_trashed_ends_the_link_and_deletes_nothing(
    db: PgPool,
) {
    let l = linked(db).await;
    l.kit.notion.unshare_data_source(l.ds);
    run_again(&l).await;
    let stopped = l.kit.payloads(l.ws, "connectors.notion_link_stopped").await;
    assert_eq!(stopped.len(), 1);
    assert_eq!(stopped[0]["reason"], "not_shared");
    assert_eq!(stopped[0]["page"], l.table.to_string());
    assert!(l.kit.notion_stream_opt(l.ds).await.is_none());
    assert!(l.kit.notion.data_source_live(l.ds));
    assert!(l.kit.notion.entries_trashed_by_client().is_empty());
}

// --- What is written ---------------------------------------------------------

fn doc(items: &[Value]) -> Value {
    json!({"version": 1, "items": items})
}

async fn ended(l: &Linked, request: Uuid) -> Vec<Value> {
    let events = l
        .kit
        .payloads(l.ws, "connectors.notion_table_changes_completed")
        .await;
    let event = events
        .into_iter()
        .find(|e| e["request"] == request.to_string())
        .expect("the request ended");
    assert_contract(
        "connectors.notion_table_changes_completed.v1.schema.json",
        &event,
    );
    event["results"].as_array().unwrap().clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_column_is_added_and_the_connector_reports_its_id_and_hash(db: PgPool) {
    let l = linked(db).await;
    let column = Uuid::now_v7();
    let request = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![table_item(0, TableItemKind::SchemaAdd, column)],
            &doc(&[json!({"seq": 0, "name": "Budget", "kind": "number", "format": "euro"})]),
            true,
        )
        .await;
    let results = ended(&l, request).await;
    assert_eq!(results[0]["outcome"], "applied");
    assert_eq!(results[0]["kind"], "schema_add");
    assert!(results[0]["external"].is_string());
    assert_eq!(results[0]["hash"].as_str().unwrap().len(), 64);
    let patches = l.kit.notion.schema_patches();
    assert_eq!(patches.len(), 1);
    assert_eq!(patches[0].1["Budget"]["number"]["format"], "euro");
    assert!(
        l.kit
            .notion
            .property_names(l.ds)
            .contains(&"Budget".to_owned())
    );
    // A column Notion has of that name is Notion's: nothing is written.
    let again = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![table_item(0, TableItemKind::SchemaAdd, Uuid::now_v7())],
            &doc(&[json!({"seq": 0, "name": "budget", "kind": "number"})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, again).await[0]["reason"], "changed_in_notion");
    // A kind the API cannot make is not tried.
    let status = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![table_item(0, TableItemKind::SchemaAdd, Uuid::now_v7())],
            &doc(&[json!({"seq": 0, "name": "Phase", "kind": "status"})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, status).await[0]["reason"], "unsupported");
    assert_eq!(l.kit.notion.schema_patches().len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_column_is_changed_only_as_the_last_sync_saw_it_and_options_are_only_added(db: PgPool) {
    let l = linked(db).await;
    let events = reads(&l).await;
    let snapshot = snapshot_of(&l, &events[0]).await;
    let stage = snapshot["schema"]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == l.stage.as_str())
        .unwrap()
        .clone();
    let mut item = table_item(0, TableItemKind::SchemaUpdate, Uuid::now_v7());
    item.external_property = Some(l.stage.clone());
    item.base_hash = Some(stage["hash"].as_str().unwrap().to_owned());
    item.schema_hash = Some(snapshot["schema"]["hash"].as_str().unwrap().to_owned());
    let request = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![item.clone()],
            &doc(&[json!({"seq": 0, "name": "Phase", "kind": "select",
                "options": [{"name": "Lead", "color": "red"}, {"name": "Won", "color": "green"}]})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, request).await[0]["outcome"], "applied");
    let sent = &l.kit.notion.schema_patches()[0].1[l.stage.as_str()];
    assert_eq!(sent["name"], "Phase");
    let options = sent["select"]["options"].as_array().unwrap();
    // The one Notion has stays by id (never renamed or recoloured from
    // here); the new one is added.
    assert!(options[0].get("id").is_some() && options[0].get("name").is_none());
    assert_eq!(options[1]["name"], "Won");
    // Notion renamed the column meanwhile: the same change is held back.
    l.kit.notion.rename_property(l.ds, &l.stage, "Pipeline");
    let held = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![item],
            &doc(&[json!({"seq": 0, "name": "Status", "kind": "select"})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, held).await[0]["reason"], "changed_in_notion");
    assert_eq!(l.kit.notion.schema_patches().len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_column_is_removed_only_with_the_members_confirmation(db: PgPool) {
    let l = linked(db).await;
    let events = reads(&l).await;
    let snapshot = snapshot_of(&l, &events[0]).await;
    let secret = snapshot["schema"]["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Secret plan")
        .unwrap()
        .clone();
    let mut item = table_item(0, TableItemKind::SchemaRemove, Uuid::now_v7());
    item.external_property = Some(secret["id"].as_str().unwrap().to_owned());
    item.base_hash = Some(secret["hash"].as_str().unwrap().to_owned());
    // Without the confirmation nothing is sent.
    let refused = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![item.clone()],
            &doc(&[json!({"seq": 0})]),
            true,
        )
        .await;
    let results = ended(&l, refused).await;
    assert_eq!(results[0]["outcome"], "skipped");
    assert_eq!(results[0]["reason"], "confirmation_required");
    assert!(l.kit.notion.schema_patches().is_empty());
    assert!(
        l.kit
            .notion
            .property_names(l.ds)
            .contains(&"Secret plan".to_owned())
    );
    // A confirmed change of a column Notion changed since is kept.
    l.kit
        .notion
        .rename_property(l.ds, secret["id"].as_str().unwrap(), "Plans");
    let mut confirmed = item.clone();
    confirmed.confirmed = true;
    let kept = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![confirmed.clone()],
            &doc(&[json!({"seq": 0})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, kept).await[0]["reason"], "changed_in_notion");
    assert!(l.kit.notion.schema_patches().is_empty());
    // Confirmed, and as the last sync saw it: removed.
    l.kit
        .notion
        .rename_property(l.ds, secret["id"].as_str().unwrap(), "Secret plan");
    let gone = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![confirmed],
            &doc(&[json!({"seq": 0})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, gone).await[0]["outcome"], "applied");
    assert!(
        !l.kit
            .notion
            .property_names(l.ds)
            .contains(&"Secret plan".to_owned())
    );
    // The database itself is never touched.
    assert!(l.kit.notion.data_source_live(l.ds));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_title_column_is_never_removed(db: PgPool) {
    let l = linked(db).await;
    let events = reads(&l).await;
    let snapshot = snapshot_of(&l, &events[0]).await;
    let title = snapshot["schema"]["properties"][0].clone();
    let mut item = table_item(0, TableItemKind::SchemaRemove, Uuid::now_v7());
    item.external_property = Some("title".to_owned());
    item.base_hash = Some(title["hash"].as_str().unwrap().to_owned());
    item.confirmed = true;
    let request = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![item],
            &doc(&[json!({"seq": 0})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, request).await[0]["reason"], "unsupported");
    assert!(l.kit.notion.schema_patches().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn rows_and_cells_are_written_checked_and_trashed_only_in_their_own_database(db: PgPool) {
    let l = linked(db).await;
    let events = reads(&l).await;
    let snapshot = snapshot_of(&l, &events[0]).await;
    let acme = &snapshot["entries"][0];
    let page: Uuid = acme["id"].as_str().unwrap().parse().unwrap();
    let cell = acme["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["property"] == l.stage.as_str())
        .unwrap();
    // A row is made, with the cells that can be written.
    let row = Uuid::now_v7();
    let made = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![table_item(0, TableItemKind::RowCreate, row)],
            &doc(&[json!({"seq": 0, "cells": [
                {"property": "title", "kind": "title", "value": [{"text": "Initech"}]},
                {"property": l.stage, "kind": "select", "value": {"name": "Lead"}},
                {"property": "nope", "kind": "formula", "value": "x"},
            ]})]),
            true,
        )
        .await;
    let results = ended(&l, made).await;
    assert_eq!(results[0]["outcome"], "applied");
    assert!(results[0]["external"].is_string());
    assert_eq!(l.kit.notion.entries_created().len(), 1);
    assert_eq!(l.kit.notion.entries_of(l.ds).len(), 3);
    // A cell is set against what the last sync saw.
    let mut set = table_item(0, TableItemKind::CellUpdate, Uuid::now_v7());
    set.property = Some(Uuid::now_v7());
    set.external_property = Some(l.stage.clone());
    set.external_page = Some(page);
    set.base_hash = Some(cell["hash"].as_str().unwrap().to_owned());
    let done = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![set.clone()],
            &doc(&[json!({"seq": 0, "kind": "select", "value": {"name": "Won"}})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, done).await[0]["outcome"], "applied");
    // The same change again is against a cell Notion holds differently now.
    let stale = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![set],
            &doc(&[json!({"seq": 0, "kind": "select", "value": {"name": "Lead"}})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, stale).await[0]["reason"], "changed_in_notion");
    assert_eq!(l.kit.notion.entry_sets().len(), 1);
    // Trashing: only a row of this database that Notion has not edited since.
    let mut trash = table_item(0, TableItemKind::RowTrash, Uuid::now_v7());
    trash.external_page = Some(page);
    trash.base_edited_at = l
        .kit
        .notion
        .entry_edited(page)
        .map(|t| t - TimeDelta::minutes(5));
    let late = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![trash.clone()],
            &doc(&[json!({"seq": 0})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, late).await[0]["reason"], "changed_in_notion");
    trash.base_edited_at = l.kit.notion.entry_edited(page);
    let trashed = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![trash],
            &doc(&[json!({"seq": 0})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, trashed).await[0]["outcome"], "applied");
    assert!(l.kit.notion.entry_trashed(page));
    // A page of another database is never trashed from here.
    let other = l.kit.notion.add_data_source("Elsewhere");
    let stranger = l.kit.notion.add_entry(other, "Stranger", &[]);
    let mut bad = table_item(0, TableItemKind::RowTrash, Uuid::now_v7());
    bad.external_page = Some(stranger);
    bad.base_edited_at = l.kit.notion.entry_edited(stranger);
    let refused = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![bad],
            &doc(&[json!({"seq": 0})]),
            true,
        )
        .await;
    assert_eq!(ended(&l, refused).await[0]["outcome"], "skipped");
    assert!(!l.kit.notion.entry_trashed(stranger));
    assert!(l.kit.notion.data_source_live(l.ds));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_request_the_pages_module_does_not_stand_behind_is_not_made(db: PgPool) {
    let l = linked(db).await;
    let request = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![table_item(0, TableItemKind::SchemaAdd, Uuid::now_v7())],
            &doc(&[json!({"seq": 0, "name": "Forged", "kind": "number"})]),
            false,
        )
        .await;
    let recorded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connectors.notion_table_changes WHERE request_id = $1",
    )
    .bind(request)
    .fetch_one(&l.kit.db)
    .await
    .unwrap();
    assert_eq!(recorded, 0);
    assert!(l.kit.notion.schema_patches().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_recorded_change_that_was_tampered_with_is_never_made(db: PgPool) {
    let l = linked(db).await;
    // Notion is busy when the stream runs: the change stays recorded.
    l.kit.notion.throttle_next(1);
    let request = l
        .kit
        .ask_notion_table_changes(
            l.ws,
            &l.connection,
            (l.table, l.ds),
            vec![table_item(0, TableItemKind::SchemaAdd, Uuid::now_v7())],
            &doc(&[json!({"seq": 0, "name": "Budget", "kind": "number"})]),
            true,
        )
        .await;
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connectors.notion_table_changes WHERE request_id = $1 AND state = 'pending'",
    )
    .bind(request)
    .fetch_one(&l.kit.db)
    .await
    .unwrap();
    assert_eq!(pending, 1, "recorded, waiting for the next run");
    // Someone with SQL turns it into a removal the member confirmed.
    sqlx::query(
        "UPDATE connectors.notion_table_changes
            SET kind = 'schema_remove', confirmed = true, external_property = $2
          WHERE request_id = $1",
    )
    .bind(request)
    .bind(&l.stage)
    .execute(&l.kit.db)
    .await
    .unwrap();
    run_again(&l).await;
    let (state, reason): (String, Option<String>) = sqlx::query_as(
        "SELECT state, reason FROM connectors.notion_table_changes WHERE request_id = $1",
    )
    .bind(request)
    .fetch_one(&l.kit.db)
    .await
    .unwrap();
    assert_eq!(
        (state.as_str(), reason.as_deref()),
        ("skipped", Some("unverified"))
    );
    assert!(l.kit.notion.schema_patches().is_empty());
    assert!(
        l.kit
            .notion
            .property_names(l.ds)
            .contains(&"Stage".to_owned())
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn no_log_or_error_of_a_run_carries_a_name_or_a_value(db: PgPool) {
    let l = linked(db).await;
    let failing: Vec<String> =
        sqlx::query_scalar("SELECT coalesce(last_error, '') FROM kernel.jobs")
            .fetch_all(&l.kit.db)
            .await
            .unwrap();
    assert!(
        failing
            .iter()
            .all(|e| !e.contains("Acme") && !e.contains("Secret plan"))
    );
    let notes: Vec<String> = sqlx::query_scalar("SELECT payload::text FROM kernel.jobs")
        .fetch_all(&l.kit.db)
        .await
        .unwrap();
    assert!(
        notes
            .iter()
            .all(|p| !p.contains("Acme") && !p.contains("Secret plan"))
    );
}
