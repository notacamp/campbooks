//! What a member's edits ask of Notion (campbooks-cloud#376): the blocks
//! that changed here since the last sync, planned from the body and the
//! base the last sync agreed.
//!
//! - A mapped block whose content differs from its base is an **update**
//!   (unless a conflict about it waits for the member: it is not sent until
//!   they choose).
//! - A block with no counterpart at Notion is an **append**, under the block
//!   it is under and after the block before it, in document order, so a
//!   block's parent and its predecessor are made first.
//! - A mapped block that is no longer in the body is a **delete**, unless it
//!   is a sub-page or a block kept in Notion: a page is never removed from
//!   here.
//!
//! Blocks kept in Notion, sub-page links and records of Campbooks shown in
//! the page are never sent. A file or an image made here goes as a
//! paragraph that names it and links to the page: its bytes stay here.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use yrs::{Doc, Transact};

use super::{
    edit,
    merge::Mapping,
    model::{Block, ChangeDocument, ChangeItem, Kind, Run, hash},
    tree::{self, Node, Resolver, Tree},
};
use crate::domain::document::{self, Inspection, Rejection};

/// What a change does at Notion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Do {
    /// Rewrite a block that is there.
    Update,
    /// Add a block.
    Append,
    /// Remove a block the member deleted.
    Delete,
}

impl Do {
    /// The name kept in the request.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Update => "update",
            Self::Append => "append",
            Self::Delete => "delete",
        }
    }
}

/// One change planned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What to do.
    pub action: Do,
    /// The block in the body.
    pub bid: Uuid,
    /// What kind of block it is.
    pub kind: Kind,
    /// The block at Notion (update, delete).
    pub external: Option<Uuid>,
    /// The block or page at Notion it goes under (append).
    pub parent: Option<Uuid>,
    /// The block of this request it goes under (append).
    pub parent_block: Option<Uuid>,
    /// The block at Notion it goes after (append).
    pub after: Option<Uuid>,
    /// The block of this request it goes after (append).
    pub after_block: Option<Uuid>,
    /// When Notion last edited the block as of the last sync.
    pub base_edited: Option<DateTime<Utc>>,
    /// The hash of the content sent (update, append).
    pub hash: [u8; 32],
}

/// A plan: the changes, and the document with what to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The changes, in the order they are made.
    pub items: Vec<Item>,
    /// What to write for them.
    pub document: ChangeDocument,
}

/// A body read for the plan: every block has a stable id.
#[derive(Debug)]
pub struct Prepared {
    /// The body, read.
    pub tree: Tree,
    /// How many blocks were given a stable id.
    pub ids_given: usize,
    /// The body after, read against the schema.
    pub inspection: Inspection,
    /// The update to store, when blocks were given an id.
    pub update: Option<Vec<u8>>,
}

/// Give the blocks of `doc` the stable ids they lack and read the body.
pub fn prepare(doc: &Doc, resolver: &Resolver) -> Result<Prepared, Rejection> {
    let before = document::state_vector(doc);
    let root = doc.get_or_insert_xml_fragment(document::ROOT);
    let ids_given = {
        let mut txn = doc.transact_mut();
        edit::assign_ids(&mut txn, &root, &mut Uuid::now_v7)
    };
    let inspection = document::inspect(doc)?;
    let tree = {
        let txn = doc.transact();
        tree::read(&txn, &root, resolver)
    };
    let update = if ids_given > 0 {
        Some(document::diff_since(doc, &before)?)
    } else {
        None
    };
    Ok(Prepared {
        tree,
        ids_given,
        inspection,
        update,
    })
}

/// Where the blocks being walked are.
#[derive(Debug, Clone, Copy)]
enum Under {
    Page,
    Mapped(Uuid),
    New(Uuid),
}

/// The block before: made at Notion already, or made earlier in the plan.
#[derive(Debug, Clone, Copy)]
enum Before {
    Nothing,
    Mapped(Uuid),
    New(Uuid),
}

struct Planner<'a> {
    tree: &'a Tree,
    by_bid: HashMap<Uuid, &'a Mapping>,
    link: Option<String>,
    items: Vec<Item>,
    blocks: Vec<Option<Block>>,
}

/// Plan what `tree` (the body) asks of Notion, given `mappings`. `link` is
/// the address of the page in the app, for the paragraph a file or an image
/// becomes.
pub fn plan(tree: &Tree, mappings: &[Mapping], link: Option<String>) -> Plan {
    let mut planner = Planner {
        tree,
        by_bid: mappings.iter().map(|m| (m.bid, m)).collect(),
        link,
        items: Vec::new(),
        blocks: Vec::new(),
    };
    planner.walk(&tree.nodes, Under::Page);
    let present: HashSet<Uuid> = tree.all().into_iter().filter_map(|n| n.bid).collect();
    let mut gone: Vec<&Mapping> = mappings
        .iter()
        .filter(|m| !present.contains(&m.bid))
        .filter(|m| !matches!(m.kind, Kind::PageLink | Kind::External))
        .collect();
    gone.sort_by_key(|m| m.bid);
    for mapping in gone {
        planner.items.push(Item {
            action: Do::Delete,
            bid: mapping.bid,
            kind: mapping.kind,
            external: Some(mapping.notion_block),
            parent: None,
            parent_block: None,
            after: None,
            after_block: None,
            base_edited: Some(mapping.base_edited),
            hash: mapping.base_hash,
        });
        planner.blocks.push(None);
    }
    let items = planner
        .blocks
        .into_iter()
        .enumerate()
        .map(|(at, block)| ChangeItem {
            seq: u32::try_from(at).unwrap_or(u32::MAX),
            block,
        })
        .collect();
    Plan {
        items: planner.items,
        document: ChangeDocument { version: 1, items },
    }
}

impl Planner<'_> {
    fn walk(&mut self, nodes: &[Node], under: Under) {
        let mut before = Before::Nothing;
        for node in nodes {
            let Some(bid) = node.bid else {
                continue;
            };
            if let Some(mapping) = self.by_bid.get(&bid).copied() {
                self.update(node, bid, mapping);
                before = Before::Mapped(mapping.notion_block);
                self.walk(&node.children, Under::Mapped(mapping.notion_block));
                continue;
            }
            let Some(block) = self.sendable(node, bid) else {
                continue;
            };
            let (parent, parent_block) = match under {
                Under::Page => (None, None),
                Under::Mapped(id) => (Some(id), None),
                Under::New(parent) => (None, Some(parent)),
            };
            let (after, after_block) = match before {
                Before::Nothing => (None, None),
                Before::Mapped(id) => (Some(id), None),
                Before::New(block) => (None, Some(block)),
            };
            self.items.push(Item {
                action: Do::Append,
                bid,
                kind: block.kind,
                external: None,
                parent,
                parent_block,
                after,
                after_block,
                base_edited: None,
                hash: hash(&block),
            });
            self.blocks.push(Some(block));
            before = Before::New(bid);
            self.walk(&node.children, Under::New(bid));
        }
    }

    /// A mapped block changed here since the last sync.
    fn update(&mut self, node: &Node, bid: Uuid, mapping: &Mapping) {
        if self.tree.conflict_of(bid).is_some()
            || !node.block.kind.editable()
            || !mapping.kind.editable()
        {
            return;
        }
        let now = hash(&node.block);
        if now == mapping.base_hash {
            return;
        }
        let mut block = node.block.clone();
        block.id = bid;
        self.items.push(Item {
            action: Do::Update,
            bid,
            kind: block.kind,
            external: Some(mapping.notion_block),
            parent: None,
            parent_block: None,
            after: None,
            after_block: None,
            base_edited: Some(mapping.base_edited),
            hash: now,
        });
        self.blocks.push(Some(block));
    }

    /// The block as it is sent when it is new: a block of a kind Notion
    /// takes, or a paragraph for a file or an image. `None` for a block
    /// that is never sent.
    fn sendable(&self, node: &Node, bid: Uuid) -> Option<Block> {
        let mut block = node.block.clone();
        block.id = bid;
        block.children = Vec::new();
        match block.kind {
            kind if kind.editable() || kind == Kind::Divider => Some(block),
            Kind::Image | Kind::File => {
                let name = block
                    .file
                    .as_ref()
                    .map_or_else(|| "File".to_owned(), |file| file.name.clone());
                let mut paragraph = Block::new(bid, Kind::Paragraph);
                paragraph.runs = vec![Run {
                    text: name,
                    link: self.link.clone(),
                    ..Run::default()
                }];
                Some(paragraph)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
