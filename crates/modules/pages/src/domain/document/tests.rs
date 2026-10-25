//! What a page's document accepts, and the forged updates it refuses.

use std::{collections::HashMap, sync::Arc};

use yrs::{
    Any, Doc, Text, Transact, Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim, types::Attrs,
};

use super::*;
use crate::testkit::docs::{self, edit, element, paragraph, s};

const PAGE: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01";
const BLOB: &str = "a3f0c1d2e4b5968778695a4b3c2d1e0fa3f0c1d2e4b5968778695a4b3c2d1e0f";

/// Apply `update` to `doc` and read the result, as a sync does.
fn accept(doc: &Doc, update: &[u8]) -> Result<Inspection, Rejection> {
    apply(doc, update)?;
    inspect(doc)
}

fn link_mark(href: &str) -> Attrs {
    let mut marks: Attrs = HashMap::new();
    marks.insert(
        "link".into(),
        Any::Map(Arc::new(HashMap::from([("href".to_owned(), s(href))]))),
    );
    marks
}

fn marked(doc: &Doc, text: &str, marks: Attrs) -> Vec<u8> {
    edit(doc, |txn, fragment| {
        let p = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
        let run = p.push_back(txn, XmlTextPrelim::new(""));
        run.insert_with_attributes(txn, 0, text, marks);
    })
}

#[test]
fn an_honest_page_is_accepted_and_read() {
    let server = Doc::new();
    let ana = docs::client(1);
    let page = PAGE;
    let update = edit(&ana, |txn, fragment| {
        let heading = fragment.push_back(txn, XmlElementPrelim::empty("heading"));
        heading.insert_attribute(txn, "level", Any::Number(2.0));
        heading.push_back(txn, XmlTextPrelim::new("Weekly plan"));
        let line = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
        let run = line.push_back(txn, XmlTextPrelim::new(""));
        run.insert_with_attributes(
            txn,
            0,
            "See the ",
            HashMap::from([("bold".into(), Any::Map(Arc::new(HashMap::new())))]),
        );
        let mention = line.push_back(txn, XmlElementPrelim::empty("mention"));
        mention.insert_attribute(txn, "ref_type", s("pages.page"));
        mention.insert_attribute(txn, "ref_id", s(page));
        let person = line.push_back(txn, XmlElementPrelim::empty("mention"));
        person.insert_attribute(txn, "ref_type", s("person"));
        person.insert_attribute(txn, "ref_id", s("jonas.weber@example.com"));
        let file = fragment.push_back(txn, XmlElementPrelim::empty("file"));
        file.insert_attribute(txn, "blob", s(BLOB));
        file.insert_attribute(txn, "name", s("Offer.pdf"));
        file.insert_attribute(txn, "size", Any::Number(1234.0));
        let child = fragment.push_back(txn, XmlElementPrelim::empty("pageLink"));
        child.insert_attribute(txn, "page", s(page));
    });
    let seen = accept(&server, &update).unwrap();
    assert_eq!(seen.text, "Weekly plan\nSee the \n");
    assert_eq!(seen.blobs.iter().collect::<Vec<_>>(), [BLOB]);
    let kinds: Vec<(&str, &str)> = seen
        .links
        .iter()
        .map(|link| (link.ref_type.as_str(), link.kind.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("pages.page", "mention"),
            ("pages.page", "child"),
            ("person", "mention")
        ]
    );
    assert!(seen.blocks >= 8);
    assert!(seen.size > 0);
}

#[test]
fn a_link_to_https_http_and_mailto_is_fine() {
    for href in [
        "https://example.com/a?b=c",
        "HTTP://example.com",
        "mailto:ana@example.com",
    ] {
        let server = Doc::new();
        let update = marked(&docs::client(2), "link", link_mark(href));
        assert!(accept(&server, &update).is_ok(), "{href}");
    }
}

#[test]
fn a_script_or_data_link_is_refused() {
    for href in [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        " javascript:alert(1)",
        "java\tscript:alert(1)",
        "data:text/html;base64,PHNjcmlwdD4=",
        "vbscript:x",
        "/relative/path",
        "//example.com",
        "",
    ] {
        let server = Doc::new();
        let update = marked(&docs::client(3), "click", link_mark(href));
        assert_eq!(
            accept(&server, &update).unwrap_err(),
            Rejection::BadLink,
            "{href}"
        );
    }
}

#[test]
fn a_link_mark_with_an_unknown_attribute_or_no_href_is_refused() {
    let server = Doc::new();
    let mut marks: Attrs = HashMap::new();
    marks.insert(
        "link".into(),
        Any::Map(Arc::new(HashMap::from([
            ("href".to_owned(), s("https://example.com")),
            ("onclick".to_owned(), s("alert(1)")),
        ]))),
    );
    let update = marked(&docs::client(4), "x", marks);
    assert_eq!(
        accept(&server, &update).unwrap_err(),
        Rejection::BadAttribute
    );
    let server = Doc::new();
    let mut marks: Attrs = HashMap::new();
    marks.insert("link".into(), Any::Map(Arc::new(HashMap::new())));
    let update = marked(&docs::client(5), "x", marks);
    assert_eq!(accept(&server, &update).unwrap_err(), Rejection::BadLink);
}

#[test]
fn an_unknown_mark_element_or_attribute_is_refused() {
    let server = Doc::new();
    let update = marked(
        &docs::client(6),
        "x",
        HashMap::from([("highlight".into(), Any::Map(Arc::new(HashMap::new())))]),
    );
    assert_eq!(
        accept(&server, &update).unwrap_err(),
        Rejection::UnknownBlock
    );

    let server = Doc::new();
    let update = element(&docs::client(7), "script", &[], None);
    assert_eq!(
        accept(&server, &update).unwrap_err(),
        Rejection::UnknownBlock
    );

    let server = Doc::new();
    let update = element(
        &docs::client(8),
        "paragraph",
        &[("onload", s("x"))],
        Some("hi"),
    );
    assert_eq!(
        accept(&server, &update).unwrap_err(),
        Rejection::BadAttribute
    );

    // Known names, wrong kinds and out-of-range values.
    for (tag, key, value) in [
        ("heading", "level", Any::Number(4.0)),
        ("heading", "level", s("1")),
        ("callout", "tone", s("danger")),
        ("codeBlock", "language", s("brainfuck")),
        ("image", "blob", s("../../etc/passwd")),
        ("taskItem", "checked", s("yes")),
        ("pageLink", "page", s("not-a-uuid")),
    ] {
        let server = Doc::new();
        let update = element(&docs::client(9), tag, &[(key, value)], None);
        assert!(accept(&server, &update).is_err(), "{tag} {key}");
    }
}

#[test]
fn mentions_and_embeds_name_a_known_kind_of_record_by_a_well_formed_id() {
    let attempts = [
        ("mention", "tasks.task", PAGE, true),
        ("embed", "conversations.conversation", PAGE, true),
        ("mention", "person", "Ana@Example.com", false),
        ("mention", "person", "not an address", false),
        ("mention", "documents.document", PAGE, false),
        ("mention", "tasks.task", "123", false),
        (
            "mention",
            "tasks.task",
            "0192F0C4-7D4E-7B39-A7F1-3A8F0E2B9D01",
            false,
        ),
        ("mention", "javascript", PAGE, false),
    ];
    for (tag, ref_type, ref_id, fine) in attempts {
        let server = Doc::new();
        let client = docs::client(10);
        let update = edit(&client, |txn, fragment| {
            let mention = if tag == "embed" {
                fragment.push_back(txn, XmlElementPrelim::empty(tag))
            } else {
                let p = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
                p.push_back(txn, XmlElementPrelim::empty(tag))
            };
            mention.insert_attribute(txn, "ref_type", s(ref_type));
            mention.insert_attribute(txn, "ref_id", s(ref_id));
        });
        assert_eq!(
            accept(&server, &update).is_ok(),
            fine,
            "{ref_type} {ref_id}"
        );
    }
}

#[test]
fn blocks_only_stand_where_the_schema_lets_them() {
    // Text straight under the document, a list item outside a list, a block
    // inside a paragraph.
    let cases: [fn(&mut yrs::TransactionMut<'_>, &yrs::XmlFragmentRef); 3] = [
        |txn, fragment| {
            fragment.push_back(txn, XmlTextPrelim::new("loose"));
        },
        |txn, fragment| {
            fragment.push_back(txn, XmlElementPrelim::empty("listItem"));
        },
        |txn, fragment| {
            let p = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
            p.push_back(txn, XmlElementPrelim::empty("heading"));
        },
    ];
    for case in cases {
        let server = Doc::new();
        let update = edit(&docs::client(11), case);
        assert_eq!(
            accept(&server, &update).unwrap_err(),
            Rejection::BadStructure
        );
    }
}

#[test]
fn a_second_root_type_is_refused() {
    let server = Doc::new();
    let client = docs::client(12);
    let hidden = client.get_or_insert_text("exfiltrate");
    let update = {
        let mut txn = client.transact_mut();
        hidden.insert(&mut txn, 0, "secret");
        txn.encode_update_v1()
    };
    assert_eq!(
        accept(&server, &update).unwrap_err(),
        Rejection::UnknownRoot
    );
}

#[test]
fn nesting_has_a_limit() {
    let server = Doc::new();
    let client = docs::client(13);
    let update = edit(&client, |txn, fragment| {
        let mut parent = fragment.push_back(txn, XmlElementPrelim::empty("blockquote"));
        for _ in 0..40 {
            parent = parent.push_back(txn, XmlElementPrelim::empty("blockquote"));
        }
    });
    assert_eq!(accept(&server, &update).unwrap_err(), Rejection::TooDeep);
}

#[test]
fn garbage_and_big_updates_are_refused() {
    let server = Doc::new();
    for garbage in [&[][..], &[1, 2, 3][..], &[255; 64][..], b"<script>"] {
        assert!(accept(&server, garbage).is_err());
    }
    let big = vec![0_u8; MAX_UPDATE_BYTES + 1];
    assert_eq!(apply(&server, &big).unwrap_err(), Rejection::UpdateTooLarge);
    // Nothing was kept.
    assert_eq!(inspect(&server).unwrap().blocks, 0);
}

#[test]
fn an_update_missing_its_dependencies_is_refused() {
    let client = docs::client(14);
    let first = paragraph(&client, "one");
    let second = paragraph(&client, "two");
    let server = Doc::new();
    // The second without the first: it cannot be placed.
    assert_eq!(apply(&server, &second).unwrap_err(), Rejection::OutOfOrder);
    let fresh = Doc::new();
    assert!(accept(&fresh, &first).is_ok());
    assert!(accept(&fresh, &second).is_ok());
}

#[test]
fn a_document_has_a_size_and_a_block_limit() {
    // Enough blocks, one small update at a time.
    let server = Doc::new();
    let client = docs::client(15);
    let mut refused = None;
    for _ in 0..12 {
        let update = edit(&client, |txn, fragment| {
            for _ in 0..1000 {
                let p = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
                p.push_back(txn, XmlTextPrelim::new("x"));
            }
        });
        if let Err(rejection) = accept(&server, &update) {
            refused = Some(rejection);
            break;
        }
    }
    assert_eq!(refused, Some(Rejection::TooManyBlocks));

    // Enough bytes, one update at a time.
    let server = Doc::new();
    let client = docs::client(16);
    let text = "y".repeat(200 * 1024);
    let mut refused = None;
    for _ in 0..24 {
        let update = paragraph(&client, &text);
        if let Err(rejection) = accept(&server, &update) {
            refused = Some(rejection);
            break;
        }
    }
    assert_eq!(refused, Some(Rejection::DocumentTooLarge));
}

#[test]
fn two_writers_merge_and_the_state_is_what_a_reader_needs() {
    let (ana, rui) = (docs::client(21), docs::client(22));
    let ana_update = paragraph(&ana, "Ana's line");
    let rui_update = paragraph(&rui, "Rui's line");
    let server = Doc::new();
    accept(&server, &ana_update).unwrap();
    accept(&server, &rui_update).unwrap();
    let merged = inspect(&server).unwrap();
    assert!(merged.text.contains("Ana's line"));
    assert!(merged.text.contains("Rui's line"));

    // What Rui lacks is Ana's line, and applying it gives him the same page.
    let missing = diff_since(&server, &state_vector(&rui)).unwrap();
    accept(&rui, &missing).unwrap();
    assert_eq!(inspect(&rui).unwrap().text, merged.text);

    // A stored snapshot opens to the same page.
    let reopened = open(Some(&full_state(&server)), &[]).unwrap();
    assert_eq!(inspect(&reopened).unwrap().text, merged.text);
    assert!(diff_since(&server, &[255, 255, 255]).is_err());
}
