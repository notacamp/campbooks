//! Block changes: what members' edits asked of a Notion page
//! (`pages.notion_changes_requested`), recorded once the pages module
//! confirmed them, bound to the request, the stream, its connection and its
//! Notion page with a MAC under a key the database never holds, made by the
//! stream's next run before it reads, and kept as the record of what became
//! of each (`connectors.notion_changes`). Ids, kinds, times and a stable
//! reason code only: the text is in the blob the pages module wrote.

use std::collections::HashMap;

use campbooks_kernel::{
    AppState, JobError, Result,
    binding::{Binding, Fields},
    blobs::BlobRef,
    outbound::{PageItemKind, StoredPageItem},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Postgres, Transaction, types::Json};
use uuid::Uuid;

use crate::{
    domain::model::{ConnectionId, StreamId},
    events::{NotionChangesCompleted, NotionItemResult, NotionOutcome, NotionReason},
};

/// The largest `notion-block-changes` document that is read.
pub const MAX_CHANGE_BYTES: usize = 2 * 1024 * 1024;

/// Runs that may meet Notion's trouble before a change is given up.
pub const MAX_ATTEMPTS: i32 = 8;

/// Most pending changes one run takes.
pub const PUSH_LIMIT: i64 = 100;

/// The encoding of a block change's binding.
const NOTION_CHANGE: &str = "campbooks/connectors.notion-change/v1";

/// How a change ended (a pending one is the absence of an end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Made at Notion.
    Applied,
    /// Nothing was written.
    Skipped,
    /// Notion refused it, or it met trouble too often.
    Failed,
}

impl State {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }

    /// The outcome an event tells.
    pub const fn outcome(self) -> NotionOutcome {
        match self {
            Self::Applied => NotionOutcome::Applied,
            Self::Skipped => NotionOutcome::Skipped,
            Self::Failed => NotionOutcome::Failed,
        }
    }
}

/// The database name of a kind of change.
const fn kind_name(kind: PageItemKind) -> &'static str {
    match kind {
        PageItemKind::Update => "update",
        PageItemKind::Append => "append",
        PageItemKind::Delete => "delete",
    }
}

/// The reason a change ended for, as it is kept.
pub const fn reason_name(reason: NotionReason) -> &'static str {
    match reason {
        NotionReason::ChangedInNotion => "changed_in_notion",
        NotionReason::Unverified => "unverified",
        NotionReason::NotConnected => "not_connected",
        NotionReason::NeedsReauth => "needs_reauth",
        NotionReason::Rejected => "rejected",
        NotionReason::NotFound => "not_found",
        NotionReason::Unavailable => "unavailable",
        NotionReason::TooManyAttempts => "too_many_attempts",
        NotionReason::Unsupported => "unsupported",
        NotionReason::ConfirmationRequired => "confirmation_required",
    }
}

/// The reason kept as `name`, when it is one.
pub fn reason_of(name: &str) -> Option<NotionReason> {
    Some(match name {
        "changed_in_notion" => NotionReason::ChangedInNotion,
        "unverified" => NotionReason::Unverified,
        "not_connected" => NotionReason::NotConnected,
        "needs_reauth" => NotionReason::NeedsReauth,
        "rejected" => NotionReason::Rejected,
        "not_found" => NotionReason::NotFound,
        "unavailable" => NotionReason::Unavailable,
        "too_many_attempts" => NotionReason::TooManyAttempts,
        "unsupported" => NotionReason::Unsupported,
        "confirmation_required" => NotionReason::ConfirmationRequired,
        _ => return None,
    })
}

/// A recorded change.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Recorded {
    /// The row (UUIDv7: the order changes were recorded in).
    pub id: Uuid,
    /// The stream whose run makes it.
    pub stream_id: StreamId,
    /// The pages module's request.
    pub request_id: Uuid,
    /// The pages module's page.
    pub page_id: Uuid,
    /// The change's place in the request.
    pub seq: i32,
    /// `update`, `append` or `delete`.
    pub kind: String,
    /// The block in the pages module.
    pub block_id: Uuid,
    /// The block at Notion (update, delete).
    pub notion_block_id: Option<Uuid>,
    /// The block or page an added block goes under.
    pub parent_id: Option<Uuid>,
    /// The block of the same request it goes under.
    pub parent_block_id: Option<Uuid>,
    /// The block at Notion it goes after.
    pub after_notion_id: Option<Uuid>,
    /// The block of the same request it goes after.
    pub after_block_id: Option<Uuid>,
    /// When the block was last edited at Notion as of the last sync.
    pub base_edited_at: Option<DateTime<Utc>>,
    /// The `notion-block-changes` v1 document.
    pub body: Json<BlobRef>,
    /// The binding.
    pub mac: Option<Vec<u8>>,
}

/// Where a change is made: the stream's connection and the Notion page it
/// reads.
#[derive(Debug, Clone, Copy)]
pub struct At {
    /// The connection whose grant makes it.
    pub connection: ConnectionId,
    /// The Notion page.
    pub notion_page: Uuid,
}

impl Recorded {
    /// What the binding covers: the row, where it is made (the stream, its
    /// connection and its Notion page), the request, page, place and kind
    /// of change, the blocks it names, and the document to write (by its
    /// hash).
    fn fields(&self, workspace: WorkspaceId, at: At) -> Fields {
        let mut fields = Fields::new(NOTION_CHANGE);
        fields
            .uuid(workspace.as_uuid())
            .uuid(&self.id)
            .uuid(self.stream_id.as_uuid())
            .uuid(at.connection.as_uuid())
            .uuid(&at.notion_page)
            .uuid(&self.request_id)
            .uuid(&self.page_id)
            .int(i64::from(self.seq))
            .str(&self.kind)
            .uuid(&self.block_id)
            .opt_uuid(self.notion_block_id.as_ref())
            .opt_uuid(self.parent_id.as_ref())
            .opt_uuid(self.parent_block_id.as_ref())
            .opt_uuid(self.after_notion_id.as_ref())
            .opt_uuid(self.after_block_id.as_ref())
            .opt_str(
                self.base_edited_at
                    .map(|time| time.timestamp_micros().to_string())
                    .as_deref(),
            )
            .bytes(self.body.0.hash.as_bytes())
            .uint(self.body.0.size)
            .str(&self.body.0.content_type);
        fields
    }
}

const COLUMNS: &str = "id, stream_id, request_id, page_id, seq, kind, block_id, notion_block_id, \
                       parent_id, parent_block_id, after_notion_id, after_block_id, \
                       base_edited_at, body, mac";

/// What to record for a request the pages module confirmed.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The pages module's request.
    pub request: Uuid,
    /// Its page.
    pub page: Uuid,
    /// The changes, as the pages module stored them.
    pub items: &'a [StoredPageItem],
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
    for (seq, item) in request.items.iter().enumerate() {
        let row = Recorded {
            id: Uuid::now_v7(),
            stream_id: stream,
            request_id: request.request,
            page_id: request.page,
            seq: i32::try_from(seq).unwrap_or(i32::MAX),
            kind: kind_name(item.kind).to_owned(),
            block_id: item.block,
            notion_block_id: item.external,
            parent_id: item.parent,
            parent_block_id: item.parent_block,
            after_notion_id: item.after,
            after_block_id: item.after_block,
            base_edited_at: item.base_edited_at,
            body: Json(request.body.clone()),
            mac: None,
        };
        let mac = binding.tag(&row.fields(workspace, at));
        let inserted = sqlx::query(
            "INSERT INTO connectors.notion_changes
                 (id, workspace_id, stream_id, request_id, page_id, seq, kind, block_id,
                  notion_block_id, parent_id, parent_block_id, after_notion_id, after_block_id,
                  base_edited_at, body, mac)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
             ON CONFLICT (request_id, seq) DO NOTHING",
        )
        .bind(row.id)
        .bind(workspace)
        .bind(stream)
        .bind(row.request_id)
        .bind(row.page_id)
        .bind(row.seq)
        .bind(&row.kind)
        .bind(row.block_id)
        .bind(row.notion_block_id)
        .bind(row.parent_id)
        .bind(row.parent_block_id)
        .bind(row.after_notion_id)
        .bind(row.after_block_id)
        .bind(row.base_edited_at)
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
    /// Pending changes whose binding does not (planted or edited by SQL, or
    /// bound to another connection or page): they ended `skipped`
    /// (`unverified`) and are never made.
    pub refused: Vec<Recorded>,
}

impl Backlog {
    /// The newest pending change taken (a run continues at once when newer
    /// ones arrived meanwhile).
    pub fn last(&self) -> Option<Uuid> {
        self.pending.last().map(|row| row.id)
    }
}

/// The pending changes of `stream` (at most [`PUSH_LIMIT`]) whose binding
/// verifies for a run of `at`. The others end `skipped` (`unverified`).
pub async fn backlog(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
    stream: StreamId,
    at: At,
    now: DateTime<Utc>,
) -> Result<Backlog> {
    let rows: Vec<Recorded> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM connectors.notion_changes
          WHERE stream_id = $1 AND state = 'pending' ORDER BY id LIMIT $2"
    )))
    .bind(stream)
    .bind(PUSH_LIMIT)
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
            "Notion block changes not bound to the request they were recorded for, or to \
             where their stream goes now; they are not made"
        );
    }
    Ok(Backlog { pending, refused })
}

/// What Notion answered when a change was applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Written {
    /// The block's id at Notion (the new one, for an added block).
    pub notion_block: Uuid,
    /// When Notion last edited it after the write.
    pub edited_at: DateTime<Utc>,
}

/// End change `id` as `state`, for `reason`, with what Notion answered when
/// it was applied.
pub async fn end(
    conn: &mut PgConnection,
    id: Uuid,
    state: State,
    reason: Option<NotionReason>,
    written: Option<Written>,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE connectors.notion_changes
            SET state = $2, reason = $3, done_at = $4, result_notion_id = $5,
                result_edited_at = $6
          WHERE id = $1 AND state = 'pending'",
    )
    .bind(id)
    .bind(state.as_str())
    .bind(reason.map(reason_name))
    .bind(now)
    .bind(written.map(|w| w.notion_block))
    .bind(written.map(|w| w.edited_at))
    .execute(conn)
    .await?;
    Ok(())
}

/// Count a run that met Notion's trouble with change `id`. Returns its
/// attempts now.
pub async fn attempted(conn: &mut PgConnection, id: Uuid) -> Result<i32> {
    Ok(sqlx::query_scalar(
        "UPDATE connectors.notion_changes SET attempts = attempts + 1
          WHERE id = $1 RETURNING attempts",
    )
    .bind(id)
    .fetch_one(conn)
    .await?)
}

/// Give up (`failed`, `unavailable`) the pending changes of `stream`
/// recorded before `before`: a change that waited that long (a long pause, a
/// re-authorisation) is never made late. Returns them.
pub async fn expire(
    conn: &mut PgConnection,
    stream: StreamId,
    before: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<Vec<Recorded>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE connectors.notion_changes
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
        "DELETE FROM connectors.notion_changes
          WHERE stream_id = $1 AND state <> 'pending' AND done_at < $2",
    )
    .bind(stream)
    .bind(before)
    .execute(conn)
    .await?;
    Ok(())
}

/// Whether `stream` has pending changes newer than `after` (all of them when
/// `after` is `None`).
pub async fn more_pending(
    conn: &mut PgConnection,
    stream: StreamId,
    after: Option<Uuid>,
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM connectors.notion_changes
                         WHERE stream_id = $1 AND state = 'pending'
                           AND ($2::uuid IS NULL OR id > $2))",
    )
    .bind(stream)
    .bind(after)
    .fetch_one(conn)
    .await?)
}

/// The blocks already made at Notion for the requests `requests`: a new
/// block's id by its id in the pages module, so a later change of the same
/// request can go under it or after it.
pub async fn made(conn: &mut PgConnection, requests: &[Uuid]) -> Result<HashMap<Uuid, Uuid>> {
    let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT block_id, result_notion_id FROM connectors.notion_changes
          WHERE request_id = ANY($1) AND state = 'applied' AND kind = 'append'
            AND result_notion_id IS NOT NULL",
    )
    .bind(requests)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().collect())
}

/// A change's end as the database keeps it: page, block, state, reason, the
/// block made and its edit time.
type CompletedRow = (
    Uuid,
    Uuid,
    String,
    Option<String>,
    Option<Uuid>,
    Option<DateTime<Utc>>,
);

/// Tell the pages module how a request ended once every one of its changes
/// has an end (`connectors.notion_changes_completed`, in `tx`).
pub async fn complete(
    tx: &mut Transaction<'static, Postgres>,
    state: &AppState,
    workspace: WorkspaceId,
    request: Uuid,
) -> std::result::Result<(), JobError> {
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM connectors.notion_changes
                         WHERE request_id = $1 AND state = 'pending')",
    )
    .bind(request)
    .fetch_one(&mut **tx)
    .await?;
    if pending {
        return Ok(());
    }
    let rows: Vec<CompletedRow> = sqlx::query_as(
        "SELECT page_id, block_id, state, reason, result_notion_id, result_edited_at
               FROM connectors.notion_changes WHERE request_id = $1 ORDER BY seq",
    )
    .bind(request)
    .fetch_all(&mut **tx)
    .await?;
    let Some(page) = rows.first().map(|row| row.0) else {
        return Ok(());
    };
    let results = rows
        .into_iter()
        .map(
            |(_, block, end, reason, notion_block, edited_at)| NotionItemResult {
                block,
                outcome: match end.as_str() {
                    "applied" => NotionOutcome::Applied,
                    "skipped" => NotionOutcome::Skipped,
                    _ => NotionOutcome::Failed,
                },
                notion_block,
                edited_at,
                reason: reason.as_deref().and_then(reason_of),
            },
        )
        .collect();
    state
        .events
        .publish(
            tx,
            &NotionChangesCompleted {
                workspace: workspace.to_string(),
                request,
                page,
                results,
            },
        )
        .await?;
    Ok(())
}

/// Re-tag, with the current key, every pending change of `workspace` (the
/// transaction's) whose MAC verifies under the previous key only (a rotation
/// of the instance secret key, campbooks-cloud#262): the MAC was just
/// verified, so what was bound stays bound and nothing new is trusted. Each
/// update is a compare-and-set on the old MAC. Returns how many were
/// re-tagged, and how many changed while this ran.
pub async fn retag(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
) -> Result<(u64, u64)> {
    let rows: Vec<Recorded> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM connectors.notion_changes
          WHERE state = 'pending' AND mac IS NOT NULL ORDER BY id"
    )))
    .fetch_all(&mut *conn)
    .await?;
    let places: HashMap<StreamId, At> = sqlx::query_as::<_, (StreamId, ConnectionId, Uuid)>(
        "SELECT stream_id, connection_id, notion_page_id FROM connectors.notion_links",
    )
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(stream, connection, notion_page)| {
        (
            stream,
            At {
                connection,
                notion_page,
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
        let done =
            sqlx::query("UPDATE connectors.notion_changes SET mac = $2 WHERE id = $1 AND mac = $3")
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
