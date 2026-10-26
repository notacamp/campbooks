//! Applying a Notion snapshot to a page's body, block by block
//! (campbooks-cloud#376).
//!
//! Each block that is the same block at Notion is compared three ways: ours
//! now, Notion's now, and the base agreed at the last sync (a hash of the
//! content, and the time Notion last edited it). Per block:
//!
//! - **Notion changed it alone**: Notion's content is written into our
//!   block, in place.
//! - **We changed it alone**: it stays; the outbound path sends it.
//! - **Neither**: nothing.
//! - **Both**, to different content: our block stays where it is and Notion's
//!   version is put beneath it as a `conflict`, for the member to choose;
//!   neither is sent until they do. The same content on both sides is no
//!   conflict.
//!
//! A block Notion has that we have not is inserted at its place (after the
//! block before it), with the blocks under it. A block we have that is gone
//! from Notion is removed, unless it (or something under it) changed here:
//! then it stays, with a conflict that says Notion removed it. Nothing is
//! removed from a snapshot that is not complete. A block is never moved.
//!
//! Only blocks of the snapshot are touched, one by one, in the document's
//! own transaction: what other people wrote in other blocks stays.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use uuid::Uuid;
use yrs::{Doc, TransactionMut, XmlElementRef, XmlFragmentRef};

use super::{
    edit::{self, Built, Place},
    model::{Block, FileRef, Kind, Snapshot, hash},
    tree::{self, Node, Resolver, Tree},
};
use crate::domain::document::{self, Inspection, Rejection};

/// What the last sync agreed about a block that is the same block at Notion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    /// The block's stable id in the body.
    pub bid: Uuid,
    /// Its id at Notion.
    pub notion_block: Uuid,
    /// What kind of block it was.
    pub kind: Kind,
    /// The hash of its content when it was last in step.
    pub base_hash: [u8; 32],
    /// When Notion last edited it as of then.
    pub base_edited: DateTime<Utc>,
}

/// What applying a snapshot did, to be stored.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Mappings to insert or update.
    pub upserts: Vec<Mapping>,
    /// Blocks whose mapping goes.
    pub deletes: Vec<Uuid>,
    /// Blocks written into the body from Notion (inserted, rewritten,
    /// removed).
    pub touched: usize,
    /// Conflicts raised that were not there before.
    pub raised: usize,
    /// Some block changed here and not at Notion: it is for the outbound
    /// path to send.
    pub ours_pending: bool,
    /// The files the blocks written name, to be held by the page.
    pub files: Vec<FileRef>,
}

/// Where the blocks being compared are.
#[derive(Debug, Clone, Copy)]
enum Parent {
    Root,
    Block(Uuid),
}

/// Where a new block goes.
enum Slot {
    /// A new element (or list) at this index of this place.
    At(Place, u32),
    /// A new item at this index of this list.
    InList(XmlElementRef, u32),
}

/// A merge in progress.
pub struct Merge<'a> {
    snapshot: &'a Snapshot,
    resolver: &'a Resolver,
    ours: &'a Tree,
    root: XmlFragmentRef,
    by_notion: HashMap<Uuid, Mapping>,
    by_bid: HashMap<Uuid, Uuid>,
    in_ours: HashMap<Uuid, &'a Node>,
    in_theirs: HashSet<Uuid>,
    out: Outcome,
}

/// Apply `snapshot` to the body `root`, which reads as `ours`, given the
/// agreed `mappings`.
pub fn apply(
    txn: &mut TransactionMut,
    root: &XmlFragmentRef,
    snapshot: &Snapshot,
    resolver: &Resolver,
    ours: &Tree,
    mappings: Vec<Mapping>,
) -> Outcome {
    let by_bid = mappings
        .iter()
        .map(|mapping| (mapping.bid, mapping.notion_block))
        .collect();
    let by_notion = mappings
        .into_iter()
        .map(|mapping| (mapping.notion_block, mapping))
        .collect();
    let in_ours = ours
        .all()
        .into_iter()
        .filter_map(|node| node.bid.map(|bid| (bid, node)))
        .collect();
    let mut in_theirs = HashSet::new();
    collect_ids(&snapshot.blocks, &mut in_theirs);
    let mut merge = Merge {
        snapshot,
        resolver,
        ours,
        root: root.clone(),
        by_notion,
        by_bid,
        in_ours,
        in_theirs,
        out: Outcome::default(),
    };
    merge.level(txn, &snapshot.blocks, Parent::Root);
    merge.out
}

fn collect_ids(blocks: &[Block], out: &mut HashSet<Uuid>) {
    for block in blocks {
        out.insert(block.id);
        collect_ids(&block.children, out);
    }
}

impl Merge<'_> {
    fn root_place(&self) -> Place {
        Place::Root(self.root.clone())
    }

    /// Compare the blocks `theirs` under `parent` with ours.
    fn level(&mut self, txn: &mut TransactionMut, theirs: &[Block], parent: Parent) {
        let mut anchor: Option<Uuid> = None;
        for block in theirs {
            let Some(mapping) = self.by_notion.get(&block.id).cloned() else {
                let bid = Uuid::now_v7();
                if self.insert_new(txn, block, parent, anchor, bid) {
                    anchor = Some(bid);
                }
                continue;
            };
            // Our block is gone (the member deleted it): the outbound path
            // removes it at Notion.
            let Some(node) = self.in_ours.get(&mapping.bid).copied() else {
                continue;
            };
            self.reconcile(txn, node, &mapping, block);
            anchor = Some(mapping.bid);
            self.level(txn, &block.children, Parent::Block(mapping.bid));
        }
        if self.snapshot.complete {
            self.removed(txn, theirs, parent);
        }
    }

    /// Our blocks under `parent` that Notion no longer has at this level.
    fn removed(&mut self, txn: &mut TransactionMut, theirs: &[Block], parent: Parent) {
        let here: HashSet<Uuid> = theirs.iter().map(|block| block.id).collect();
        let ours: &Tree = self.ours;
        let children: Vec<&Node> = match parent {
            Parent::Root => ours.nodes.iter().collect(),
            Parent::Block(bid) => self
                .in_ours
                .get(&bid)
                .copied()
                .map(|node| node.children.iter().collect())
                .unwrap_or_default(),
        };
        for node in children {
            let Some(bid) = node.bid else {
                continue;
            };
            let Some(notion) = self.by_bid.get(&bid).copied() else {
                continue;
            };
            // Gone from this level, and from the whole page: a block that
            // moved is still there.
            if here.contains(&notion) || self.in_theirs.contains(&notion) {
                continue;
            }
            self.gone(txn, node);
        }
    }

    /// Notion removed the block `node`.
    fn gone(&mut self, txn: &mut TransactionMut, node: &Node) {
        let Some(bid) = node.bid else {
            return;
        };
        if self.changed_below(node) {
            // We changed it (or something under it): it stays, and says so.
            if self.ours.conflict_of(bid).is_none() {
                let Some(found) = edit::locate(&*txn, &self.root_place(), bid) else {
                    return;
                };
                let (place, index) = found.after();
                edit::build_conflict(
                    txn,
                    &place,
                    index,
                    bid,
                    ([0; 32], None),
                    None,
                    self.resolver,
                );
                self.out.raised += 1;
            }
            return;
        }
        let mut below = Vec::new();
        node.walk(&mut below);
        let bids: Vec<Uuid> = below.iter().filter_map(|n| n.bid).collect();
        if edit::remove(txn, &self.root, bid) {
            self.out.touched += 1;
            self.out.deletes.extend(bids);
        }
    }

    /// Whether the block or something under it is not what the last sync
    /// agreed: changed here, or new here.
    fn changed_below(&self, node: &Node) -> bool {
        let mut below = Vec::new();
        node.walk(&mut below);
        below.into_iter().any(|n| {
            let Some(bid) = n.bid else {
                return n.block.kind.editable();
            };
            match self.by_bid.get(&bid).and_then(|id| self.by_notion.get(id)) {
                Some(mapping) => hash(&n.block) != mapping.base_hash && n.block.kind.editable(),
                None => !matches!(n.block.kind, Kind::External),
            }
        })
    }

    /// Compare our block with Notion's version of it.
    fn reconcile(
        &mut self,
        txn: &mut TransactionMut,
        node: &Node,
        mapping: &Mapping,
        theirs: &Block,
    ) {
        let edited = theirs.edited.unwrap_or(self.snapshot.edited);
        let theirs_hash = hash(theirs);
        let ours_hash = hash(&node.block);
        let editable = node.block.kind.editable();
        let theirs_changed = theirs_hash != mapping.base_hash;
        // A block the member cannot edit here is never changed here.
        let ours_changed = editable && ours_hash != mapping.base_hash;
        match (theirs_changed, ours_changed) {
            (false, false) => {
                if edited != mapping.base_edited {
                    self.upsert(mapping.bid, theirs, theirs_hash, edited);
                }
            }
            (true, false) => self.take_theirs(txn, node, theirs, theirs_hash, edited),
            (false, true) => self.out.ours_pending = true,
            (true, true) => {
                if theirs_hash == ours_hash {
                    self.upsert(mapping.bid, theirs, theirs_hash, edited);
                } else {
                    self.conflict(txn, node, theirs, theirs_hash, edited);
                }
            }
        }
    }

    fn upsert(&mut self, bid: Uuid, theirs: &Block, base_hash: [u8; 32], edited: DateTime<Utc>) {
        self.out.upserts.push(Mapping {
            bid,
            notion_block: theirs.id,
            kind: theirs.kind,
            base_hash,
            base_edited: edited,
        });
    }

    /// Write Notion's version into our block.
    fn take_theirs(
        &mut self,
        txn: &mut TransactionMut,
        node: &Node,
        theirs: &Block,
        theirs_hash: [u8; 32],
        edited: DateTime<Utc>,
    ) {
        let Some(bid) = node.bid else {
            return;
        };
        let Some(found) = edit::locate(&*txn, &self.root_place(), bid) else {
            return;
        };
        // Notion's version settles whatever disagreement was shown.
        self.drop_conflict(txn, bid);
        if !edit::rewrite(txn, &found, theirs, self.resolver) {
            // Another kind of block now. An element cannot become another
            // kind: with nothing under it, it is made again; otherwise the
            // member decides.
            if node.children.is_empty() && !found_is_item(&found, theirs.kind) {
                let (place, index) = (found.place.clone(), found.index);
                if edit::build(txn, &place, index, theirs, Some(bid), self.resolver).is_some() {
                    place.remove(txn, index + 1);
                    self.out.touched += 1;
                    self.hold(theirs);
                    self.upsert(bid, theirs, theirs_hash, edited);
                }
            } else {
                self.conflict(txn, node, theirs, theirs_hash, edited);
            }
            return;
        }
        self.out.touched += 1;
        self.hold(theirs);
        self.upsert(bid, theirs, theirs_hash, edited);
    }

    /// Remove the conflict about block `bid`, if the body has one.
    fn drop_conflict(&self, txn: &mut TransactionMut, bid: Uuid) {
        let place = self.root_place();
        for (of, at_place, index) in edit::conflicts(&*txn, &place) {
            if of == bid {
                at_place.remove(txn, index);
                break;
            }
        }
    }

    fn hold(&mut self, block: &Block) {
        if let Some(file) = &block.file {
            self.out.files.push(file.clone());
        }
    }

    /// Put Notion's version beneath ours as a conflict.
    fn conflict(
        &mut self,
        txn: &mut TransactionMut,
        node: &Node,
        theirs: &Block,
        theirs_hash: [u8; 32],
        edited: DateTime<Utc>,
    ) {
        let Some(bid) = node.bid else {
            return;
        };
        match self.ours.conflict_of(bid) {
            // Already shown, the same version: nothing new.
            Some(existing) if existing.hash == Some(theirs_hash) => return,
            Some(_) => {
                // A newer version of Notion's: the old conflict goes.
                self.drop_conflict(txn, bid);
            }
            None => self.out.raised += 1,
        }
        let Some(found) = edit::locate(&*txn, &self.root_place(), bid) else {
            return;
        };
        let (place, index) = found.after();
        edit::build_conflict(
            txn,
            &place,
            index,
            bid,
            (theirs_hash, Some(edited)),
            Some(theirs),
            self.resolver,
        );
    }

    /// Insert a block Notion has and we have not, at its place, with the
    /// blocks under it. `false` when it cannot be made.
    fn insert_new(
        &mut self,
        txn: &mut TransactionMut,
        block: &Block,
        parent: Parent,
        anchor: Option<Uuid>,
        bid: Uuid,
    ) -> bool {
        let Some(slot) = self.slot(txn, block.kind, parent, anchor) else {
            return false;
        };
        let Some(built) = self.make(txn, slot, block, bid) else {
            return false;
        };
        self.after_insert(txn, block, bid, &built);
        true
    }

    /// Where a new block of `kind` goes: after the block before it, or first
    /// under its parent.
    fn slot(
        &self,
        txn: &mut TransactionMut,
        kind: Kind,
        parent: Parent,
        anchor: Option<Uuid>,
    ) -> Option<Slot> {
        if let Some(anchor) = anchor {
            let found = edit::locate(&*txn, &self.root_place(), anchor)?;
            if let (Some(list), Some(wanted)) = (&found.list, edit::list_tag_of(kind))
                && &**list.element.tag() == wanted
            {
                return Some(Slot::InList(list.element.clone(), found.index + 1));
            }
            let (place, index) = found.after();
            return Some(Slot::At(place, index));
        }
        let place = match parent {
            Parent::Root => self.root_place(),
            Parent::Block(bid) => {
                Place::Element(edit::locate(&*txn, &self.root_place(), bid)?.element)
            }
        };
        let index = edit::first_index(&*txn, &place);
        Some(Slot::At(place, index))
    }

    fn make(
        &self,
        txn: &mut TransactionMut,
        slot: Slot,
        block: &Block,
        bid: Uuid,
    ) -> Option<Built> {
        match slot {
            Slot::InList(list, index) => {
                let element = edit::build_item(txn, &list, index, block, Some(bid), self.resolver);
                Some(Built {
                    element,
                    list: Some(list),
                })
            }
            Slot::At(place, index) => {
                edit::build(txn, &place, index, block, Some(bid), self.resolver)
            }
        }
    }

    /// A block was made: it is the same block as Notion's, and what is
    /// under it is made too.
    fn after_insert(&mut self, txn: &mut TransactionMut, block: &Block, bid: Uuid, built: &Built) {
        self.out.touched += 1;
        self.hold(block);
        let edited = block.edited.unwrap_or(self.snapshot.edited);
        self.out.upserts.push(Mapping {
            bid,
            notion_block: block.id,
            kind: block.kind,
            base_hash: hash(block),
            base_edited: edited,
        });
        self.by_bid.insert(bid, block.id);
        let place = Place::Element(built.element.clone());
        let first = edit::first_index(&*txn, &place);
        self.insert_all(txn, &place, first, &block.children);
    }

    /// Insert `blocks` one after the other from `index` of `place`;
    /// consecutive list items share a list.
    fn insert_all(
        &mut self,
        txn: &mut TransactionMut,
        place: &Place,
        mut index: u32,
        blocks: &[Block],
    ) {
        let mut open: Option<(&'static str, XmlElementRef)> = None;
        for block in blocks {
            let bid = Uuid::now_v7();
            let wanted = edit::list_tag_of(block.kind);
            let slot = match (&open, wanted) {
                (Some((tag, list)), Some(wanted)) if *tag == wanted => {
                    Slot::InList(list.clone(), list.len(&*txn))
                }
                _ => Slot::At(place.clone(), index),
            };
            let joins = matches!(slot, Slot::InList(..));
            let Some(built) = self.make(txn, slot, block, bid) else {
                continue;
            };
            if !joins {
                index += 1;
            }
            open = wanted.zip(built.list.clone());
            self.after_insert(txn, block, bid, &built);
        }
    }
}

/// Whether the block at `found` is a list item (which cannot be made again
/// in its list from here).
fn found_is_item(found: &edit::Located, new_kind: Kind) -> bool {
    found.list.is_some() || edit::list_tag_of(new_kind).is_some()
}

/// A snapshot merged into a document.
#[derive(Debug)]
pub struct Merged {
    /// What applying did.
    pub outcome: Outcome,
    /// How many blocks were given a stable id.
    pub ids_given: usize,
    /// The body after, read against the schema.
    pub inspection: Inspection,
    /// How many conflicts the body has now.
    pub conflicts: usize,
    /// The update to store, when the body changed.
    pub update: Option<Vec<u8>>,
}

/// Merge `snapshot` into `doc` (the page's body as stored): every block
/// gets a stable id it lacks, then the snapshot is applied block by block.
/// Refused when the body would not hold the result; nothing is stored here.
pub fn merge_into(
    doc: &Doc,
    snapshot: &Snapshot,
    resolver: &Resolver,
    mappings: Vec<Mapping>,
) -> Result<Merged, Rejection> {
    let before = document::state_vector(doc);
    let before_state = document::full_state(doc);
    let root = doc.get_or_insert_xml_fragment(document::ROOT);
    let (outcome, ids_given) = {
        let mut txn = doc.transact_mut();
        let ids_given = edit::assign_ids(&mut txn, &root, &mut Uuid::now_v7);
        let ours = tree::read(&txn, &root, resolver);
        let outcome = apply(&mut txn, &root, snapshot, resolver, &ours, mappings);
        (outcome, ids_given)
    };
    let inspection = document::inspect(doc)?;
    let conflicts = {
        let txn = doc.transact();
        tree::read(&txn, &root, resolver).conflicts.len()
    };
    let update = if document::full_state(doc) == before_state {
        None
    } else {
        Some(document::diff_since(doc, &before)?)
    };
    Ok(Merged {
        outcome,
        ids_given,
        inspection,
        conflicts,
        update,
    })
}

#[cfg(test)]
mod tests;
