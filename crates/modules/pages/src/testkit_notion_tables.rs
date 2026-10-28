//! Fixtures for the Notion databases' tests: a database imported as a table,
//! snapshots handed over as the connectors module would (a blob and an
//! event), the requests the table makes of the connector and their ends,
//! and edits made as a client does. No test talks to Notion.

use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    domain::notion::tables::model::{
        NCell, NEntry, NOption, NProperty, NSchema, SNAPSHOT_CONTENT_TYPE, Snapshot, hex,
        remote_hash,
    },
    events::{
        NotionDataSourceRead, NotionOutcome, NotionTableChangesCompleted, NotionTableItemResult,
    },
    testkit::Kit,
    testkit_notion::Rig,
};

/// A fixed identifier.
pub fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// A time `minutes` after the tests' start.
pub fn at(minutes: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_792_918_800 + minutes * 60, 0).unwrap_or_default()
}

/// A Notion property.
pub fn prop(id: &str, name: &str, kind: &str) -> NProperty {
    with_options(id, name, kind, &[])
}

/// A Notion select-like property with options `(id, name)`.
pub fn with_options(pid: &str, name: &str, kind: &str, options: &[(&str, &str)]) -> NProperty {
    let options: Vec<NOption> = options
        .iter()
        .map(|(oid, oname)| NOption {
            id: (*oid).to_owned(),
            name: (*oname).to_owned(),
            color: "default".to_owned(),
            group: (kind == "status").then(|| "todo".to_owned()),
        })
        .collect();
    let described = json!({"id": pid, "name": name, "kind": kind,
        "options": options.iter().map(|o| json!([o.id, o.name])).collect::<Vec<_>>()});
    NProperty {
        id: pid.to_owned(),
        name: name.to_owned(),
        kind: kind.to_owned(),
        options,
        format: None,
        hash: hex(&remote_hash(&described)),
    }
}

/// A schema of `properties`.
pub fn schema(properties: Vec<NProperty>) -> NSchema {
    let hashes: Vec<Value> = properties.iter().map(|p| json!([p.id, p.hash])).collect();
    NSchema {
        hash: hex(&remote_hash(&Value::Array(hashes))),
        properties,
    }
}

/// Runs of plain text.
pub fn text(text: &str) -> Value {
    json!([{ "text": text }])
}

/// An entry edited at minute `edited` with `cells` (property id, neutral
/// value).
pub fn entry(n: u128, edited: i64, cells: &[(&str, Value)]) -> NEntry {
    NEntry {
        id: id(n),
        edited: at(edited),
        cells: cells
            .iter()
            .map(|(property, value)| NCell {
                property: (*property).to_owned(),
                hash: hex(&remote_hash(value)),
                value: value.clone(),
            })
            .collect(),
    }
}

/// A snapshot read at minute `read`.
pub fn snapshot(
    data_source: Uuid,
    read: i64,
    schema: Option<NSchema>,
    entries: Vec<NEntry>,
) -> Snapshot {
    Snapshot {
        version: 1,
        data_source,
        title: "Clients".to_owned(),
        read_at: at(read),
        schema,
        entries,
        sweep: false,
        present: None,
    }
}

impl Rig {
    /// Import Notion database `data_source` through Marta's connection: the
    /// answer.
    pub async fn import_database_response(
        &self,
        data_source: Uuid,
        title: &str,
    ) -> campbooks_kernel::testing::TestResponse {
        self.kit
            .post(
                &self.token,
                &format!("{}/notion/imports", Kit::pages(self.ws)),
                Some(json!({
                    "connection": self.connection,
                    "data_sources": [{"data_source": data_source, "title": title}],
                })),
            )
            .await
    }

    /// Import one database: the table made.
    pub async fn import_database(&self, data_source: Uuid, title: &str) -> String {
        let made = self.import_database_response(data_source, title).await;
        assert_eq!(made.status.as_u16(), 201, "{:?}", made.json);
        made.json["tables"][0]["table"].as_str().unwrap().to_owned()
    }

    /// The connector read `data_source` and hands `snapshot` over for
    /// `table`; the jobs run.
    pub async fn read_table(&self, table: &str, data_source: Uuid, snapshot: &Snapshot) {
        self.read_table_as(table, self.connection, data_source, snapshot)
            .await;
    }

    /// The same, claiming a given connection.
    pub async fn read_table_as(
        &self,
        table: &str,
        connection: Uuid,
        data_source: Uuid,
        snapshot: &Snapshot,
    ) {
        let bytes = serde_json::to_vec(snapshot).unwrap();
        let blob = self
            .kit
            .state
            .blobs
            .put(&self.ws.to_string(), bytes, SNAPSHOT_CONTENT_TYPE)
            .await
            .unwrap();
        let mut tx = self.kit.begin(self.ws).await;
        self.kit
            .state
            .events
            .publish(
                &mut tx,
                &NotionDataSourceRead {
                    workspace: self.ws.to_string(),
                    connection,
                    data_source,
                    table: table.parse().unwrap(),
                    snapshot: blob,
                    read_at: snapshot.read_at,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        self.kit.run_jobs().await;
    }

    /// The table as the API shows it.
    pub async fn table_detail(&self, table: &str) -> Value {
        let got = self
            .kit
            .get(&self.token, &format!("{}/{table}", Kit::tables(self.ws)))
            .await;
        assert_eq!(got.status.as_u16(), 200, "{:?}", got.json);
        got.json
    }

    /// The id of the property of `table` named `name`.
    pub async fn property_named(&self, table: &str, name: &str) -> String {
        self.table_detail(table).await["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap_or_else(|| panic!("no property {name}"))["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// The names of the properties of `table`, in order.
    pub async fn property_names(&self, table: &str) -> Vec<String> {
        self.table_detail(table).await["properties"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The rows of `table` (a query without a filter), by title.
    pub async fn rows_of(&self, table: &str) -> Vec<Value> {
        let got = self
            .kit
            .post(
                &self.token,
                &format!("{}/{table}/query", Kit::tables(self.ws)),
                Some(json!({"limit": 100})),
            )
            .await;
        assert_eq!(got.status.as_u16(), 200, "{:?}", got.json);
        got.json["rows"].as_array().cloned().unwrap_or_default()
    }

    /// The row of `table` titled `title`.
    pub async fn row_titled(&self, table: &str, title: &str) -> Value {
        self.rows_of(table)
            .await
            .into_iter()
            .find(|r| r["title"] == title)
            .unwrap_or_else(|| panic!("no row {title}"))
    }

    /// `GET …/tables/{table}/notion`.
    pub async fn sync_of(&self, table: &str) -> Value {
        let got = self
            .kit
            .get(
                &self.token,
                &format!("{}/{table}/notion", Kit::tables(self.ws)),
            )
            .await;
        assert_eq!(got.status.as_u16(), 200, "{:?}", got.json);
        got.json
    }

    /// The table's push job runs now.
    pub async fn push_table_now(&self) {
        sqlx::query(
            "UPDATE kernel.jobs SET run_at = now()
              WHERE kind = 'pages.push_table_to_notion' AND state = 'pending'",
        )
        .execute(&self.kit.db)
        .await
        .unwrap();
        self.kit.run_jobs().await;
    }

    /// The requests of a table, oldest first: `(id, state, items)`.
    pub async fn table_requests(&self, table: &str) -> Vec<(Uuid, String, Value)> {
        sqlx::query_as(
            "SELECT id, state, items FROM pages.notion_table_requests WHERE table_id = $1
              ORDER BY id",
        )
        .bind(table.parse::<Uuid>().unwrap())
        .fetch_all(&self.kit.db)
        .await
        .unwrap()
    }

    /// What the connectors module is told for `request`, as the port gives
    /// it: `None` when it does not stand.
    pub async fn table_stands(
        &self,
        request: Uuid,
    ) -> Option<campbooks_kernel::outbound::StoredTableChanges> {
        let mut tx = self.kit.begin(self.ws).await;
        let stored = crate::domain::notion::requests::page_requests()
            .stored_table_changes(&self.kit.state, &mut tx, self.ws, request)
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        stored
    }

    /// The `notion-table-changes` document of a request.
    pub async fn table_document(&self, request: Uuid) -> Value {
        let stored = self
            .table_stands(request)
            .await
            .expect("the request stands");
        self.blob_json(&serde_json::to_value(&stored.body).unwrap())
            .await
    }

    /// The connector tells how `request` ended.
    pub async fn complete_table(
        &self,
        table: &str,
        request: Uuid,
        results: Vec<NotionTableItemResult>,
    ) {
        let mut tx = self.kit.begin(self.ws).await;
        self.kit
            .state
            .events
            .publish(
                &mut tx,
                &NotionTableChangesCompleted {
                    workspace: self.ws.to_string(),
                    request,
                    table: table.parse().unwrap(),
                    results,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        self.kit.run_jobs().await;
    }

    /// Every item of the waiting request of `table` applied, the way the
    /// connector does: columns made get Notion ids `col-<n>`, rows made get
    /// pages, and everything is as written. The request.
    pub async fn apply_requests(&self, table: &str) -> Uuid {
        let (request, _, items) = self
            .table_requests(table)
            .await
            .into_iter()
            .find(|(_, state, _)| state == "waiting")
            .expect("a request waits");
        let results = items
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(n, item)| {
                let action = item["action"].as_str().unwrap();
                NotionTableItemResult {
                    subject: item["subject"].as_str().unwrap().parse().unwrap(),
                    kind: action.to_owned(),
                    outcome: NotionOutcome::Applied,
                    external: match action {
                        "schema_add" => Some(format!("col-{n}")),
                        "row_create" => Some(Uuid::now_v7().to_string()),
                        _ => item["external_property"].as_str().map(ToOwned::to_owned),
                    },
                    edited_at: Some(at(90)),
                    hash: Some(hex(&remote_hash(&json!([action, n])))),
                }
            })
            .collect();
        self.complete_table(table, request, results).await;
        request
    }

    /// A table's counts of the bases: `(columns, cells)`.
    pub async fn base_counts(&self, table: &str) -> (i64, i64) {
        let table = table.parse::<Uuid>().unwrap();
        let columns = sqlx::query_scalar(
            "SELECT count(*) FROM pages.external_properties WHERE table_id = $1",
        )
        .bind(table)
        .fetch_one(&self.kit.db)
        .await
        .unwrap();
        let cells =
            sqlx::query_scalar("SELECT count(*) FROM pages.external_cells WHERE table_id = $1")
                .bind(table)
                .fetch_one(&self.kit.db)
                .await
                .unwrap();
        (columns, cells)
    }
}

/// The stored items of a request that carry `action`.
pub fn actions(items: &Value, action: &str) -> Vec<Value> {
    items
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["action"] == action)
        .cloned()
        .collect()
}
