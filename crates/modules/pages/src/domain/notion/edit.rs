//! Changing a page's body, block by block, in a Yjs transaction
//! (campbooks-cloud#376): finding a block by its stable id, making a block
//! of any kind, rewriting one in place, removing one, and the `conflict`
//! element a disagreement with Notion leaves.
//!
//! Every function works on the document inside one `TransactionMut`, and
//! only touches the blocks it is asked to: what other people wrote in other
//! blocks is untouched, so a Notion change never overwrites it.

use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, SecondsFormat, Utc};
use uuid::Uuid;
use yrs::{
    Any, ReadTxn, Text, TransactionMut, Xml, XmlElementPrelim, XmlElementRef, XmlFragment,
    XmlFragmentRef, XmlOut, XmlTextPrelim, types::Attrs,
};

use super::{
    model::{Block, Kind, Run, hash_hex},
    tree::{Resolver, attributes, text_attribute, uuid_attribute},
};

/// A place blocks live in: the body, or an element.
#[derive(Debug, Clone)]
pub enum Place {
    /// The body.
    Root(XmlFragmentRef),
    /// A block's element.
    Element(XmlElementRef),
}

impl Place {
    /// How many children it has.
    pub fn len<T: ReadTxn>(&self, txn: &T) -> u32 {
        match self {
            Self::Root(root) => root.len(txn),
            Self::Element(element) => element.len(txn),
        }
    }

    /// Its children.
    pub fn children<T: ReadTxn>(&self, txn: &T) -> Vec<XmlOut> {
        match self {
            Self::Root(root) => root.children(txn).collect(),
            Self::Element(element) => element.children(txn).collect(),
        }
    }

    /// A new element `tag` at `index`.
    fn insert_element(&self, txn: &mut TransactionMut, index: u32, tag: &str) -> XmlElementRef {
        match self {
            Self::Root(root) => root.insert(txn, index, XmlElementPrelim::empty(tag)),
            Self::Element(element) => element.insert(txn, index, XmlElementPrelim::empty(tag)),
        }
    }

    /// Remove the child at `index`.
    pub fn remove(&self, txn: &mut TransactionMut, index: u32) {
        match self {
            Self::Root(root) => root.remove(txn, index),
            Self::Element(element) => element.remove(txn, index),
        }
    }
}

/// Where a block is: its element, the place it is a child of and its index
/// there, and, for a list item, the list element holding it.
#[derive(Debug, Clone)]
pub struct Located {
    /// The block's element.
    pub element: XmlElementRef,
    /// The place it is in.
    pub place: Place,
    /// Its index there.
    pub index: u32,
    /// The list it is an item of.
    pub list: Option<ListAt>,
}

/// A list element and where it is.
#[derive(Debug, Clone)]
pub struct ListAt {
    /// The list element (`bulletList`, `orderedList` or `taskList`).
    pub element: XmlElementRef,
    /// The place it is in.
    pub place: Place,
    /// Its index there.
    pub index: u32,
}

impl Located {
    /// The place and index a sibling that follows this block goes at: after
    /// the block, or after its list when it is an item.
    pub fn after(&self) -> (Place, u32) {
        match &self.list {
            Some(list) => (list.place.clone(), list.index + 1),
            None => (self.place.clone(), self.index + 1),
        }
    }
}

/// The index of `at` as the document counts.
fn index_of(at: usize) -> u32 {
    u32::try_from(at).unwrap_or(u32::MAX)
}

fn is_list(tag: &str) -> bool {
    matches!(tag, "bulletList" | "orderedList" | "taskList")
}

/// The element a block of `kind` is.
pub const fn tag_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Paragraph => "paragraph",
        Kind::Heading => "heading",
        Kind::BulletedItem | Kind::NumberedItem => "listItem",
        Kind::Todo => "taskItem",
        Kind::Toggle => "toggle",
        Kind::Quote => "blockquote",
        Kind::Callout => "callout",
        Kind::Divider => "horizontalRule",
        Kind::Code => "codeBlock",
        Kind::Image => "image",
        Kind::File => "file",
        Kind::PageLink => "pageLink",
        Kind::External => "external",
    }
}

/// The list element an item of `kind` is held by.
pub const fn list_tag_of(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::BulletedItem => Some("bulletList"),
        Kind::NumberedItem => Some("orderedList"),
        Kind::Todo => Some("taskList"),
        _ => None,
    }
}

/// Find the block with stable id `bid` under `place`.
pub fn locate<T: ReadTxn>(txn: &T, place: &Place, bid: Uuid) -> Option<Located> {
    search(txn, place, bid, None)
}

fn search<T: ReadTxn>(txn: &T, place: &Place, bid: Uuid, list: Option<ListAt>) -> Option<Located> {
    for (at, child) in place.children(txn).into_iter().enumerate() {
        let XmlOut::Element(element) = child else {
            continue;
        };
        let index = index_of(at);
        if uuid_attribute(&attributes(txn, &element), "bid") == Some(bid) {
            return Some(Located {
                element,
                place: place.clone(),
                index,
                list,
            });
        }
        let tag = element.tag().to_string();
        let inside = if is_list(&tag) {
            Some(ListAt {
                element: element.clone(),
                place: place.clone(),
                index,
            })
        } else {
            None
        };
        if let Some(found) = search(txn, &Place::Element(element), bid, inside) {
            return Some(found);
        }
    }
    None
}

/// Every `conflict` element under `place`, with where each is.
pub fn conflicts<T: ReadTxn>(txn: &T, place: &Place) -> Vec<(Uuid, Place, u32)> {
    let mut out = Vec::new();
    for (at, child) in place.children(txn).into_iter().enumerate() {
        let XmlOut::Element(element) = child else {
            continue;
        };
        if &**element.tag() == "conflict" {
            if let Some(of) = uuid_attribute(&attributes(txn, &element), "of") {
                out.push((of, place.clone(), index_of(at)));
            }
            continue;
        }
        out.extend(conflicts(txn, &Place::Element(element)));
    }
    out
}

fn string(text: &str) -> Any {
    Any::String(text.into())
}

/// The empty value a mark takes.
fn on() -> Any {
    Any::Map(Arc::new(HashMap::new()))
}

/// A link Pages keeps: `https`, `http` or `mailto`, no spaces or control
/// characters.
fn safe_link(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    ["https:", "http:", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
        && href.len() <= 2048
        && href.chars().all(|c| !c.is_control() && !c.is_whitespace())
}

/// Remove every child of `element`.
pub fn clear(txn: &mut TransactionMut, element: &XmlElementRef) {
    let count = element.len(&*txn);
    if count > 0 {
        element.remove_range(txn, 0, count);
    }
}

/// Write `runs` as the content of `element`: text in as many pieces as
/// there are mentions between them, each with its marks. A page mention
/// becomes a Pages mention when that Notion page is synced too and a link
/// to Notion's address of it otherwise.
pub fn write_runs(
    txn: &mut TransactionMut,
    element: &XmlElementRef,
    runs: &[Run],
    resolver: &Resolver,
) {
    let mut current: Option<yrs::XmlTextRef> = None;
    for run in runs {
        if let Some(notion) = run.page {
            if let Some(page) = resolver.page_of.get(&notion) {
                let mention = element.push_back(txn, XmlElementPrelim::empty("mention"));
                mention.insert_attribute(txn, "ref_type", string("pages.page"));
                mention.insert_attribute(txn, "ref_id", string(&page.to_string()));
                current = None;
                continue;
            }
            let words = if run.text.is_empty() {
                "Notion page"
            } else {
                run.text.as_str()
            };
            let href = format!("https://www.notion.so/{}", notion.simple());
            let linked = Run {
                text: words.to_owned(),
                link: Some(href),
                ..run.clone()
            };
            push_text(txn, element, &mut current, &linked);
            continue;
        }
        push_text(txn, element, &mut current, run);
    }
}

fn push_text(
    txn: &mut TransactionMut,
    element: &XmlElementRef,
    current: &mut Option<yrs::XmlTextRef>,
    run: &Run,
) {
    if run.text.is_empty() {
        return;
    }
    if current.is_none() {
        *current = Some(element.push_back(txn, XmlTextPrelim::new("")));
    }
    let Some(text) = current.as_ref() else {
        return;
    };
    let mut marks: Attrs = HashMap::new();
    if run.bold {
        marks.insert("bold".into(), on());
    }
    if run.italic {
        marks.insert("italic".into(), on());
    }
    if run.strike {
        marks.insert("strike".into(), on());
    }
    if run.code {
        marks.insert("code".into(), on());
    }
    if let Some(href) = run.link.as_deref().filter(|href| safe_link(href)) {
        marks.insert(
            "link".into(),
            Any::Map(Arc::new(HashMap::from([("href".to_owned(), string(href))]))),
        );
    }
    let at = text.len(&*txn);
    text.insert_with_attributes(txn, at, &run.text, marks);
}

/// Write `text` as the plain content of a code block.
fn write_plain(txn: &mut TransactionMut, element: &XmlElementRef, text: &str) {
    if !text.is_empty() {
        element.push_back(txn, XmlTextPrelim::new(text));
    }
}

/// Set the attributes and the own content of `element`, an element of
/// `block`'s kind: its stable id `bid`, what its kind has of attributes, its
/// text. The blocks under it are the caller's.
pub fn fill(
    txn: &mut TransactionMut,
    element: &XmlElementRef,
    block: &Block,
    bid: Option<Uuid>,
    resolver: &Resolver,
) {
    if let Some(bid) = bid {
        element.insert_attribute(txn, "bid", string(&bid.to_string()));
    }
    match block.kind {
        Kind::Paragraph => write_runs(txn, element, &block.runs, resolver),
        Kind::Heading => {
            element.insert_attribute(
                txn,
                "level",
                Any::Number(f64::from(block.level.unwrap_or(1).clamp(1, 3))),
            );
            write_runs(txn, element, &block.runs, resolver);
        }
        Kind::Code => {
            element.insert_attribute(
                txn,
                "language",
                string(block.language.as_deref().unwrap_or("plaintext")),
            );
            write_plain(txn, element, &block.text());
        }
        Kind::BulletedItem
        | Kind::NumberedItem
        | Kind::Todo
        | Kind::Toggle
        | Kind::Quote
        | Kind::Callout => {
            if block.kind == Kind::Todo {
                element.insert_attribute(txn, "checked", Any::Bool(block.checked.unwrap_or(false)));
            }
            let lead = element.push_back(txn, XmlElementPrelim::empty("paragraph"));
            write_runs(txn, &lead, &block.runs, resolver);
        }
        Kind::Divider => {}
        Kind::Image | Kind::File => {
            if let Some(file) = &block.file {
                element.insert_attribute(txn, "blob", string(&file.blob));
                element.insert_attribute(txn, "name", string(&file.name));
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "a file's size is far below 2^53"
                )]
                let size = file.size as f64;
                element.insert_attribute(txn, "size", Any::Number(size));
            }
        }
        Kind::PageLink => {
            if let Some(page) = block.page.and_then(|notion| resolver.page_of.get(&notion)) {
                element.insert_attribute(txn, "page", string(&page.to_string()));
            }
        }
        Kind::External => {
            element.insert_attribute(
                txn,
                "kind",
                string(block.label.as_deref().unwrap_or("unsupported")),
            );
        }
    }
}

/// A block made.
#[derive(Debug, Clone)]
pub struct Built {
    /// The block's element (a list item for a list item).
    pub element: XmlElementRef,
    /// The list element made for it, when it is a list item made in a list
    /// of its own.
    pub list: Option<XmlElementRef>,
}

/// Make a block of `block`'s kind at `index` of `place`, with the stable id
/// `bid`: for a list item, in a list of its own. Not the blocks under it.
/// `None` for a block that cannot be made (a sub-page link whose page is
/// not known here).
pub fn build(
    txn: &mut TransactionMut,
    place: &Place,
    index: u32,
    block: &Block,
    bid: Option<Uuid>,
    resolver: &Resolver,
) -> Option<Built> {
    if block.kind == Kind::PageLink
        && !block
            .page
            .is_some_and(|notion| resolver.page_of.contains_key(&notion))
    {
        return None;
    }
    if let Some(list_tag) = list_tag_of(block.kind) {
        let list = place.insert_element(txn, index, list_tag);
        let element = build_item(txn, &list, 0, block, bid, resolver);
        return Some(Built {
            element,
            list: Some(list),
        });
    }
    let element = place.insert_element(txn, index, tag_of(block.kind));
    fill(txn, &element, block, bid, resolver);
    Some(Built {
        element,
        list: None,
    })
}

/// Make a list item of `block`'s kind at `index` of the list `list`.
pub fn build_item(
    txn: &mut TransactionMut,
    list: &XmlElementRef,
    index: u32,
    block: &Block,
    bid: Option<Uuid>,
    resolver: &Resolver,
) -> XmlElementRef {
    let item = list.insert(txn, index, XmlElementPrelim::empty(tag_of(block.kind)));
    fill(txn, &item, block, bid, resolver);
    item
}

/// Give the block at `found` the content of `block`, in place: the same
/// element, the same stable id, whatever else is in the page around it.
/// The blocks under it are untouched. `false` when the kinds differ (the
/// element cannot become another kind: the caller replaces it).
pub fn rewrite(
    txn: &mut TransactionMut,
    found: &Located,
    block: &Block,
    resolver: &Resolver,
) -> bool {
    let element = &found.element;
    let tag = element.tag().to_string();
    if tag != tag_of(block.kind) {
        return false;
    }
    match block.kind {
        Kind::Paragraph | Kind::Heading | Kind::Code => {
            clear(txn, element);
            match block.kind {
                Kind::Heading => {
                    element.insert_attribute(
                        txn,
                        "level",
                        Any::Number(f64::from(block.level.unwrap_or(1).clamp(1, 3))),
                    );
                    write_runs(txn, element, &block.runs, resolver);
                }
                Kind::Code => {
                    element.insert_attribute(
                        txn,
                        "language",
                        string(block.language.as_deref().unwrap_or("plaintext")),
                    );
                    write_plain(txn, element, &block.text());
                }
                _ => write_runs(txn, element, &block.runs, resolver),
            }
        }
        Kind::BulletedItem
        | Kind::NumberedItem
        | Kind::Todo
        | Kind::Toggle
        | Kind::Quote
        | Kind::Callout => {
            if block.kind == Kind::Todo {
                element.insert_attribute(txn, "checked", Any::Bool(block.checked.unwrap_or(false)));
            }
            let first = Place::Element(element.clone())
                .children(&*txn)
                .into_iter()
                .next();
            match first {
                Some(XmlOut::Element(lead)) if &**lead.tag() == "paragraph" => {
                    clear(txn, &lead);
                    write_runs(txn, &lead, &block.runs, resolver);
                }
                _ => {
                    let lead = element.insert(txn, 0, XmlElementPrelim::empty("paragraph"));
                    write_runs(txn, &lead, &block.runs, resolver);
                }
            }
        }
        Kind::Image | Kind::File => {
            if let Some(file) = &block.file {
                element.insert_attribute(txn, "blob", string(&file.blob));
                element.insert_attribute(txn, "name", string(&file.name));
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "a file's size is far below 2^53"
                )]
                let size = file.size as f64;
                element.insert_attribute(txn, "size", Any::Number(size));
            }
        }
        Kind::External => {
            element.insert_attribute(
                txn,
                "kind",
                string(block.label.as_deref().unwrap_or("unsupported")),
            );
        }
        Kind::Divider | Kind::PageLink => {}
    }
    true
}

/// Remove the block with stable id `bid`, with the blocks under it; its
/// list too when it was the list's last item. `false` when there is none.
pub fn remove(txn: &mut TransactionMut, root: &XmlFragmentRef, bid: Uuid) -> bool {
    let Some(found) = locate(&*txn, &Place::Root(root.clone()), bid) else {
        return false;
    };
    found.place.remove(txn, found.index);
    if let Some(list) = found.list
        && list.element.len(&*txn) == 0
    {
        list.place.remove(txn, list.index);
    }
    true
}

/// Where a new first block under `place` goes: after the own text of a block
/// that leads with it, first anywhere else.
pub fn first_index<T: ReadTxn>(txn: &T, place: &Place) -> u32 {
    match place {
        Place::Root(_) => 0,
        Place::Element(element) => {
            let tag = element.tag().to_string();
            let leads = matches!(
                tag.as_str(),
                "listItem" | "taskItem" | "toggle" | "blockquote" | "callout"
            );
            let first = place.children(txn).into_iter().next();
            let paragraph_first = matches!(
                first,
                Some(XmlOut::Element(lead)) if &**lead.tag() == "paragraph"
            );
            u32::from(leads && paragraph_first)
        }
    }
}

/// Make the `conflict` of block `of` at `index` of `place`: Notion's
/// version `theirs` (the block as it is there, or none when Notion removed
/// it), with the hash and edit time that move the base when it is settled.
#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
pub fn build_conflict(
    txn: &mut TransactionMut,
    place: &Place,
    index: u32,
    of: Uuid,
    (hash, edited): ([u8; 32], Option<DateTime<Utc>>),
    theirs: Option<&Block>,
    resolver: &Resolver,
) -> XmlElementRef {
    let element = place.insert_element(txn, index, "conflict");
    element.insert_attribute(txn, "of", string(&of.to_string()));
    element.insert_attribute(txn, "hash", string(&hash_hex(&hash)));
    if let Some(edited) = edited {
        element.insert_attribute(
            txn,
            "edited",
            string(&edited.to_rfc3339_opts(SecondsFormat::Millis, true)),
        );
    }
    element.insert_attribute(txn, "removed", Any::Bool(theirs.is_none()));
    if let Some(theirs) = theirs {
        build(
            txn,
            &Place::Element(element.clone()),
            0,
            theirs,
            None,
            resolver,
        );
    }
    element
}

/// Give every block of the body a stable id it lacks (not the own text a
/// block leads with, not a conflict, not a record shown in the page):
/// blocks written before ids, or by a client that did not add one. Returns
/// how many were given one.
pub fn assign_ids(
    txn: &mut TransactionMut,
    root: &XmlFragmentRef,
    fresh: &mut impl FnMut() -> Uuid,
) -> usize {
    assign_under(txn, &Place::Root(root.clone()), fresh, 0)
}

/// The blocks under `place`, from child `skip` on.
fn assign_under(
    txn: &mut TransactionMut,
    place: &Place,
    fresh: &mut impl FnMut() -> Uuid,
    skip: usize,
) -> usize {
    let mut given = 0;
    for child in place.children(&*txn).into_iter().skip(skip) {
        if let XmlOut::Element(element) = child {
            given += assign_one(txn, &element, fresh);
        }
    }
    given
}

fn assign_one(
    txn: &mut TransactionMut,
    element: &XmlElementRef,
    fresh: &mut impl FnMut() -> Uuid,
) -> usize {
    let tag = element.tag().to_string();
    let lacks = text_attribute(&attributes(&*txn, element), "bid").is_none();
    let mut given = 0;
    match tag.as_str() {
        "bulletList" | "orderedList" | "taskList" => {
            given += assign_under(txn, &Place::Element(element.clone()), fresh, 0);
        }
        "paragraph" | "heading" | "codeBlock" | "horizontalRule" | "image" | "file"
        | "pageLink" | "external" => {
            if lacks {
                element.insert_attribute(txn, "bid", string(&fresh().to_string()));
                given += 1;
            }
        }
        "listItem" | "taskItem" | "toggle" | "blockquote" | "callout" => {
            if lacks {
                element.insert_attribute(txn, "bid", string(&fresh().to_string()));
                given += 1;
            }
            // The first paragraph is the block's own text, not a block.
            let place = Place::Element(element.clone());
            let skip = usize::from(matches!(
                place.children(&*txn).first(),
                Some(XmlOut::Element(lead)) if &**lead.tag() == "paragraph"
            ));
            given += assign_under(txn, &place, fresh, skip);
        }
        _ => {}
    }
    given
}

#[cfg(test)]
mod tests;
