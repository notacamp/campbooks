//! Table changes: what members' edits of a synced table asked of a Notion
//! database (`pages.notion_table_changes_requested`), recorded once the
//! pages module confirmed them, bound to the request, the stream, its
//! connection and its data source with a MAC under a key the database never
//! holds, made by the stream's next run before it reads, and kept as the
//! record of what became of each (`connectors.notion_table_changes`). Ids,
//! kinds, times, hashes and a stable reason code only: the names, options
//! and values are in the blob the pages module wrote.

use campbooks_kernel::{
    AppState, JobError, Result,
    binding::{Binding, Fields},
    blobs::BlobRef,
    outbound::{StoredTableItem, TableItemKind},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Postgres, Transaction, types::Json};
use uuid::Uuid;

use super::changes::{State, reason_name};
use crate::{
    domain::model::{ConnectionId, StreamId},
    events::{
        NotionOutcome, NotionReason, NotionTableChange, NotionTableChangesCompleted,
        NotionTableItemResult,
    },
};

/// The largest `notion-table-changes` document that is read.
pub const MAX_CHANGE_BYTES: usize = 4 * 1024 * 1024;

/// The encoding of a table change's binding.
const NOTION_TABLE_CHANGE: &str = "campbooks/connectors.notion-table-change/v1";

/// The database name of a kind of change.
pub const fn kind_name(kind: TableItemKind) -> &'static str {
    match kind {
        TableItemKind::SchemaAdd => "schema_add",
        TableItemKind::SchemaUpdate => "schema_update",
        TableItemKind::SchemaRemove => "schema_remove",
        TableItemKind::RowCreate => "row_create",
        TableItemKind::CellUpdate => "cell_update",
        TableItemKind::RowTrash => "row_trash",
    }
}

/// A recorded table change.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Recorded {
    /// The row (UUIDv7: the order changes were recorded in).
    pub id: Uuid,
    /// The stream whose run makes it.
    pub stream_id: StreamId,
    /// The pages module's request.
    pub request_id: Uuid,
    /// The pages module's table.
    pub table_id: Uuid,
    /// The change's place in the request.
    pub seq: i32,
    /// The kind of change.
    pub kind: String,
    /// The column or row in the pages module.
    pub subject_id: Uuid,
    /// The column a cell is in.
    pub property_id: Option<Uuid>,
    /// The column at Notion.
    pub external_property: Option<String>,
    /// The row at Notion.
    pub external_page: Option<Uuid>,
    /// What Notion held for the column or cell at the last sync.
    pub base_hash: Option<String>,
    /// The whole schema's hash then.
    pub schema_hash: Option<String>,
    /// When Notion last edited the row then.
    pub base_edited_at: Option<DateTime<Utc>>,
    /// The member confirmed removing the column at Notion too.
    pub confirmed: bool,
    /// The `notion-table-changes` v1 document.
    pub body: Json<BlobRef>,
    /// The binding.
    pub mac: Option<Vec<u8>>,
}

/// Where a change is made: the stream's connection and the data source it
/// reads.
#[derive(Debug, Clone, Copy)]
pub struct At {
    /// The connection whose grant makes it.
    pub connection: ConnectionId,
    /// The data source.
    pub data_source: Uuid,
}

impl Recorded {
    /// What the binding covers: the row, where it is made, the request,
    /// table, place and kind of change, what it names, the bases it checks,
    /// the confirmation, and the document to write (by its hash).
    fn fields(&self, workspace: WorkspaceId, at: At) -> Fields {
        let mut fields = Fields::new(NOTION_TABLE_CHANGE);
        fields
            .uuid(workspace.as_uuid())
            .uuid(&self.id)
            .uuid(self.stream_id.as_uuid())
            .uuid(at.connection.as_uuid())
            .uuid(&at.data_source)
            .uuid(&self.request_id)
            .uuid(&self.table_id)
            .int(i64::from(self.seq))
            .str(&self.kind)
            .uuid(&self.subject_id)
            .opt_uuid(self.property_id.as_ref())
            .opt_str(self.external_property.as_deref())
            .opt_uuid(self.external_page.as_ref())
            .opt_str(self.base_hash.as_deref())
            .opt_str(self.schema_hash.as_deref())
            .opt_str(
                self.base_edited_at
                    .map(|time| time.timestamp_micros().to_string())
                    .as_deref(),
            )
            .uint(u64::from(self.confirmed))
            .bytes(self.body.0.hash.as_bytes())
            .uint(self.body.0.size)
            .str(&self.body.0.content_type);
        fields
    }
}

const COLUMNS: &str = "id, stream_id, request_id, table_id, seq, kind, subject_id, property_id, \
                       external_property, external_page, base_hash, schema_hash, base_edited_at, \
                       confirmed, body, mac";

/// What to record for a request the pages module confirmed.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The pages module's request.
    pub request: Uuid,
    /// Its table.
    pub table: Uuid,
    /// The changes, as the pages module stored them.
    pub items: &'a [StoredTableItem],
    /// The document the pages module stored.
    pub body: &'a BlobRef,
}

/// Record `request` for `stream` and bind each change. Returns how many
/// were recorded (none when the request was recorded before). Without an
/// instance secret the rows stay unbound and are never made.
pub async fn record(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
    stream: StreamId,
    at: At,
    request: &Request<'_>,
) -> Result<usize> {
    let mut recorded = 0;
    for item in request.items {
        let row = Recorded {
            id: Uuid::now_v7(),
            stream_id: stream,
            request_id: request.request,
            table_id: request.table,
            seq: i32::try_from(item.seq).unwrap_or(i32::MAX),
            kind: kind_name(item.kind).to_owned(),
            subject_id: item.subject,
            property_id: item.property,
            external_property: item.external_property.clone(),
            external_page: item.external_page,
            base_hash: item.base_hash.clone(),
            schema_hash: item.schema_hash.clone(),
            base_edited_at: item.base_edited_at,
            confirmed: item.confirmed,
            body: Json(request.body.clone()),
            mac: None,
        };
        let mac = binding.tag(&row.fields(workspace, at));
        let inserted = sqlx::query(
            "INSERT INTO connectors.notion_table_changes
                 (id, workspace_id, stream_id, request_id, table_id, seq, kind, subject_id,
                  property_id, external_property, external_page, base_hash, schema_hash,
                  base_edited_at, confirmed, body, mac)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
             ON CONFLICT (request_id, seq) DO NOTHING",
        )
        .bind(row.id)
        .bind(workspace)
        .bind(stream)
        .bind(row.request_id)
        .bind(row.table_id)
        .bind(row.seq)
        .bind(&row.kind)
        .bind(row.subject_id)
        .bind(row.property_id)
        .bind(&row.external_property)
        .bind(row.external_page)
        .bind(&row.base_hash)
        .bind(&row.schema_hash)
        .bind(row.base_edited_at)
        .bind(row.confirmed)
        .bind(&row.body)
        .bind(mac)
        .execute(&mut *conn)
        .await?
        .rows_affected();
        recorded += usize::try_from(inserted).unwrap_or(0);
    }
    Ok(recorded)
}

/// What a run has to push.
#[derive(Debug, Clone, Default)]
pub struct Backlog {
    /// Pending changes whose binding verifies, oldest first.
    pub pending: Vec<Recorded>,
    /// Pending changes whose binding does not: they ended `skipped`
    /// (`unverified`) and are never made.
    pub refused: Vec<Recorded>,
}

impl Backlog {
    /// The newest pending change taken.
    pub fn last(&self) -> Option<Uuid> {
        self.pending.last().map(|row| row.id)
    }
}

/// The pending changes of `stream` (at most [`super::changes::PUSH_LIMIT`])
/// whose binding verifies for a run of `at`. The others end `skipped`
/// (`unverified`).
pub async fn backlog(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
    stream: StreamId,
    at: At,
    now: DateTime<Utc>,
) -> Result<Backlog> {
    let rows: Vec<Recorded> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM connectors.notion_table_changes
          WHERE stream_id = $1 AND state = 'pending' ORDER BY id LIMIT $2"
    )))
    .bind(stream)
    .bind(super::changes::PUSH_LIMIT)
    .fetch_all(&mut *conn)
    .await?;
    let (pending, refused): (Vec<Recorded>, Vec<Recorded>) = rows.into_iter().partition(|row| {
        row.stream_id == stream && binding.verify(&row.fields(workspace, at), row.mac.as_deref())
    });
    for row in &refused {
        end(
            conn,
            row.id,
            State::Skipped,
            Some(NotionReason::Unverified),
            None,
            now,
        )
        .await?;
    }
    if !refused.is_empty() {
        tracing::warn!(
            %workspace,
            %stream,
            refused = refused.len(),
            "Notion table changes not bound to the request they were recorded for, or to where \
             their stream goes now; they are not made"
        );
    }
    Ok(Backlog { pending, refused })
}

/// What Notion answered when a change was applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The column's or the row's id at Notion (made, or changed).
    pub external: Option<String>,
    /// When Notion last edited it after the write.
    pub edited_at: Option<DateTime<Utc>>,
    /// The hash of what Notion holds now, in the connector's form.
    pub hash: Option<String>,
}

/// End change `id` as `state`, for `reason`, with what Notion answered.
pub async fn end(
    conn: &mut PgConnection,
    id: Uuid,
    state: State,
    reason: Option<NotionReason>,
    written: Option<Written>,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE connectors.notion_table_changes
            SET state = $2, reason = $3, done_at = $4, result_external = $5,
                result_edited_at = $6, result_hash = $7
          WHERE id = $1 AND state = 'pending'",
    )
    .bind(id)
    .bind(state.as_str())
    .bind(reason.map(reason_name))
    .bind(now)
    .bind(written.as_ref().and_then(|w| w.external.clone()))
    .bind(written.as_ref().and_then(|w| w.edited_at))
    .bind(written.as_ref().and_then(|w| w.hash.clone()))
    .execute(conn)
    .await?;
    Ok(())
}

/// Count a run that met Notion's trouble with change `id`. Returns its
/// attempts now.
pub async fn attempted(conn: &mut PgConnection, id: Uuid) -> Result<i32> {
    Ok(sqlx::query_scalar(
        "UPDATE connectors.notion_table_changes SET attempts = attempts + 1
          WHERE id = $1 RETURNING attempts",
    )
    .bind(id)
    .fetch_one(conn)
    .await?)
}

/// Give up (`failed`, `unavailable`) the pending changes of `stream`
/// recorded before `before`. Returns them.
pub async fn expire(
    conn: &mut PgConnection,
    stream: StreamId,
    before: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<Vec<Recorded>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE connectors.notion_table_changes
            SET state = 'failed', reason = 'unavailable', done_at = $3
          WHERE stream_id = $1 AND state = 'pending' AND created_at < $2
      RETURNING {COLUMNS}"
    )))
    .bind(stream)
    .bind(before)
    .bind(now)
    .fetch_all(conn)
    .await?)
}

/// Release the finished changes of `stream` older than `before`.
pub async fn prune(conn: &mut PgConnection, stream: StreamId, before: DateTime<Utc>) -> Result<()> {
    sqlx::query(
        "DELETE FROM connectors.notion_table_changes
          WHERE stream_id = $1 AND state <> 'pending' AND done_at < $2",
    )
    .bind(stream)
    .bind(before)
    .execute(conn)
    .await?;
    Ok(())
}

/// Whether `stream` has pending changes newer than `after`.
pub async fn more_pending(
    conn: &mut PgConnection,
    stream: StreamId,
    after: Option<Uuid>,
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM connectors.notion_table_changes
                         WHERE stream_id = $1 AND state = 'pending'
                           AND ($2::uuid IS NULL OR id > $2))",
    )
    .bind(stream)
    .bind(after)
    .fetch_one(conn)
    .await?)
}

/// The rows already made at Notion for the requests `requests`: a new row's
/// page by its id in the pages module.
pub async fn made(
    conn: &mut PgConnection,
    requests: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Uuid>> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT subject_id, result_external FROM connectors.notion_table_changes
          WHERE request_id = ANY($1) AND state = 'applied' AND kind = 'row_create'
            AND result_external IS NOT NULL",
    )
    .bind(requests)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(row, page)| Some((row, Uuid::parse_str(&page).ok()?)))
        .collect())
}

/// A change's end as the database keeps it.
type CompletedRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<DateTime<Utc>>,
    Option<String>,
);

/// Tell the pages module how a request ended once every one of its changes
/// has an end (`connectors.notion_table_changes_completed`, in `tx`).
pub async fn complete(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    workspace: WorkspaceId,
    request: Uuid,
) -> std::result::Result<(), JobError> {
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM connectors.notion_table_changes
                         WHERE request_id = $1 AND state = 'pending')",
    )
    .bind(request)
    .fetch_one(&mut **tx)
    .await?;
    if pending {
        return Ok(());
    }
    let rows: Vec<CompletedRow> = sqlx::query_as(
        "SELECT table_id, subject_id, kind, state, reason, result_external, result_edited_at,
                result_hash
           FROM connectors.notion_table_changes WHERE request_id = $1 ORDER BY seq",
    )
    .bind(request)
    .fetch_all(&mut **tx)
    .await?;
    let Some(table) = rows.first().map(|row| row.0) else {
        return Ok(());
    };
    let results = rows
        .into_iter()
        .map(
            |(_, subject, kind, end, reason, external, edited_at, hash)| NotionTableItemResult {
                subject,
                kind: NotionTableChange::of(&kind).unwrap_or(NotionTableChange::SchemaUpdate),
                outcome: match end.as_str() {
                    "applied" => NotionOutcome::Applied,
                    "skipped" => NotionOutcome::Skipped,
                    _ => NotionOutcome::Failed,
                },
                external,
                edited_at,
                hash,
                reason: reason.as_deref().and_then(super::changes::reason_of),
            },
        )
        .collect();
    state
        .events
        .publish(
            tx,
            &NotionTableChangesCompleted {
                workspace: workspace.to_string(),
                request,
                table,
                results,
            },
        )
        .await?;
    Ok(())
}

/// Re-tag, with the current key, every pending table change of `workspace`
/// whose MAC verifies under the previous key only (a rotation of the
/// instance secret key). Returns how many were re-tagged, and how many
/// changed while this ran.
pub async fn retag(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
) -> Result<(u64, u64)> {
    let rows: Vec<Recorded> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM connectors.notion_table_changes
          WHERE state = 'pending' AND mac IS NOT NULL ORDER BY id"
    )))
    .fetch_all(&mut *conn)
    .await?;
    let places: std::collections::HashMap<StreamId, At> =
        sqlx::query_as::<_, (StreamId, ConnectionId, Uuid)>(
            "SELECT stream_id, connection_id, notion_page_id FROM connectors.notion_links
              WHERE kind = 'data_source'",
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|(stream, connection, data_source)| {
            (
                stream,
                At {
                    connection,
                    data_source,
                },
            )
        })
        .collect();
    let (mut retagged, mut left) = (0, 0);
    for row in rows {
        let Some(at) = places.get(&row.stream_id) else {
            continue;
        };
        let Some(mac) = binding.retag(&row.fields(workspace, *at), row.mac.as_deref()) else {
            continue;
        };
        let done = sqlx::query(
            "UPDATE connectors.notion_table_changes SET mac = $2 WHERE id = $1 AND mac = $3",
        )
        .bind(row.id)
        .bind(mac)
        .bind(row.mac.as_deref())
        .execute(&mut *conn)
        .await?;
        if done.rows_affected() == 0 {
            left += 1;
        } else {
            retagged += 1;
        }
    }
    Ok((retagged, left))
}
