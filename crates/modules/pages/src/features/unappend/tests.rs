//! Taking back a section.

use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;
use yrs::{Text, XmlFragment, XmlOut};

use crate::testkit::{Kit, docs};

fn page_uri(workspace: WorkspaceId, page: &str) -> String {
    format!("{}/{page}", Kit::pages(workspace))
}

async fn append(
    kit: &Kit,
    token: &str,
    workspace: WorkspaceId,
    page: &str,
    markdown: &str,
) -> Value {
    let added = kit
        .post(
            token,
            &format!("{}/append", page_uri(workspace, page)),
            Some(json!({"markdown": markdown})),
        )
        .await;
    assert_eq!(added.status, StatusCode::OK, "{:?}", added.json);
    added.json
}

async fn unappend(
    kit: &Kit,
    token: &str,
    workspace: WorkspaceId,
    page: &str,
    append_id: &Value,
) -> campbooks_kernel::testing::TestResponse {
    kit.post(
        token,
        &format!("{}/unappend", page_uri(workspace, page)),
        Some(json!({"append_id": append_id})),
    )
    .await
}

async fn fixture(db: PgPool) -> (Kit, WorkspaceId, String, String) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Client onboarding"}))
        .await;
    let id = page["id"].as_str().unwrap().to_owned();
    (kit, workspace, ana, id)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn taking_a_section_back_leaves_the_page_as_it_was(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let before = kit.page_text(&ana, workspace, &page).await;
    let added = append(
        &kit,
        &ana,
        workspace,
        &page,
        "## Kickoff\n\n- one\n- two\n\nDone.",
    )
    .await;
    assert_ne!(kit.page_text(&ana, workspace, &page).await, before);

    let undone = unappend(&kit, &ana, workspace, &page, &added["append_id"]).await;
    assert_eq!(undone.status, StatusCode::OK, "{:?}", undone.json);
    assert_eq!(undone.json, json!({"removed": 4, "stayed": 0}));
    assert_eq!(kit.page_text(&ana, workspace, &page).await, before);

    // Sent again, it removes nothing: an undo can be repeated.
    let again = unappend(&kit, &ana, workspace, &page, &added["append_id"]).await;
    assert_eq!(again.json, json!({"removed": 0, "stayed": 0}));
    // And the section can be put back, as its own append.
    let back = append(
        &kit,
        &ana,
        workspace,
        &page,
        "## Kickoff\n\n- one\n- two\n\nDone.",
    )
    .await;
    assert_ne!(back["append_id"], added["append_id"]);
    assert!(
        kit.page_text(&ana, workspace, &page)
            .await
            .contains("## Kickoff")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_block_someone_edited_since_stays(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let added = append(&kit, &ana, workspace, &page, "- one\n- two\n- three").await;

    // Ana edits the second bullet in her editor, and syncs.
    let client = docs::client(41);
    let pull = kit.sync(&ana, workspace, &page, &client, None).await;
    docs::apply(
        &client,
        &STANDARD
            .decode(pull.json["update"].as_str().unwrap())
            .unwrap(),
    );
    let edited = docs::edit(&client, |txn, root| {
        let Some(XmlOut::Element(list)) = root.get(txn, 0) else {
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
        run.insert(txn, end, " (agreed)");
    });
    let synced = kit
        .sync(&ana, workspace, &page, &client, Some(&edited))
        .await;
    assert_eq!(synced.status, StatusCode::OK, "{:?}", synced.json);

    let undone = unappend(&kit, &ana, workspace, &page, &added["append_id"]).await;
    assert_eq!(undone.json, json!({"removed": 2, "stayed": 1}));
    assert_eq!(
        kit.page_text(&ana, workspace, &page).await,
        "# Client onboarding\n\n- two (agreed)"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_append_of_another_page_or_one_that_never_was_removes_nothing(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let other = kit.add(&ana, workspace, json!({"title": "Other"})).await;
    let other = other["id"].as_str().unwrap();
    let on_other = append(&kit, &ana, workspace, other, "Keep me").await;
    append(&kit, &ana, workspace, &page, "Mine").await;

    // This page cannot take back the other page's section.
    let wrong = unappend(&kit, &ana, workspace, &page, &on_other["append_id"]).await;
    assert_eq!(wrong.json, json!({"removed": 0, "stayed": 0}));
    assert!(
        kit.page_text(&ana, workspace, other)
            .await
            .contains("Keep me")
    );
    let unknown = unappend(&kit, &ana, workspace, &page, &json!(uuid::Uuid::now_v7())).await;
    assert_eq!(unknown.json, json!({"removed": 0, "stayed": 0}));
    assert!(kit.page_text(&ana, workspace, &page).await.contains("Mine"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_a_writer_of_the_page_takes_a_section_back(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let (rui, rui_id) = kit.member("rui", workspace);
    let added = append(&kit, &ana, workspace, &page, "Mine").await;
    // Rui cannot see the private page.
    let unseen = unappend(&kit, &rui, workspace, &page, &added["append_id"]).await;
    assert_eq!(unseen.status, StatusCode::NOT_FOUND);
    kit.share(&ana, workspace, &page, rui_id, "reader").await;
    let forbidden = unappend(&kit, &rui, workspace, &page, &added["append_id"]).await;
    assert_eq!(forbidden.status, StatusCode::FORBIDDEN);
    assert!(kit.page_text(&ana, workspace, &page).await.contains("Mine"));
    // A writer may.
    kit.share(&ana, workspace, &page, rui_id, "writer").await;
    let undone = unappend(&kit, &rui, workspace, &page, &added["append_id"]).await;
    assert_eq!(undone.json, json!({"removed": 1, "stayed": 0}));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_in_the_trash_keeps_its_section(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let added = append(&kit, &ana, workspace, &page, "Mine").await;
    kit.post(&ana, &format!("{}/trash", page_uri(workspace, &page)), None)
        .await;
    let undone = unappend(&kit, &ana, workspace, &page, &added["append_id"]).await;
    assert_eq!(undone.status, StatusCode::CONFLICT);
    assert_eq!(undone.json["code"], "page_trashed");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_edited_block_keeps_the_append_so_a_second_undo_can_still_remove_it(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let added = append(&kit, &ana, workspace, &page, "- one\n- two").await;
    let records = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM pages.appends")
            .fetch_one(&kit.db)
            .await
            .unwrap()
    };
    assert_eq!(records().await, 1);

    // Ana adds a word to the second bullet, and syncs.
    let client = docs::client(51);
    let pull = kit.sync(&ana, workspace, &page, &client, None).await;
    docs::apply(
        &client,
        &STANDARD
            .decode(pull.json["update"].as_str().unwrap())
            .unwrap(),
    );
    let item_run = |txn: &mut yrs::TransactionMut, root: &yrs::XmlFragmentRef| {
        let Some(XmlOut::Element(list)) = root.get(txn, 0) else {
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
        run
    };
    let typed = docs::edit(&client, |txn, root| {
        let run = item_run(txn, root);
        let end = run.len(txn);
        run.insert(txn, end, "!");
    });
    assert_eq!(
        kit.sync(&ana, workspace, &page, &client, Some(&typed))
            .await
            .status,
        StatusCode::OK
    );

    let first = unappend(&kit, &ana, workspace, &page, &added["append_id"]).await;
    assert_eq!(first.json, json!({"removed": 1, "stayed": 1}));
    // The record stays: one block of the append is still on the page.
    assert_eq!(records().await, 1);
    let again = unappend(&kit, &ana, workspace, &page, &added["append_id"]).await;
    assert_eq!(again.json, json!({"removed": 0, "stayed": 1}));

    // Ana takes her word back out; now it is as it was added.
    let erased = docs::edit(&client, |txn, root| {
        let run = item_run(txn, root);
        let end = run.len(txn);
        run.remove_range(txn, end - 1, 1);
    });
    kit.sync(&ana, workspace, &page, &client, Some(&erased))
        .await;
    let last = unappend(&kit, &ana, workspace, &page, &added["append_id"]).await;
    assert_eq!(last.json, json!({"removed": 1, "stayed": 0}));
    // Every block of it is gone: the record goes too.
    assert_eq!(records().await, 0);
    assert_eq!(
        kit.page_text(&ana, workspace, &page).await,
        "# Client onboarding"
    );
}
