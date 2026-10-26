//! What a member's edits ask of Notion: updates, appends in order, deletes,
//! and the things that are never sent.

use super::*;
use crate::domain::notion::{
    edit::{self, Place},
    model::FileRef,
    tests::{Synced, block, id, para, resolver, snapshot},
};

fn plan_of(s: &Synced) -> Plan {
    let prepared = prepare(&s.doc, &s.resolver).expect("a valid body");
    plan(&prepared.tree, &s.mappings, s.resolver.page_link())
}

/// Add a block of ours after the block `after` (none: first), with its id.
fn add_after(s: &Synced, after: Option<Uuid>, block: &Block, bid: Uuid) {
    let root = s.doc.get_or_insert_xml_fragment(document::ROOT);
    let mut txn = s.doc.transact_mut();
    let (place, index) = match after {
        Some(after) => edit::locate(&txn, &Place::Root(root.clone()), after)
            .expect("the block is there")
            .after(),
        None => (Place::Root(root), 0),
    };
    edit::build(&mut txn, &place, index, block, Some(bid), &s.resolver).expect("made");
}

fn synced(blocks: Vec<Block>) -> Synced {
    let mut s = Synced::new();
    s.apply(&snapshot(blocks, 5, true));
    s
}

#[test]
fn a_page_in_step_asks_for_nothing() {
    let s = synced(vec![para(1, "A", 1), para(2, "B", 1)]);
    let plan = plan_of(&s);
    assert!(plan.items.is_empty());
    assert!(plan.document.items.is_empty());
    assert_eq!(plan.document.version, 1);
}

#[test]
fn an_edited_block_is_an_update_with_the_base_it_was_edited_from() {
    let s = synced(vec![para(1, "A", 1), para(2, "B", 1)]);
    let b = s.bid_of("B");
    s.edit_text(b, "B, edited");
    let plan = plan_of(&s);
    assert_eq!(plan.items.len(), 1);
    let item = &plan.items[0];
    assert_eq!(item.action, Do::Update);
    assert_eq!(item.bid, b);
    assert_eq!(item.external, Some(id(2)));
    assert_eq!(item.base_edited, Some(crate::domain::notion::tests::at(1)));
    let sent = plan.document.items[0].block.as_ref().unwrap();
    assert_eq!(sent.id, b, "the block's id here, not Notion's");
    assert_eq!(sent.text(), "B, edited");
    assert_eq!(plan.document.items[0].seq, 0);
}

#[test]
fn a_block_waiting_on_a_conflict_is_not_sent() {
    let mut s = synced(vec![para(1, "A", 1)]);
    let a = s.bid_of("A");
    s.edit_text(a, "Mine");
    s.apply(&snapshot(vec![para(1, "Theirs", 7)], 8, true));
    assert_eq!(s.tree().conflicts.len(), 1);
    assert!(plan_of(&s).items.is_empty(), "neither is sent until the member chooses");
}

#[test]
fn new_blocks_are_appended_in_document_order_each_after_the_one_before() {
    let s = synced(vec![para(1, "A", 1), para(2, "B", 1)]);
    let a = s.bid_of("A");
    let (x, y) = (Uuid::now_v7(), Uuid::now_v7());
    add_after(&s, Some(a), &Block::paragraph(x, "X"), x);
    add_after(&s, Some(x), &Block::paragraph(y, "Y"), y);
    let plan = plan_of(&s);
    assert_eq!(plan.items.len(), 2);
    assert_eq!(plan.items[0].action, Do::Append);
    assert_eq!(plan.items[0].bid, x);
    assert_eq!(plan.items[0].after, Some(id(1)), "after A at Notion");
    assert_eq!(plan.items[0].after_block, None);
    assert_eq!(plan.items[1].bid, y);
    assert_eq!(plan.items[1].after, None);
    assert_eq!(plan.items[1].after_block, Some(x), "after X, made first");
    assert_eq!(plan.items[0].parent, None);
    assert_eq!(plan.items[0].base_edited, None);
    assert_eq!(plan.document.items.len(), 2);
    assert_eq!(plan.document.items[1].block.as_ref().unwrap().text(), "Y");
}

#[test]
fn a_new_block_under_a_synced_toggle_and_under_a_new_one_say_where() {
    let mut toggle = block(10, Kind::Toggle, "More", 1);
    toggle.children = vec![para(11, "Inside", 1)];
    let s = synced(vec![toggle]);
    let inside = s.bid_of("Inside");
    let (n, m) = (Uuid::now_v7(), Uuid::now_v7());
    // After the existing child, under the synced toggle.
    add_after(&s, Some(inside), &Block::paragraph(n, "New child"), n);
    // A new toggle at the end, with a child of its own.
    let new_toggle = Uuid::now_v7();
    {
        let root = s.doc.get_or_insert_xml_fragment(document::ROOT);
        let mut txn = s.doc.transact_mut();
        let len = root.len(&txn);
        let built = edit::build(
            &mut txn,
            &Place::Root(root),
            len,
            &Block::new(new_toggle, Kind::Toggle),
            Some(new_toggle),
            &s.resolver,
        )
        .unwrap();
        edit::build(
            &mut txn,
            &Place::Element(built.element),
            1,
            &Block::paragraph(m, "Under new"),
            Some(m),
            &s.resolver,
        );
    }
    let plan = plan_of(&s);
    let by_bid = |bid: Uuid| plan.items.iter().find(|i| i.bid == bid).unwrap();
    assert_eq!(by_bid(n).parent, Some(id(10)));
    assert_eq!(by_bid(n).after, Some(id(11)));
    assert_eq!(by_bid(new_toggle).parent, None);
    assert_eq!(by_bid(new_toggle).after, Some(id(10)));
    assert_eq!(by_bid(m).parent_block, Some(new_toggle));
    assert_eq!(by_bid(m).parent, None);
    // A parent is made before what goes under it.
    let at = |bid: Uuid| plan.items.iter().position(|i| i.bid == bid).unwrap();
    assert!(at(new_toggle) < at(m));
}

#[test]
fn a_deleted_block_is_a_delete_but_never_a_page_or_a_kept_block() {
    let mut link = block(2, Kind::PageLink, "Sub-page", 1);
    link.page = Some(id(0x70));
    let mut kept = block(3, Kind::External, "", 1);
    kept.label = Some("column_list".to_owned());
    let mut s = Synced::new();
    s.resolver.page_of.insert(id(0x70), Uuid::now_v7());
    s.apply(&snapshot(vec![para(1, "A", 1), link, kept], 5, true));
    assert_eq!(s.kinds(), [Kind::Paragraph, Kind::PageLink, Kind::External]);
    let a = s.bid_of("A");
    // The member deletes every block.
    for node in s.tree().all() {
        s.delete(node.bid.unwrap());
    }
    let plan = plan_of(&s);
    assert_eq!(plan.items.len(), 1, "{:?}", plan.items);
    assert_eq!(plan.items[0].action, Do::Delete);
    assert_eq!(plan.items[0].bid, a);
    assert_eq!(plan.items[0].external, Some(id(1)));
    assert_eq!(plan.items[0].base_edited, Some(crate::domain::notion::tests::at(1)));
    assert!(plan.document.items[0].block.is_none());
    assert!(plan.items.iter().all(|i| i.kind != Kind::PageLink));
}

#[test]
fn kept_blocks_are_never_sent_and_files_go_as_a_link_to_the_page() {
    let s = synced(vec![para(1, "A", 1)]);
    let a = s.bid_of("A");
    let (kept, image, rule) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let mut external = Block::new(kept, Kind::External);
    external.label = Some("column_list".to_owned());
    add_after(&s, Some(a), &external, kept);
    let mut file = Block::new(image, Kind::Image);
    file.file = Some(FileRef {
        blob: "a".repeat(64),
        size: 3,
        content_type: "image/png".to_owned(),
        name: "diagram.png".to_owned(),
    });
    add_after(&s, Some(kept), &file, image);
    add_after(&s, Some(image), &Block::new(rule, Kind::Divider), rule);
    let plan = plan_of(&s);
    assert!(
        plan.items.iter().all(|i| i.bid != kept),
        "a block kept in Notion is never sent"
    );
    let sent_image = plan.items.iter().find(|i| i.bid == image).unwrap();
    assert_eq!(sent_image.kind, Kind::Paragraph, "a file goes as a paragraph");
    let block = plan
        .document
        .items
        .iter()
        .filter_map(|i| i.block.as_ref())
        .find(|b| b.id == image)
        .unwrap();
    assert_eq!(block.text(), "diagram.png");
    assert_eq!(
        block.runs[0].link.as_deref(),
        s.resolver.page_link().as_deref(),
        "it links to the page: the bytes stay here"
    );
    assert!(plan.items.iter().any(|i| i.bid == rule && i.kind == Kind::Divider));
}

#[test]
fn a_block_not_editable_here_is_not_an_update() {
    let mut kept = block(2, Kind::External, "", 1);
    kept.label = Some("column_list".to_owned());
    let s = synced(vec![para(1, "A", 1), kept]);
    assert!(plan_of(&s).items.is_empty());
}

#[test]
fn preparing_gives_every_block_an_id_and_reports_the_update_to_store() {
    let doc = crate::testkit::docs::client(5);
    crate::testkit::docs::paragraph(&doc, "no id yet");
    let local = yrs::Doc::new();
    document::apply(&local, &document::full_state(&doc)).unwrap();
    let prepared = prepare(&local, &resolver()).unwrap();
    assert_eq!(prepared.ids_given, 1);
    assert!(prepared.update.is_some());
    assert!(prepared.tree.nodes[0].bid.is_some());
    let again = prepare(&local, &resolver()).unwrap();
    assert_eq!(again.ids_given, 0);
    assert!(again.update.is_none());
}
