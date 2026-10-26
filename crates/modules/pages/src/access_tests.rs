//! Who may do what with a page: every row of the access model, asked of the
//! central check with the module's real facts, and the module's answers to
//! the change feed and to search.

use campbooks_kernel::{
    RecordRef,
    authz::{self, Action, Decision},
    changes::Visibility as _,
    search::SourceContext,
    tenancy::{Access, Role, UserId, WorkspaceId},
};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    domain::resources::{PAGE_RECORD, PageVisibility},
    testkit::{Kit, docs},
};

fn access(workspace: WorkspaceId, user: Option<UserId>, role: Option<Role>) -> Access {
    Access {
        workspace,
        user,
        role,
    }
}

async fn ask(kit: &Kit, access: &Access, action: Action, page: &str) -> Decision {
    let mut tx = kit.begin(access.workspace).await;
    let decision = authz::authorize(
        &kit.state,
        &mut tx,
        access,
        action,
        &RecordRef::new(PAGE_RECORD, page),
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    decision
}

async fn row(kit: &Kit, access: &Access, page: &str) -> [Decision; 5] {
    [
        ask(kit, access, Action::Read, page).await,
        ask(kit, access, Action::Write, page).await,
        ask(kit, access, Action::Create, page).await,
        ask(kit, access, Action::Share, page).await,
        ask(kit, access, Action::Delete, page).await,
    ]
}

use Decision::{Allow, Forbidden, NotFound};

fn context<'a>(
    kit: &'a Kit,
    conn: &'a mut sqlx::PgConnection,
    workspace: WorkspaceId,
) -> SourceContext<'a> {
    SourceContext {
        conn,
        blobs: &kit.state.blobs,
        workspace,
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_private_page_is_its_creators_and_whoever_it_is_shared_with(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let (_, wes_id) = kit.owner("wes", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Notes"})).await;
    let id = page["id"].as_str().unwrap();
    let ana_access = access(workspace, Some(ana_id), Some(Role::Member));
    let rui_access = access(workspace, Some(rui_id), Some(Role::Member));
    let wes_access = access(workspace, Some(wes_id), Some(Role::Owner));
    let client = access(workspace, None, None);

    // Order: read, write, create, share, delete.
    assert_eq!(row(&kit, &ana_access, id).await, [Allow; 5]);
    for nobody in [&rui_access, &wes_access, &client] {
        assert_eq!(row(&kit, nobody, id).await, [NotFound; 5]);
    }

    let share = |role: &'static str| {
        let (kit, ana) = (&kit, ana.clone());
        async move {
            let granted = kit
                .put(
                    &ana,
                    &format!("{}/{id}/grants/{rui_id}", Kit::pages(workspace)),
                    json!({"role": role}),
                )
                .await;
            assert_eq!(
                granted.status,
                axum::http::StatusCode::OK,
                "{:?}",
                granted.json
            );
        }
    };
    share("reader").await;
    assert_eq!(
        row(&kit, &rui_access, id).await,
        [Allow, Forbidden, Forbidden, Forbidden, Forbidden]
    );
    share("writer").await;
    assert_eq!(
        row(&kit, &rui_access, id).await,
        [Allow, Allow, Allow, Forbidden, Forbidden]
    );
    share("manager").await;
    assert_eq!(row(&kit, &rui_access, id).await, [Allow; 5]);
    // The workspace's owner still has nothing here.
    assert_eq!(row(&kit, &wes_access, id).await, [NotFound; 5]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_workspace_page_is_every_members_and_shared_by_its_creator_and_the_owner(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let other = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let (_, wes_id) = kit.owner("wes", workspace);
    let (_, bo_id) = kit.owner("bo", other);
    let page = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Handbook", "place": "workspace"}),
        )
        .await;
    let id = page["id"].as_str().unwrap();
    assert_eq!(
        row(
            &kit,
            &access(workspace, Some(ana_id), Some(Role::Member)),
            id
        )
        .await,
        [Allow; 5]
    );
    assert_eq!(
        row(
            &kit,
            &access(workspace, Some(rui_id), Some(Role::Member)),
            id
        )
        .await,
        [Allow, Allow, Allow, Forbidden, Forbidden]
    );
    assert_eq!(
        row(
            &kit,
            &access(workspace, Some(wes_id), Some(Role::Owner)),
            id
        )
        .await,
        [Allow; 5]
    );
    assert_eq!(
        row(&kit, &access(workspace, None, None), id).await,
        [NotFound; 5]
    );
    // From another workspace the page is not there, owner or not.
    let mut tx = kit.begin(other).await;
    let elsewhere = authz::authorize(
        &kit.state,
        &mut tx,
        &access(other, Some(bo_id), Some(Role::Owner)),
        Action::Read,
        &RecordRef::new(PAGE_RECORD, id),
    )
    .await
    .unwrap();
    assert_eq!(elsewhere, NotFound);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_nearest_grant_and_the_creator_of_a_page_above_decide_in_a_tree(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let top = kit.add(&ana, workspace, json!({"title": "Top"})).await;
    let mid = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Mid", "parent": top["id"]}),
        )
        .await;
    let leaf = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Leaf", "parent": mid["id"]}),
        )
        .await;
    let (top, mid, leaf) = (
        top["id"].as_str().unwrap(),
        mid["id"].as_str().unwrap(),
        leaf["id"].as_str().unwrap(),
    );
    let uri = Kit::pages(workspace);
    for (page, role) in [(top, "writer"), (mid, "reader")] {
        kit.put(
            &ana,
            &format!("{uri}/{page}/grants/{rui_id}"),
            json!({"role": role}),
        )
        .await;
    }
    let rui = access(workspace, Some(rui_id), Some(Role::Member));
    // Top: writer. Mid and below: the nearer grant, reader.
    assert_eq!(row(&kit, &rui, top).await[1], Allow);
    assert_eq!(row(&kit, &rui, mid).await[1], Forbidden);
    assert_eq!(row(&kit, &rui, leaf).await[1], Forbidden);
    assert_eq!(row(&kit, &rui, leaf).await[0], Allow);
    // Ana made the top page, so she holds every page under it.
    let ana = access(workspace, Some(ana_id), Some(Role::Member));
    assert_eq!(row(&kit, &ana, leaf).await, [Allow; 5]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_feed_and_search_see_the_same_pages_the_catalogue_does(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let private = kit
        .add(&ana, workspace, json!({"title": "Private plan"}))
        .await;
    let shared = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Shared plan", "place": "workspace"}),
        )
        .await;
    let private_id = private["id"].as_str().unwrap();
    let shared_id = shared["id"].as_str().unwrap();
    let ana_access = access(workspace, Some(ana_id), Some(Role::Member));
    let rui_access = access(workspace, Some(rui_id), Some(Role::Member));
    let port = PageVisibility;

    let ids = |containers: Vec<RecordRef>| {
        let mut ids: Vec<String> = containers.into_iter().map(|c| c.id).collect();
        ids.sort();
        ids
    };
    let mut expected_ana = vec![private_id.to_owned(), shared_id.to_owned()];
    expected_ana.sort();
    assert_eq!(
        ids(port
            .visible_containers(&kit.state.db, &ana_access, PAGE_RECORD)
            .await
            .unwrap()),
        expected_ana
    );
    assert_eq!(
        ids(port
            .visible_containers(&kit.state.db, &rui_access, PAGE_RECORD)
            .await
            .unwrap()),
        [shared_id]
    );
    for (who, page, seen) in [
        (&ana_access, private_id, true),
        (&rui_access, private_id, false),
        (&rui_access, shared_id, true),
    ] {
        let visible = port
            .container_visible(&kit.state.db, who, &RecordRef::new(PAGE_RECORD, page))
            .await
            .unwrap();
        assert_eq!(visible, seen);
    }
    // A client token acting for nobody sees none.
    let nobody = access(workspace, None, None);
    assert!(
        port.visible_containers(&kit.state.db, &nobody, PAGE_RECORD)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn search_reads_a_pages_title_and_text_and_a_trashed_page_leaves_the_index(db: PgPool) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let doc = docs::client(1);
    let body = docs::paragraph(&doc, "Quarterly numbers");
    let page = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Finance", "document": STANDARD.encode(&body)}),
        )
        .await;
    let id = page["id"].as_str().unwrap().to_owned();
    let source = kit.state.search_sources.get(PAGE_RECORD).unwrap().clone();

    let mut tx = kit.begin(workspace).await;
    let group = source
        .group(context(&kit, &mut tx, workspace), &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group.title, "Finance");
    assert_eq!(group.container, RecordRef::new(PAGE_RECORD, &id));
    assert_eq!(group.entries.len(), 1);
    let texts = source
        .texts(
            context(&kit, &mut tx, workspace),
            std::slice::from_ref(&group.entries[0].record),
        )
        .await
        .unwrap();
    assert_eq!(texts[0].text, "Quarterly numbers\n");
    let listed = source
        .page(context(&kit, &mut tx, workspace), None, 10)
        .await
        .unwrap();
    assert_eq!(listed.groups, std::slice::from_ref(&id));
    assert_eq!(listed.next, None);
    tx.rollback().await.unwrap();

    // In the trash it is no group at all; restored, it is one again.
    let uri = format!("{}/{id}", Kit::pages(workspace));
    kit.post(&ana, &format!("{uri}/trash"), None).await;
    let mut tx = kit.begin(workspace).await;
    assert!(
        source
            .group(context(&kit, &mut tx, workspace), &id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        source
            .page(context(&kit, &mut tx, workspace), None, 10)
            .await
            .unwrap()
            .groups
            .is_empty()
    );
    tx.rollback().await.unwrap();
    kit.post(&ana, &format!("{uri}/restore"), None).await;
    let mut tx = kit.begin(workspace).await;
    assert!(
        source
            .group(context(&kit, &mut tx, workspace), &id)
            .await
            .unwrap()
            .is_some()
    );
}

// --- Tables: search, containers, retention ---

#[sqlx::test(migrations = "../../../migrations")]
async fn search_reads_a_rows_title_and_text_values_and_only_its_tables_readers_find_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let table = kit
        .post(
            &ana,
            &Kit::tables(workspace),
            Some(json!({"title": "Clients"})),
        )
        .await
        .json;
    let t = table["page"]["id"].as_str().unwrap().to_owned();
    let notes = kit
        .property(
            &ana,
            workspace,
            &t,
            json!({"name": "Notes", "type": "text"}),
        )
        .await;
    let number = kit
        .property(
            &ana,
            workspace,
            &t,
            json!({"name": "Value", "type": "number"}),
        )
        .await;
    let row = kit
        .row(
            &ana,
            workspace,
            &t,
            json!({"title": "Northwind", "cells": [
                {"property": notes["property"]["id"], "value": "prefers invoices on Fridays"},
                {"property": number["property"]["id"], "value": "4242"}]}),
        )
        .await;
    let id = row["id"].as_str().unwrap().to_owned();
    let source = kit.state.search_sources.get(PAGE_RECORD).unwrap().clone();
    let mut tx = kit.begin(workspace).await;
    let group = source
        .group(context(&kit, &mut tx, workspace), &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group.title, "Northwind");
    // A row lives in its table's container, so a table of thousands of rows is
    // one thing to the index and to the feed.
    assert_eq!(group.container, RecordRef::new(PAGE_RECORD, &t));
    let texts = source
        .texts(
            context(&kit, &mut tx, workspace),
            std::slice::from_ref(&group.entries[0].record),
        )
        .await
        .unwrap();
    assert!(texts[0].text.contains("prefers invoices on Fridays"));
    assert!(
        !texts[0].text.contains("4242"),
        "only text values are searched"
    );
    // A cell edit moves the entry's stamp, so the index reads it again.
    let before = group.entries[0].stamp.clone();
    tx.rollback().await.unwrap();
    kit.cells(
        &ana,
        workspace,
        &t,
        &id,
        json!([{"property": notes["property"]["id"], "value": "new"}]),
    )
    .await;
    let mut tx = kit.begin(workspace).await;
    let after = source
        .group(context(&kit, &mut tx, workspace), &id)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(after.entries[0].stamp, before);
    tx.rollback().await.unwrap();

    // The table is private: Rui cannot see its container, and the row is not
    // among the containers listed for him; shared, he can.
    let rui = access(workspace, Some(rui_id), Some(Role::Member));
    let port = PageVisibility;
    let container = RecordRef::new(PAGE_RECORD, &t);
    assert!(
        !port
            .container_visible(&kit.state.db, &rui, &container)
            .await
            .unwrap()
    );
    let listed = port
        .visible_containers(&kit.state.db, &rui, PAGE_RECORD)
        .await
        .unwrap();
    assert!(listed.is_empty());
    kit.share(&ana, workspace, &t, rui_id, "reader").await;
    assert!(
        port.container_visible(&kit.state.db, &rui, &container)
            .await
            .unwrap()
    );
    let listed = port
        .visible_containers(&kit.state.db, &rui, PAGE_RECORD)
        .await
        .unwrap();
    assert_eq!(listed, [container], "the table, not its 20,000 rows");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_rows_change_is_announced_in_its_tables_container(db: PgPool) {
    use campbooks_kernel::DomainEvent as _;
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let t = kit.table(&ana, workspace).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let row = kit.row(&ana, workspace, &t, json!({"title": "A"})).await;
    let events = kit.events("pages.page_changed").await;
    let row_event: crate::events::PageChanged = events
        .iter()
        .map(|event| serde_json::from_value(event.clone()).unwrap())
        .find(|event: &crate::events::PageChanged| {
            event.page.to_string() == row["id"].as_str().unwrap()
        })
        .unwrap();
    assert!(row_event.row);
    let change = &row_event.changes()[0];
    assert_eq!(change.container, Some(RecordRef::new(PAGE_RECORD, &t)));
    assert_eq!(change.record.id, row_event.page.to_string());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn deleting_a_row_for_good_leaves_the_relation_cells_that_held_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let a = kit.table(&ana, workspace).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let b = kit.table(&ana, workspace).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let rel = kit
        .property(
            &ana,
            workspace,
            &a,
            json!({"name": "Link", "type": "relation", "config": {"table": b}}),
        )
        .await["property"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let keep = kit.row(&ana, workspace, &b, json!({"title": "keep"})).await;
    let gone = kit.row(&ana, workspace, &b, json!({"title": "gone"})).await;
    let holder = kit
        .row(
            &ana,
            workspace,
            &a,
            json!({"cells": [{"property": rel, "value": [keep["id"], gone["id"]]}]}),
        )
        .await;
    let gone_id = gone["id"].as_str().unwrap();
    let uri = format!("{}/{gone_id}", Kit::pages(workspace));
    kit.post(&ana, &format!("{uri}/trash"), None).await;
    // Trashed rows stay in the cell (restoring brings them back); purged ones go.
    assert_eq!(
        kit.delete(&ana, &uri).await.status,
        axum::http::StatusCode::NO_CONTENT
    );
    let q = kit.query(&ana, workspace, &a, json!({})).await;
    assert_eq!(
        q.json["rows"][0]["cells"][&rel]["value"],
        json!([keep["id"]])
    );
    assert_eq!(q.json["rows"][0]["id"], holder["id"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn what_a_table_keeps_for_an_undo_goes_after_thirty_days(db: PgPool) {
    use campbooks_kernel::{jobs, jobs::EnqueueOptions};
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let t = kit.table(&ana, workspace).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let removed = kit
        .property(&ana, workspace, &t, json!({"name": "Old", "type": "text"}))
        .await["property"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let converted = kit
        .property(&ana, workspace, &t, json!({"name": "Conv", "type": "text"}))
        .await["property"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    kit.row(&ana, workspace, &t, json!({"cells": [{"property": removed, "value": "x"}, {"property": converted, "value": "12"}]})).await;
    let base = format!("{}/{t}/properties", Kit::tables(workspace));
    kit.delete(&ana, &format!("{base}/{removed}")).await;
    kit.patch(
        &ana,
        &format!("{base}/{converted}"),
        json!({"type": "number"}),
    )
    .await;
    sqlx::query("UPDATE pages.table_properties SET removed_at = now() - interval '31 days' WHERE removed_at IS NOT NULL")
        .execute(&kit.db).await.unwrap();
    sqlx::query("UPDATE pages.table_properties SET converted_at = now() - interval '31 days' WHERE converted_at IS NOT NULL")
        .execute(&kit.db).await.unwrap();
    // The daily dispatcher finds the workspace through the maintenance scope.
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(
        &mut tx,
        &crate::features::purge_trash::DispatchPurgeTrash,
        EnqueueOptions::default(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pages.table_properties WHERE id = $1::uuid")
            .bind(&removed)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(left, 0, "the removed property and its cells are gone");
    let cells: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pages.row_values WHERE property_id = $1::uuid")
            .bind(&removed)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(cells, 0);
    let backup: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.table_properties WHERE id = $1::uuid AND previous_type IS NOT NULL")
        .bind(&converted).fetch_one(&kit.db).await.unwrap();
    assert_eq!(backup, 0, "the old type is no longer kept");
    let nothing = kit
        .post(&ana, &format!("{base}/{converted}/restore"), None)
        .await;
    assert_eq!(nothing.json["code"], "nothing_to_restore");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_deleted_workspace_loses_its_tables_and_cells(db: PgPool) {
    use crate::events::WorkspaceDeleting;
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let t = kit.table(&ana, workspace).await["page"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let p = kit
        .property(&ana, workspace, &t, json!({"name": "N", "type": "number"}))
        .await["property"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    kit.row(
        &ana,
        workspace,
        &t,
        json!({"cells": [{"property": p, "value": "1"}]}),
    )
    .await;
    let mut tx = kit.begin(workspace).await;
    kit.state
        .events
        .publish(
            &mut tx,
            &WorkspaceDeleting {
                workspace: workspace.to_string(),
                purge_after: chrono::Utc::now() - chrono::TimeDelta::hours(1),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    for table in [
        "pages",
        "tables",
        "table_properties",
        "table_rows",
        "row_values",
        "table_views",
    ] {
        let rows: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM pages.{table} WHERE workspace_id = $1"
        )))
        .bind(workspace.as_uuid())
        .fetch_one(&kit.db)
        .await
        .unwrap();
        assert_eq!(rows, 0, "{table}");
    }
}
