//! A page kept in step with a page in a member's Notion account
//! (campbooks-cloud#376): the link, what the last sync agreed about each
//! block, and the way changes cross.
//!
//! - **Inbound** (`merge`): the connectors module reads the Notion page into
//!   a snapshot of neutral blocks (`model`); this module compares each block
//!   with ours and the base the last sync agreed, and writes Notion's
//!   changes into the body block by block (`edit` is the Yjs side, `tree` the
//!   reading of the body).
//! - **Outbound** (`push`): a debounced job compares the body with the base
//!   and records the blocks that changed as a request for the connectors
//!   module (`requests`), bound with a MAC and confirmed back through the
//!   kernel's page-requests port before anything is written at Notion.
//! - **Stopping** (`stop`) ends a link, with a reason, and deletes nothing on
//!   either side.
//!
//! The mapping is structural, block kind to block kind; nothing here reads
//! what a page says to decide anything. Ids, hashes, kinds and times are all
//! this module keeps of Notion in its tables: the words are in the body.

pub mod edit;
pub mod merge;
pub mod model;
pub mod push;
pub mod requests;
pub mod resolve;
pub mod tables;
pub mod tree;

use campbooks_kernel::{
    AppState, Result,
    blobs::BlobOwner,
    jobs::{self, EnqueueOptions},
    tenancy::{Access, UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

use self::{merge::Mapping, model::Kind, tree::Resolver};
use super::{PageId, announce, tracked};
use crate::{
    events::{NotionLinkEnded, PageChangeKind},
    features::push_to_notion::PushToNotion,
};

/// The blob owner type of a request's document.
pub const REQUEST_OWNER: &str = "pages.notion_request";

/// The largest snapshot read, in bytes.
pub const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;

/// How long a page rests after the last edit before its changes are sent.
pub const DEBOUNCE_SECONDS: i64 = 10;

/// Why a link ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = NotionStopReason)]
pub enum StopReason {
    /// A member stopped syncing.
    StoppedByMember,
    /// The Notion connection was disconnected.
    Disconnected,
    /// The page was put in the trash here.
    TrashedHere,
    /// The page was trashed in Notion.
    TrashedInNotion,
    /// Notion no longer shares the page.
    NotShared,
    /// The member whose Notion account it was left the workspace.
    MemberLeft,
    /// Notion is not offered on this instance any more.
    NotionUnavailable,
}

impl StopReason {
    /// The name kept in `pages.external_links.stopped_reason`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StoppedByMember => "stopped_by_member",
            Self::Disconnected => "disconnected",
            Self::TrashedHere => "trashed_here",
            Self::TrashedInNotion => "trashed_in_notion",
            Self::NotShared => "not_shared",
            Self::MemberLeft => "member_left",
            Self::NotionUnavailable => "notion_unavailable",
        }
    }
}

/// Where a link is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = NotionLinkState)]
pub enum LinkState {
    /// Kept in step both ways.
    Syncing,
    /// Not kept in step for now.
    Paused,
    /// Ended; see the reason.
    Stopped,
}

/// A page's link to its Notion page, as the API shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(as = NotionLink)]
pub struct LinkView {
    /// The page.
    pub page: PageId,
    /// The Notion connection it is read through.
    pub connection: Uuid,
    /// The page at Notion.
    pub notion_page: Uuid,
    /// Where the link is.
    pub state: LinkState,
    /// Why it ended, when it did.
    pub stopped_reason: Option<StopReason>,
    /// The member whose Notion account it is.
    pub owner: UserId,
    /// How many conflicts wait for a choice.
    pub conflicts: i32,
    /// When a change from Notion was last applied.
    pub last_inbound_at: Option<DateTime<Utc>>,
    /// When a change here was last written to Notion.
    pub last_outbound_at: Option<DateTime<Utc>>,
}

/// A page's link to its Notion page.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Link {
    pub page_id: PageId,
    pub connection_id: Uuid,
    pub notion_page_id: Uuid,
    pub state: String,
    pub stopped_reason: Option<String>,
    pub created_by: UserId,
    pub conflicts: i32,
    pub last_inbound_at: Option<DateTime<Utc>>,
    pub last_outbound_at: Option<DateTime<Utc>>,
    pub last_read_at: Option<DateTime<Utc>>,
    pub last_title: Option<String>,
}

impl Link {
    /// Whether the link is syncing.
    pub fn syncing(&self) -> bool {
        self.state == "syncing"
    }

    /// The link as the API shows it.
    pub fn view(&self) -> LinkView {
        let reason = match self.stopped_reason.as_deref() {
            Some("stopped_by_member") => Some(StopReason::StoppedByMember),
            Some("disconnected") => Some(StopReason::Disconnected),
            Some("trashed_here") => Some(StopReason::TrashedHere),
            Some("trashed_in_notion") => Some(StopReason::TrashedInNotion),
            Some("not_shared") => Some(StopReason::NotShared),
            Some("member_left") => Some(StopReason::MemberLeft),
            Some("notion_unavailable") => Some(StopReason::NotionUnavailable),
            _ => None,
        };
        LinkView {
            page: self.page_id,
            connection: self.connection_id,
            notion_page: self.notion_page_id,
            state: match self.state.as_str() {
                "paused" => LinkState::Paused,
                "stopped" => LinkState::Stopped,
                _ => LinkState::Syncing,
            },
            stopped_reason: reason,
            owner: self.created_by,
            conflicts: self.conflicts,
            last_inbound_at: self.last_inbound_at,
            last_outbound_at: self.last_outbound_at,
        }
    }
}

const LINK_COLUMNS: &str = "page_id, connection_id, notion_page_id, state, stopped_reason, \
                            created_by, conflicts, last_inbound_at, last_outbound_at, \
                            last_read_at, last_title";

/// The link of `page`, if it has one.
pub async fn link_of(conn: &mut PgConnection, page: PageId) -> Result<Option<Link>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {LINK_COLUMNS} FROM pages.external_links WHERE page_id = $1"
    )))
    .bind(page)
    .fetch_optional(conn)
    .await?)
}

/// The link of `page`, locked for the change.
pub async fn lock_link(conn: &mut PgConnection, page: PageId) -> Result<Option<Link>> {
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {LINK_COLUMNS} FROM pages.external_links WHERE page_id = $1 FOR UPDATE"
    )))
    .bind(page)
    .fetch_optional(conn)
    .await?)
}

/// A hash as the database keeps it.
fn hash_of(bytes: &[u8]) -> Option<[u8; 32]> {
    <[u8; 32]>::try_from(bytes).ok()
}

/// A mapping as the database keeps it: block, Notion block, kind, hash, time.
type MappingRow = (Uuid, Uuid, String, Vec<u8>, DateTime<Utc>);

/// What the last sync agreed about each of `page`'s blocks.
pub async fn mappings(conn: &mut PgConnection, page: PageId) -> Result<Vec<Mapping>> {
    let rows: Vec<MappingRow> = sqlx::query_as(
        "SELECT bid, notion_block_id, kind, base_hash, base_edited_at
           FROM pages.external_blocks WHERE page_id = $1",
    )
    .bind(page)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(bid, notion_block, kind, base_hash, base_edited)| {
            Some(Mapping {
                bid,
                notion_block,
                kind: Kind::parse(&kind)?,
                base_hash: hash_of(&base_hash)?,
                base_edited,
            })
        })
        .collect())
}

/// Store the mappings an apply made, and forget those it ended.
pub async fn write_mappings(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    page: PageId,
    upserts: &[Mapping],
    deletes: &[Uuid],
) -> Result<()> {
    if !deletes.is_empty() {
        sqlx::query("DELETE FROM pages.external_blocks WHERE page_id = $1 AND bid = ANY($2)")
            .bind(page)
            .bind(deletes)
            .execute(&mut *conn)
            .await?;
    }
    for mapping in upserts {
        // A Notion block is one block of the page: whichever block held it
        // before lets go.
        sqlx::query(
            "DELETE FROM pages.external_blocks
              WHERE page_id = $1 AND notion_block_id = $2 AND bid <> $3",
        )
        .bind(page)
        .bind(mapping.notion_block)
        .bind(mapping.bid)
        .execute(&mut *conn)
        .await?;
        sqlx::query(
            "INSERT INTO pages.external_blocks
                 (page_id, workspace_id, bid, notion_block_id, kind, base_hash, base_edited_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (page_id, bid) DO UPDATE
                SET notion_block_id = EXCLUDED.notion_block_id, kind = EXCLUDED.kind,
                    base_hash = EXCLUDED.base_hash, base_edited_at = EXCLUDED.base_edited_at",
        )
        .bind(page)
        .bind(workspace.as_uuid())
        .bind(mapping.bid)
        .bind(mapping.notion_block)
        .bind(mapping.kind.as_str())
        .bind(mapping.base_hash.as_slice())
        .bind(mapping.base_edited)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// What reading a body needs to know of the rest of the workspace.
pub async fn resolver(
    conn: &mut PgConnection,
    state: &AppState,
    workspace: WorkspaceId,
    page: PageId,
) -> Result<Resolver> {
    let links: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT page_id, notion_page_id FROM pages.external_links WHERE state <> 'stopped'",
    )
    .fetch_all(conn)
    .await?;
    let base = state.config.public_url.trim().trim_end_matches('/');
    Ok(Resolver {
        notion_of: links.iter().copied().collect(),
        page_of: links
            .iter()
            .map(|(page, notion)| (*notion, *page))
            .collect(),
        app: if base.is_empty() {
            String::new()
        } else {
            format!("{base}/w/{workspace}")
        },
        this: *page.as_uuid(),
    })
}

/// End `page`'s link for `reason`: the page stays as it is, the requests
/// still waiting for the connector end, the connector is told to stop
/// reading, and nothing is deleted at Notion. `false` when there was no
/// link, or it had ended already.
pub async fn stop_link(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    reason: StopReason,
) -> Result<bool> {
    let ended: Option<(Uuid, Uuid)> = sqlx::query_as(
        "UPDATE pages.external_links
            SET state = 'stopped', stopped_reason = $2, stopped_at = now()
          WHERE page_id = $1 AND state <> 'stopped'
      RETURNING connection_id, notion_page_id",
    )
    .bind(page)
    .bind(reason.as_str())
    .fetch_optional(&mut **tx)
    .await?;
    let Some((connection, notion_page)) = ended else {
        return Ok(false);
    };
    let waiting: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE pages.notion_requests SET state = 'done', done_at = now()
          WHERE page_id = $1 AND state = 'waiting' RETURNING id",
    )
    .bind(page)
    .fetch_all(&mut **tx)
    .await?;
    let key = workspace.to_string();
    for request in waiting {
        state
            .blobs
            .detach_owner(tx, &key, &BlobOwner::new(REQUEST_OWNER, request))
            .await?;
    }
    state
        .events
        .publish(
            tx,
            &NotionLinkEnded {
                workspace: key,
                page,
                connection,
                notion_page,
            },
        )
        .await?;
    let access = Access {
        workspace,
        user: None,
        role: None,
    };
    announce(state, tx, &access, page, vec![PageChangeKind::Meta]).await?;
    tracked::notion_stopped(state, tx, workspace, page, reason).await?;
    Ok(true)
}

/// End the link of `page`, a page or a table kept in step with a Notion
/// database, for `reason` (see [`stop_link`]). `false` when there was no
/// link, or it had ended already.
pub async fn stop(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    reason: StopReason,
) -> Result<bool> {
    if stop_link(state, tx, workspace, page, reason).await? {
        return Ok(true);
    }
    tables::stop_table(state, tx, workspace, page, reason).await
}

/// Ask for `page`'s changes to be looked at, [`DEBOUNCE_SECONDS`] after the
/// last edit, when the page is kept in step with Notion. Another edit in
/// that time moves the time on (the job checks it).
pub async fn schedule_push(
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    edited_at: DateTime<Utc>,
) -> Result<()> {
    let syncing: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pages.external_links
                         WHERE page_id = $1 AND state = 'syncing')",
    )
    .bind(page)
    .fetch_one(&mut **tx)
    .await?;
    if !syncing {
        return Ok(());
    }
    jobs::enqueue(
        tx,
        &PushToNotion { page },
        EnqueueOptions::for_workspace(workspace.to_string())
            .unique(format!("pages.push_to_notion:{page}"))
            .run_at(edited_at + chrono::TimeDelta::seconds(DEBOUNCE_SECONDS)),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
