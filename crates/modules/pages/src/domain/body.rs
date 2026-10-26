//! Taking a client's update to a page's body: check it, store it, and keep
//! what is derived from it (the links) current, in the caller's transaction.

use std::collections::BTreeSet;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::{
    AppState, Error, Result,
    blobs::{BlobHash, BlobOwner, BlobRef},
    jobs::{self, EnqueueOptions},
    authz::Action,
    tenancy::Access,
};
use chrono::{TimeDelta, Utc};
use sqlx::{Postgres, Transaction};
use yrs::Doc;

use super::{PageId, document, store::Loaded};
use crate::features::compact::Compact;

/// The largest update, as text, that is decoded at all: 256 KiB in base64,
/// with a little room.
const MAX_UPDATE_TEXT: usize = document::MAX_UPDATE_BYTES.div_ceil(3) * 4 + 16;

/// Updates after the snapshot at which the page is compacted at once.
pub const COMPACT_AFTER: i64 = 200;

/// How long after the first update since the last snapshot the page is
/// compacted, if fewer than [`COMPACT_AFTER`] come.
pub const COMPACT_IDLE: TimeDelta = TimeDelta::minutes(10);

/// `text` as bytes (standard base64), refusing what is too long to be an
/// update or a state vector.
pub fn decode(text: &str, limit: usize) -> Result<Vec<u8>> {
    if text.len() > limit {
        return Err(document::Rejection::UpdateTooLarge.into());
    }
    STANDARD
        .decode(text)
        .map_err(|_| document::Rejection::Malformed.into())
}

/// A client's update as bytes.
pub fn decode_update(text: &str) -> Result<Vec<u8>> {
    decode(text, MAX_UPDATE_TEXT)
}

/// A state vector as bytes (it is small: a few bytes per client).
pub fn decode_vector(text: &str) -> Result<Vec<u8>> {
    decode(text, 64 * 1024)
}

/// Bytes as standard base64.
pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// What an update did.
pub struct Accepted {
    /// Whether the document changed (an update it already had changes
    /// nothing).
    pub changed: bool,
    /// The document after it, read against the schema.
    pub inspection: document::Inspection,
}

/// Apply `update` (a client's, unchecked) to `doc`, the page's document as
/// stored, and read the result against the schema. Nothing is stored: a
/// refusal leaves nothing behind, and the caller stores the update with
/// [`store`] when it changed the document.
pub async fn accept(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    page: PageId,
    doc: &Doc,
    update: &[u8],
) -> Result<Accepted> {
    let before = document::inspect(doc)?;
    let before_state = document::full_state(doc);
    document::apply(doc, update)?;
    let inspection = document::inspect(doc)?;
    let changed = document::full_state(doc) != before_state;
    let added: Vec<String> = inspection
        .blobs
        .difference(&before.blobs)
        .cloned()
        .collect();
    if !added.is_empty() {
        hold_blobs(state, tx, access, page, &added).await?;
    }
    let shown: Vec<&(String, String)> = inspection
        .table_views
        .difference(&before.table_views)
        .collect();
    if !shown.is_empty() {
        check_table_views(state, tx, access, &shown).await?;
    }
    Ok(Accepted {
        changed,
        inspection,
    })
}

/// Every table a body newly shows must be a live table the caller may read,
/// with the view one of its own. Anything else is the one answer, so a
/// block cannot be used to find out what exists.
async fn check_table_views(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    shown: &[&(String, String)],
) -> Result<()> {
    let refused = || Error::BadRequest(crate::MESSAGES.reason("bad_table_view"));
    for (table, view) in shown {
        let (Ok(table), Ok(view)) = (table.parse::<PageId>(), view.parse::<sqlx::types::Uuid>())
        else {
            return Err(refused());
        };
        let row = match super::access::require_page(state, tx, access, Action::Read, table, false)
            .await
        {
            Ok(row) => row,
            Err(Error::NotFound | Error::Denied(_) | Error::Forbidden) => return Err(refused()),
            Err(other) => return Err(other),
        };
        let own: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pages.table_views WHERE id = $1 AND table_id = $2)",
        )
        .bind(view)
        .bind(table)
        .fetch_one(&mut **tx)
        .await?;
        if row.kind != "table" || row.is_trashed() || !own {
            return Err(refused());
        }
    }
    Ok(())
}

/// Every blob in `hashes` must be one the caller uploaded (to any page) or
/// one this page holds already; a page naming one it does not hold yet comes
/// to hold it (a file copied from another page), so the sweeper keeps it for
/// as long as this page does.
async fn hold_blobs(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    page: PageId,
    hashes: &[String],
) -> Result<()> {
    let Some(caller) = access.user else {
        return Err(Error::Forbidden);
    };
    let found: Vec<(String, i64, String, PageId)> = sqlx::query_as(
        "SELECT DISTINCT ON (hash) hash, size, content_type, page_id
           FROM pages.files
          WHERE hash = ANY($1) AND (page_id = $2 OR uploaded_by = $3)
          ORDER BY hash, (page_id = $2) DESC",
    )
    .bind(hashes)
    .bind(page)
    .bind(caller)
    .fetch_all(&mut **tx)
    .await?;
    let known: BTreeSet<&str> = found.iter().map(|(hash, ..)| hash.as_str()).collect();
    if hashes.iter().any(|hash| !known.contains(hash.as_str())) {
        return Err(Error::BadRequest(crate::MESSAGES.reason("foreign_blob")));
    }
    let workspace = access.workspace.to_string();
    for (hash, size, content_type, held_by) in found {
        if held_by == page {
            continue;
        }
        let blob = BlobRef {
            hash: hash
                .parse::<BlobHash>()
                .map_err(|_| Error::internal("a stored blob hash is not one"))?,
            size: u64::try_from(size).unwrap_or(0),
            content_type: content_type.clone(),
        };
        sqlx::query(
            "INSERT INTO pages.files (page_id, workspace_id, hash, size, content_type, uploaded_by)
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
        )
        .bind(page)
        .bind(access.workspace.as_uuid())
        .bind(&hash)
        .bind(size)
        .bind(&content_type)
        .bind(caller)
        .execute(&mut **tx)
        .await?;
        state
            .blobs
            .attach(
                tx,
                &workspace,
                &blob,
                &BlobOwner::new("pages.page", *page.as_uuid()),
            )
            .await?;
    }
    Ok(())
}

/// Keep `update` (already checked) as the page's newest, rebuild its links
/// from `inspection`, and mark the page edited. The page row is locked by
/// the caller. Asks for a compaction when the tail is long, or in ten
/// minutes.
pub async fn store(
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    page: PageId,
    loaded: &Loaded,
    update: &[u8],
    inspection: &document::Inspection,
) -> Result<()> {
    let Some(author) = access.user else {
        return Err(Error::Forbidden);
    };
    sqlx::query(
        "INSERT INTO pages.updates (page_id, workspace_id, seq, update, author)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(page)
    .bind(access.workspace.as_uuid())
    .bind(loaded.last_seq + 1)
    .bind(update)
    .bind(author)
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM pages.links WHERE page_id = $1")
        .bind(page)
        .execute(&mut **tx)
        .await?;
    if !inspection.links.is_empty() {
        let types: Vec<&str> = inspection
            .links
            .iter()
            .map(|l| l.ref_type.as_str())
            .collect();
        let ids: Vec<&str> = inspection.links.iter().map(|l| l.ref_id.as_str()).collect();
        let kinds: Vec<&str> = inspection.links.iter().map(|l| l.kind.as_str()).collect();
        sqlx::query(
            "INSERT INTO pages.links (page_id, workspace_id, ref_type, ref_id, kind)
             SELECT $1, $2, t, i, k FROM unnest($3::text[], $4::text[], $5::text[]) AS x(t, i, k)",
        )
        .bind(page)
        .bind(access.workspace.as_uuid())
        .bind(&types)
        .bind(&ids)
        .bind(&kinds)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query(
        "UPDATE pages.pages SET edited_by = $2, edited_at = now(), revision = revision + 1
          WHERE id = $1",
    )
    .bind(page)
    .bind(author)
    .execute(&mut **tx)
    .await?;
    let workspace = access.workspace.to_string();
    let job = Compact { page };
    if loaded.tail + 1 >= COMPACT_AFTER {
        jobs::enqueue(
            tx,
            &job,
            EnqueueOptions::for_workspace(workspace).unique(format!("pages.compact:{page}:now")),
        )
        .await?;
    } else {
        jobs::enqueue(
            tx,
            &job,
            EnqueueOptions::for_workspace(workspace)
                .unique(format!("pages.compact:{page}:idle"))
                .run_at(Utc::now() + COMPACT_IDLE),
        )
        .await?;
    }
    Ok(())
}
