//! The requests this module makes of the connectors module for a synced
//! table: columns added, changed or removed, rows added or trashed, cells
//! set (campbooks-cloud#377).
//!
//! Any transaction working in a workspace can append that workspace's
//! events, so `pages.notion_table_changes_requested` alone proves nothing
//! (campbooks-cloud#227). A request is stored here, **bound** to the
//! workspace, table, connection, data source, every change (its kind, what
//! it names, the bases it was made against, whether the member confirmed a
//! removal) and the document it writes, with a MAC under a key the database
//! never holds. The connectors module asks this module through the kernel's
//! page-requests port ([`stored_table_changes`](PageRequests::stored_table_changes))
//! before it records anything: the request must still wait for it, its MAC
//! must verify, and its table's link must be syncing.

use campbooks_kernel::{
    AppState, Result,
    binding::Fields,
    blobs::{BlobOwner, BlobRef},
    outbound::{StoredTableChanges, StoredTableItem, TableItemKind},
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction, types::Json};
use uuid::Uuid;

use super::{
    ExternalTable, REQUEST_OWNER,
    model::{CHANGES_CONTENT_TYPE, ChangeDocument},
};
use crate::{domain::PageId, events::NotionTableChangesRequested};

/// The encoding of a request's binding.
const NOTION_TABLE_REQUEST: &str = "campbooks/pages.notion-table-request/v1";

/// One change of a stored request: ids, kinds, times and hashes. The
/// column's name travels in a schema item so that its base can be written
/// once it is made; no value does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredItem {
    /// The change's place in the request.
    pub seq: u32,
    /// `schema_add`, `schema_update`, `schema_remove`, `row_create`,
    /// `cell_update` or `row_trash`.
    pub action: String,
    /// The column or row.
    pub subject: Uuid,
    /// The column a cell is in.
    #[serde(default)]
    pub property: Option<Uuid>,
    /// The column at Notion.
    #[serde(default)]
    pub external_property: Option<String>,
    /// The row at Notion.
    #[serde(default)]
    pub external_page: Option<Uuid>,
    /// What Notion held for the column or cell at the last sync.
    #[serde(default)]
    pub base_hash: Option<String>,
    /// The whole schema's hash then.
    #[serde(default)]
    pub schema_hash: Option<String>,
    /// When Notion last edited the row then.
    #[serde(default)]
    pub base_edited: Option<DateTime<Utc>>,
    /// The member confirmed removing the column at Notion too.
    #[serde(default)]
    pub confirmed: bool,
    /// The column's name as sent (schema items).
    #[serde(default)]
    pub name: Option<String>,
    /// The Notion kind sent (schema items, cells).
    #[serde(default)]
    pub notion_kind: Option<String>,
    /// Hashes of the type and the config sent (schema items).
    #[serde(default)]
    pub type_hash: Option<String>,
    #[serde(default)]
    pub options_hash: Option<String>,
    /// The hash of the cell's value as sent, in this module's form.
    #[serde(default)]
    pub hash: Option<String>,
}

impl StoredItem {
    /// An item of `action` about `subject`.
    pub fn new(seq: usize, action: &str, subject: Uuid) -> Self {
        Self {
            seq: u32::try_from(seq).unwrap_or(u32::MAX),
            action: action.to_owned(),
            subject,
            property: None,
            external_property: None,
            external_page: None,
            base_hash: None,
            schema_hash: None,
            base_edited: None,
            confirmed: false,
            name: None,
            notion_kind: None,
            type_hash: None,
            options_hash: None,
            hash: None,
        }
    }

    /// The change as the connectors module takes it.
    fn for_connector(&self) -> Option<StoredTableItem> {
        let kind = match self.action.as_str() {
            "schema_add" => TableItemKind::SchemaAdd,
            "schema_update" => TableItemKind::SchemaUpdate,
            "schema_remove" => TableItemKind::SchemaRemove,
            "row_create" => TableItemKind::RowCreate,
            "cell_update" => TableItemKind::CellUpdate,
            "row_trash" => TableItemKind::RowTrash,
            _ => return None,
        };
        Some(StoredTableItem {
            seq: self.seq,
            kind,
            subject: self.subject,
            property: self.property,
            external_property: self.external_property.clone(),
            external_page: self.external_page,
            base_hash: self.base_hash.clone(),
            schema_hash: self.schema_hash.clone(),
            base_edited_at: self.base_edited,
            confirmed: self.confirmed,
        })
    }
}

/// What a request's binding covers.
struct Bound<'a> {
    workspace: WorkspaceId,
    request: Uuid,
    table: PageId,
    connection: Uuid,
    data_source: Uuid,
    items: &'a [StoredItem],
    body: &'a BlobRef,
}

impl Bound<'_> {
    fn fields(&self) -> Fields {
        let mut fields = Fields::new(NOTION_TABLE_REQUEST);
        fields
            .uuid(self.workspace.as_uuid())
            .uuid(&self.request)
            .uuid(self.table.as_uuid())
            .uuid(&self.connection)
            .uuid(&self.data_source)
            .count(self.items.len());
        for item in self.items {
            fields
                .uint(u64::from(item.seq))
                .str(&item.action)
                .uuid(&item.subject)
                .opt_uuid(item.property.as_ref())
                .opt_str(item.external_property.as_deref())
                .opt_uuid(item.external_page.as_ref())
                .opt_str(item.base_hash.as_deref())
                .opt_str(item.schema_hash.as_deref())
                .opt_str(
                    item.base_edited
                        .map(|time| time.timestamp_micros().to_string())
                        .as_deref(),
                )
                .uint(u64::from(item.confirmed))
                .opt_str(item.name.as_deref())
                .opt_str(item.notion_kind.as_deref())
                .opt_str(item.type_hash.as_deref())
                .opt_str(item.options_hash.as_deref())
                .opt_str(item.hash.as_deref());
        }
        fields
            .bytes(self.body.hash.as_bytes())
            .uint(self.body.size)
            .str(&self.body.content_type);
        fields
    }
}

/// Store `items` and `document` as a request of `table`'s link, bound and
/// waiting for the connectors module, and announce it. Returns its id.
pub async fn create(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    ext: &ExternalTable,
    by: UserId,
    items: &[StoredItem],
    document: &ChangeDocument,
) -> Result<Uuid> {
    let key = workspace.to_string();
    let bytes = serde_json::to_vec(document).map_err(campbooks_kernel::Error::internal)?;
    let body = state.blobs.put(&key, bytes, CHANGES_CONTENT_TYPE).await?;
    let request = Uuid::now_v7();
    let mac = state.binding.tag(
        &Bound {
            workspace,
            request,
            table: ext.table_id,
            connection: ext.connection_id,
            data_source: ext.notion_data_source_id,
            items,
            body: &body,
        }
        .fields(),
    );
    sqlx::query(
        "INSERT INTO pages.notion_table_requests
             (id, table_id, workspace_id, connection_id, notion_data_source_id, items, body, mac,
              created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(request)
    .bind(ext.table_id)
    .bind(workspace.as_uuid())
    .bind(ext.connection_id)
    .bind(ext.notion_data_source_id)
    .bind(Json(items))
    .bind(Json(&body))
    .bind(mac)
    .bind(by)
    .execute(&mut **tx)
    .await?;
    state
        .blobs
        .attach(tx, &key, &body, &BlobOwner::new(REQUEST_OWNER, request))
        .await?;
    state
        .events
        .publish(
            tx,
            &NotionTableChangesRequested {
                workspace: key,
                request,
                table: ext.table_id,
                connection: ext.connection_id,
                data_source: ext.notion_data_source_id,
                body,
            },
        )
        .await?;
    Ok(request)
}

/// The items of request `request`, as stored.
pub async fn items_of(conn: &mut PgConnection, request: Uuid) -> Result<Option<Vec<StoredItem>>> {
    let row: Option<(Json<Vec<StoredItem>>,)> =
        sqlx::query_as("SELECT items FROM pages.notion_table_requests WHERE id = $1")
            .bind(request)
            .fetch_optional(conn)
            .await?;
    Ok(row.map(|(items,)| items.0))
}

type Row = (
    PageId,
    Uuid,
    Uuid,
    Json<Vec<StoredItem>>,
    Json<BlobRef>,
    Option<Vec<u8>>,
);

/// The stored changes of `request`, if they stand (see the module docs).
pub async fn stored(
    state: &AppState,
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    request: Uuid,
) -> Result<Option<StoredTableChanges>> {
    let Some((table, connection, data_source, items, body, mac)) = sqlx::query_as::<_, Row>(
        "SELECT table_id, connection_id, notion_data_source_id, items, body, mac
           FROM pages.notion_table_requests
          WHERE id = $1 AND state = 'waiting'
            FOR SHARE",
    )
    .bind(request)
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let (items, body) = (items.0, body.0);
    let bound = Bound {
        workspace,
        request,
        table,
        connection,
        data_source,
        items: &items,
        body: &body,
    };
    if !state.binding.verify(&bound.fields(), mac.as_deref()) {
        tracing::warn!(
            %workspace,
            %request,
            "a stored Notion table request is not bound to the request that made it; it is not \
             made"
        );
        return Ok(None);
    }
    if !standing(conn, table, connection, data_source).await? {
        return Ok(None);
    }
    let for_connector: Option<Vec<StoredTableItem>> =
        items.iter().map(StoredItem::for_connector).collect();
    Ok(for_connector.map(|items| StoredTableChanges {
        table: *table.as_uuid(),
        connection,
        external: data_source,
        items,
        body,
    }))
}

/// Whether `table` is linked to `data_source` through `connection` and the
/// link is syncing.
pub async fn standing(
    conn: &mut PgConnection,
    table: PageId,
    connection: Uuid,
    data_source: Uuid,
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pages.external_tables
                         WHERE table_id = $1 AND connection_id = $2
                           AND notion_data_source_id = $3 AND state = 'syncing')",
    )
    .bind(table)
    .bind(connection)
    .bind(data_source)
    .fetch_one(conn)
    .await?)
}
