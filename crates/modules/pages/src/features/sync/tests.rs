//! Keeping a page's body in step: merging, and refusing forged updates.

use std::{collections::HashMap, sync::Arc};

use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;
use yrs::{Any, Text, Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim, types::Attrs};

use crate::testkit::{
    Kit,
    docs::{self, edit, element, paragraph, s},
};

struct Fixture {
    kit: Kit,
    workspace: WorkspaceId,
    ana: String,
    rui: String,
    page: String,
}

/// Ana's workspace page "Weekly plan", which every member writes.
async fn fixture(db: PgPool) -> Fixture {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let made = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Weekly plan", "place": "workspace"}),
        )
        .await;
    Fixture {
        page: made["id"].as_str().unwrap().to_owned(),
        kit,
        workspace,
        ana,
        rui,
    }
}

fn uuid(text: &str) -> sqlx::types::Uuid {
    text.parse().unwrap()
}

async fn stored_updates(kit: &Kit, page: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM pages.updates WHERE page_id = $1")
        .bind(uuid(page))
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn two_writers_from_the_same_start_both_keep_what_they_wrote(db: PgPool) {
    let Fixture {
        kit,
        workspace,
        ana,
        rui,
        page,
    } = fixture(db).await;
    let (ana_doc, rui_doc) = (docs::client(11), docs::client(12));
    let ana_update = paragraph(&ana_doc, "Ana: book the room");
    let rui_update = paragraph(&rui_doc, "Rui: order lunch");

    let first = kit
        .sync(&ana, workspace, &page, &ana_doc, Some(&ana_update))
        .await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.json);
    // Rui synced from the same starting state: his client had not seen
    // Ana's line when he wrote his.
    let second = kit
        .sync(&rui, workspace, &page, &rui_doc, Some(&rui_update))
        .await;
    assert_eq!(second.status, StatusCode::OK, "{:?}", second.json);
    // What the server sends back to Rui is what he lacks: Ana's line.
    docs::apply(
        &rui_doc,
        &STANDARD
            .decode(second.json["update"].as_str().unwrap())
            .unwrap(),
    );
    let text = docs::text(&rui_doc);
    assert!(text.contains("Ana: book the room"), "{text}");
    assert!(text.contains("Rui: order lunch"), "{text}");

    // Ana pulls and has both too.
    let pull = kit.sync(&ana, workspace, &page, &ana_doc, None).await;
    docs::apply(
        &ana_doc,
        &STANDARD
            .decode(pull.json["update"].as_str().unwrap())
            .unwrap(),
    );
    assert_eq!(docs::text(&ana_doc), text);

    // And a fresh reader of the page gets both.
    let got = kit
        .get(&rui, &format!("{}/{page}", Kit::pages(workspace)))
        .await;
    let reader = docs::client(13);
    docs::apply(
        &reader,
        &STANDARD
            .decode(got.json["document"]["update"].as_str().unwrap())
            .unwrap(),
    );
    assert_eq!(docs::text(&reader), text);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn forged_updates_are_refused_and_the_page_is_left_as_it_was(db: PgPool) {
    let Fixture {
        kit,
        workspace,
        ana,
        page,
        ..
    } = fixture(db).await;
    let honest = docs::client(20);
    let first = paragraph(&honest, "An honest line");
    assert_eq!(
        kit.sync(&ana, workspace, &page, &honest, Some(&first))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(stored_updates(&kit, &page).await, 1);

    let marks = |href: &str| -> Attrs {
        HashMap::from([(
            "link".into(),
            Any::Map(Arc::new(HashMap::from([("href".to_owned(), s(href))]))),
        )])
    };
    let forged: Vec<(&str, Vec<u8>, &str)> = vec![
        (
            "a script link",
            edit(&docs::client(21), |txn, fragment| {
                let p = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
                let run = p.push_back(txn, XmlTextPrelim::new(""));
                run.insert_with_attributes(txn, 0, "click", marks("javascript:alert(1)"));
            }),
            "link_scheme",
        ),
        (
            "an unknown element",
            element(
                &docs::client(22),
                "iframe",
                &[("src", s("https://evil.example"))],
                None,
            ),
            "unknown_block",
        ),
        (
            "an unknown attribute",
            element(
                &docs::client(23),
                "paragraph",
                &[("onclick", s("x"))],
                Some("hi"),
            ),
            "unknown_block",
        ),
        (
            "a mention of an unknown kind of record",
            edit(&docs::client(24), |txn, fragment| {
                let p = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
                let m = p.push_back(txn, XmlElementPrelim::empty("mention"));
                m.insert_attribute(txn, "ref_type", s("documents.document"));
                m.insert_attribute(txn, "ref_id", s("0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01"));
            }),
            "unknown_block",
        ),
        (
            "an update over 256 KiB",
            paragraph(&docs::client(25), &"x".repeat(300 * 1024)),
            "update_too_large",
        ),
        ("garbage", vec![1, 2, 3, 4, 5], "invalid_update"),
        (
            "an update that needs one the server lacks",
            {
                let c = docs::client(26);
                paragraph(&c, "lost");
                paragraph(&c, "dependent")
            },
            "invalid_update",
        ),
    ];
    for (what, update, code) in forged {
        let refused = kit
            .sync(&ana, workspace, &page, &docs::client(99), Some(&update))
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{what}");
        assert_eq!(refused.json["code"], code, "{what}");
    }
    // A file naming a blob nobody uploaded.
    let foreign = element(
        &docs::client(27),
        "image",
        &[
            ("blob", s(&"a".repeat(64))),
            ("name", s("x.png")),
            ("size", Any::Number(10.0)),
        ],
        None,
    );
    let refused = kit
        .sync(&ana, workspace, &page, &docs::client(98), Some(&foreign))
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json["code"], "foreign_blob");

    // Nothing was stored, and the page is what it was.
    assert_eq!(stored_updates(&kit, &page).await, 1);
    let pull = kit
        .sync(&ana, workspace, &page, &docs::client(97), None)
        .await;
    let reader = docs::client(96);
    docs::apply(
        &reader,
        &STANDARD
            .decode(pull.json["update"].as_str().unwrap())
            .unwrap(),
    );
    assert_eq!(docs::text(&reader), "An honest line\n");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reader_pulls_but_does_not_push_and_a_stranger_finds_nothing(db: PgPool) {
    let Fixture {
        kit,
        workspace,
        ana,
        ..
    } = fixture(db).await;
    // Ana's private page, shared with Rui as a reader.
    let (rui, rui_id) = kit.member("rui2", workspace);
    let private = kit.add(&ana, workspace, json!({"title": "Private"})).await;
    let private_id = private["id"].as_str().unwrap();
    let uri = Kit::pages(workspace);
    // A stranger gets 404 on a page that exists, pull or push.
    let client = docs::client(30);
    let update = paragraph(&client, "hello");
    let stranger = kit
        .sync(&rui, workspace, private_id, &client, Some(&update))
        .await;
    assert_eq!(stranger.status, StatusCode::NOT_FOUND);
    let stranger = kit.sync(&rui, workspace, private_id, &client, None).await;
    assert_eq!(stranger.status, StatusCode::NOT_FOUND);

    kit.put(
        &ana,
        &format!("{uri}/{private_id}/grants/{rui_id}"),
        json!({"role": "reader"}),
    )
    .await;
    let pull = kit.sync(&rui, workspace, private_id, &client, None).await;
    assert_eq!(pull.status, StatusCode::OK, "{:?}", pull.json);
    let push = kit
        .sync(&rui, workspace, private_id, &client, Some(&update))
        .await;
    assert_eq!(push.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_in_the_trash_takes_no_changes(db: PgPool) {
    let Fixture {
        kit,
        workspace,
        ana,
        page,
        ..
    } = fixture(db).await;
    let uri = format!("{}/{page}", Kit::pages(workspace));
    let trashed = kit.post(&ana, &format!("{uri}/trash"), None).await;
    assert_eq!(trashed.status, StatusCode::OK);
    let client = docs::client(31);
    let update = paragraph(&client, "late");
    let refused = kit
        .sync(&ana, workspace, &page, &client, Some(&update))
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(refused.json["code"], "page_trashed");
    // Reading it is still allowed.
    assert_eq!(
        kit.sync(&ana, workspace, &page, &client, None).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn body_changes_are_announced_once_every_few_seconds_and_hold_no_text(db: PgPool) {
    let Fixture {
        kit,
        workspace,
        ana,
        page,
        ..
    } = fixture(db).await;
    let client = docs::client(40);
    for line in ["one", "two", "three"] {
        let update = paragraph(&client, line);
        let synced = kit
            .sync(&ana, workspace, &page, &client, Some(&update))
            .await;
        assert_eq!(synced.status, StatusCode::OK, "{:?}", synced.json);
    }
    let events = kit.events("pages.page_changed").await;
    let bodies: Vec<_> = events
        .iter()
        .filter(|event| event["kinds"] == json!(["body"]))
        .collect();
    // Three syncs within a few seconds: one announcement.
    assert_eq!(bodies.len(), 1, "{events:?}");
    for event in &events {
        let text = event.to_string();
        assert!(!text.contains("one") || !text.contains("two"));
        assert!(!text.contains("Weekly"));
    }
    // The revision still grew with every change.
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM pages.pages WHERE id = $1")
        .bind(uuid(&page))
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(revision, 4);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_update_it_already_has_changes_nothing(db: PgPool) {
    let Fixture {
        kit,
        workspace,
        ana,
        page,
        ..
    } = fixture(db).await;
    let client = docs::client(50);
    let update = paragraph(&client, "once");
    for _ in 0..2 {
        let synced = kit
            .sync(&ana, workspace, &page, &docs::client(51), Some(&update))
            .await;
        assert_eq!(synced.status, StatusCode::OK, "{:?}", synced.json);
    }
    assert_eq!(stored_updates(&kit, &page).await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_file_uploaded_to_the_page_can_be_named_and_one_from_a_stranger_cannot(db: PgPool) {
    let Fixture {
        kit,
        workspace,
        ana,
        rui,
        page,
    } = fixture(db).await;
    let uri = Kit::pages(workspace);
    let uploaded = kit
        .upload(
            &ana,
            &format!("{uri}/{page}/files"),
            "image/png",
            b"\x89PNG\r\n\x1a\nfake image bytes".to_vec(),
        )
        .await;
    assert_eq!(uploaded.status, StatusCode::CREATED, "{:?}", uploaded.json);
    let blob = uploaded.json["blob"].as_str().unwrap().to_owned();
    let image = |doc: &yrs::Doc| {
        element(
            doc,
            "image",
            &[
                ("blob", s(&blob)),
                ("name", s("diagram.png")),
                ("size", Any::Number(26.0)),
            ],
            None,
        )
    };
    // Rui never uploaded it, and it is on this page: any writer of the page
    // may name it.
    let rui_doc = docs::client(60);
    let named = image(&rui_doc);
    let ok = kit
        .sync(&rui, workspace, &page, &rui_doc, Some(&named))
        .await;
    assert_eq!(ok.status, StatusCode::OK, "{:?}", ok.json);

    // Another page of Ana's, where the blob is not: Rui may not name it
    // there, since he did not upload it.
    let other = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Other", "place": "workspace"}),
        )
        .await;
    let other_id = other["id"].as_str().unwrap();
    let rui_doc = docs::client(61);
    let named = image(&rui_doc);
    let refused = kit
        .sync(&rui, workspace, other_id, &rui_doc, Some(&named))
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json["code"], "foreign_blob");
    // Ana uploaded it, so she may use it on her other page, which then
    // holds it too.
    let ana_doc = docs::client(62);
    let named = image(&ana_doc);
    let ok = kit
        .sync(&ana, workspace, other_id, &ana_doc, Some(&named))
        .await;
    assert_eq!(ok.status, StatusCode::OK, "{:?}", ok.json);
    let held: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.files WHERE hash = $1")
        .bind(&blob)
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(held, 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_update_text_never_reaches_the_event_log_or_the_job_queue(db: PgPool) {
    let Fixture {
        kit,
        workspace,
        ana,
        page,
        ..
    } = fixture(db).await;
    let client = docs::client(70);
    let update = paragraph(&client, "Quarterly numbers: confidential");
    kit.sync(&ana, workspace, &page, &client, Some(&update))
        .await;
    let events: Vec<String> = sqlx::query_scalar("SELECT payload::text FROM kernel.events")
        .fetch_all(&kit.db)
        .await
        .unwrap();
    let jobs: Vec<String> = sqlx::query_scalar("SELECT payload::text FROM kernel.jobs")
        .fetch_all(&kit.db)
        .await
        .unwrap();
    for text in events.iter().chain(&jobs) {
        assert!(!text.contains("confidential"), "{text}");
        assert!(!text.contains("Quarterly"), "{text}");
        assert!(!text.contains("Weekly"), "{text}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_table_block_shows_a_table_the_writer_can_read_and_nothing_else(db: PgPool) {
    let Fixture {
        kit,
        workspace,
        ana,
        rui,
        page,
    } = fixture(db).await;
    let shared = kit.table(&ana, workspace).await;
    let table = shared["page"]["id"].as_str().unwrap().to_owned();
    let view = shared["views"][0]["id"].as_str().unwrap().to_owned();
    // A private table of Ana's that Rui cannot see, and an ordinary page.
    let secret = kit
        .post(
            &ana,
            &Kit::tables(workspace),
            Some(json!({"title": "Private", "place": "private"})),
        )
        .await
        .json;
    let secret_table = secret["page"]["id"].as_str().unwrap().to_owned();
    let secret_view = secret["views"][0]["id"].as_str().unwrap().to_owned();

    let block = |seed: u64, table: &str, view: &str| {
        element(
            &docs::client(seed),
            "tableView",
            &[("table", s(table)), ("view", s(view))],
            None,
        )
    };
    let shown = kit
        .sync(
            &rui,
            workspace,
            &page,
            &docs::client(1),
            Some(&block(31, &table, &view)),
        )
        .await;
    assert_eq!(shown.status, StatusCode::OK, "{:?}", shown.json);
    // The page records the table as embedded.
    let embedded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pages.links WHERE page_id = $1 AND ref_id = $2 AND kind = 'embed'",
    )
    .bind(uuid(&page))
    .bind(&table)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(embedded, 1);

    // Another table's view, an ordinary page and a table Rui cannot read all
    // get the same answer.
    let other_view = block(32, &table, &secret_view);
    let not_a_table = block(33, &page, &view);
    let hidden = block(34, &secret_table, &secret_view);
    for (what, update) in [
        ("a view of another table", other_view),
        ("a page that is no table", not_a_table),
        ("a table Rui cannot read", hidden),
    ] {
        let refused = kit
            .sync(&rui, workspace, &page, &docs::client(40), Some(&update))
            .await;
        assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{what}");
        assert_eq!(refused.json["code"], "bad_table_view", "{what}");
    }
    // A block with a malformed id is refused as a bad attribute.
    let malformed = element(
        &docs::client(35),
        "tableView",
        &[("table", s("nope")), ("view", s(&view))],
        None,
    );
    let refused = kit
        .sync(&rui, workspace, &page, &docs::client(41), Some(&malformed))
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
}
