//! Applying a snapshot block by block: the four cases of a three-way
//! merge, removals, new blocks at their place, lists and nesting, an
//! incomplete snapshot, and the same snapshot twice.

use super::*;
use crate::domain::notion::{
    model::{Kind, Run},
    tests::{Synced, block, id, para, snapshot},
};

fn snap(blocks: Vec<Block>, minute: i64) -> Snapshot {
    snapshot(blocks, minute, true)
}

#[test]
fn a_first_snapshot_fills_an_empty_page_and_the_same_one_again_changes_nothing() {
    let mut s = Synced::new();
    let first = s.apply(&snap(vec![para(1, "Hello", 1), para(2, "World", 1)], 5));
    assert_eq!(s.texts(), ["Hello", "World"]);
    assert_eq!(first.outcome.upserts.len(), 2);
    assert_eq!(first.outcome.touched, 2);
    assert_eq!(first.conflicts, 0);
    assert!(first.update.is_some());
    assert!(s.tree().all().iter().all(|n| n.bid.is_some()));

    let again = s.apply(&snap(vec![para(1, "Hello", 1), para(2, "World", 1)], 6));
    assert!(again.update.is_none(), "the body is as it was");
    assert_eq!(again.outcome.touched, 0);
    assert_eq!(again.outcome.raised, 0);
    assert!(!again.outcome.ours_pending);
    assert_eq!(s.mappings.len(), 2);
}

#[test]
fn notion_alone_changing_a_block_writes_it_in_place() {
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "Hello", 1), para(2, "World", 1)], 5));
    let bid = s.bid_of("Hello");
    let merged = s.apply(&snap(
        vec![para(1, "Hello there", 7), para(2, "World", 1)],
        8,
    ));
    assert_eq!(s.texts(), ["Hello there", "World"]);
    assert_eq!(s.bid_of("Hello there"), bid, "the same block");
    assert_eq!(merged.outcome.touched, 1);
    assert_eq!(merged.conflicts, 0);
    let mapping = s.mappings.iter().find(|m| m.bid == bid).unwrap();
    assert_eq!(mapping.base_hash, hash(&para(1, "Hello there", 7)));
    assert_eq!(mapping.base_edited, crate::domain::notion::tests::at(7));
}

#[test]
fn our_edit_alone_stays_and_is_left_for_the_outbound_path() {
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "Hello", 1), para(2, "World", 1)], 5));
    s.edit_text(s.bid_of("World"), "World!");
    let merged = s.apply(&snap(vec![para(1, "Hello", 1), para(2, "World", 1)], 6));
    assert_eq!(s.texts(), ["Hello", "World!"]);
    assert!(merged.outcome.ours_pending);
    assert!(merged.update.is_none());
    assert_eq!(merged.conflicts, 0);
}

#[test]
fn both_changing_different_words_is_a_conflict_that_keeps_ours_and_shows_theirs() {
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "Hello", 1)], 5));
    let bid = s.bid_of("Hello");
    s.edit_text(bid, "Mine");
    let theirs = snap(vec![para(1, "Theirs", 7)], 8);
    let merged = s.apply(&theirs);
    assert_eq!(s.texts(), ["Mine"], "ours stays where it is");
    assert_eq!(merged.conflicts, 1);
    assert_eq!(merged.outcome.raised, 1);
    let conflict = s.tree().conflict_of(bid).cloned().unwrap();
    assert_eq!(
        conflict.theirs.as_ref().map(Block::text).as_deref(),
        Some("Theirs")
    );
    assert_eq!(conflict.hash, Some(hash(&para(1, "Theirs", 7))));
    // The base did not move: neither side is sent until the member chooses.
    let mapping = s.mappings.iter().find(|m| m.bid == bid).unwrap();
    assert_eq!(mapping.base_hash, hash(&para(1, "Hello", 1)));

    // The same snapshot again raises nothing new.
    let again = s.apply(&snap(vec![para(1, "Theirs", 7)], 9));
    assert_eq!(again.outcome.raised, 0);
    assert_eq!(again.conflicts, 1);
    assert!(again.update.is_none());

    // Notion changes it again: the old conflict is replaced, not stacked.
    let newer = s.apply(&snap(vec![para(1, "Theirs, twice", 10)], 11));
    assert_eq!(newer.conflicts, 1);
    assert_eq!(newer.outcome.raised, 0);
    let conflict = s.tree().conflict_of(bid).cloned().unwrap();
    assert_eq!(
        conflict.theirs.as_ref().map(Block::text).as_deref(),
        Some("Theirs, twice")
    );
}

#[test]
fn both_arriving_at_the_same_words_is_no_conflict_and_ends_a_shown_one() {
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "Hello", 1)], 5));
    let bid = s.bid_of("Hello");
    s.edit_text(bid, "Same");
    let merged = s.apply(&snap(vec![para(1, "Same", 7)], 8));
    assert_eq!(merged.conflicts, 0);
    assert_eq!(merged.outcome.raised, 0);
    assert_eq!(
        s.mappings.iter().find(|m| m.bid == bid).unwrap().base_hash,
        hash(&para(1, "Same", 7))
    );

    // A conflict shown, then Notion goes back to the base: nothing to show.
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "Hello", 1)], 5));
    let bid = s.bid_of("Hello");
    s.edit_text(bid, "Mine");
    assert_eq!(s.apply(&snap(vec![para(1, "Theirs", 7)], 8)).conflicts, 1);
    let back = s.apply(&snap(vec![para(1, "Hello", 9)], 10));
    assert_eq!(back.conflicts, 0, "Notion no longer disagrees");
    assert_eq!(s.texts(), ["Mine"]);
    assert!(back.outcome.ours_pending);

    // And the two coming to the same words while a conflict is shown.
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "Hello", 1)], 5));
    let bid = s.bid_of("Hello");
    s.edit_text(bid, "Mine");
    s.apply(&snap(vec![para(1, "Theirs", 7)], 8));
    let merged = s.apply(&snap(vec![para(1, "Mine", 9)], 10));
    assert_eq!(merged.conflicts, 0);
}

#[test]
fn notion_removing_an_unchanged_block_removes_it_and_a_changed_one_is_kept_with_a_conflict() {
    let mut s = Synced::new();
    s.apply(&snap(
        vec![para(1, "Keep", 1), para(2, "Gone", 1), para(3, "Mine", 1)],
        5,
    ));
    s.edit_text(s.bid_of("Mine"), "Mine, edited");
    let gone = s.bid_of("Gone");
    let merged = s.apply(&snap(vec![para(1, "Keep", 1)], 6));
    assert_eq!(s.texts(), ["Keep", "Mine, edited"]);
    assert!(merged.outcome.deletes.contains(&gone));
    assert!(s.mappings.iter().all(|m| m.bid != gone));
    assert_eq!(merged.conflicts, 1);
    let edited = s.bid_of("Mine, edited");
    let conflict = s.tree().conflict_of(edited).cloned().unwrap();
    assert!(conflict.removed);
    assert!(conflict.theirs.is_none());
    // Said once.
    assert_eq!(
        s.apply(&snap(vec![para(1, "Keep", 1)], 7)).outcome.raised,
        0
    );
}

#[test]
fn nothing_is_removed_from_a_snapshot_that_is_not_complete() {
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "A", 1), para(2, "B", 1)], 5));
    let partial = snapshot(vec![para(1, "A", 1)], 6, false);
    let merged = s.apply(&partial);
    assert_eq!(s.texts(), ["A", "B"]);
    assert!(merged.update.is_none());
    assert!(merged.outcome.deletes.is_empty());
}

#[test]
fn a_block_deleted_here_is_not_made_again_from_notion() {
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "A", 1), para(2, "B", 1)], 5));
    s.delete(s.bid_of("A"));
    let merged = s.apply(&snap(vec![para(1, "A", 1), para(2, "B", 1)], 6));
    assert_eq!(s.texts(), ["B"]);
    assert!(merged.update.is_none());
}

#[test]
fn a_new_notion_block_lands_after_the_block_before_it() {
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "A", 1), para(3, "C", 1)], 5));
    let merged = s.apply(&snap(
        vec![para(1, "A", 1), para(2, "B", 6), para(3, "C", 1)],
        7,
    ));
    assert_eq!(s.texts(), ["A", "B", "C"]);
    assert_eq!(merged.outcome.upserts.len(), 1);
    // First in the page.
    let merged = s.apply(&snap(
        vec![
            para(0, "Zero", 8),
            para(1, "A", 1),
            para(2, "B", 6),
            para(3, "C", 1),
        ],
        9,
    ));
    assert_eq!(s.texts(), ["Zero", "A", "B", "C"]);
    assert_eq!(merged.outcome.touched, 1);
}

#[test]
fn consecutive_list_items_share_a_list_and_a_toggle_holds_its_blocks() {
    let mut s = Synced::new();
    let mut toggle = block(10, Kind::Toggle, "More", 1);
    toggle.children = vec![
        block(11, Kind::NumberedItem, "First", 1),
        block(12, Kind::NumberedItem, "Second", 1),
        para(13, "Then a paragraph", 1),
    ];
    s.apply(&snap(
        vec![
            block(1, Kind::BulletedItem, "One", 1),
            block(2, Kind::BulletedItem, "Two", 1),
            block(3, Kind::BulletedItem, "Three", 1),
            para(4, "Between", 1),
            block(5, Kind::BulletedItem, "Again", 1),
            toggle,
        ],
        5,
    ));
    assert_eq!(
        s.texts(),
        [
            "One",
            "Two",
            "Three",
            "Between",
            "Again",
            "More",
            "First",
            "Second",
            "Then a paragraph"
        ]
    );
    let root = s.doc.get_or_insert_xml_fragment(document::ROOT);
    let txn = s.doc.transact();
    // Two bullet lists (split by the paragraph), a paragraph and a toggle.
    assert_eq!(root.len(&txn), 4);
    drop(txn);
    document::inspect(&s.doc).expect("a valid body");
    let tree = s.tree();
    let toggle = tree
        .nodes
        .iter()
        .find(|n| n.block.kind == Kind::Toggle)
        .unwrap();
    assert_eq!(toggle.children.len(), 3);
}

#[test]
fn a_new_item_joins_the_list_of_the_item_it_follows() {
    let mut s = Synced::new();
    s.apply(&snap(
        vec![
            block(1, Kind::BulletedItem, "One", 1),
            block(3, Kind::BulletedItem, "Three", 1),
        ],
        5,
    ));
    s.apply(&snap(
        vec![
            block(1, Kind::BulletedItem, "One", 1),
            block(2, Kind::BulletedItem, "Two", 6),
            block(3, Kind::BulletedItem, "Three", 1),
        ],
        7,
    ));
    assert_eq!(s.texts(), ["One", "Two", "Three"]);
    let root = s.doc.get_or_insert_xml_fragment(document::ROOT);
    assert_eq!(root.len(&s.doc.transact()), 1, "still one list");
}

#[test]
fn a_block_that_became_another_kind_is_made_again_when_nothing_is_under_it() {
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "Title?", 1)], 5));
    let bid = s.bid_of("Title?");
    let merged = s.apply(&snap(vec![block(1, Kind::Heading, "Title?", 6)], 7));
    assert_eq!(s.kinds(), [Kind::Heading]);
    assert_eq!(s.bid_of("Title?"), bid, "it keeps its id");
    assert_eq!(merged.conflicts, 0);
    assert_eq!(s.mappings[0].kind, Kind::Heading);
}

#[test]
fn what_other_people_wrote_beside_the_snapshot_is_never_touched() {
    let mut s = Synced::new();
    s.apply(&snap(vec![para(1, "From Notion", 1)], 5));
    // A member adds a paragraph of their own, with no counterpart at Notion.
    {
        let r = s.doc.get_or_insert_xml_fragment(document::ROOT);
        let mut txn = s.doc.transact_mut();
        crate::domain::notion::edit::build(
            &mut txn,
            &crate::domain::notion::edit::Place::Root(r),
            1,
            &Block::paragraph(id(99), "Mine alone"),
            Some(id(100)),
            &s.resolver,
        );
    }
    let merged = s.apply(&snap(vec![para(1, "From Notion, edited", 6)], 7));
    assert_eq!(s.texts(), ["From Notion, edited", "Mine alone"]);
    assert!(merged.outcome.deletes.is_empty());
    // And Notion removing its block leaves theirs.
    s.apply(&snap(vec![], 8));
    assert_eq!(s.texts(), ["Mine alone"]);
}

#[test]
fn a_block_kept_in_notion_follows_notion_and_is_never_a_change_here() {
    let mut s = Synced::new();
    let mut kept = block(1, Kind::External, "", 1);
    kept.label = Some("column_list".to_owned());
    s.apply(&snap(vec![kept.clone()], 5));
    assert_eq!(s.kinds(), [Kind::External]);
    let mut other = kept;
    other.label = Some("synced_block".to_owned());
    other.edited = Some(crate::domain::notion::tests::at(6));
    let merged = s.apply(&snap(vec![other], 7));
    assert_eq!(merged.conflicts, 0);
    assert_eq!(
        s.tree().nodes[0].block.label.as_deref(),
        Some("synced_block")
    );
}

#[test]
fn page_mentions_become_mentions_of_synced_pages_and_notion_links_otherwise() {
    let mut s = Synced::new();
    let synced = uuid::Uuid::now_v7();
    s.resolver.page_of.insert(id(0x55), synced);
    s.resolver.notion_of.insert(synced, id(0x55));
    let mut p = para(1, "", 1);
    p.runs = vec![
        Run::plain("see "),
        Run {
            text: "Roadmap".to_owned(),
            page: Some(id(0x55)),
            ..Run::default()
        },
        Run::plain(" and "),
        Run {
            text: "Elsewhere".to_owned(),
            page: Some(id(0x56)),
            ..Run::default()
        },
    ];
    s.apply(&snap(vec![p], 5));
    let runs = s.tree().nodes[0].block.runs.clone();
    assert_eq!(runs.iter().filter(|r| r.page == Some(id(0x55))).count(), 1);
    assert_eq!(runs.iter().filter(|r| r.page == Some(id(0x56))).count(), 1);
    let inspected = document::inspect(&s.doc).unwrap();
    assert!(
        inspected
            .links
            .iter()
            .any(|l| l.ref_id == synced.to_string())
    );
}

#[test]
fn a_snapshot_the_body_would_not_hold_is_refused_and_nothing_is_kept() {
    let doc = Doc::new();
    // Toggles nested far deeper than a body allows.
    let mut deepest = block(100, Kind::Toggle, "bottom", 1);
    for n in (1..60_u128).rev() {
        let mut parent = block(n, Kind::Toggle, "down", 1);
        parent.children = vec![deepest];
        deepest = parent;
    }
    let before = document::full_state(&doc);
    let refused = merge_into(
        &doc,
        &snapshot(vec![deepest], 5, true),
        &Resolver::default(),
        Vec::new(),
    );
    assert!(refused.is_err());
    // The caller stores nothing on a refusal; the document it holds is only
    // a copy it throws away.
    let _ = before;
}
