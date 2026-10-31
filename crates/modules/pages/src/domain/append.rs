//! Adding blocks at the end of a page, and taking them off again
//! (`pages.append`, `pages.unappend`).
//!
//! Both work on a scratch copy of the page's document and hand back the
//! Yjs update the change makes, which the caller then runs through the one
//! checker every update passes (`body::accept`): nothing here stores
//! anything, and nothing here is trusted by the schema.
//!
//! An appended block is told apart by the stable id (`bid`) it was given,
//! and by a hash of what it held when it was added. Taking the append back
//! removes the blocks that still hold exactly that; a block someone has
//! changed since stays.

use std::{collections::BTreeMap, fmt::Write as _};

use campbooks_kernel::{RecordRef, blobs::BlobHash};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use yrs::{
    Any, Doc, Out, ReadTxn, Text, Transact, Xml, XmlElementPrelim, XmlElementRef, XmlFragment,
    XmlOut, XmlTextPrelim, types::text::YChange,
};

use super::{
    document::{self, ROOT, Rejection},
    notion::{
        edit::{self, Place},
        model::{Block, Kind},
        tree::{Resolver, attributes},
    },
};

/// One thing an append added: a block, or an item of a list, by its stable
/// id, and what it held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unit {
    /// Its stable id.
    pub bid: Uuid,
    /// A hash of its content when it was added.
    pub hash: String,
}

/// What appending makes.
#[derive(Debug, Clone)]
pub struct Appended {
    /// The change, as one Yjs update (v1).
    pub update: Vec<u8>,
    /// What was added.
    pub units: Vec<Unit>,
}

/// What taking an append back does.
#[derive(Debug, Clone)]
pub struct Removed {
    /// The change, as one Yjs update (v1).
    pub update: Vec<u8>,
    /// How many of its blocks were removed.
    pub removed: usize,
    /// How many stayed because they were changed since.
    pub stayed: usize,
    /// Those that stayed, as they were added: taking the append back again
    /// still finds them.
    pub kept: Vec<Unit>,
}

/// A copy of `doc` to change.
fn scratch(doc: &Doc) -> Result<Doc, Rejection> {
    document::open(Some(&document::full_state(doc)), &[])
}

/// Add `blocks` at the end of `doc`, then a line of mentions of `sources`
/// when there are any.
pub fn append(doc: &Doc, blocks: &[Block], sources: &[RecordRef]) -> Result<Appended, Rejection> {
    let work = scratch(doc)?;
    let before = document::state_vector(doc);
    let resolver = Resolver::default();
    let mut units = Vec::new();
    {
        let root = work.get_or_insert_xml_fragment(ROOT);
        let mut txn = work.transact_mut();
        let place = Place::Root(root.clone());
        // The list the previous block was an item of, and its kind.
        let mut open: Option<(Kind, XmlElementRef)> = None;
        for block in blocks {
            if block.kind.is_item() {
                let item = match &open {
                    Some((kind, list)) if *kind == block.kind => {
                        let at = list.len(&txn);
                        edit::build_item(&mut txn, list, at, block, Some(block.id), &resolver)
                    }
                    _ => {
                        let at = root.len(&txn);
                        let Some(built) =
                            edit::build(&mut txn, &place, at, block, Some(block.id), &resolver)
                        else {
                            continue;
                        };
                        open = built.list.map(|list| (block.kind, list));
                        built.element
                    }
                };
                units.push(Unit {
                    bid: block.id,
                    hash: hash_of(&txn, &item),
                });
                continue;
            }
            open = None;
            let at = root.len(&txn);
            let Some(built) = edit::build(&mut txn, &place, at, block, Some(block.id), &resolver)
            else {
                continue;
            };
            children(&mut txn, &built.element, block, &resolver);
            units.push(Unit {
                bid: block.id,
                hash: hash_of(&txn, &built.element),
            });
        }
        if !sources.is_empty() {
            let id = Uuid::now_v7();
            let at = root.len(&txn);
            let line = root.insert(&mut txn, at, XmlElementPrelim::empty("paragraph"));
            line.insert_attribute(&mut txn, "bid", Any::String(id.to_string().into()));
            for (n, source) in sources.iter().enumerate() {
                if n > 0 {
                    line.push_back(&mut txn, XmlTextPrelim::new(" "));
                }
                let mention = line.push_back(&mut txn, XmlElementPrelim::empty("mention"));
                mention.insert_attribute(
                    &mut txn,
                    "ref_type",
                    Any::String(source.record_type.as_str().into()),
                );
                mention.insert_attribute(
                    &mut txn,
                    "ref_id",
                    Any::String(source.id.as_str().into()),
                );
            }
            units.push(Unit {
                bid: id,
                hash: hash_of(&txn, &line),
            });
        }
    }
    let update = document::diff_since(&work, &before)?;
    Ok(Appended { update, units })
}

/// The blocks under `element`, made from `block`'s children.
fn children(
    txn: &mut yrs::TransactionMut,
    element: &XmlElementRef,
    block: &Block,
    resolver: &Resolver,
) {
    for child in &block.children {
        let at = element.len(&*txn);
        if let Some(built) = edit::build(
            txn,
            &Place::Element(element.clone()),
            at,
            child,
            Some(child.id),
            resolver,
        ) {
            children(txn, &built.element, child, resolver);
        }
    }
}

/// Take `units` off `doc`: each one that still holds what it did. A unit
/// that is not there any more is neither removed nor stayed.
pub fn remove(doc: &Doc, units: &[Unit]) -> Result<Removed, Rejection> {
    let work = scratch(doc)?;
    let before = document::state_vector(doc);
    let mut removed = 0;
    let mut kept: Vec<Unit> = Vec::new();
    {
        let root = work.get_or_insert_xml_fragment(ROOT);
        let mut txn = work.transact_mut();
        let place = Place::Root(root);
        for unit in units {
            let Some(found) = edit::locate(&txn, &place, unit.bid) else {
                continue;
            };
            if hash_of(&txn, &found.element) != unit.hash {
                kept.push(unit.clone());
                continue;
            }
            found.place.remove(&mut txn, found.index);
            // The list an item was taken from goes with its last item.
            if let Some(list) = &found.list
                && list.element.len(&txn) == 0
            {
                list.place.remove(&mut txn, list.index);
            }
            removed += 1;
        }
    }
    let update = document::diff_since(&work, &before)?;
    Ok(Removed {
        update,
        removed,
        stayed: kept.len(),
        kept,
    })
}

/// A hash of what `element` holds: its name, attributes, text and marks and
/// the blocks inside it, in an order that does not depend on how the
/// document stores them.
fn hash_of<T: ReadTxn>(txn: &T, element: &XmlElementRef) -> String {
    let mut text = String::new();
    canonical(txn, element, &mut text);
    BlobHash::of(text.as_bytes()).to_string()
}

fn canonical<T: ReadTxn>(txn: &T, element: &XmlElementRef, out: &mut String) {
    let _ = write!(out, "<{}", element.tag());
    for (key, value) in attributes(txn, element) {
        let _ = write!(out, " {key}=");
        canonical_any(&value, out);
    }
    out.push('>');
    for child in element.children(txn) {
        match child {
            XmlOut::Element(inner) => canonical(txn, &inner, out),
            XmlOut::Text(run) => {
                for chunk in run.diff(txn, YChange::identity) {
                    let Out::Any(any) = &chunk.insert else {
                        continue;
                    };
                    out.push('{');
                    if let Some(marks) = &chunk.attributes {
                        let sorted: BTreeMap<&str, &Any> =
                            marks.iter().map(|(key, value)| (&**key, value)).collect();
                        for (key, value) in sorted {
                            let _ = write!(out, "{key}:");
                            canonical_any(value, out);
                            out.push(',');
                        }
                    }
                    out.push('|');
                    canonical_any(any, out);
                    out.push('}');
                }
            }
            XmlOut::Fragment(_) => {}
        }
    }
    let _ = write!(out, "</{}>", element.tag());
}

fn canonical_any(value: &Any, out: &mut String) {
    match value {
        Any::Map(map) => {
            let sorted: BTreeMap<&str, &Any> = map.iter().map(|(k, v)| (k.as_str(), v)).collect();
            out.push('(');
            for (key, inner) in sorted {
                let _ = write!(out, "{key}:");
                canonical_any(inner, out);
                out.push(',');
            }
            out.push(')');
        }
        other => {
            let _ = write!(out, "{other:?}");
        }
    }
}

#[cfg(test)]
mod tests;
