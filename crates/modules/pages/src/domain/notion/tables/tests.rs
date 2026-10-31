//! A Notion database kept in step with a table: the import, the first read,
//! both ways, the conflicts, the removal of a column, and what the
//! connectors module is told.

use campbooks_kernel::testing;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    testkit::Kit,
    testkit_notion::Rig,
    testkit_notion_tables::{actions, at, entry, id, prop, schema, snapshot, text, with_options},
};

const DS: u128 = 0xD5;

fn ds() -> Uuid {
    id(DS)
}

/// The seven columns of "Clients".
fn clients_schema() -> crate::domain::notion::tables::model::NSchema {
    schema(vec![
        prop("title", "Name", "title"),
        with_options("p1", "Stage", "select", &[("a", "Lead"), ("b", "Won")]),
        prop("p2", "Value", "number"),
        prop("p3", "Region", "rich_text"),
        prop("p4", "Active", "checkbox"),
        prop("p5", "Site", "url"),
        prop("p6", "Total", "formula"),
    ])
}

fn stage(name: &str) -> Value {
    json!({"id": if name == "Lead" { "a" } else { "b" }, "name": name})
}

/// The entries of "Clients": `n` rows named "Client 1" …
fn clients(n: u128) -> Vec<crate::domain::notion::tables::model::NEntry> {
    (1..=n)
        .map(|i| {
            entry(
                0x1000 + i,
                i64::try_from(i).unwrap(),
                &[
                    ("title", text(&format!("Client {i}"))),
                    ("p1", stage("Lead")),
                    ("p2", json!(100 + i)),
                    ("p3", text(&format!("Region {i}"))),
                    ("p4", json!(false)),
                    ("p5", Value::Null),
                    ("p6", json!(format!("T{i}"))),
                ],
            )
        })
        .collect()
}

/// Marta imports "Clients" and the first read brings `n` rows.
async fn imported(db: PgPool, n: u128) -> (Rig, String) {
    let rig = Rig::new(db).await;
    let table = rig.import_database(ds(), "Clients").await;
    let snap = snapshot(ds(), 100, Some(clients_schema()), clients(n));
    rig.read_table(&table, ds(), &snap).await;
    (rig, table)
}

fn cell<'a>(row: &'a Value, property: &str) -> &'a Value {
    &row["cells"][property]["value"]
}

#[sqlx::test(migrations = "../../../migrations")]
async fn importing_a_database_makes_a_private_linked_table(db: PgPool) {
    let rig = Rig::new(db).await;
    let made = rig.import_database_response(ds(), "Clients").await;
    assert_eq!(made.status.as_u16(), 201, "{:?}", made.json);
    assert_eq!(made.json["pages"], json!([]));
    let table = made.json["tables"][0]["table"].as_str().unwrap().to_owned();
    assert_eq!(made.json["tables"][0]["data_source"], ds().to_string());
    // A private table of Marta's, titled as she was shown it.
    let detail = rig.table_detail(&table).await;
    assert_eq!(detail["page"]["place"], "private");
    assert_eq!(detail["page"]["title"], "Clients");
    assert_eq!(detail["page"]["role"], "manager");
    let sync = rig.sync_of(&table).await;
    assert_eq!(sync["link"]["state"], "syncing");
    assert_eq!(sync["link"]["data_source"], ds().to_string());
    assert_eq!(sync["link"]["connection"], rig.connection.to_string());
    assert_eq!(sync["conflicts"], json!([]));
    // The connectors module is asked for a stream of the database.
    let asked = rig.kit.events("pages.notion_link_requested").await;
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0]["kind"], "data_source");
    // The same database again is left alone.
    let again = rig.import_database_response(ds(), "Clients").await;
    assert_eq!(again.status.as_u16(), 201);
    assert_eq!(again.json["tables"], json!([]));
    assert_eq!(again.json["skipped"], 1);
    // Nothing to import is refused.
    let none = rig
        .kit
        .post(
            &rig.token,
            &format!("{}/notion/imports", Kit::pages(rig.ws)),
            Some(json!({"connection": rig.connection})),
        )
        .await;
    assert_eq!(none.status.as_u16(), 400);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_first_read_fills_the_table_with_its_columns_and_rows(db: PgPool) {
    let (rig, table) = imported(db, 38).await;
    assert_eq!(
        rig.property_names(&table).await,
        [
            "Name", "Stage", "Value", "Region", "Active", "Site", "Total"
        ]
    );
    let detail = rig.table_detail(&table).await;
    assert_eq!(detail["rows"], 38);
    let types: Vec<&str> = detail["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        [
            "title",
            "select",
            "number",
            "text",
            "checkbox",
            "url",
            "notion_formula"
        ]
    );
    let rows = rig.rows_of(&table).await;
    assert_eq!(rows.len(), 38);
    let stage_id = rig.property_named(&table, "Stage").await;
    let value_id = rig.property_named(&table, "Value").await;
    let total_id = rig.property_named(&table, "Total").await;
    let row = rig.row_titled(&table, "Client 7").await;
    assert_eq!(cell(&row, &value_id), "107");
    assert_eq!(cell(&row, &total_id), "T7");
    // The option is the table's own, named as Notion names it.
    let options = detail["properties"][1]["config"]["options"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(options.len(), 2);
    assert_eq!(cell(&row, &stage_id), &options[0]["id"]);
    // Each row is linked to its Notion page; the first rows' bodies get a
    // stream of their own.
    let linked: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.external_links")
        .fetch_one(&rig.kit.db)
        .await
        .unwrap();
    assert_eq!(linked, 38);
    assert_eq!(
        rig.kit.events("pages.notion_link_requested").await.len(),
        1 + 38
    );
    // The bases: 7 columns, and a cell for every one of them that goes both
    // ways and holds something (the formula's is Notion's alone).
    let (columns, cells) = rig.base_counts(&table).await;
    assert_eq!(columns, 7);
    assert_eq!(cells, 38 * 5);
    let sync = rig.sync_of(&table).await;
    assert!(sync["link"]["last_inbound_at"].is_string());
    // A read-only column is never written by hand.
    let refused = rig
        .kit
        .cells(
            &rig.token,
            rig.ws,
            &table,
            row["id"].as_str().unwrap(),
            json!([{"property": total_id, "value": "x"}]),
        )
        .await;
    assert_eq!(refused.status.as_u16(), 400);
    // Nothing needs sending: Notion has what we have.
    rig.push_table_now().await;
    assert!(rig.table_requests(&table).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_older_read_is_never_applied(db: PgPool) {
    let (rig, table) = imported(db, 3).await;
    let mut older = snapshot(ds(), 50, Some(clients_schema()), clients(3));
    older.entries[0].cells[0] =
        crate::testkit_notion_tables::entry(1, 1, &[("title", text("Stale"))]).cells[0].clone();
    rig.read_table(&table, ds(), &older).await;
    assert!(
        rig.rows_of(&table)
            .await
            .iter()
            .all(|r| r["title"] != "Stale")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_read_for_another_connection_or_database_is_ignored(db: PgPool) {
    let rig = Rig::new(db).await;
    let table = rig.import_database(ds(), "Clients").await;
    let snap = snapshot(ds(), 100, Some(clients_schema()), clients(2));
    rig.read_table_as(&table, Uuid::now_v7(), ds(), &snap).await;
    assert!(rig.rows_of(&table).await.is_empty());
    let other = snapshot(id(0xEE), 100, Some(clients_schema()), clients(2));
    rig.read_table(&table, id(0xEE), &other).await;
    assert!(rig.rows_of(&table).await.is_empty());
    rig.read_table(&table, ds(), &snap).await;
    assert_eq!(rig.rows_of(&table).await.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn different_cells_of_one_row_merge_without_a_conflict(db: PgPool) {
    let (rig, table) = imported(db, 3).await;
    let row = rig.row_titled(&table, "Client 2").await;
    let value_id = rig.property_named(&table, "Value").await;
    let stage_id = rig.property_named(&table, "Stage").await;
    // Marta sets Value here ...
    let set = rig
        .kit
        .cells(
            &rig.token,
            rig.ws,
            &table,
            row["id"].as_str().unwrap(),
            json!([{"property": value_id, "value": "5000"}]),
        )
        .await;
    assert_eq!(set.status.as_u16(), 200, "{:?}", set.json);
    // ... and Rui sets Stage there, in the same row.
    let mut entries = clients(3);
    entries[1].cells[1] =
        crate::testkit_notion_tables::entry(0, 0, &[("p1", stage("Won"))]).cells[0].clone();
    entries[1].edited = at(60);
    let snap = snapshot(ds(), 101, None, vec![entries[1].clone()]);
    rig.read_table(&table, ds(), &snap).await;
    let row = rig.row_titled(&table, "Client 2").await;
    assert_eq!(cell(&row, &value_id), "5000");
    let won = rig.table_detail(&table).await["properties"][1]["config"]["options"][1]["id"].clone();
    assert_eq!(cell(&row, &stage_id), &won);
    assert_eq!(rig.sync_of(&table).await["conflicts"], json!([]));
    // Only Marta's cell goes to Notion.
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    assert_eq!(requests.len(), 1);
    let updates = actions(&requests[0].2, "cell_update");
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0]["external_property"], "p2");
    assert_eq!(updates[0]["subject"], row["id"]);
    let document = rig.table_document(requests[0].0).await;
    assert_eq!(document["items"][0]["value"], json!(5000));
    assert_eq!(document["items"][0]["kind"], "number");
}

/// Marta sets a cell of the row titled `title`.
async fn set(rig: &Rig, table: &str, title: &str, property: &str, value: Value) {
    let row = rig.row_titled(table, title).await;
    let done = rig
        .kit
        .cells(
            &rig.token,
            rig.ws,
            table,
            row["id"].as_str().unwrap(),
            json!([{"property": property, "value": value}]),
        )
        .await;
    assert_eq!(done.status.as_u16(), 200, "{:?}", done.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_same_cell_on_both_sides_is_a_conflict_until_a_choice(db: PgPool) {
    let (rig, table) = imported(db, 3).await;
    let value_id = rig.property_named(&table, "Value").await;
    set(&rig, &table, "Client 2", &value_id, json!("5000")).await;
    let mut theirs = clients(3);
    theirs[1].cells[2] = entry(0, 0, &[("p2", json!(7000))]).cells[0].clone();
    theirs[1].edited = at(60);
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 101, None, vec![theirs[1].clone()]),
    )
    .await;
    // Both versions are kept, and neither is sent.
    let sync = rig.sync_of(&table).await;
    let conflicts = sync["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["kind"], "cell");
    assert_eq!(conflicts[0]["ours"], "5000");
    assert_eq!(conflicts[0]["theirs"], "7000");
    assert_eq!(sync["link"]["conflicts"], 1);
    assert_eq!(
        cell(&rig.row_titled(&table, "Client 2").await, &value_id),
        "5000"
    );
    rig.push_table_now().await;
    assert!(rig.table_requests(&table).await.is_empty());
    // Notion's version, then back, then Campbooks' (which then goes out).
    let conflict = conflicts[0]["id"].as_str().unwrap();
    let uri = format!(
        "{}/{table}/notion/conflicts/{conflict}",
        Kit::tables(rig.ws)
    );
    let chosen = rig
        .kit
        .post(&rig.token, &uri, Some(json!({"keep": "theirs"})))
        .await;
    assert_eq!(chosen.status.as_u16(), 200, "{:?}", chosen.json);
    assert_eq!(chosen.json["conflicts"], 0);
    assert_eq!(
        cell(&rig.row_titled(&table, "Client 2").await, &value_id),
        "7000"
    );
    let again = rig
        .kit
        .post(&rig.token, &uri, Some(json!({"keep": "ours"})))
        .await;
    assert_eq!(again.status.as_u16(), 409, "{:?}", again.json);
    let undone = rig.kit.post(&rig.token, &format!("{uri}/undo"), None).await;
    assert_eq!(undone.status.as_u16(), 200, "{:?}", undone.json);
    assert_eq!(undone.json["conflicts"], 1);
    assert_eq!(
        cell(&rig.row_titled(&table, "Client 2").await, &value_id),
        "5000"
    );
    let kept = rig
        .kit
        .post(&rig.token, &uri, Some(json!({"keep": "ours"})))
        .await;
    assert_eq!(kept.status.as_u16(), 200, "{:?}", kept.json);
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    assert_eq!(requests.len(), 1);
    let updates = actions(&requests[0].2, "cell_update");
    assert_eq!(updates.len(), 1);
    // Checked against what Notion held when the conflict was found.
    assert_eq!(updates[0]["base_hash"], hex_of(&json!(7000)));
}

fn hex_of(value: &Value) -> Value {
    json!(crate::domain::notion::tables::model::hex(
        &crate::domain::notion::tables::model::remote_hash(value)
    ))
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_cell_changed_the_same_way_on_both_sides_is_no_conflict(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let value_id = rig.property_named(&table, "Value").await;
    set(&rig, &table, "Client 1", &value_id, json!("42")).await;
    let mut theirs = clients(2);
    theirs[0].cells[2] = entry(0, 0, &[("p2", json!(42))]).cells[0].clone();
    theirs[0].edited = at(70);
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 101, None, vec![theirs[0].clone()]),
    )
    .await;
    assert_eq!(rig.sync_of(&table).await["conflicts"], json!([]));
    rig.push_table_now().await;
    assert!(rig.table_requests(&table).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_column_renamed_on_both_sides_is_a_conflict_until_a_choice(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let value_id = rig.property_named(&table, "Value").await;
    let renamed = rig
        .kit
        .patch(
            &rig.token,
            &format!("{}/{table}/properties/{value_id}", Kit::tables(rig.ws)),
            json!({"name": "Deal size"}),
        )
        .await;
    assert_eq!(renamed.status.as_u16(), 200, "{:?}", renamed.json);
    let mut columns = clients_schema();
    columns.properties[2] = prop("p2", "Contract value", "number");
    let columns = schema(columns.properties);
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 101, Some(columns), Vec::new()),
    )
    .await;
    // Both names are offered, and neither is applied or sent.
    let sync = rig.sync_of(&table).await;
    let conflicts = sync["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["kind"], "column");
    assert_eq!(conflicts[0]["ours"]["name"], "Deal size");
    assert_eq!(conflicts[0]["theirs"]["name"], "Contract value");
    assert!(
        rig.property_names(&table)
            .await
            .contains(&"Deal size".to_owned())
    );
    rig.push_table_now().await;
    assert!(rig.table_requests(&table).await.is_empty());
    // Marta keeps hers: it is what goes out, against what Notion holds.
    let uri = format!(
        "{}/{table}/notion/conflicts/{}",
        Kit::tables(rig.ws),
        conflicts[0]["id"].as_str().unwrap()
    );
    let kept = rig
        .kit
        .post(&rig.token, &uri, Some(json!({"keep": "ours"})))
        .await;
    assert_eq!(kept.status.as_u16(), 200, "{:?}", kept.json);
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    let updates = actions(&requests[0].2, "schema_update");
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0]["external_property"], "p2");
    let document = rig.table_document(requests[0].0).await;
    assert_eq!(document["items"][0]["name"], "Deal size");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_column_changed_in_notion_alone_is_applied(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let mut columns = clients_schema().properties;
    columns[2] = prop("p2", "Contract value", "number");
    columns.push(prop("p7", "Notes", "rich_text"));
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 101, Some(schema(columns)), Vec::new()),
    )
    .await;
    let names = rig.property_names(&table).await;
    assert!(names.contains(&"Contract value".to_owned()) && names.contains(&"Notes".to_owned()));
    assert_eq!(rig.sync_of(&table).await["conflicts"], json!([]));
    rig.push_table_now().await;
    assert!(rig.table_requests(&table).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_synced_column_is_removed_only_with_the_confirmation(db: PgPool) {
    let (rig, table) = imported(db, 3).await;
    let region = rig.property_named(&table, "Region").await;
    let uri = format!("{}/{table}/properties/{region}", Kit::tables(rig.ws));
    // Without the confirmation: 400, and it stays in both places.
    let refused = rig.kit.delete(&rig.token, &uri).await;
    assert_eq!(refused.status.as_u16(), 400, "{:?}", refused.json);
    assert!(
        rig.property_names(&table)
            .await
            .contains(&"Region".to_owned())
    );
    rig.push_table_now().await;
    assert!(rig.table_requests(&table).await.is_empty());
    // Confirmed: it goes, here at once and at Notion with the next push.
    let gone = rig
        .kit
        .delete(&rig.token, &format!("{uri}?confirm_remote_delete=true"))
        .await;
    assert_eq!(gone.status.as_u16(), 200, "{:?}", gone.json);
    assert!(
        !rig.property_names(&table)
            .await
            .contains(&"Region".to_owned())
    );
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    assert_eq!(requests.len(), 1);
    let removals = actions(&requests[0].2, "schema_remove");
    assert_eq!(removals.len(), 1);
    assert_eq!(removals[0]["confirmed"], true);
    assert_eq!(removals[0]["external_property"], "p3");
    // The connectors module is told exactly that, bound to the request.
    let stored = rig.table_stands(requests[0].0).await.expect("it stands");
    assert!(stored.items[0].confirmed);
    // Made at Notion: the column is no longer mapped, its values are kept.
    rig.apply_requests(&table).await;
    let (columns, _) = rig.base_counts(&table).await;
    assert_eq!(columns, 6);
    // Restored within 30 days: it comes back in both places, with its values.
    let restored = rig
        .kit
        .post(&rig.token, &format!("{uri}/restore"), None)
        .await;
    assert_eq!(restored.status.as_u16(), 200, "{:?}", restored.json);
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    let added = actions(&requests[1].2, "schema_add");
    assert_eq!(added.len(), 1);
    assert_eq!(added[0]["subject"], region);
    rig.apply_requests(&table).await;
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    let updates = actions(&requests[2].2, "cell_update");
    assert_eq!(updates.len(), 3, "the three values are written back");
    assert!(updates.iter().all(|u| u["external_property"] == "col-0"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_removal_that_is_not_confirmed_is_never_sent_even_when_forced_in(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let region = rig.property_named(&table, "Region").await;
    // Whatever put it there (nothing in the API can), a column removed
    // without the member's confirmation is not asked of Notion.
    sqlx::query("UPDATE pages.table_properties SET removed_at = now() WHERE id = $1")
        .bind(region.parse::<Uuid>().unwrap())
        .execute(&rig.kit.db)
        .await
        .unwrap();
    let mut tx = rig.kit.begin(rig.ws).await;
    crate::domain::notion::tables::schedule_push(&mut tx, rig.ws, table.parse().unwrap())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rig.push_table_now().await;
    assert!(rig.table_requests(&table).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_column_removed_here_and_changed_in_notion_is_kept(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let region = rig.property_named(&table, "Region").await;
    let uri = format!(
        "{}/{table}/properties/{region}?confirm_remote_delete=true",
        Kit::tables(rig.ws)
    );
    assert_eq!(rig.kit.delete(&rig.token, &uri).await.status.as_u16(), 200);
    let mut columns = clients_schema().properties;
    columns[3] = prop("p3", "Area", "rich_text");
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 101, Some(schema(columns)), Vec::new()),
    )
    .await;
    let names = rig.property_names(&table).await;
    assert!(names.contains(&"Area".to_owned()), "{names:?}");
    rig.push_table_now().await;
    assert!(rig.table_requests(&table).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_column_removed_in_notion_goes_unless_it_was_changed_here(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let region = rig.property_named(&table, "Region").await;
    let site = rig.property_named(&table, "Site").await;
    // Site is renamed here; Region is untouched.
    let renamed = rig
        .kit
        .patch(
            &rig.token,
            &format!("{}/{table}/properties/{site}", Kit::tables(rig.ws)),
            json!({"name": "Website"}),
        )
        .await;
    assert_eq!(renamed.status.as_u16(), 200);
    let columns: Vec<_> = clients_schema()
        .properties
        .into_iter()
        .filter(|p| p.id != "p3" && p.id != "p5")
        .collect();
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 101, Some(schema(columns)), Vec::new()),
    )
    .await;
    let names = rig.property_names(&table).await;
    assert!(
        !names.contains(&"Region".to_owned()),
        "removed with Notion's: {names:?}"
    );
    assert!(
        names.contains(&"Website".to_owned()),
        "kept, being changed here"
    );
    // The removed one can still be taken back; the kept one is sent again.
    let back = rig
        .kit
        .post(
            &rig.token,
            &format!(
                "{}/{table}/properties/{region}/restore",
                Kit::tables(rig.ws)
            ),
            None,
        )
        .await;
    assert_eq!(back.status.as_u16(), 200, "{:?}", back.json);
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    let added = actions(&requests[0].2, "schema_add");
    assert_eq!(added.len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn columns_only_campbooks_has_are_never_sent(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    for body in [
        json!({"name": "Links", "type": "campbooks_link"}),
        json!({"name": "Who", "type": "person"}),
        json!({"name": "Since", "type": "created"}),
        json!({"name": "No.", "type": "unique_id"}),
        json!({"name": "Phase", "type": "status", "config": {"options": [{"name": "Open"}]}}),
    ] {
        rig.kit.property(&rig.token, rig.ws, &table, body).await;
    }
    rig.kit
        .property(
            &rig.token,
            rig.ws,
            &table,
            json!({"name": "Notes", "type": "text"}),
        )
        .await;
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    assert_eq!(requests.len(), 1);
    let added = actions(&requests[0].2, "schema_add");
    // Only the one Notion can have.
    assert_eq!(added.len(), 1);
    let document = rig.table_document(requests[0].0).await;
    assert_eq!(document["items"][0]["name"], "Notes");
    assert_eq!(document["items"][0]["kind"], "rich_text");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_row_made_here_is_sent_whole_and_linked_once_made(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let value_id = rig.property_named(&table, "Value").await;
    let made = rig
        .kit
        .row(
            &rig.token,
            rig.ws,
            &table,
            json!({"title": "Newcomer", "cells": [{"property": value_id, "value": "12.5"}]}),
        )
        .await;
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    assert_eq!(requests.len(), 1);
    let creates = actions(&requests[0].2, "row_create");
    assert_eq!(creates.len(), 1);
    assert_eq!(creates[0]["subject"], made["id"]);
    let document = rig.table_document(requests[0].0).await;
    let cells = document["items"][0]["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 2, "its name and its value");
    assert!(
        cells
            .iter()
            .any(|c| c["property"] == "title" && c["kind"] == "title")
    );
    assert!(
        cells
            .iter()
            .any(|c| c["property"] == "p2" && c["value"] == json!(12.5))
    );
    // Made at Notion: it is that page's row, and its body syncs.
    let before = rig.kit.events("pages.notion_link_requested").await.len();
    rig.apply_requests(&table).await;
    let after = rig.kit.events("pages.notion_link_requested").await.len();
    assert_eq!(after, before + 1);
    let linked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pages.external_links WHERE page_id = $1 AND state = 'syncing'",
    )
    .bind(made["id"].as_str().unwrap().parse::<Uuid>().unwrap())
    .fetch_one(&rig.kit.db)
    .await
    .unwrap();
    assert_eq!(linked, 1);
    rig.push_table_now().await;
    // Not made twice: it is a row of Notion's now (its cells settle on the
    // next read).
    let again = rig.table_requests(&table).await;
    assert_eq!(again.len(), 1, "not made twice");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_row_put_in_the_trash_here_is_trashed_in_notion_never_deleted(db: PgPool) {
    let (rig, table) = imported(db, 3).await;
    let row = rig.row_titled(&table, "Client 3").await;
    let trashed = rig
        .kit
        .post(
            &rig.token,
            &format!(
                "{}/{}/trash",
                Kit::pages(rig.ws),
                row["id"].as_str().unwrap()
            ),
            None,
        )
        .await;
    assert_eq!(trashed.status.as_u16(), 200, "{:?}", trashed.json);
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    assert_eq!(requests.len(), 1);
    let trashes = actions(&requests[0].2, "row_trash");
    assert_eq!(trashes.len(), 1);
    assert_eq!(trashes[0]["external_page"], id(0x1003).to_string());
    assert!(
        trashes[0]["base_edited"].is_string(),
        "checked against the edit time seen"
    );
    // Nothing else is asked, and nothing is a deletion.
    let kinds: Vec<&str> = requests[0]
        .2
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["action"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["row_trash"]);
    rig.apply_requests(&table).await;
    rig.push_table_now().await;
    assert_eq!(rig.table_requests(&table).await.len(), 1, "sent once");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_row_gone_from_notion_is_trashed_here_unless_it_was_changed_here(db: PgPool) {
    let (rig, table) = imported(db, 4).await;
    let value_id = rig.property_named(&table, "Value").await;
    set(&rig, &table, "Client 2", &value_id, json!("1")).await;
    let mut sweep = snapshot(ds(), 101, None, Vec::new());
    sweep.sweep = true;
    sweep.present = Some(vec![id(0x1001), id(0x1004)]);
    rig.read_table(&table, ds(), &sweep).await;
    let titles: Vec<String> = rig
        .rows_of(&table)
        .await
        .iter()
        .map(|r| r["title"].as_str().unwrap().to_owned())
        .collect();
    // Client 3 was unchanged here: in the trash (30 days). Client 2 was
    // changed here: kept, and sent again as a row of its own.
    assert!(!titles.contains(&"Client 3".to_owned()), "{titles:?}");
    assert!(titles.contains(&"Client 2".to_owned()));
    rig.push_table_now().await;
    let requests = rig.table_requests(&table).await;
    let creates = actions(&requests[0].2, "row_create");
    assert_eq!(creates.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn notions_own_columns_are_notions_alone(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let total = rig.property_named(&table, "Total").await;
    let mut theirs = clients(2);
    theirs[0].cells[6] = entry(0, 0, &[("p6", json!("changed"))]).cells[0].clone();
    theirs[0].edited = at(80);
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 101, None, vec![theirs[0].clone()]),
    )
    .await;
    assert_eq!(
        cell(&rig.row_titled(&table, "Client 1").await, &total),
        "changed"
    );
    rig.push_table_now().await;
    assert!(rig.table_requests(&table).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_request_stands_only_as_the_pages_module_stored_it(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let value_id = rig.property_named(&table, "Value").await;
    set(&rig, &table, "Client 1", &value_id, json!("9")).await;
    rig.push_table_now().await;
    let (request, _, _) = rig.table_requests(&table).await.pop().unwrap();
    assert!(rig.table_stands(request).await.is_some());
    // A request nobody stored does not stand.
    assert!(rig.table_stands(Uuid::now_v7()).await.is_none());
    // A change to what it names (the confirmation, a base) breaks its MAC.
    sqlx::query(
        "UPDATE pages.notion_table_requests
            SET items = jsonb_set(items, '{0,confirmed}', 'true') WHERE id = $1",
    )
    .bind(request)
    .execute(&rig.kit.db)
    .await
    .unwrap();
    assert!(rig.table_stands(request).await.is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_request_does_not_stand_once_the_link_ended(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let value_id = rig.property_named(&table, "Value").await;
    set(&rig, &table, "Client 1", &value_id, json!("9")).await;
    rig.push_table_now().await;
    let (request, _, _) = rig.table_requests(&table).await.pop().unwrap();
    let stopped = rig
        .kit
        .post(
            &rig.token,
            &format!("{}/{table}/notion/stop", Kit::tables(rig.ws)),
            None,
        )
        .await;
    assert_eq!(stopped.status.as_u16(), 200, "{:?}", stopped.json);
    assert_eq!(stopped.json["state"], "stopped");
    assert_eq!(stopped.json["stopped_reason"], "stopped_by_member");
    assert!(rig.table_stands(request).await.is_none());
    // The table and its rows stay; the rows' own links end; the stream goes.
    assert_eq!(rig.rows_of(&table).await.len(), 2);
    let ended = rig.kit.events("pages.notion_link_ended").await;
    assert!(ended.iter().any(|e| e["page"] == table));
    let live: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pages.external_links WHERE state <> 'stopped'")
            .fetch_one(&rig.kit.db)
            .await
            .unwrap();
    assert_eq!(live, 0);
    // Nothing more is asked, and a second stop is refused.
    set(&rig, &table, "Client 2", &value_id, json!("8")).await;
    rig.push_table_now().await;
    assert_eq!(rig.table_requests(&table).await.len(), 1);
    let again = rig
        .kit
        .post(
            &rig.token,
            &format!("{}/{table}/notion/stop", Kit::tables(rig.ws)),
            None,
        )
        .await;
    assert_eq!(again.status.as_u16(), 409);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn trashing_the_table_stops_the_link(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let trashed = rig
        .kit
        .post(
            &rig.token,
            &format!("{}/{table}/trash", Kit::pages(rig.ws)),
            None,
        )
        .await;
    assert_eq!(trashed.status.as_u16(), 200, "{:?}", trashed.json);
    let reason: Option<String> =
        sqlx::query_scalar("SELECT stopped_reason FROM pages.external_tables WHERE table_id = $1")
            .bind(table.parse::<Uuid>().unwrap())
            .fetch_one(&rig.kit.db)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("trashed_here"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_the_owner_of_a_private_table_reaches_it(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let (rui, _) = rig.kit.member("rui", rig.ws);
    let sync = format!("{}/{table}/notion", Kit::tables(rig.ws));
    assert_eq!(rig.kit.get(&rui, &sync).await.status.as_u16(), 404);
    assert_eq!(
        rig.kit
            .post(&rui, &format!("{sync}/stop"), None)
            .await
            .status
            .as_u16(),
        404
    );
    let resolve = rig
        .kit
        .post(
            &rui,
            &format!("{sync}/conflicts/{}", Uuid::now_v7()),
            Some(json!({"keep": "ours"})),
        )
        .await;
    assert_eq!(resolve.status.as_u16(), 404);
    let undo = rig
        .kit
        .post(
            &rui,
            &format!("{sync}/conflicts/{}/undo", Uuid::now_v7()),
            None,
        )
        .await;
    assert_eq!(undo.status.as_u16(), 404);
    // Nor can she bring a database in through Marta's connection.
    let theirs = rig
        .kit
        .post(
            &rui,
            &format!("{}/notion/imports", Kit::pages(rig.ws)),
            Some(
                json!({"connection": rig.connection, "data_sources": [{"data_source": id(0xF1)}]}),
            ),
        )
        .await;
    assert_eq!(theirs.status.as_u16(), 404);
    // Nothing was linked.
    let linked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pages.external_tables WHERE notion_data_source_id = $1",
    )
    .bind(id(0xF1))
    .fetch_one(&rig.kit.db)
    .await
    .unwrap();
    assert_eq!(linked, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sharing_a_table_does_not_hand_out_the_account(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let (rui, rui_id) = rig.kit.member("rui", rig.ws);
    rig.kit
        .share(&rig.token, rig.ws, &table, rui_id, "reader")
        .await;
    let sync = format!("{}/{table}/notion", Kit::tables(rig.ws));
    // A reader sees how it is going, but cannot change it.
    assert_eq!(rig.kit.get(&rui, &sync).await.status.as_u16(), 200);
    assert_eq!(
        rig.kit
            .post(&rui, &format!("{sync}/stop"), None)
            .await
            .status
            .as_u16(),
        403
    );
    let resolve = rig
        .kit
        .post(
            &rui,
            &format!("{sync}/conflicts/{}", Uuid::now_v7()),
            Some(json!({"keep": "ours"})),
        )
        .await;
    assert_eq!(resolve.status.as_u16(), 403);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn databases_off_there_is_nothing_to_reach(db: PgPool) {
    let rig = Rig::new(db).await.databases_off();
    let made = rig.import_database_response(ds(), "Clients").await;
    assert_eq!(made.status.as_u16(), 404);
    let sync = rig
        .kit
        .get(
            &rig.token,
            &format!("{}/{}/notion", Kit::tables(rig.ws), Uuid::now_v7()),
        )
        .await;
    assert_eq!(sync.status.as_u16(), 404);
    // Pages still import.
    let page = rig.import(id(0xAB), "A page").await;
    assert!(!page.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn no_event_carries_a_name_an_option_or_a_value(db: PgPool) {
    let (rig, table) = imported(db, 5).await;
    let value_id = rig.property_named(&table, "Value").await;
    set(&rig, &table, "Client 2", &value_id, json!("5000.25")).await;
    let mut theirs = clients(5);
    theirs[1].cells[2] = entry(0, 0, &[("p2", json!(7000.75))]).cells[0].clone();
    theirs[1].edited = at(60);
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 101, None, vec![theirs[1].clone()]),
    )
    .await;
    set(&rig, &table, "Client 3", &value_id, json!("31337.5")).await;
    rig.push_table_now().await;
    let region = rig.property_named(&table, "Region").await;
    let uri = format!(
        "{}/{table}/properties/{region}?confirm_remote_delete=true",
        Kit::tables(rig.ws)
    );
    rig.kit.delete(&rig.token, &uri).await;
    rig.push_table_now().await;
    let all: Vec<String> = sqlx::query_scalar("SELECT payload::text FROM kernel.events")
        .fetch_all(&rig.kit.db)
        .await
        .unwrap();
    let jobs: Vec<String> = sqlx::query_scalar("SELECT payload::text FROM kernel.jobs")
        .fetch_all(&rig.kit.db)
        .await
        .unwrap();
    // Values with a decimal point: ids, hashes and times never hold one
    // between digits like these, so a bare number cannot match by chance.
    for word in [
        "Client 2", "Region", "Lead", "Won", "5000.25", "7000.75", "31337.5", "Contract", "Stage",
        "Value",
    ] {
        assert!(
            all.iter().chain(&jobs).all(|p| !p.contains(word)),
            "{word} is in an event or a job"
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_changes_asked_for_are_tracked_without_names_or_values(db: PgPool) {
    let rig = Rig::tracking(db).await;
    let table = rig.import_database(ds(), "Clients").await;
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 100, Some(clients_schema()), clients(3)),
    )
    .await;
    let value_id = rig.property_named(&table, "Value").await;
    set(&rig, &table, "Client 2", &value_id, json!("5000.25")).await;
    let mut theirs = clients(3);
    theirs[1].cells[2] = entry(0, 0, &[("p2", json!(7000.75))]).cells[0].clone();
    theirs[1].edited = at(60);
    rig.read_table(
        &table,
        ds(),
        &snapshot(ds(), 101, None, vec![theirs[1].clone()]),
    )
    .await;
    let mut seen = Vec::new();
    for name in [
        "pages.notion_databases_imported",
        "pages.notion_table_synced",
        "pages.notion_table_conflict_raised",
    ] {
        let rows = testing::tracked_events(&rig.kit.db, name).await;
        assert!(!rows.is_empty(), "{name}");
        seen.extend(rows.iter().map(|r| format!("{} {}", r.envelope, r.body)));
    }
    let text = seen.join("\n");
    for word in ["Client", "Region", "5000.25", "7000.75", "Clients"] {
        assert!(!text.contains(word), "{word}");
    }
}

async fn stopped_because(rig: &Rig, table: &str) -> (String, Option<String>) {
    sqlx::query_as("SELECT state, stopped_reason FROM pages.external_tables WHERE table_id = $1")
        .bind(table.parse::<Uuid>().unwrap())
        .fetch_one(&rig.kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn disconnecting_stops_the_tables_read_through_the_connection(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let mut tx = rig.kit.begin(rig.ws).await;
    rig.kit
        .state
        .events
        .publish(
            &mut tx,
            &crate::events::ConnectionRemoved {
                workspace: rig.ws.to_string(),
                connection: rig.connection,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rig.kit.run_jobs().await;
    assert_eq!(
        stopped_because(&rig, &table).await,
        ("stopped".to_owned(), Some("disconnected".to_owned()))
    );
    assert_eq!(rig.rows_of(&table).await.len(), 2, "nothing is deleted");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_who_leaves_stops_their_tables(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    let mut tx = rig.kit.begin(rig.ws).await;
    rig.kit
        .state
        .events
        .publish(
            &mut tx,
            &crate::events::MemberLeft {
                workspace: rig.ws.to_string(),
                user: rig.user,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rig.kit.remove_member("marta", rig.ws);
    rig.kit.run_jobs().await;
    assert_eq!(
        stopped_because(&rig, &table).await,
        ("stopped".to_owned(), Some("member_left".to_owned()))
    );
}

async fn publish_stop(
    rig: &Rig,
    table: &str,
    connection: Uuid,
    data_source: Uuid,
    reason: crate::events::NotionStop,
) {
    let event = crate::events::NotionLinkStopped {
        workspace: rig.ws.to_string(),
        connection,
        notion_page: data_source,
        page: table.parse().unwrap(),
        reason,
    };
    let mut tx = rig.kit.begin(rig.ws).await;
    rig.kit.state.events.publish(&mut tx, &event).await.unwrap();
    tx.commit().await.unwrap();
    rig.kit.run_jobs().await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_connector_ending_a_link_stops_the_table_only_when_it_is_the_one(db: PgPool) {
    let (rig, table) = imported(db, 2).await;
    // Another database, or another connection, is none of this link's.
    publish_stop(
        &rig,
        &table,
        rig.connection,
        id(0xEE),
        crate::events::NotionStop::NotShared,
    )
    .await;
    publish_stop(
        &rig,
        &table,
        Uuid::now_v7(),
        ds(),
        crate::events::NotionStop::NotShared,
    )
    .await;
    assert_eq!(stopped_because(&rig, &table).await.0, "syncing");
    publish_stop(
        &rig,
        &table,
        rig.connection,
        ds(),
        crate::events::NotionStop::TrashedInNotion,
    )
    .await;
    assert_eq!(
        stopped_because(&rig, &table).await,
        ("stopped".to_owned(), Some("trashed_in_notion".to_owned()))
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn what_crosses_to_the_connector_follows_its_contracts(db: PgPool) {
    use crate::testkit_notion::assert_contract;
    let (rig, table) = imported(db, 2).await;
    let snap = snapshot(ds(), 100, Some(clients_schema()), clients(2));
    assert_contract(
        "notion-data-source-snapshot.v1.schema.json",
        &serde_json::to_value(&snap).unwrap(),
    );
    let value_id = rig.property_named(&table, "Value").await;
    set(&rig, &table, "Client 1", &value_id, json!("9")).await;
    rig.kit
        .row(&rig.token, rig.ws, &table, json!({"title": "New"}))
        .await;
    rig.kit
        .property(
            &rig.token,
            rig.ws,
            &table,
            json!({"name": "Notes", "type": "text"}),
        )
        .await;
    let region = rig.property_named(&table, "Region").await;
    let uri = format!(
        "{}/{table}/properties/{region}?confirm_remote_delete=true",
        Kit::tables(rig.ws)
    );
    rig.kit.delete(&rig.token, &uri).await;
    let row = rig.row_titled(&table, "Client 2").await;
    rig.kit
        .post(
            &rig.token,
            &format!(
                "{}/{}/trash",
                Kit::pages(rig.ws),
                row["id"].as_str().unwrap()
            ),
            None,
        )
        .await;
    rig.push_table_now().await;
    let (request, _, items) = rig.table_requests(&table).await.pop().unwrap();
    let mut kinds: Vec<String> = items
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["action"].as_str().unwrap().to_owned())
        .collect();
    kinds.sort();
    kinds.dedup();
    assert_eq!(
        kinds,
        [
            "cell_update",
            "row_create",
            "row_trash",
            "schema_add",
            "schema_remove"
        ]
    );
    assert_contract(
        "notion-table-changes.v1.schema.json",
        &rig.table_document(request).await,
    );
    let asked = rig.kit.events("pages.notion_table_changes_requested").await;
    assert_eq!(asked.len(), 1);
    assert_contract(
        "pages.notion_table_changes_requested.v1.schema.json",
        &asked[0],
    );
    let linked = rig.kit.events("pages.notion_link_requested").await;
    assert_contract("pages.notion_link_requested.v1.schema.json", &linked[0]);
    // What the connector tells is read as the contract says.
    let read = rig.kit.events("connectors.notion_data_source_read").await;
    assert_contract(
        "connectors.notion_data_source_read.v1.schema.json",
        &read[0],
    );
}
