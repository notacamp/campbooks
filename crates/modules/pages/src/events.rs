//! The events this module publishes (ADR 0012), and the other modules'
//! events it consumes, as their published contracts describe them (modules
//! never import each other).
//!
//! **No page content.** `pages.page_changed` holds ids and enums: never a
//! title, never any text, never an address.

use campbooks_kernel::{DomainEvent, RecordRef, blobs::BlobRef, changes};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::{PageId, Place, resources::PAGE_RECORD};

/// What changed in a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PageChangeKind {
    /// Its title or icon, or it was made.
    Meta,
    /// Its body.
    Body,
    /// Who it is shared with.
    Share,
    /// Its parent, place or position.
    Move,
    /// It went to the trash, or came back.
    Trash,
    /// It was deleted for good.
    Purge,
    /// A table's properties or options changed.
    Schema,
    /// A row's values changed.
    Values,
    /// A table's views changed.
    Views,
}

/// `pages.page_changed` v1 (`contracts/pages.page_changed.v1.schema.json`):
/// a page was made or changed. A snapshot, so a consumer applies only newer
/// revisions and stays order-proof. Body changes are announced at most once
/// every 5 seconds per page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PageChanged {
    /// The workspace.
    pub workspace: String,
    /// The page.
    pub page: PageId,
    /// Grows with every change of this page.
    pub revision: i64,
    /// Its parent.
    pub parent: Option<PageId>,
    /// Private or workspace.
    pub place: Place,
    /// Whether it is in the trash.
    pub trashed: bool,
    /// What changed.
    pub kinds: Vec<PageChangeKind>,
    /// Whether the page is a row of a table (its parent): a row is followed
    /// through its table.
    #[serde(default)]
    pub row: bool,
}

impl PageChanged {
    /// Whether the page is gone for good.
    pub fn purged(&self) -> bool {
        self.kinds.contains(&PageChangeKind::Purge)
    }
}

impl DomainEvent for PageChanged {
    const NAME: &'static str = "pages.page_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    /// The page changed, for whoever may read it: it lives in a container of
    /// its own, `pages.page:<id>`, whose visibility the module answers
    /// (a page's readers follow its place, its ancestors and its grants).
    fn changes(&self) -> Vec<changes::Change> {
        let page = RecordRef::new(PAGE_RECORD, &self.page);
        // A row has no container of its own: it lives in its table's, so a
        // table of 20,000 rows is one container to the change feed.
        let within = match self.parent.filter(|_| self.row) {
            Some(table) => RecordRef::new(PAGE_RECORD, &table),
            None => page.clone(),
        };
        if self.purged() {
            vec![changes::Change::delete(page).within(within)]
        } else {
            vec![changes::Change::upsert(page).within(within)]
        }
    }
}

/// What changed in a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TableChangeKind {
    /// Its properties or options.
    Schema,
    /// Its views.
    Views,
    /// A row was added, trashed, restored or deleted for good.
    Rows,
}

/// `pages.table_changed` v1 (`contracts/pages.table_changed.v1.schema.json`):
/// a table's schema, views or set of rows changed, so a view that is open
/// asks for the table again. A snapshot: `revision` grows with every change
/// of the table, and a consumer applies only newer ones. Ids and enums only:
/// never a property's name, an option, a view's name or any value. (A cell
/// edit is announced as `pages.page_changed` of its row, kind `values`.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TableChanged {
    /// The workspace.
    pub workspace: String,
    /// The table (a page).
    pub table: PageId,
    /// Grows with every change of the table.
    pub revision: i64,
    /// What changed.
    pub kinds: Vec<TableChangeKind>,
}

impl DomainEvent for TableChanged {
    const NAME: &'static str = "pages.table_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    /// The table changed, for whoever may read it: the same record and
    /// container as the page itself, so a stream that follows the page
    /// follows its table.
    fn changes(&self) -> Vec<changes::Change> {
        let page = RecordRef::new(PAGE_RECORD, &self.table);
        vec![changes::Change::upsert(page.clone()).within(page)]
    }
}

/// How a [`BlobRef`] looks on the wire, for the events catalogue.
#[derive(ToSchema)]
#[schema(as = BlobRef)]
#[allow(dead_code, reason = "a schema description only, never built")]
pub struct BlobSchema {
    /// SHA-256 of the bytes, 64 lower-case hex digits.
    hash: String,
    /// Length in bytes.
    size: u64,
    /// The media type given when it was stored.
    content_type: String,
}

/// `pages.notion_link_requested` v1
/// (`contracts/pages.notion_link_requested.v1.schema.json`): a page was
/// linked to a page in a member's Notion account, which now needs reading.
/// The connectors module asks this module, through the kernel's
/// page-requests port, whether the link stands before it makes the stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionLinkRequested {
    /// The workspace.
    pub workspace: String,
    /// The page.
    pub page: PageId,
    /// The Notion connection it is read through (a connectors id).
    pub connection: Uuid,
    /// The Notion page, or the Notion database (data source) for a table.
    pub notion_page: Uuid,
    /// What is linked: a page (the default) or a table.
    #[serde(default)]
    pub kind: LinkKind,
}

/// What a Notion link links.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// A page, kept in step block by block.
    #[default]
    Page,
    /// A table, kept in step with a Notion database (a data source).
    DataSource,
}

impl DomainEvent for NotionLinkRequested {
    const NAME: &'static str = "pages.notion_link_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `pages.notion_link_ended` v1
/// (`contracts/pages.notion_link_ended.v1.schema.json`): a page stopped
/// being kept in step with its Notion page, so nothing reads it any more.
/// Nothing is deleted at Notion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionLinkEnded {
    /// The workspace.
    pub workspace: String,
    /// The page.
    pub page: PageId,
    /// The Notion connection it was read through.
    pub connection: Uuid,
    /// The Notion page.
    pub notion_page: Uuid,
}

impl DomainEvent for NotionLinkEnded {
    const NAME: &'static str = "pages.notion_link_ended";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `pages.notion_changes_requested` v1
/// (`contracts/pages.notion_changes_requested.v1.schema.json`): block
/// changes a member's edits asked for, to be made at Notion. Ids and a blob
/// reference only: what to write is in the blob, which this module stored
/// and which the connectors module reads through the page-requests port,
/// never from the event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionChangesRequested {
    /// The workspace.
    pub workspace: String,
    /// The request.
    pub request: Uuid,
    /// The page.
    pub page: PageId,
    /// The Notion connection whose grant makes the changes.
    pub connection: Uuid,
    /// The Notion page.
    pub notion_page: Uuid,
    /// The `notion-block-changes` v1 document, a blob of the workspace.
    #[schema(value_type = BlobSchema)]
    pub body: BlobRef,
}

impl DomainEvent for NotionChangesRequested {
    const NAME: &'static str = "pages.notion_changes_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `pages.notion_table_changes_requested` v1
/// (`contracts/pages.notion_table_changes_requested.v1.schema.json`): table
/// changes a member's edits asked for, to be made at Notion. Ids and a blob
/// reference only: what to write is in the blob, which this module stored and
/// which the connectors module reads through the page-requests port, never
/// from the event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionTableChangesRequested {
    /// The workspace.
    pub workspace: String,
    /// The request.
    pub request: Uuid,
    /// The table.
    pub table: PageId,
    /// The Notion connection whose grant makes the changes.
    pub connection: Uuid,
    /// The database (data source) at Notion.
    pub data_source: Uuid,
    /// The `notion-table-changes` v1 document, a blob of the workspace.
    #[schema(value_type = BlobSchema)]
    pub body: BlobRef,
}

impl DomainEvent for NotionTableChangesRequested {
    const NAME: &'static str = "pages.notion_table_changes_requested";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `connectors.notion_data_source_read` v1, as this module reads it
/// (`contracts/connectors.notion_data_source_read.v1.schema.json`): a Notion
/// database was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionDataSourceRead {
    /// The workspace.
    pub workspace: String,
    /// The Notion connection.
    pub connection: Uuid,
    /// The data source.
    pub data_source: Uuid,
    /// The table it is linked to.
    pub table: PageId,
    /// The `notion-data-source-snapshot` v1 document, a blob of the
    /// workspace.
    pub snapshot: BlobRef,
    /// When it was read.
    pub read_at: DateTime<Utc>,
}

impl DomainEvent for NotionDataSourceRead {
    const NAME: &'static str = "connectors.notion_data_source_read";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// How one table change of a request ended, as the connector tells it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionTableItemResult {
    /// The column or row (an id of this module).
    pub subject: Uuid,
    /// The kind of change.
    pub kind: String,
    /// How it ended.
    pub outcome: NotionOutcome,
    /// The column's or row's id at Notion (applied).
    #[serde(default)]
    pub external: Option<String>,
    /// When Notion last edited it after the write (applied).
    #[serde(default)]
    pub edited_at: Option<DateTime<Utc>>,
    /// A hash of what Notion holds now (applied).
    #[serde(default)]
    pub hash: Option<String>,
}

/// `connectors.notion_table_changes_completed` v1, as this module reads it
/// (`contracts/connectors.notion_table_changes_completed.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionTableChangesCompleted {
    /// The workspace.
    pub workspace: String,
    /// The request.
    pub request: Uuid,
    /// The table.
    pub table: PageId,
    /// How each item ended, in the request's order.
    pub results: Vec<NotionTableItemResult>,
}

impl DomainEvent for NotionTableChangesCompleted {
    const NAME: &'static str = "connectors.notion_table_changes_completed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `connectors.notion_page_read` v1, as this module reads it
/// (`contracts/connectors.notion_page_read.v1.schema.json`): a Notion page's
/// blocks, read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionPageRead {
    /// The workspace.
    pub workspace: String,
    /// The Notion connection.
    pub connection: Uuid,
    /// The Notion page.
    pub notion_page: Uuid,
    /// The page it is linked to.
    pub page: PageId,
    /// The `notion-page-snapshot` v1 document, a blob of the workspace.
    pub snapshot: BlobRef,
    /// When it was read.
    pub read_at: DateTime<Utc>,
}

impl DomainEvent for NotionPageRead {
    const NAME: &'static str = "connectors.notion_page_read";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// Why a Notion page is no longer read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotionStop {
    /// Notion no longer shares the page with the integration.
    NotShared,
    /// The page went to the trash in Notion.
    TrashedInNotion,
    /// Notion is not offered on this instance any more.
    NotionUnavailable,
}

/// `connectors.notion_link_stopped` v1, as this module reads it
/// (`contracts/connectors.notion_link_stopped.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionLinkStopped {
    /// The workspace.
    pub workspace: String,
    /// The Notion connection.
    pub connection: Uuid,
    /// The Notion page.
    pub notion_page: Uuid,
    /// The page it was linked to.
    pub page: PageId,
    /// Why.
    pub reason: NotionStop,
}

impl DomainEvent for NotionLinkStopped {
    const NAME: &'static str = "connectors.notion_link_stopped";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// How one block change of a request ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotionOutcome {
    /// Written at Notion.
    Applied,
    /// Nothing was written.
    Skipped,
    /// It failed.
    Failed,
}

/// How one block change of a request ended, as the connector tells it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionItemResult {
    /// The block (an id of this module).
    pub block: Uuid,
    /// How it ended.
    pub outcome: NotionOutcome,
    /// The block's id at Notion (applied).
    #[serde(default)]
    pub notion_block: Option<Uuid>,
    /// When Notion last edited it after the write (applied).
    #[serde(default)]
    pub edited_at: Option<DateTime<Utc>>,
}

/// `connectors.notion_changes_completed` v1, as this module reads it
/// (`contracts/connectors.notion_changes_completed.v1.schema.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotionChangesCompleted {
    /// The workspace.
    pub workspace: String,
    /// The request.
    pub request: Uuid,
    /// The page.
    pub page: PageId,
    /// How each item ended, in the request's order.
    pub results: Vec<NotionItemResult>,
}

impl DomainEvent for NotionChangesCompleted {
    const NAME: &'static str = "connectors.notion_changes_completed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `connectors.connection_removed` v1, as this module reads it: a connection
/// was disconnected, or its member left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionRemoved {
    /// The workspace.
    pub workspace: String,
    /// The connection.
    pub connection: Uuid,
}

impl DomainEvent for ConnectionRemoved {
    const NAME: &'static str = "connectors.connection_removed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.member_left`: a member was removed or left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberLeft {
    /// The workspace.
    pub workspace: String,
    /// The former member.
    pub user: campbooks_kernel::tenancy::UserId,
}

impl DomainEvent for MemberLeft {
    const NAME: &'static str = "workspaces.member_left";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

/// `workspaces.workspace_deleting`: the workspace is gone at once and its
/// data is purged at `purge_after`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDeleting {
    /// The workspace.
    pub workspace: String,
    /// When its data goes.
    pub purge_after: DateTime<Utc>,
}

impl DomainEvent for WorkspaceDeleting {
    const NAME: &'static str = "workspaces.workspace_deleting";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::changes::ChangeOp;
    use serde_json::json;

    use super::*;

    fn event(kinds: &[&str]) -> PageChanged {
        serde_json::from_value(json!({
            "workspace": "w", "page": "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01",
            "revision": 3, "parent": null, "place": "workspace", "trashed": false,
            "kinds": kinds,
        }))
        .unwrap()
    }

    #[test]
    fn a_change_is_in_the_pages_own_container() {
        let changed = event(&["meta"]).changes();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].op, ChangeOp::Upsert);
        assert_eq!(changed[0].container, Some(changed[0].record.clone()));
    }

    #[test]
    fn a_purge_is_a_delete() {
        assert_eq!(event(&["purge"]).changes()[0].op, ChangeOp::Delete);
    }

    #[test]
    fn the_payload_has_no_title_or_text() {
        let value = serde_json::to_value(event(&["body"])).unwrap();
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for key in keys {
            assert!(
                [
                    "workspace",
                    "page",
                    "revision",
                    "parent",
                    "place",
                    "trashed",
                    "kinds",
                    "row"
                ]
                .contains(&key)
            );
        }
    }
}
