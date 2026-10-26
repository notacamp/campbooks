use serde_json::json;

use super::*;

const ID: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01";
const OTHER: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d02";
const TIME: &str = "2026-10-25T10:00:00.000Z";

fn raw(type_name: &str, body: &serde_json::Value) -> serde_json::Value {
    let mut block = json!({
        "object": "block",
        "id": ID,
        "type": type_name,
        "last_edited_time": TIME,
        "has_children": false,
        "archived": false,
    });
    block[type_name] = body.clone();
    block
}

fn text(content: &str) -> serde_json::Value {
    json!({
        "type": "text",
        "text": {"content": content, "link": null},
        "plain_text": content,
        "annotations": {"bold": false, "italic": false, "strikethrough": false,
                        "underline": false, "code": false, "color": "default"},
        "href": null,
    })
}

#[test]
fn a_paragraph_keeps_its_text_and_marks() {
    let mut bold = text("Launch ");
    bold["annotations"]["bold"] = json!(true);
    let mut link = text("plan");
    link["text"]["link"] = json!({"url": "https://example.com/plan"});
    link["href"] = json!("https://example.com/plan");
    let parsed = parse(&raw("paragraph", &json!({"rich_text": [bold, link]}))).unwrap();
    assert_eq!(parsed.block.kind, Kind::Paragraph);
    assert_eq!(
        parsed.block.edited.unwrap().to_rfc3339(),
        "2026-10-25T10:00:00+00:00"
    );
    assert_eq!(
        parsed.block.runs,
        vec![
            Run {
                text: "Launch ".into(),
                bold: true,
                ..Run::default()
            },
            Run {
                text: "plan".into(),
                link: Some("https://example.com/plan".into()),
                ..Run::default()
            },
        ]
    );
    assert!(!parsed.recurse && parsed.file.is_none());
}

#[test]
fn neighbours_with_the_same_marks_are_one_run() {
    let runs = runs_of(Some(&json!([
        text("Hel"),
        text("lo "),
        text(""),
        text("world")
    ])));
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "Hello world");
}

#[test]
fn a_page_mention_keeps_its_page_and_the_words_shown() {
    let mention = json!({
        "type": "mention",
        "mention": {"type": "page", "page": {"id": OTHER}},
        "plain_text": "Roadmap",
        "href": "https://www.notion.so/roadmap",
        "annotations": {"bold": false, "italic": false, "strikethrough": false,
                        "underline": false, "code": false, "color": "default"},
    });
    let runs = runs_of(Some(&json!([mention])));
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "Roadmap");
    assert_eq!(runs[0].page, Some(OTHER.parse().unwrap()));
    assert_eq!(runs[0].link, None);
    // A person is plain text.
    let person = json!({
        "type": "mention",
        "mention": {"type": "user", "user": {"id": OTHER}},
        "plain_text": "@Marta",
        "href": null,
    });
    let runs = runs_of(Some(&json!([person])));
    assert_eq!((runs[0].text.as_str(), runs[0].page), ("@Marta", None));
}

#[test]
fn links_are_only_http_https_and_mailto() {
    assert_eq!(
        clean_link("https://example.com/a").as_deref(),
        Some("https://example.com/a")
    );
    assert_eq!(
        clean_link("/0192f0c47d4e").as_deref(),
        Some("https://www.notion.so/0192f0c47d4e")
    );
    assert_eq!(
        clean_link("mailto:ana@example.com").as_deref(),
        Some("mailto:ana@example.com")
    );
    for bad in [
        "javascript:alert(1)",
        "//example.com",
        "data:text/html;base64,AA==",
        "https://exa mple.com",
        "ftp://example.com",
        "",
    ] {
        assert_eq!(clean_link(bad), None, "{bad}");
    }
}

#[test]
fn list_items_to_dos_and_toggles_read_their_children() {
    let mut item = raw("bulleted_list_item", &json!({"rich_text": [text("Item")]}));
    item["has_children"] = json!(true);
    let parsed = parse(&item).unwrap();
    assert_eq!(parsed.block.kind, Kind::BulletedItem);
    assert!(parsed.recurse);
    let todo = parse(&raw(
        "to_do",
        &json!({"rich_text": [text("Ship")], "checked": true}),
    ))
    .unwrap();
    assert_eq!(
        (todo.block.kind, todo.block.checked),
        (Kind::Todo, Some(true))
    );
    for (name, kind) in [
        ("numbered_list_item", Kind::NumberedItem),
        ("toggle", Kind::Toggle),
        ("quote", Kind::Quote),
        ("callout", Kind::Callout),
    ] {
        let parsed = parse(&raw(name, &json!({"rich_text": [text("x")]}))).unwrap();
        assert_eq!(parsed.block.kind, kind, "{name}");
    }
}

#[test]
fn headings_have_levels_and_toggle_headings_are_kept_in_notion() {
    let heading = parse(&raw("heading_2", &json!({"rich_text": [text("Goals")]}))).unwrap();
    assert_eq!(
        (heading.block.kind, heading.block.level),
        (Kind::Heading, Some(2))
    );
    let toggle = parse(&raw(
        "heading_1",
        &json!({"rich_text": [text("Goals")], "is_toggleable": true}),
    ))
    .unwrap();
    assert_eq!(toggle.block.kind, Kind::External);
    assert_eq!(toggle.block.label.as_deref(), Some("heading_1"));
    assert!(toggle.block.runs.is_empty());
}

#[test]
fn what_pages_lacks_is_external_with_its_type_and_nothing_else() {
    for name in [
        "table",
        "synced_block",
        "column_list",
        "embed",
        "equation",
        "child_database",
    ] {
        let parsed = parse(&raw(name, &json!({"rich_text": [text("secret")]}))).unwrap();
        assert_eq!(parsed.block.kind, Kind::External, "{name}");
        assert_eq!(parsed.block.label.as_deref(), Some(name));
        assert!(parsed.block.runs.is_empty() && !parsed.recurse, "{name}");
        let value = serde_json::to_string(&parsed.block).unwrap();
        assert!(!value.contains("secret"), "{name}");
    }
    // A paragraph with blocks under it cannot be held by a Pages paragraph.
    let mut nested = raw("paragraph", &json!({"rich_text": [text("Parent")]}));
    nested["has_children"] = json!(true);
    let parsed = parse(&nested).unwrap();
    assert_eq!(parsed.block.kind, Kind::External);
    assert!(parsed.block.runs.is_empty());
}

#[test]
fn files_notion_hosts_are_wanted_and_others_are_kept_in_notion() {
    let hosted = parse(&raw(
        "image",
        &json!({"type": "file", "file": {
            "url": "https://prod-files-secure.s3.us-west-2.amazonaws.com/a/Team%20photo.png?sig=1",
            "expiry_time": "2026-10-25T11:00:00.000Z"}, "caption": []}),
    ))
    .unwrap();
    assert_eq!(hosted.block.kind, Kind::Image);
    let want = hosted.file.unwrap();
    assert_eq!(want.name, "Team photo.png");
    let named = parse(&raw(
        "file",
        &json!({"type": "file", "name": "Budget.xlsx", "file": {"url": "https://example.com/x"}}),
    ))
    .unwrap();
    assert_eq!(named.block.kind, Kind::File);
    assert_eq!(named.file.unwrap().name, "Budget.xlsx");
    let linked = parse(&raw(
        "image",
        &json!({"type": "external", "external": {"url": "https://example.com/cat.png"}}),
    ))
    .unwrap();
    assert_eq!(linked.block.kind, Kind::External);
    assert_eq!(linked.block.label.as_deref(), Some("image"));
    assert!(linked.file.is_none());
}

#[test]
fn a_sub_page_is_a_link_with_its_title() {
    let parsed = parse(&raw("child_page", &json!({"title": "Q4 OKRs"}))).unwrap();
    assert_eq!(parsed.block.kind, Kind::PageLink);
    assert_eq!(parsed.block.page, Some(ID.parse().unwrap()));
    assert_eq!(parsed.block.runs[0].text, "Q4 OKRs");
    assert!(!parsed.recurse);
}

#[test]
fn a_block_in_the_trash_or_without_a_time_is_not_read() {
    let mut gone = raw("paragraph", &json!({"rich_text": []}));
    gone["archived"] = json!(true);
    assert!(parse(&gone).is_none());
    let mut trashed = raw("paragraph", &json!({"rich_text": []}));
    trashed["in_trash"] = json!(true);
    assert!(parse(&trashed).is_none());
    let mut timeless = raw("paragraph", &json!({"rich_text": []}));
    timeless.as_object_mut().unwrap().remove("last_edited_time");
    assert!(parse(&timeless).is_none());
}

#[test]
fn code_languages_map_both_ways() {
    let code = parse(&raw(
        "code",
        &json!({"rich_text": [text("let x = 1;")], "language": "c++"}),
    ))
    .unwrap();
    assert_eq!(code.block.language.as_deref(), Some("cpp"));
    let unknown = parse(&raw(
        "code",
        &json!({"rich_text": [text("x")], "language": "brainfuck"}),
    ))
    .unwrap();
    assert_eq!(unknown.block.language.as_deref(), Some("plaintext"));
    let written = update_json(&code.block).unwrap();
    assert_eq!(written["code"]["language"], "c++");
    let mut plain = code.block.clone();
    plain.language = Some("plaintext".into());
    assert_eq!(
        update_json(&plain).unwrap()["code"]["language"],
        "plain text"
    );
}

#[test]
fn what_is_written_has_the_shape_notion_takes() {
    let mut block = Block::new(ID.parse().unwrap(), Kind::Todo);
    block.checked = Some(true);
    block.runs = vec![Run {
        text: "Ship it".into(),
        bold: true,
        link: Some("https://example.com".into()),
        ..Run::default()
    }];
    let created = create_json(&block).unwrap();
    assert_eq!(created["object"], "block");
    assert_eq!(created["type"], "to_do");
    assert_eq!(created["to_do"]["checked"], true);
    let item = &created["to_do"]["rich_text"][0];
    assert_eq!(item["text"]["content"], "Ship it");
    assert_eq!(item["text"]["link"]["url"], "https://example.com");
    assert_eq!(item["annotations"]["bold"], true);
    assert_eq!(item["annotations"]["underline"], false);
    let updated = update_json(&block).unwrap();
    assert!(updated.get("object").is_none() && updated.get("type").is_none());
    assert_eq!(
        updated["to_do"]["rich_text"][0]["text"]["content"],
        "Ship it"
    );
    // A heading's level is its type.
    let mut heading = Block::new(ID.parse().unwrap(), Kind::Heading);
    heading.level = Some(3);
    assert_eq!(create_json(&heading).unwrap()["type"], "heading_3");
    heading.level = Some(9);
    assert_eq!(create_json(&heading).unwrap()["type"], "heading_3");
    // A divider is added and never rewritten; nothing Pages lacks is written.
    let divider = Block::new(ID.parse().unwrap(), Kind::Divider);
    assert!(create_json(&divider).is_some() && update_json(&divider).is_none());
    for kind in [Kind::Image, Kind::File, Kind::PageLink, Kind::External] {
        let block = Block::new(ID.parse().unwrap(), kind);
        assert!(
            create_json(&block).is_none() && update_json(&block).is_none(),
            "{kind:?}"
        );
        assert!(!kind.writable());
    }
}

#[test]
fn long_text_is_split_within_notions_limit() {
    let run = Run {
        text: "é".repeat(RICH_TEXT_LIMIT * 2 + 5),
        ..Run::default()
    };
    let items = rich_text(&[run]);
    assert_eq!(items.len(), 3);
    for item in &items[..2] {
        assert_eq!(
            item["text"]["content"].as_str().unwrap().chars().count(),
            RICH_TEXT_LIMIT
        );
    }
    assert_eq!(
        items[2]["text"]["content"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        5
    );
}

#[test]
fn a_page_mention_is_written_as_a_mention() {
    let run = Run {
        text: "Roadmap".into(),
        page: Some(OTHER.parse().unwrap()),
        ..Run::default()
    };
    let items = rich_text(&[run]);
    assert_eq!(items[0]["type"], "mention");
    assert_eq!(items[0]["mention"]["page"]["id"], OTHER);
}

#[test]
fn a_snapshot_round_trips_and_carries_no_hash() {
    let mut block = Block::new(ID.parse().unwrap(), Kind::Paragraph);
    block.edited = Some(TIME.parse().unwrap());
    block.runs = vec![Run {
        text: "Hello".into(),
        ..Run::default()
    }];
    let snapshot = Snapshot {
        version: 1,
        notion_page: OTHER.parse().unwrap(),
        title: "Product roadmap".into(),
        edited: TIME.parse().unwrap(),
        read_at: TIME.parse().unwrap(),
        complete: true,
        blocks: vec![block],
    };
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    let back: Snapshot = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(back, snapshot);
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["blocks"][0]["kind"], "paragraph");
    assert!(value["blocks"][0].get("hash").is_none());
}
