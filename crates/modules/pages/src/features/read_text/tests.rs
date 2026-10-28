//! A page as text for Scout.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;
use yrs::{Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim};

use crate::{
    domain::markdown::MAX_TEXT_BYTES,
    testkit::{Kit, docs},
};

const TASK: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c33";

fn page_uri(workspace: WorkspaceId, page: &str) -> String {
    format!("{}/{page}", Kit::pages(workspace))
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

async fn read(
    kit: &Kit,
    token: &str,
    workspace: WorkspaceId,
    page: &str,
) -> campbooks_kernel::testing::TestResponse {
    kit.get(token, &format!("{}/text", page_uri(workspace, page)))
        .await
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_mention_is_a_reference_and_never_the_record_it_points_at(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let client = docs::client(51);
    let update = docs::edit(&client, |txn, root| {
        let line = root.push_back(txn, XmlElementPrelim::empty("paragraph"));
        line.push_back(txn, XmlTextPrelim::new("Send the welcome pack "));
        let mention = line.push_back(txn, XmlElementPrelim::empty("mention"));
        mention.insert_attribute(txn, "ref_type", docs::s("tasks.task"));
        mention.insert_attribute(txn, "ref_id", docs::s(TASK));
    });
    kit.sync(&ana, workspace, &page, &client, Some(&update))
        .await;

    let got = read(&kit, &ana, workspace, &page).await;
    assert_eq!(got.status, StatusCode::OK, "{:?}", got.json);
    assert_eq!(got.json["truncated"], false);
    assert_eq!(
        got.json["markdown"],
        format!("# Client onboarding\n\nSend the welcome pack [tasks.task:{TASK}]")
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_a_page_the_caller_may_read_is_read(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let (rui, rui_id) = kit.member("rui", workspace);
    let (wes, _) = kit.owner("wes", workspace);
    kit.post(
        &ana,
        &format!("{}/append", page_uri(workspace, &page)),
        Some(json!({"markdown": "Private words"})),
    )
    .await;
    for who in [&rui, &wes] {
        let got = read(&kit, who, workspace, &page).await;
        assert_eq!(got.status, StatusCode::NOT_FOUND);
        assert!(!got.json.to_string().contains("Private"));
    }
    kit.share(&ana, workspace, &page, rui_id, "reader").await;
    assert!(
        kit.page_text(&rui, workspace, &page)
            .await
            .contains("Private words")
    );
    // A client token acting for nobody has no page.
    let client = kit.client(workspace, "pages:read");
    assert_eq!(
        read(&kit, &client, workspace, &page).await.status,
        StatusCode::NOT_FOUND
    );
    // A token without the scope reads nothing.
    let no_scope = kit.token_with("ana", "pages:write");
    assert_eq!(
        read(&kit, &no_scope, workspace, &page).await.status,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_long_page_is_cut_at_a_block_and_says_so(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    let paragraph = format!("{}\n\n", "lorem ipsum ".repeat(100).trim_end());
    let section = paragraph.repeat(12);
    assert!(section.len() < 16 * 1024);
    for _ in 0..8 {
        let added = kit
            .post(
                &ana,
                &format!("{}/append", page_uri(workspace, &page)),
                Some(json!({"markdown": section})),
            )
            .await;
        assert_eq!(added.status, StatusCode::OK, "{:?}", added.json);
    }
    let got = read(&kit, &ana, workspace, &page).await;
    assert_eq!(got.json["truncated"], true);
    let markdown = got.json["markdown"].as_str().unwrap();
    assert!(markdown.len() <= MAX_TEXT_BYTES);
    // Whole paragraphs only.
    for block in markdown.split("\n\n").skip(1) {
        assert!(block.ends_with("ipsum"), "{block:?}");
    }
    assert!(markdown.len() > MAX_TEXT_BYTES - 2 * 1300);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_in_the_trash_is_not_read(db: PgPool) {
    let (kit, workspace, ana, page) = fixture(db).await;
    kit.post(&ana, &format!("{}/trash", page_uri(workspace, &page)), None)
        .await;
    let got: Value = read(&kit, &ana, workspace, &page).await.json;
    assert_eq!(got["code"], "page_trashed");
}
