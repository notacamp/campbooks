//! Changing a body block by block: only the blocks asked for are touched,
//! and what other people wrote beside them stays.

use uuid::Uuid;
use yrs::{Doc, Transact};

use super::*;
use crate::domain::{
    document,
    notion::{
        model::{Block, Kind, Run},
        tests::{Synced, block, id, para, resolver, snapshot},
    },
};

fn root(doc: &Doc) -> XmlFragmentRef {
    doc.get_or_insert_xml_fragment(document::ROOT)
}

fn place(doc: &Doc) -> Place {
    Place::Root(root(doc))
}

#[test]
fn a_block_is_found_by_its_id_at_any_depth_and_inside_its_list() {
    let mut s = Synced::new();
    let mut toggle = block(1, Kind::Toggle, "More", 1);
    toggle.children = vec![
        block(2, Kind::BulletedItem, "One", 1),
        block(3, Kind::BulletedItem, "Two", 1),
    ];
    s.apply(&snapshot(vec![toggle], 5, true));
    let two = s.bid_of("Two");
    let txn = s.doc.transact();
    let found = locate(&txn, &place(&s.doc), two).expect("found");
    assert_eq!(found.index, 1);
    let list = found.list.as_ref().expect("an item of a list");
    assert_eq!(&**list.element.tag(), "bulletList");
    let (after_place, after_index) = found.after();
    assert!(matches!(after_place, Place::Element(_)));
    assert_eq!(after_index, list.index + 1, "a sibling goes after the list");
    assert!(locate(&txn, &place(&s.doc), Uuid::now_v7()).is_none());
}

#[test]
fn rewriting_keeps_the_element_and_the_neighbours() {
    let mut s = Synced::new();
    s.apply(&snapshot(vec![para(1, "A", 1), para(2, "B", 1), para(3, "C", 1)], 5, true));
    let (a, b) = (s.bid_of("A"), s.bid_of("B"));
    let mut next = Block::new(b, Kind::Paragraph);
    next.runs = vec![Run {
        text: "Bee".to_owned(),
        bold: true,
        ..Run::default()
    }];
    {
        let mut txn = s.doc.transact_mut();
        let found = locate(&txn, &place(&s.doc), b).unwrap();
        assert!(rewrite(&mut txn, &found, &next, &resolver()));
        // Another kind cannot be written into this element.
        let heading = Block::new(b, Kind::Heading);
        assert!(!rewrite(&mut txn, &found, &heading, &resolver()));
    }
    assert_eq!(s.texts(), ["A", "Bee", "C"]);
    assert_eq!(s.bid_of("A"), a);
    assert_eq!(s.bid_of("Bee"), b, "the same block keeps its id");
    assert!(s.tree().find(b).unwrap().block.runs[0].bold);
}

#[test]
fn removing_a_block_takes_what_is_under_it_and_an_emptied_list() {
    let mut s = Synced::new();
    let mut toggle = block(1, Kind::Toggle, "More", 1);
    toggle.children = vec![para(2, "Inside", 1)];
    s.apply(&snapshot(
        vec![toggle, block(3, Kind::BulletedItem, "Only item", 1), para(4, "After", 1)],
        5,
        true,
    ));
    let (more, item) = (s.bid_of("More"), s.bid_of("Only item"));
    {
        let r = root(&s.doc);
        let mut txn = s.doc.transact_mut();
        assert!(remove(&mut txn, &r, more));
        assert!(remove(&mut txn, &r, item));
        assert!(!remove(&mut txn, &r, Uuid::now_v7()));
    }
    assert_eq!(s.texts(), ["After"]);
    let txn = s.doc.transact();
    assert_eq!(root(&s.doc).len(&txn), 1, "the emptied list went too");
}

#[test]
fn a_new_first_block_goes_after_the_own_text_of_a_block_that_leads_with_it() {
    let mut s = Synced::new();
    s.apply(&snapshot(vec![block(1, Kind::Toggle, "More", 1), para(2, "Body", 1)], 5, true));
    let more = s.bid_of("More");
    let txn = s.doc.transact();
    assert_eq!(first_index(&txn, &place(&s.doc)), 0);
    let found = locate(&txn, &place(&s.doc), more).unwrap();
    assert_eq!(first_index(&txn, &Place::Element(found.element)), 1);
}

#[test]
fn blocks_lacking_an_id_are_given_one_but_not_their_own_text() {
    let doc = crate::testkit::docs::client(5);
    crate::testkit::docs::paragraph(&doc, "no id yet");
    let mut n = 0_u128;
    let given = {
        let r = root(&doc);
        let mut txn = doc.transact_mut();
        assign_ids(&mut txn, &r, &mut || {
            n += 1;
            id(n)
        })
    };
    assert_eq!(given, 1);
    let again = {
        let r = root(&doc);
        let mut txn = doc.transact_mut();
        assign_ids(&mut txn, &r, &mut Uuid::now_v7)
    };
    assert_eq!(again, 0, "a block that has one keeps it");

    // A toggle gets one for itself and for the blocks under it, not for the
    // paragraph that is its own text.
    let mut s = Synced::new();
    let mut toggle = block(1, Kind::Toggle, "More", 1);
    toggle.children = vec![para(2, "Inside", 1)];
    let merged = s.apply(&snapshot(vec![toggle], 5, true));
    assert_eq!(merged.ids_given, 0, "blocks made from Notion are given theirs on the way");
    assert_eq!(s.tree().all().len(), 2);
}

#[test]
fn a_conflict_is_made_beneath_a_block_and_listed() {
    let mut s = Synced::new();
    s.apply(&snapshot(vec![para(1, "A", 1)], 5, true));
    let a = s.bid_of("A");
    {
        let mut txn = s.doc.transact_mut();
        let found = locate(&txn, &place(&s.doc), a).unwrap();
        let (p, i) = found.after();
        build_conflict(
            &mut txn,
            &p,
            i,
            a,
            ([7; 32], Some(crate::domain::notion::tests::at(3))),
            Some(&Block::paragraph(id(9), "Theirs")),
            &resolver(),
        );
    }
    let txn = s.doc.transact();
    let listed = conflicts(&txn, &place(&s.doc));
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, a);
    drop(txn);
    document::inspect(&s.doc).expect("the body holds a conflict");
    assert_eq!(s.tree().conflicts.len(), 1);
}

#[test]
fn a_mention_of_a_synced_page_is_a_mention_and_of_another_a_link_to_notion() {
    let page = Uuid::now_v7();
    let mut resolver = resolver();
    resolver.page_of.insert(id(0x55), page);
    let doc = Doc::new();
    {
        let r = root(&doc);
        let mut txn = doc.transact_mut();
        let built = build(
            &mut txn,
            &Place::Root(r),
            0,
            &{
                let mut b = Block::new(id(1), Kind::Paragraph);
                b.runs = vec![
                    Run::plain("see "),
                    Run {
                        text: "Roadmap".to_owned(),
                        page: Some(id(0x55)),
                        ..Run::default()
                    },
                    Run {
                        text: "Elsewhere".to_owned(),
                        page: Some(id(0x56)),
                        ..Run::default()
                    },
                ];
                b
            },
            Some(id(100)),
            &resolver,
        )
        .expect("a paragraph can be made");
        assert!(built.list.is_none());
    }
    let inspected = document::inspect(&doc).unwrap();
    assert!(inspected.links.iter().any(|l| l.ref_id == page.to_string()));
    assert!(inspected.text.contains("Elsewhere"), "{}", inspected.text);
}

#[test]
fn a_sub_page_link_is_made_only_for_a_page_known_here() {
    let doc = Doc::new();
    let mut link = Block::new(id(1), Kind::PageLink);
    link.page = Some(id(0x66));
    let r = root(&doc);
    let mut txn = doc.transact_mut();
    assert!(build(&mut txn, &Place::Root(r.clone()), 0, &link, None, &resolver()).is_none());
    let mut known = resolver();
    known.page_of.insert(id(0x66), Uuid::now_v7());
    assert!(build(&mut txn, &Place::Root(r), 0, &link, None, &known).is_some());
}

#[test]
fn unsafe_links_are_not_written() {
    let doc = Doc::new();
    let r = root(&doc);
    {
        let mut txn = doc.transact_mut();
        let mut b = Block::new(id(1), Kind::Paragraph);
        b.runs = vec![Run {
            text: "click".to_owned(),
            link: Some("javascript:alert(1)".to_owned()),
            ..Run::default()
        }];
        build(&mut txn, &Place::Root(r), 0, &b, None, &resolver());
    }
    // The body would be refused for a bad link; none was written.
    document::inspect(&doc).expect("the body holds only safe links");
}
