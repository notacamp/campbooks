//! The events this module publishes (ADR 0012), and the other modules'
//! events it consumes, as their published contracts describe them (modules
//! never import each other).
//!
//! **No page content.** `pages.page_changed` holds ids and enums: never a
//! title, never any text, never an address.

use campbooks_kernel::{DomainEvent, RecordRef, changes};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
        if self.purged() {
            vec![changes::Change::delete(page.clone()).within(page)]
        } else {
            vec![changes::Change::upsert(page.clone()).within(page)]
        }
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
                    "kinds"
                ]
                .contains(&key)
            );
        }
    }
}
