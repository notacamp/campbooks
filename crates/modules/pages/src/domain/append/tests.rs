//! Appending blocks and taking them back, on documents alone.

use yrs::{Doc, Text, Transact, XmlElementPrelim, XmlFragment, XmlOut, XmlTextPrelim};

use super::{Unit, append, remove};
use crate::{
    domain::{
        document::{self, ROOT},
        markdown::{self, MAX_TEXT_BYTES},
    },
    testkit::docs::{edit, paragraph},
};

fn page_with(text: &str) -> Doc {
    let doc = Doc::new();
    paragraph(&doc, text);
    doc
}

fn text_of(doc: &Doc) -> String {
    markdown::render(doc, "", MAX_TEXT_BYTES).markdown
}

/// The server's way: apply `update` to `doc`, then check the whole.
fn take(doc: &Doc, update: &[u8]) {
    document::apply(doc, update).unwrap();
    document::inspect(doc).unwrap();
}

#[test]
fn blocks_go_at_the_end_and_the_page_before_them_is_untouched() {
    let doc = page_with("Ana wrote this");
    let blocks = markdown::parse("## Kickoff\n\n- one\n- two\n\nDone.");
    let added = append(&doc, &blocks, &[]).unwrap();
    take(&doc, &added.update);
    assert_eq!(
        text_of(&doc),
        "Ana wrote this\n\n## Kickoff\n\n- one\n- two\n\nDone."
    );
    // A heading, two list items and a paragraph: four units.
    assert_eq!(added.units.len(), 4);
}

#[test]
fn it_takes_everything_back_when_nothing_changed() {
    let doc = page_with("Ana wrote this");
    let blocks = markdown::parse("## Kickoff\n\n- one\n- two\n\n1. a\n\nDone.");
    let added = append(&doc, &blocks, &[]).unwrap();
    take(&doc, &added.update);
    let taken = remove(&doc, &added.units).unwrap();
    assert_eq!((taken.removed, taken.stayed), (5, 0));
    take(&doc, &taken.update);
    // The lists went with their last item, and Ana's line is as it was.
    assert_eq!(text_of(&doc), "Ana wrote this");
}

#[test]
fn a_block_edited_since_stays_and_the_others_go() {
    let doc = page_with("Ana wrote this");
    let blocks = markdown::parse("- one\n- two\n- three");
    let added = append(&doc, &blocks, &[]).unwrap();
    take(&doc, &added.update);
    // Ana edits the second bullet.
    let edited = edit(&doc, |txn, root| {
        let Some(XmlOut::Element(list)) = root.get(txn, 1) else {
            panic!("no list");
        };
        let Some(XmlOut::Element(item)) = list.get(txn, 1) else {
            panic!("no item");
        };
        let Some(XmlOut::Element(lead)) = item.get(txn, 0) else {
            panic!("no lead");
        };
        let Some(XmlOut::Text(run)) = lead.get(txn, 0) else {
            panic!("no text");
        };
        let end = run.len(txn);
        run.insert(txn, end, " (Ana's note)");
    });
    take(&doc, &edited);
    let taken = remove(&doc, &added.units).unwrap();
    assert_eq!((taken.removed, taken.stayed), (2, 1));
    take(&doc, &taken.update);
    assert_eq!(text_of(&doc), "Ana wrote this\n\n- two (Ana's note)");
}

#[test]
fn a_block_that_is_gone_is_neither_removed_nor_stayed() {
    let doc = page_with("Ana");
    let added = append(&doc, &markdown::parse("one\n\ntwo"), &[]).unwrap();
    take(&doc, &added.update);
    let first = remove(&doc, &added.units[..1]).unwrap();
    take(&doc, &first.update);
    let again = remove(&doc, &added.units).unwrap();
    assert_eq!((again.removed, again.stayed), (1, 0));
}

#[test]
fn a_unit_the_page_never_had_changes_nothing() {
    let doc = page_with("Ana");
    let strange = Unit {
        bid: uuid::Uuid::now_v7(),
        hash: "0".repeat(64),
    };
    let taken = remove(&doc, &[strange]).unwrap();
    assert_eq!((taken.removed, taken.stayed), (0, 0));
}

#[test]
fn the_hash_does_not_depend_on_the_copy_of_the_document_that_reads_it() {
    // The page as the server loads it again: a fresh document from the full
    // state, never the one the blocks were made in.
    let doc = page_with("Ana");
    let added = append(
        &doc,
        &markdown::parse("**bold** and [a link](https://acme.example) and `code`\n\n- [x] done"),
        &[],
    )
    .unwrap();
    take(&doc, &added.update);
    let reloaded = document::open(Some(&document::full_state(&doc)), &[]).unwrap();
    let taken = remove(&reloaded, &added.units).unwrap();
    assert_eq!((taken.removed, taken.stayed), (2, 0));
}

#[test]
fn sources_end_the_section_as_mentions() {
    let doc = page_with("Ana");
    let thread = campbooks_kernel::RecordRef::new(
        "conversations.conversation",
        "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22",
    );
    let task =
        campbooks_kernel::RecordRef::new("tasks.task", "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c33");
    let added = append(&doc, &markdown::parse("Notes"), &[thread, task]).unwrap();
    take(&doc, &added.update);
    assert_eq!(added.units.len(), 2);
    assert_eq!(
        text_of(&doc),
        "Ana\n\nNotes\n\n[conversations.conversation:0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22] \
         [tasks.task:0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c33]"
    );
    let links = document::inspect(&doc).unwrap().links;
    assert_eq!(links.len(), 2);
}

#[test]
fn a_page_with_a_trailing_empty_paragraph_gets_the_section_after_it() {
    let doc = Doc::new();
    let update = edit(&doc, |txn, root| {
        let line = root.push_back(txn, XmlElementPrelim::empty("paragraph"));
        line.push_back(txn, XmlTextPrelim::new(""));
    });
    take(&doc, &update);
    let added = append(&doc, &markdown::parse("After"), &[]).unwrap();
    take(&doc, &added.update);
    let root = doc.get_or_insert_xml_fragment(ROOT);
    let txn = doc.transact();
    assert_eq!(root.len(&txn), 2);
}
