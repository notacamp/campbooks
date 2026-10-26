//! Reading a body as neutral blocks, and what a mention becomes for Notion:
//! another synced page is a page mention, anything else of Campbooks is the
//! words "Campbooks link" and an address of this app, never its subject.

use std::sync::Arc;

use uuid::Uuid;
use yrs::{Any, Doc, Transact, Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim};

use super::*;
use crate::{
    domain::{
        document,
        notion::{
            model::Kind,
            tests::{Synced, block, id, para, snapshot},
        },
    },
    testkit::docs,
};

fn read_doc(doc: &Doc, resolver: &Resolver) -> Tree {
    let root = doc.get_or_insert_xml_fragment(document::ROOT);
    let txn = doc.transact();
    read(&txn, &root, resolver)
}

/// A paragraph that says "see " and mentions `ref_type`/`ref_id`.
fn mentioning(ref_type: &str, ref_id: &str) -> Doc {
    let doc = docs::client(7);
    docs::edit(&doc, |txn, fragment| {
        let line = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
        line.push_back(txn, XmlTextPrelim::new("see "));
        let mention = line.push_back(txn, XmlElementPrelim::empty("mention"));
        mention.insert_attribute(txn, "ref_type", docs::s(ref_type));
        mention.insert_attribute(txn, "ref_id", docs::s(ref_id));
    });
    doc
}

fn runs_of(doc: &Doc, resolver: &Resolver) -> Vec<Run> {
    read_doc(doc, resolver).nodes[0].block.runs.clone()
}

#[test]
fn every_kind_reads_back_as_it_was_written() {
    let mut s = Synced::new();
    let mut toggle = block(3, Kind::Toggle, "More", 1);
    toggle.children = vec![para(4, "Inside", 1)];
    let mut code = block(7, Kind::Code, "let x = 1;", 1);
    code.language = Some("rust".to_owned());
    let mut kept = block(8, Kind::External, "", 1);
    kept.label = Some("column_list".to_owned());
    let mut todo = block(6, Kind::Todo, "Ship it", 1);
    todo.checked = Some(true);
    s.apply(&snapshot(
        vec![
            para(1, "Hello", 1),
            block(2, Kind::Heading, "Title", 1),
            toggle,
            block(5, Kind::BulletedItem, "One", 1),
            block(9, Kind::BulletedItem, "Two", 1),
            todo,
            code,
            block(10, Kind::Quote, "Said", 1),
            block(11, Kind::Callout, "Note", 1),
            block(12, Kind::Divider, "", 1),
            kept,
        ],
        5,
        true,
    ));
    let tree = s.tree();
    let kinds: Vec<Kind> = tree.all().iter().map(|n| n.block.kind).collect();
    assert_eq!(
        kinds,
        [
            Kind::Paragraph,
            Kind::Heading,
            Kind::Toggle,
            Kind::Paragraph,
            Kind::BulletedItem,
            Kind::BulletedItem,
            Kind::Todo,
            Kind::Code,
            Kind::Quote,
            Kind::Callout,
            Kind::Divider,
            Kind::External,
        ]
    );
    let all = tree.all();
    assert_eq!(all[1].block.level, Some(2));
    assert_eq!(all[2].children.len(), 1);
    assert_eq!(all[2].children[0].block.text(), "Inside");
    assert_eq!(all[6].block.checked, Some(true));
    assert_eq!(all[7].block.language.as_deref(), Some("rust"));
    assert_eq!(all[7].block.text(), "let x = 1;");
    assert_eq!(all[11].block.label.as_deref(), Some("column_list"));
    assert!(all.iter().all(|node| node.bid.is_some()), "every block has an id");
}

#[test]
fn marks_and_links_are_read_as_runs() {
    let mut s = Synced::new();
    let mut p = para(1, "", 1);
    p.runs = vec![
        Run::plain("plain "),
        Run {
            text: "bold".to_owned(),
            bold: true,
            ..Run::default()
        },
        Run {
            text: " all".to_owned(),
            italic: true,
            strike: true,
            code: true,
            ..Run::default()
        },
        Run {
            text: " site".to_owned(),
            link: Some("https://example.com/a".to_owned()),
            ..Run::default()
        },
    ];
    s.apply(&snapshot(vec![p.clone()], 5, true));
    assert_eq!(s.tree().nodes[0].block.runs, p.runs);
}

#[test]
fn a_link_to_a_page_at_notion_reads_as_that_pages_mention() {
    let mut s = Synced::new();
    let mut p = para(1, "", 1);
    p.runs = vec![Run {
        text: "Roadmap".to_owned(),
        page: Some(id(0x77)),
        ..Run::default()
    }];
    s.apply(&snapshot(vec![p], 5, true));
    // Written as a link to Notion's address (the page is not synced here),
    // read back as the mention it is.
    let runs = s.tree().nodes[0].block.runs.clone();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].page, Some(id(0x77)));
    assert_eq!(runs[0].link, None);
}

#[test]
fn another_synced_page_is_a_page_mention() {
    let page = Uuid::now_v7();
    let mut resolver = resolver_for_test();
    resolver.notion_of.insert(page, id(0x55));
    let doc = mentioning("pages.page", &page.to_string());
    let runs = runs_of(&doc, &resolver);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[1].page, Some(id(0x55)));
    assert!(runs[1].text.is_empty(), "no words of the page travel");
}

fn resolver_for_test() -> Resolver {
    crate::domain::notion::tests::resolver()
}

#[test]
fn everything_else_is_exactly_campbooks_link_with_an_address_of_this_app() {
    let resolver = resolver_for_test();
    let thread = Uuid::now_v7().to_string();
    let task = Uuid::now_v7().to_string();
    let page = Uuid::now_v7().to_string();
    let event = Uuid::now_v7().to_string();
    for (ref_type, ref_id, address) in [
        (
            "conversations.conversation",
            thread.as_str(),
            format!("https://app.example.com/w/acme/search/{thread}"),
        ),
        (
            "tasks.task",
            task.as_str(),
            format!("https://app.example.com/w/acme/tasks/{task}"),
        ),
        (
            "schedule.event",
            event.as_str(),
            "https://app.example.com/w/acme/schedule".to_owned(),
        ),
        // A page that is not synced.
        (
            "pages.page",
            page.as_str(),
            format!("https://app.example.com/w/acme/pages/{page}"),
        ),
    ] {
        let runs = runs_of(&mentioning(ref_type, ref_id), &resolver);
        assert_eq!(runs.len(), 2, "{ref_type}");
        assert_eq!(runs[1].text, CAMPBOOKS_LINK, "{ref_type}");
        assert_eq!(runs[1].link.as_deref(), Some(address.as_str()), "{ref_type}");
        assert_eq!(runs[1].page, None);
    }
}

#[test]
fn a_person_or_a_member_is_never_named_by_their_address_or_name() {
    let resolver = resolver_for_test();
    for (ref_type, ref_id) in [
        ("person", "marta@example.com"),
        ("workspaces.member", "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01"),
    ] {
        let runs = runs_of(&mentioning(ref_type, ref_id), &resolver);
        assert_eq!(runs[1].text, "Campbooks link");
        // The link is the page that mentions them, not them.
        assert_eq!(
            runs[1].link.as_deref(),
            Some(format!("https://app.example.com/w/acme/pages/{}", resolver.this).as_str())
        );
        let shown = format!("{runs:?}");
        assert!(!shown.contains("marta"), "{shown}");
        assert!(!shown.contains("example.com/w/acme/search"), "{shown}");
    }
}

#[test]
fn without_an_address_for_the_app_a_mention_is_still_only_the_words() {
    let mut resolver = resolver_for_test();
    resolver.app = String::new();
    let runs = runs_of(
        &mentioning("tasks.task", &Uuid::now_v7().to_string()),
        &resolver,
    );
    assert_eq!(runs[1].text, CAMPBOOKS_LINK);
    assert_eq!(runs[1].link, None);
    assert_eq!(resolver.page_link(), None);
}

#[test]
fn the_pages_own_address_is_its_app_address() {
    let resolver = resolver_for_test();
    assert_eq!(
        resolver.page_link().as_deref(),
        Some(format!("https://app.example.com/w/acme/pages/{}", resolver.this).as_str())
    );
    let mut odd = resolver;
    odd.app = "javascript:alert(1)//".to_owned();
    assert_eq!(odd.page_link(), None, "only http(s) addresses are links");
}

#[test]
fn a_hard_break_is_a_newline_and_unknown_things_are_ignored() {
    let doc = docs::client(9);
    docs::edit(&doc, |txn, fragment| {
        let line = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
        line.push_back(txn, XmlTextPrelim::new("a"));
        line.push_back(txn, XmlElementPrelim::empty("hardBreak"));
        line.push_back(txn, XmlTextPrelim::new("b"));
        fragment.push_back(txn, XmlElementPrelim::empty("somethingElse"));
    });
    let tree = read_doc(&doc, &resolver_for_test());
    assert_eq!(tree.nodes.len(), 1);
    assert_eq!(tree.nodes[0].block.text(), "a\nb");
}

#[test]
fn a_conflict_is_kept_apart_with_notions_version() {
    let mut s = Synced::new();
    s.apply(&snapshot(vec![para(1, "Hello", 1)], 5, true));
    let bid = s.bid_of("Hello");
    s.edit_text(bid, "Mine");
    s.apply(&snapshot(vec![para(1, "Theirs", 6)], 7, true));
    let tree = s.tree();
    assert_eq!(tree.nodes.len(), 1, "a conflict is not a block");
    let conflict = tree.conflict_of(bid).expect("a conflict about the block");
    assert!(!conflict.removed);
    assert_eq!(conflict.theirs.as_ref().map(Block::text).as_deref(), Some("Theirs"));
    assert!(conflict.hash.is_some() && conflict.edited.is_some());
    assert_eq!(tree.find(bid).unwrap().block.text(), "Mine");
    assert!(tree.find(Uuid::now_v7()).is_none());
}

#[test]
fn attribute_helpers_read_only_what_has_the_right_shape() {
    let doc = docs::client(3);
    docs::edit(&doc, |txn, fragment| {
        let node = fragment.push_back(txn, XmlElementPrelim::empty("heading"));
        node.insert_attribute(txn, "level", Any::Number(2.0));
        node.insert_attribute(txn, "bid", docs::s("not-a-uuid"));
        node.insert_attribute(txn, "other", Any::Map(Arc::new(std::collections::HashMap::new())));
    });
    let root = doc.get_or_insert_xml_fragment(document::ROOT);
    let txn = doc.transact();
    let XmlOut::Element(element) = root.get(&txn, 0).unwrap() else {
        panic!("an element");
    };
    let attrs = attributes(&txn, &element);
    assert_eq!(number_attribute(&attrs, "level"), Some(2));
    assert_eq!(text_attribute(&attrs, "bid").as_deref(), Some("not-a-uuid"));
    assert_eq!(uuid_attribute(&attrs, "bid"), None);
    assert_eq!(text_attribute(&attrs, "level"), None);
    assert_eq!(bool_attribute(&attrs, "level"), None);
    assert!(!attrs.contains_key("other"));
    // A heading without a usable level reads as level 1.
    drop(txn);
    let tree = read_doc(&doc, &resolver_for_test());
    assert_eq!(tree.nodes[0].block.level, Some(2));
}
