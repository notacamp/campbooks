//! Shared fixtures for the sync's tests, and the names a link goes by.

use chrono::{DateTime, Utc};
use uuid::Uuid;
use yrs::{Doc, Transact};

use super::{
    LinkState, StopReason, edit,
    edit::Place,
    merge::{self, Mapping, Merged},
    model::{Block, Kind, Run, Snapshot},
    tree::{self, Resolver, Tree},
};
use crate::domain::document;

/// A fixed identifier.
pub fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// A time `minutes` after the tests' start.
pub fn at(minutes: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_792_918_800 + minutes * 60, 0).unwrap_or_default()
}

/// A Notion block `n` of `kind` with `text`, last edited at minute `edited`.
pub fn block(n: u128, kind: Kind, text: &str, edited: i64) -> Block {
    let mut block = Block::new(id(n), kind);
    if !text.is_empty() {
        block.runs = vec![Run::plain(text)];
    }
    block.edited = Some(at(edited));
    if kind == Kind::Heading {
        block.level = Some(2);
    }
    if kind == Kind::Todo {
        block.checked = Some(false);
    }
    block
}

/// A Notion paragraph.
pub fn para(n: u128, text: &str, edited: i64) -> Block {
    block(n, Kind::Paragraph, text, edited)
}

/// A snapshot read at minute `read` (the page edited then too).
pub fn snapshot(blocks: Vec<Block>, read: i64, complete: bool) -> Snapshot {
    Snapshot {
        version: 1,
        notion_page: id(0xAA),
        title: "Product roadmap".to_owned(),
        edited: at(read),
        read_at: at(read),
        complete,
        blocks,
    }
}

/// What reading a body needs to know, with the app at a demo address.
pub fn resolver() -> Resolver {
    Resolver {
        app: "https://app.example.com/w/acme".to_owned(),
        this: id(0xBB),
        ..Resolver::default()
    }
}

/// A body kept in step with a Notion page: the document and what the last
/// sync agreed.
pub struct Synced {
    pub doc: Doc,
    pub mappings: Vec<Mapping>,
    pub resolver: Resolver,
}

impl Synced {
    /// An empty page not synced yet.
    pub fn new() -> Self {
        Self {
            doc: Doc::new(),
            mappings: Vec::new(),
            resolver: resolver(),
        }
    }

    /// Apply `snapshot` and keep the mappings it made.
    pub fn apply(&mut self, snapshot: &Snapshot) -> Merged {
        let merged = merge::merge_into(&self.doc, snapshot, &self.resolver, self.mappings.clone())
            .expect("the body holds the result");
        for upsert in &merged.outcome.upserts {
            self.mappings
                .retain(|m| m.bid != upsert.bid && m.notion_block != upsert.notion_block);
            self.mappings.push(upsert.clone());
        }
        for gone in &merged.outcome.deletes {
            self.mappings.retain(|m| m.bid != *gone);
        }
        merged
    }

    /// The body, read.
    pub fn tree(&self) -> Tree {
        let root = self.doc.get_or_insert_xml_fragment(document::ROOT);
        let txn = self.doc.transact();
        tree::read(&txn, &root, &self.resolver)
    }

    /// The text of every block, in order (conflicts are not blocks).
    pub fn texts(&self) -> Vec<String> {
        self.tree()
            .all()
            .into_iter()
            .map(|node| node.block.text())
            .collect()
    }

    /// The kind of every block, in order.
    pub fn kinds(&self) -> Vec<Kind> {
        self.tree()
            .all()
            .into_iter()
            .map(|node| node.block.kind)
            .collect()
    }

    /// The stable id of the block whose text is `text`.
    pub fn bid_of(&self, text: &str) -> Uuid {
        self.tree()
            .all()
            .into_iter()
            .find(|node| node.block.text() == text)
            .and_then(|node| node.bid)
            .unwrap_or_else(|| panic!("no block says {text:?}"))
    }

    /// A member changes the words of block `bid`, in place.
    pub fn edit_text(&self, bid: Uuid, text: &str) {
        let root = self.doc.get_or_insert_xml_fragment(document::ROOT);
        let kind = self
            .tree()
            .find(bid)
            .map(|node| node.block.kind)
            .expect("the block is there");
        let mut block = Block::new(bid, kind);
        block.runs = vec![Run::plain(text)];
        let mut txn = self.doc.transact_mut();
        let found = edit::locate(&txn, &Place::Root(root), bid).expect("the block is there");
        assert!(edit::rewrite(&mut txn, &found, &block, &self.resolver));
    }

    /// A member deletes block `bid` (with what is under it).
    pub fn delete(&self, bid: Uuid) {
        let root = self.doc.get_or_insert_xml_fragment(document::ROOT);
        let mut txn = self.doc.transact_mut();
        assert!(edit::remove(&mut txn, &root, bid));
    }
}

#[test]
fn every_stop_reason_has_a_name_the_database_accepts() {
    for (reason, name) in [
        (StopReason::StoppedByMember, "stopped_by_member"),
        (StopReason::Disconnected, "disconnected"),
        (StopReason::TrashedHere, "trashed_here"),
        (StopReason::TrashedInNotion, "trashed_in_notion"),
        (StopReason::NotShared, "not_shared"),
        (StopReason::MemberLeft, "member_left"),
        (StopReason::NotionUnavailable, "notion_unavailable"),
    ] {
        assert_eq!(reason.as_str(), name);
        let json = serde_json::to_value(reason).unwrap();
        assert_eq!(json, name);
    }
}

#[test]
fn a_link_shows_its_state_and_reason() {
    let link = |state: &str, reason: Option<&str>| super::Link {
        page_id: crate::domain::PageId::new(),
        connection_id: id(1),
        notion_page_id: id(2),
        state: state.to_owned(),
        stopped_reason: reason.map(ToOwned::to_owned),
        created_by: campbooks_kernel::tenancy::UserId::new(),
        conflicts: 2,
        last_inbound_at: None,
        last_outbound_at: None,
        last_read_at: None,
        last_title: None,
    };
    let syncing = link("syncing", None);
    assert!(syncing.syncing());
    assert_eq!(syncing.view().state, LinkState::Syncing);
    assert_eq!(syncing.view().conflicts, 2);
    let paused = link("paused", None);
    assert!(!paused.syncing());
    assert_eq!(paused.view().state, LinkState::Paused);
    let stopped = link("stopped", Some("not_shared"));
    assert_eq!(stopped.view().state, LinkState::Stopped);
    assert_eq!(stopped.view().stopped_reason, Some(StopReason::NotShared));
    // A reason the code does not know shows as none.
    assert_eq!(link("stopped", Some("odd")).view().stopped_reason, None);
}
