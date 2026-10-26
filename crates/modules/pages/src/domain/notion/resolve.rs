//! Settling a conflict with Notion, and taking the choice back
//! (campbooks-cloud#376): the changes to the body, in one transaction of its
//! document, and what the member's choice moves in the base.
//!
//! - **Keep mine**: our block stays as it is; the conflict goes; the base
//!   becomes Notion's version, so our block reads as changed since and is
//!   sent. (When Notion had removed the block, it is ours alone from now on:
//!   it is no longer the same block at Notion, and is added there again.)
//! - **Use Notion's**: our block takes Notion's content; the conflict goes;
//!   the base is Notion's version, so nothing is sent for it. (When Notion
//!   had removed the block, ours is removed too.)
//! - **Keep both**: our block stays, Notion's version becomes an ordinary
//!   block after it (a block of ours, to be added at Notion), the conflict
//!   goes, and the base is Notion's version.
//!
//! Undoing puts the conflict back as it was and the base where it stood.

use chrono::{DateTime, Utc};
use uuid::Uuid;
use yrs::{Doc, Transact};

use super::{
    edit::{self, Place},
    merge::Mapping,
    model::{Block, hash},
    tree::{self, ConflictNode, Resolver},
};
use crate::domain::{document, tracked::Choice};

/// Why a conflict could not be settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The body has no conflict about that block.
    NoConflict,
    /// The block the conflict is about is gone.
    BlockGone,
    /// The body would not hold what the choice makes.
    Body(document::Rejection),
}

impl From<document::Rejection> for Refused {
    fn from(rejection: document::Rejection) -> Self {
        Self::Body(rejection)
    }
}

/// What a choice did.
#[derive(Debug)]
pub struct Resolution {
    /// The update to store, when the body changed.
    pub update: Option<Vec<u8>>,
    /// The body after, read against the schema.
    pub inspection: document::Inspection,
    /// The mapping to write, when the choice moved the base.
    pub upsert: Option<Mapping>,
    /// The blocks whose mapping goes.
    pub deletes: Vec<Uuid>,
    /// The block "keep both" made of Notion's version.
    pub extra: Option<Uuid>,
    /// Our block as it was, and Notion's version, for undoing.
    pub ours: Block,
    /// Notion's version (none: Notion had removed the block).
    pub theirs: Option<Block>,
    /// The hash and edit time of Notion's version (for the base).
    pub theirs_edited: Option<DateTime<Utc>>,
    /// How many conflicts the body has now.
    pub conflicts: usize,
}

/// Settle the conflict about block `bid` with `choice`. `mapping` is what
/// the last sync agreed about the block, if anything.
#[allow(
    clippy::too_many_lines,
    reason = "one transaction of the document: find, remove the conflict, apply the choice"
)]
pub fn resolve(
    doc: &Doc,
    resolver: &Resolver,
    bid: Uuid,
    choice: Choice,
    mapping: Option<&Mapping>,
) -> Result<Resolution, Refused> {
    let before = document::state_vector(doc);
    let before_state = document::full_state(doc);
    let root = doc.get_or_insert_xml_fragment(document::ROOT);
    let (conflict, ours, upsert, deletes, extra) = {
        let mut txn = doc.transact_mut();
        let read = tree::read(&txn, &root, resolver);
        let conflict: ConflictNode = read.conflict_of(bid).cloned().ok_or(Refused::NoConflict)?;
        let node = read.find(bid).ok_or(Refused::BlockGone)?;
        let ours = node.block.clone();
        let children_empty = node.children.is_empty();
        let place = Place::Root(root.clone());
        // The conflict goes, whatever the choice.
        for (of, at_place, index) in edit::conflicts(&txn, &place) {
            if of == bid {
                at_place.remove(&mut txn, index);
                break;
            }
        }
        let mut upsert = None;
        let mut deletes = Vec::new();
        let mut extra = None;
        let theirs_hash = conflict
            .hash
            .unwrap_or_else(|| mapping.map_or([0; 32], |m| m.base_hash));
        let base = |edited: Option<DateTime<Utc>>| {
            mapping.map(|m| Mapping {
                bid,
                notion_block: m.notion_block,
                kind: m.kind,
                base_hash: theirs_hash,
                base_edited: edited.unwrap_or(m.base_edited),
            })
        };
        match choice {
            Choice::Mine => {
                if conflict.removed {
                    deletes.push(bid);
                } else {
                    upsert = base(conflict.edited);
                }
            }
            Choice::Notion => {
                if let (Some(theirs), false) = (&conflict.theirs, conflict.removed) {
                    let Some(located) = edit::locate(&txn, &place, bid) else {
                        return Err(Refused::BlockGone);
                    };
                    if !edit::rewrite(&mut txn, &located, theirs, resolver) {
                        // Another kind of block: made again in its place.
                        if children_empty && located.list.is_none() {
                            let (at_place, index) = (located.place.clone(), located.index);
                            if edit::build(&mut txn, &at_place, index, theirs, Some(bid), resolver)
                                .is_some()
                            {
                                at_place.remove(&mut txn, index + 1);
                            }
                        } else {
                            return Err(Refused::Body(document::Rejection::BadStructure));
                        }
                    }
                    upsert = base(conflict.edited);
                } else {
                    // Notion removed it: so is ours, with what is under it.
                    let mut below = Vec::new();
                    node.walk(&mut below);
                    deletes.extend(below.iter().filter_map(|n| n.bid));
                    edit::remove(&mut txn, &root, bid);
                }
            }
            Choice::Both => {
                if conflict.removed {
                    deletes.push(bid);
                } else {
                    if let (Some(theirs), Some(located)) =
                        (&conflict.theirs, edit::locate(&txn, &place, bid))
                    {
                        let copy = Uuid::now_v7();
                        let (at_place, index) = located.after();
                        // Beside ours, in a list of its own when it is an item.
                        if edit::build(&mut txn, &at_place, index, theirs, Some(copy), resolver)
                            .is_some()
                        {
                            extra = Some(copy);
                        }
                    }
                    upsert = base(conflict.edited);
                }
            }
        }
        (conflict, ours, upsert, deletes, extra)
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
    let theirs_edited = conflict.edited;
    Ok(Resolution {
        update,
        inspection,
        upsert,
        deletes,
        extra,
        ours,
        theirs: conflict.theirs,
        theirs_edited,
        conflicts,
    })
}

/// What undoing a choice needs of the choice.
#[derive(Debug, Clone)]
pub struct Taken<'a> {
    /// The block the conflict was about.
    pub bid: Uuid,
    /// What was chosen.
    pub choice: Choice,
    /// Our block as it was.
    pub ours: &'a Block,
    /// Notion's version.
    pub theirs: Option<&'a Block>,
    /// When Notion last edited its version.
    pub theirs_edited: Option<DateTime<Utc>>,
    /// The block "keep both" made.
    pub extra: Option<Uuid>,
}

/// What undoing did.
#[derive(Debug)]
pub struct Undone {
    /// The update to store.
    pub update: Option<Vec<u8>>,
    /// The body after, read against the schema.
    pub inspection: document::Inspection,
    /// How many conflicts the body has now.
    pub conflicts: usize,
}

/// Put the conflict back as it was before `taken`.
pub fn undo(doc: &Doc, resolver: &Resolver, taken: &Taken<'_>) -> Result<Undone, Refused> {
    let before = document::state_vector(doc);
    let before_state = document::full_state(doc);
    let root = doc.get_or_insert_xml_fragment(document::ROOT);
    {
        let mut txn = doc.transact_mut();
        let place = Place::Root(root.clone());
        if edit::conflicts(&txn, &place)
            .iter()
            .any(|(of, ..)| *of == taken.bid)
        {
            return Err(Refused::Body(document::Rejection::BadStructure));
        }
        if taken.choice == Choice::Both
            && let Some(extra) = taken.extra
        {
            edit::remove(&mut txn, &root, extra);
        }
        let Some(found) = edit::locate(&txn, &place, taken.bid) else {
            return Err(Refused::BlockGone);
        };
        if taken.choice == Choice::Notion {
            // Our block takes back what it had.
            if !edit::rewrite(&mut txn, &found, taken.ours, resolver) {
                return Err(Refused::Body(document::Rejection::BadStructure));
            }
        }
        let found = edit::locate(&txn, &place, taken.bid).ok_or(Refused::BlockGone)?;
        let (at_place, index) = found.after();
        let theirs_hash = taken.theirs.map_or([0; 32], hash);
        edit::build_conflict(
            &mut txn,
            &at_place,
            index,
            taken.bid,
            (theirs_hash, taken.theirs_edited),
            taken.theirs,
            resolver,
        );
    }
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
    Ok(Undone {
        update,
        inspection,
        conflicts,
    })
}

#[cfg(test)]
mod tests;
