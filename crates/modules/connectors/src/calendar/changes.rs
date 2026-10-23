//! Calendar changes: what members asked of a provider calendar
//! (`schedule.outbound_requested`), recorded once the schedule module
//! confirmed them, bound to the request with a MAC, made by the stream's
//! next run before it reads, and kept as the record of what became of each
//! (`connectors.calendar_changes`). Ids, a stable reason code and the
//! provider's opaque id and etag only: the text is in the blob the schedule
//! module wrote.

use std::collections::HashMap;

use campbooks_kernel::{
    JobError, Result,
    binding::{Binding, Fields},
    blobs::{BlobRef, BlobStore},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use serde::de::DeserializeOwned;
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

use super::Written;
use crate::domain::model::{ConnectionId, StreamId};

/// The largest `calendar-change` document that is read.
pub const MAX_CHANGE_BYTES: usize = 256 * 1024;

/// Runs that may meet provider trouble before a change is given up.
pub const MAX_ATTEMPTS: i32 = 8;

/// Most pending changes one run takes.
pub const PUSH_LIMIT: i64 = 200;

/// The encoding of a calendar change's binding.
const CALENDAR_CHANGE: &str = "campbooks/connectors.calendar-change/v1";

/// How a change ended (a pending one is the absence of an end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Made at the provider.
    Applied,
    /// The event had changed at the provider first.
    Superseded,
    /// Nothing was written.
    Skipped,
    /// The provider refused it, or it met trouble too often.
    Failed,
}

impl State {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Superseded => "superseded",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }
}

/// A recorded change.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Recorded {
    /// The row (UUIDv7: the order changes were recorded in).
    pub id: Uuid,
    /// The stream whose run makes it.
    pub stream_id: StreamId,
    /// The schedule module's request.
    pub request_id: Uuid,
    /// The schedule module's calendar.
    pub calendar_id: Uuid,
    /// The schedule module's event.
    pub event_id: Uuid,
    /// `create`, `update`, `update_occurrence`, `delete` or `respond`.
    pub change: String,
    /// The `calendar-change` v1 document.
    pub body: Json<BlobRef>,
    /// The binding.
    pub mac: Option<Vec<u8>>,
}

impl Recorded {
    /// What the binding covers: the row, where it is made (the stream and its
    /// connection), the request, calendar, event and kind of change, and the
    /// document to write (by its hash).
    fn fields(&self, workspace: WorkspaceId, connection: ConnectionId) -> Fields {
        let mut fields = Fields::new(CALENDAR_CHANGE);
        fields
            .uuid(workspace.as_uuid())
            .uuid(&self.id)
            .uuid(self.stream_id.as_uuid())
            .uuid(connection.as_uuid())
            .uuid(&self.request_id)
            .uuid(&self.calendar_id)
            .uuid(&self.event_id)
            .str(&self.change)
            .bytes(self.body.0.hash.as_bytes())
            .uint(self.body.0.size)
            .str(&self.body.0.content_type);
        fields
    }
}

const COLUMNS: &str = "id, stream_id, request_id, calendar_id, event_id, change, body, mac";

/// What to record for a request the schedule module confirmed.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The schedule module's request.
    pub request: Uuid,
    /// Its calendar.
    pub calendar: Uuid,
    /// Its event.
    pub event: Uuid,
    /// The kind of change.
    pub change: &'a str,
    /// The `calendar-change` document the schedule module stored.
    pub body: &'a BlobRef,
}

/// Record `request` for `stream` (of `connection`) and bind it. Returns the
/// row, or `None` when the request was recorded before. Without an instance
/// secret the row stays unbound and is never made.
pub async fn record(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
    (stream, connection): (StreamId, ConnectionId),
    request: &Request<'_>,
) -> Result<Option<Uuid>> {
    let id = Uuid::now_v7();
    let inserted = sqlx::query(
        "INSERT INTO connectors.calendar_changes
             (id, workspace_id, stream_id, request_id, calendar_id, event_id, change, body)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (request_id) DO NOTHING",
    )
    .bind(id)
    .bind(workspace)
    .bind(stream)
    .bind(request.request)
    .bind(request.calendar)
    .bind(request.event)
    .bind(request.change)
    .bind(Json(request.body))
    .execute(&mut *conn)
    .await?
    .rows_affected();
    if inserted == 0 {
        return Ok(None);
    }
    let row = recorded(conn, &[id]).await?.into_iter().next();
    if let Some(row) = row {
        sqlx::query("UPDATE connectors.calendar_changes SET mac = $2 WHERE id = $1")
            .bind(row.id)
            .bind(binding.tag(&row.fields(workspace, connection)))
            .execute(&mut *conn)
            .await?;
    }
    Ok(Some(id))
}

/// The changes `ids`, as recorded.
async fn recorded(conn: &mut PgConnection, ids: &[Uuid]) -> Result<Vec<Recorded>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM connectors.calendar_changes WHERE id = ANY($1) ORDER BY id"
    )))
    .bind(ids)
    .fetch_all(conn)
    .await?)
}

/// What a run has to push.
#[derive(Debug, Clone, Default)]
pub struct Backlog {
    /// Pending changes whose binding verifies, oldest first.
    pub pending: Vec<Recorded>,
    /// Pending changes whose binding does not (planted or edited by SQL, or
    /// bound to another connection): they ended `skipped` (`unverified`) and
    /// are never made.
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
/// verifies for a run of `connection`'s stream. The others end `skipped`
/// (`unverified`).
pub async fn backlog(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
    (stream, connection): (StreamId, ConnectionId),
    now: DateTime<Utc>,
) -> Result<Backlog> {
    let rows: Vec<Recorded> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM connectors.calendar_changes
          WHERE stream_id = $1 AND state = 'pending' ORDER BY id LIMIT $2"
    )))
    .bind(stream)
    .bind(PUSH_LIMIT)
    .fetch_all(&mut *conn)
    .await?;
    let (pending, refused): (Vec<Recorded>, Vec<Recorded>) = rows.into_iter().partition(|row| {
        row.stream_id == stream
            && binding.verify(&row.fields(workspace, connection), row.mac.as_deref())
    });
    for row in &refused {
        end(conn, row.id, State::Skipped, Some("unverified"), None, now).await?;
    }
    if !refused.is_empty() {
        tracing::warn!(
            %workspace,
            %stream,
            refused = refused.len(),
            "calendar changes not bound to the request they were recorded for, or to where \
             their stream goes now; they are not made"
        );
    }
    Ok(Backlog { pending, refused })
}

/// End change `id` as `state`, for `reason`, with what the provider answered
/// when it was applied.
pub async fn end(
    conn: &mut PgConnection,
    id: Uuid,
    state: State,
    reason: Option<&str>,
    written: Option<&Written>,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE connectors.calendar_changes
            SET state = $2, reason = $3, done_at = $4, provider_event_id = $5, etag = $6
          WHERE id = $1 AND state = 'pending'",
    )
    .bind(id)
    .bind(state.as_str())
    .bind(reason)
    .bind(now)
    .bind(written.map(|w| w.provider_event_id.as_str()))
    .bind(written.and_then(|w| w.etag.as_deref()))
    .execute(conn)
    .await?;
    Ok(())
}

/// Count a run that met provider trouble with change `id`. Returns its
/// attempts now.
pub async fn attempted(conn: &mut PgConnection, id: Uuid) -> Result<i32> {
    Ok(sqlx::query_scalar(
        "UPDATE connectors.calendar_changes SET attempts = attempts + 1
          WHERE id = $1 RETURNING attempts",
    )
    .bind(id)
    .fetch_one(conn)
    .await?)
}

/// Give up (`failed`, `too_many_attempts` is the caller's; here `expired`)
/// the pending changes of `stream` recorded before `before`: a change that
/// waited that long (a long pause, a re-authorisation) is never made late.
/// Returns them.
pub async fn expire(
    conn: &mut PgConnection,
    stream: StreamId,
    before: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<Vec<Recorded>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "UPDATE connectors.calendar_changes
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
        "DELETE FROM connectors.calendar_changes
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
        "SELECT EXISTS (SELECT 1 FROM connectors.calendar_changes
                         WHERE stream_id = $1 AND state = 'pending'
                           AND ($2::uuid IS NULL OR id > $2))",
    )
    .bind(stream)
    .bind(after)
    .fetch_one(conn)
    .await?)
}

/// The versions this module wrote lately (the provider's event id and the
/// etag it answered): the provider's own report of them is not a change.
pub async fn echoes(
    conn: &mut PgConnection,
    stream: StreamId,
    since: DateTime<Utc>,
) -> Result<HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT provider_event_id, etag FROM connectors.calendar_changes
          WHERE stream_id = $1 AND state = 'applied' AND done_at >= $2
            AND provider_event_id IS NOT NULL AND etag IS NOT NULL
          ORDER BY id",
    )
    .bind(stream)
    .bind(since)
    .fetch_all(conn)
    .await?;
    // The newest write of an event is the one that counts.
    Ok(rows.into_iter().collect())
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
        "SELECT {COLUMNS} FROM connectors.calendar_changes
          WHERE state = 'pending' AND mac IS NOT NULL ORDER BY id"
    )))
    .fetch_all(&mut *conn)
    .await?;
    let connections: HashMap<StreamId, ConnectionId> =
        sqlx::query_as::<_, (StreamId, ConnectionId)>(
            "SELECT id, connection_id FROM connectors.streams WHERE capability = 'calendar'",
        )
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .collect();
    let (mut retagged, mut left) = (0, 0);
    for row in rows {
        let Some(connection) = connections.get(&row.stream_id) else {
            continue;
        };
        let Some(mac) = binding.retag(&row.fields(workspace, *connection), row.mac.as_deref())
        else {
            continue;
        };
        let done = sqlx::query(
            "UPDATE connectors.calendar_changes SET mac = $2 WHERE id = $1 AND mac = $3",
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

/// The JSON document `blob` holds in `workspace`, when it is at most `max`
/// bytes and parses as `T` (`None` otherwise: a document the module does
/// not write is not made).
pub async fn read_json<T: DeserializeOwned>(
    blobs: &BlobStore,
    workspace: WorkspaceId,
    blob: &BlobRef,
    max: usize,
) -> std::result::Result<Option<T>, JobError> {
    if usize::try_from(blob.size).unwrap_or(usize::MAX) > max {
        return Ok(None);
    }
    let mut stream = blobs.get(&workspace.to_string(), blob).await?;
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
        if bytes.len() > max {
            return Ok(None);
        }
    }
    Ok(serde_json::from_slice(&bytes).ok())
}
