//! Taking back a choice: only while the block is there, only once, and only
//! by someone who may write the page.

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    domain::notion::tests::{id, para, snapshot},
    testkit::Kit,
    testkit_notion::Rig,
};

async fn settled(rig: &Rig, keep: &str) -> (String, String) {
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "t").await;
    let mut first = snapshot(vec![para(1, "Hello", 1)], 5, true);
    first.notion_page = notion_page;
    rig.read(&page, notion_page, &first).await;
    rig.type_into(&page, 0, " mine").await;
    let mut theirs = snapshot(vec![para(1, "Hello theirs", 7)], 8, true);
    theirs.notion_page = notion_page;
    rig.read(&page, notion_page, &theirs).await;
    let bid = rig.tree(&page).await.conflicts[0].of;
    let done = rig
        .kit
        .post(
            &rig.token,
            &format!("{}/{page}/notion/conflicts/{bid}", Kit::pages(rig.ws)),
            Some(json!({"keep": keep})),
        )
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    (page, done.json["resolution"].as_str().unwrap().to_owned())
}

fn undo(rig: &Rig, page: &str, resolution: &str) -> String {
    format!(
        "{}/{page}/notion/resolutions/{resolution}/undo",
        Kit::pages(rig.ws)
    )
}

#[sqlx::test(migrations = "../../../migrations")]
async fn keep_both_is_taken_back_with_the_copy_it_made(db: PgPool) {
    let rig = Rig::new(db).await;
    let (page, resolution) = settled(&rig, "both").await;
    assert_eq!(rig.texts(&page).await.len(), 2);
    let back = rig.kit.post(&rig.token, &undo(&rig, &page, &resolution), None).await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
    assert_eq!(rig.texts(&page).await, ["Hello mine"]);
    assert_eq!(rig.tree(&page).await.conflicts.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_member_or_another_page_cannot_take_it_back(db: PgPool) {
    let rig = Rig::new(db).await;
    let (page, resolution) = settled(&rig, "notion").await;
    let (rui, _) = rig.kit.member("rui", rig.ws);
    let hidden = rig.kit.post(&rui, &undo(&rig, &page, &resolution), None).await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
    // The choice of one page is not taken back through another.
    let other = rig
        .kit
        .add(&rig.token, rig.ws, json!({"title": "Other", "place": "private"}))
        .await;
    let crossed = rig
        .kit
        .post(&rig.token, &undo(&rig, other["id"].as_str().unwrap(), &resolution), None)
        .await;
    assert_eq!(crossed.status, StatusCode::CONFLICT);
    assert_eq!(rig.texts(&page).await, ["Hello theirs"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn it_cannot_be_taken_back_once_the_block_is_gone(db: PgPool) {
    let rig = Rig::new(db).await;
    let (page, resolution) = settled(&rig, "mine").await;
    rig.edit_client(&page, |txn, fragment| {
        use yrs::XmlFragment as _;
        fragment.remove_range(txn, 0, 1);
    })
    .await;
    let back = rig.kit.post(&rig.token, &undo(&rig, &page, &resolution), None).await;
    assert_eq!(back.status, StatusCode::CONFLICT, "{:?}", back.json);
}
