//! A block's hash tells an edit from a re-serialisation; the neutral model
//! is the contract's, and the contract files agree with it.

use super::*;
use crate::domain::notion::tests::{block, id, para, snapshot};

fn h(block: &Block) -> [u8; 32] {
    hash(block)
}

#[test]
fn the_hash_ignores_ids_times_and_how_runs_were_cut() {
    let a = para(1, "Launch plan", 1);
    let mut b = para(2, "", 9);
    b.runs = vec![Run::plain("Launch "), Run::plain("plan")];
    // The same text split in runs by a client is the same text once it has
    // its one shape (blocks read from a body or a snapshot always have it).
    b.runs = normalize(b.runs);
    assert_eq!(h(&a), h(&b), "same words, same kind");
}

#[test]
fn the_hash_sees_words_marks_kind_and_attributes() {
    let base = para(1, "Launch plan", 1);
    let mut words = base.clone();
    words.runs = vec![Run::plain("Launch plans")];
    let mut bold = base.clone();
    bold.runs[0].bold = true;
    let mut link = base.clone();
    link.runs[0].link = Some("https://example.com".to_owned());
    let kind = block(1, Kind::Quote, "Launch plan", 1);
    let mut heading = block(1, Kind::Heading, "Launch plan", 1);
    let h2 = h(&heading);
    heading.level = Some(3);
    let checked = {
        let mut todo = block(1, Kind::Todo, "Launch plan", 1);
        let before = h(&todo);
        todo.checked = Some(true);
        (before, h(&todo))
    };
    let all = [
        h(&base),
        h(&words),
        h(&bold),
        h(&link),
        h(&kind),
        h2,
        h(&heading),
    ];
    for (n, one) in all.iter().enumerate() {
        for other in &all[n + 1..] {
            assert_ne!(one, other);
        }
    }
    assert_ne!(checked.0, checked.1);
}

#[test]
fn a_page_mention_is_its_page_and_not_the_words_notion_showed() {
    let mention = |words: &str| {
        let mut block = para(1, "", 1);
        block.runs = vec![Run {
            text: words.to_owned(),
            page: Some(id(77)),
            ..Run::default()
        }];
        block
    };
    assert_eq!(h(&mention("Roadmap")), h(&mention("Roadmap (renamed)")));
    let mut other = mention("Roadmap");
    other.runs[0].page = Some(id(78));
    assert_ne!(h(&mention("Roadmap")), h(&other));
}

#[test]
fn a_sub_page_link_and_a_kept_block_are_their_page_and_type_alone() {
    let mut link = block(1, Kind::PageLink, "Old title", 1);
    link.page = Some(id(5));
    let mut renamed = link.clone();
    renamed.runs = vec![Run::plain("New title")];
    assert_eq!(h(&link), h(&renamed));
    let mut external = block(2, Kind::External, "", 1);
    external.label = Some("column_list".to_owned());
    let mut other = external.clone();
    other.label = Some("synced_block".to_owned());
    assert_ne!(h(&external), h(&other));
}

#[test]
fn hashes_round_trip_as_hex() {
    let hash = h(&para(1, "Launch plan", 1));
    let hex = hash_hex(&hash);
    assert_eq!(hex.len(), 64);
    assert!(
        hex.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    assert_eq!(hash_of_hex(&hex), Some(hash));
    let (g, short, wide) = ("g".repeat(64), "a".repeat(63), "é".repeat(32));
    for bad in ["", "abc", g.as_str(), short.as_str(), wide.as_str()] {
        assert_eq!(hash_of_hex(bad), None, "{bad}");
    }
}

#[test]
fn runs_are_normalised_to_one_shape() {
    let joined = normalize(vec![
        Run::plain("a"),
        Run::plain(""),
        Run::plain("b"),
        Run {
            text: "c".to_owned(),
            bold: true,
            ..Run::default()
        },
        Run::plain("d"),
    ]);
    assert_eq!(
        joined.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        ["ab", "c", "d"]
    );
    // A mention is never joined into the text around it.
    let mention = Run {
        page: Some(id(1)),
        ..Run::default()
    };
    let kept = normalize(vec![
        Run::plain("see "),
        mention.clone(),
        Run::plain(" now"),
    ]);
    assert_eq!(kept.len(), 3);
    assert_eq!(kept[1], mention);
}

#[test]
fn kinds_round_trip_by_name_and_say_what_the_member_can_edit() {
    for kind in [
        Kind::Paragraph,
        Kind::Heading,
        Kind::BulletedItem,
        Kind::NumberedItem,
        Kind::Todo,
        Kind::Toggle,
        Kind::Quote,
        Kind::Callout,
        Kind::Divider,
        Kind::Code,
        Kind::Image,
        Kind::File,
        Kind::PageLink,
        Kind::External,
    ] {
        assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        let json = serde_json::to_value(kind).unwrap();
        assert_eq!(json, kind.as_str());
    }
    assert_eq!(Kind::parse("table"), None);
    // What is kept in Notion, linked or a file is never written from here.
    for kind in [
        Kind::External,
        Kind::PageLink,
        Kind::Image,
        Kind::File,
        Kind::Divider,
    ] {
        assert!(!kind.editable(), "{kind:?}");
    }
    assert!(Kind::Code.editable() && Kind::Paragraph.editable());
    assert!(Kind::Todo.is_item() && !Kind::Toggle.is_item());
    assert!(Kind::Toggle.leads_with_text() && !Kind::Paragraph.leads_with_text());
}

#[test]
fn a_snapshot_and_a_change_document_round_trip_and_match_the_contracts() {
    let snap = snapshot(
        vec![para(1, "Hello", 1), {
            let mut b = block(2, Kind::Toggle, "More", 1);
            b.children = vec![para(3, "Inside", 1)];
            b
        }],
        5,
        true,
    );
    let value = serde_json::to_value(&snap).unwrap();
    assert_contract("notion-page-snapshot.v1.schema.json", &value);
    let back: Snapshot = serde_json::from_value(value).unwrap();
    assert_eq!(back, snap);
    // The snapshot has no hash: pages computes it.
    assert!(serde_json::to_string(&snap).unwrap().find("hash").is_none());

    let changes = ChangeDocument {
        version: 1,
        items: vec![
            ChangeItem {
                seq: 0,
                block: Some(para(7, "Edited here", 0)),
            },
            ChangeItem {
                seq: 1,
                block: None,
            },
        ],
    };
    let value = serde_json::to_value(&changes).unwrap();
    assert_contract("notion-block-changes.v1.schema.json", &value);
}

/// Validate `value` against the contract `file`.
fn assert_contract(file: &str, value: &serde_json::Value) {
    let path = format!("{}/../../../contracts/{file}", env!("CARGO_MANIFEST_DIR"));
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut schemas = boon::Schemas::new();
    let mut compiler = boon::Compiler::new();
    compiler.add_resource(&path, schema).unwrap();
    let index = compiler.compile(&path, &mut schemas).unwrap();
    if let Err(err) = schemas.validate(value, index) {
        panic!("{file}: {err:#}\n{value:#}");
    }
}
