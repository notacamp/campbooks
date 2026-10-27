//! The requests this module makes of the connectors module: block changes a
//! member's edits asked for, to be made at Notion (campbooks-cloud#376).
//!
//! Any transaction working in a workspace can append that workspace's
//! events, so `pages.notion_changes_requested` alone proves nothing
//! (campbooks-cloud#227). A request is therefore stored here, **bound** to
//! the workspace, the page, the connection, the Notion page, every change
//! (its kind, its blocks, the edit time it was based on, the hash of what it
//! writes) and the document it writes, with a MAC under a key the database
//! never holds. The connectors module asks this module through the kernel's
//! page-requests port ([`stored_changes`](PageRequests::stored_changes))
//! before it records anything: the request must still wait for it, its MAC
//! must verify, and its page's link must be syncing. What it records and
//! makes is what is stored here, never what the event names.

use std::sync::Arc;

use campbooks_kernel::{
    AppState, Result,
    binding::Fields,
    blobs::{BlobOwner, BlobRef},
    outbound::{
        LinkStandingFuture, PageItemKind, PageRequests, PageRequestsPort, StoredPageChanges,
        StoredPageChangesFuture, StoredPageItem,
    },
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction, types::Json};
use uuid::Uuid;

use super::{
    REQUEST_OWNER,
    model::{CHANGES_CONTENT_TYPE, hash_hex},
    push::Plan,
};
use crate::{domain::PageId, events::NotionChangesRequested};

/// The encoding of a request's binding.
const NOTION_REQUEST: &str = "campbooks/pages.notion-request/v1";

/// One change of a stored request: ids, kinds, times and a hash. Never any
/// text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredItem {
    /// The change's place in the request.
    pub seq: u32,
    /// `update`, `append` or `delete`.
    pub action: String,
    /// The block in the body.
    pub bid: Uuid,
    /// What kind of block.
    pub kind: String,
    /// The block at Notion.
    #[serde(default)]
    pub external: Option<Uuid>,
    /// The block or page at Notion it goes under.
    #[serde(default)]
    pub parent: Option<Uuid>,
    /// The block of this request it goes under.
    #[serde(default)]
    pub parent_block: Option<Uuid>,
    /// The block at Notion it goes after.
    #[serde(default)]
    pub after: Option<Uuid>,
    /// The block of this request it goes after.
    #[serde(default)]
    pub after_block: Option<Uuid>,
    /// When Notion last edited the block as of the last sync.
    #[serde(default)]
    pub base_edited: Option<DateTime<Utc>>,
    /// The hash of what is written, in hexadecimal.
    pub hash: String,
}

impl StoredItem {
    fn of(seq: usize, item: &super::push::Item) -> Self {
        Self {
            seq: u32::try_from(seq).unwrap_or(u32::MAX),
            action: item.action.as_str().to_owned(),
            bid: item.bid,
            kind: item.kind.as_str().to_owned(),
            external: item.external,
            parent: item.parent,
            parent_block: item.parent_block,
            after: item.after,
            after_block: item.after_block,
            base_edited: item.base_edited,
            hash: hash_hex(&item.hash),
        }
    }

    /// The change as the connectors module takes it.
    fn for_connector(&self) -> Option<StoredPageItem> {
        let kind = match self.action.as_str() {
            "update" => PageItemKind::Update,
            "append" => PageItemKind::Append,
            "delete" => PageItemKind::Delete,
            _ => return None,
        };
        Some(StoredPageItem {
            kind,
            block: self.bid,
            external: self.external,
            parent: self.parent,
            parent_block: self.parent_block,
            after: self.after,
            after_block: self.after_block,
            base_edited_at: self.base_edited,
        })
    }
}

/// What a request's binding covers.
struct Bound<'a> {
    workspace: WorkspaceId,
    request: Uuid,
    page: PageId,
    connection: Uuid,
    notion_page: Uuid,
    items: &'a [StoredItem],
    body: &'a BlobRef,
}

impl Bound<'_> {
    fn fields(&self) -> Fields {
        let mut fields = Fields::new(NOTION_REQUEST);
        fields
            .uuid(self.workspace.as_uuid())
            .uuid(&self.request)
            .uuid(self.page.as_uuid())
            .uuid(&self.connection)
            .uuid(&self.notion_page)
            .count(self.items.len());
        for item in self.items {
            fields
                .uint(u64::from(item.seq))
                .str(&item.action)
                .uuid(&item.bid)
                .str(&item.kind)
                .opt_uuid(item.external.as_ref())
                .opt_uuid(item.parent.as_ref())
                .opt_uuid(item.parent_block.as_ref())
                .opt_uuid(item.after.as_ref())
                .opt_uuid(item.after_block.as_ref())
                .opt_str(
                    item.base_edited
                        .map(|time| time.timestamp_micros().to_string())
                        .as_deref(),
                )
                .str(&item.hash);
        }
        fields
            .bytes(self.body.hash.as_bytes())
            .uint(self.body.size)
            .str(&self.body.content_type);
        fields
    }
}

/// Store `plan` as a request of `page`'s link, bound and waiting for the
/// connectors module, and announce it. `by` is the member whose Notion
/// account it is. Returns the request's id.
pub async fn create(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    page: PageId,
    link: &super::Link,
    by: UserId,
    plan: &Plan,
) -> Result<Uuid> {
    let key = workspace.to_string();
    let bytes = serde_json::to_vec(&plan.document).map_err(campbooks_kernel::Error::internal)?;
    let body = state.blobs.put(&key, bytes, CHANGES_CONTENT_TYPE).await?;
    let request = Uuid::now_v7();
    let items: Vec<StoredItem> = plan
        .items
        .iter()
        .enumerate()
        .map(|(seq, item)| StoredItem::of(seq, item))
        .collect();
    let mac = state.binding.tag(
        &Bound {
            workspace,
            request,
            page,
            connection: link.connection_id,
            notion_page: link.notion_page_id,
            items: &items,
            body: &body,
        }
        .fields(),
    );
    sqlx::query(
        "INSERT INTO pages.notion_requests
             (id, page_id, workspace_id, connection_id, notion_page_id, items, body, mac,
              created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(request)
    .bind(page)
    .bind(workspace.as_uuid())
    .bind(link.connection_id)
    .bind(link.notion_page_id)
    .bind(Json(&items))
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
            &NotionChangesRequested {
                workspace: key,
                request,
                page,
                connection: link.connection_id,
                notion_page: link.notion_page_id,
                body,
            },
        )
        .await?;
    Ok(request)
}

/// The items of request `request`, as stored.
pub async fn items_of(conn: &mut PgConnection, request: Uuid) -> Result<Option<Vec<StoredItem>>> {
    let row: Option<(Json<Vec<StoredItem>>,)> =
        sqlx::query_as("SELECT items FROM pages.notion_requests WHERE id = $1")
            .bind(request)
            .fetch_optional(conn)
            .await?;
    Ok(row.map(|(items,)| items.0))
}

#[derive(Debug, Clone, Copy)]
struct StoredRequests;

/// The port, for the server to register.
pub fn page_requests() -> PageRequestsPort {
    Arc::new(StoredRequests)
}

type Row = (
    PageId,
    Uuid,
    Uuid,
    Json<Vec<StoredItem>>,
    Json<BlobRef>,
    Option<Vec<u8>>,
);

impl PageRequests for StoredRequests {
    fn table_link_standing<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        table: Uuid,
        connection: Uuid,
        external: Uuid,
    ) -> LinkStandingFuture<'a> {
        Box::pin(async move {
            super::tables::requests::standing(conn, PageId::from_uuid(table), connection, external)
                .await
        })
    }

    fn stored_table_changes<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        request: Uuid,
    ) -> campbooks_kernel::outbound::StoredTableChangesFuture<'a> {
        Box::pin(
            async move { super::tables::requests::stored(state, conn, workspace, request).await },
        )
    }

    fn stored_changes<'a>(
        &'a self,
        state: &'a AppState,
        conn: &'a mut PgConnection,
        workspace: WorkspaceId,
        request: Uuid,
    ) -> StoredPageChangesFuture<'a> {
        Box::pin(async move {
            let Some((page, connection, notion_page, items, body, mac)) = sqlx::query_as::<_, Row>(
                "SELECT page_id, connection_id, notion_page_id, items, body, mac
                       FROM pages.notion_requests
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
                page,
                connection,
                notion_page,
                items: &items,
                body: &body,
            };
            if !state.binding.verify(&bound.fields(), mac.as_deref()) {
                tracing::warn!(
                    %workspace,
                    %request,
                    "a stored Notion request is not bound to the request that made it; it is \
                     not made"
                );
                return Ok(None);
            }
            // The page is still kept in step with that Notion page.
            let standing: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pages.external_links
                                 WHERE page_id = $1 AND connection_id = $2
                                   AND notion_page_id = $3 AND state = 'syncing')",
            )
            .bind(page)
            .bind(connection)
            .bind(notion_page)
            .fetch_one(&mut *conn)
            .await?;
            if !standing {
                return Ok(None);
            }
            let for_connector: Option<Vec<StoredPageItem>> =
                items.iter().map(StoredItem::for_connector).collect();
            Ok(for_connector.map(|items| StoredPageChanges {
                page: *page.as_uuid(),
                connection,
                external_page: notion_page,
                items,
                body,
            }))
        })
    }

    fn link_standing<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        page: Uuid,
        connection: Uuid,
        external_page: Uuid,
    ) -> LinkStandingFuture<'a> {
        Box::pin(async move {
            Ok(sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pages.external_links
                                 WHERE page_id = $1 AND connection_id = $2
                                   AND notion_page_id = $3 AND state = 'syncing')",
            )
            .bind(page)
            .bind(connection)
            .bind(external_page)
            .fetch_one(conn)
            .await?)
        })
    }
}

#[cfg(test)]
mod tests;
