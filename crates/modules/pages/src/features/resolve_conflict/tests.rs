//! Settling a conflict with Notion over the API: each choice, who may, and
//! what is asked of Notion afterwards.

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::notion::tests::{id, para, snapshot},
    testkit::Kit,
    testkit_notion::Rig,
};

/// A page with one conflict: Marta typed " mine" after "Hello"; Notion made
/// it "Hello theirs". Returns the rig, the page and the block's stable id.
async fn conflicted(db: PgPool) -> (Rig, String, Uuid) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "t").await;
    let mut first = snapshot(vec![para(1, "Hello", 1), para(2, "Other", 1)], 5, true);
    first.notion_page = notion_page;
    rig.read(&page, notion_page, &first).await;
    rig.type_into(&page, 0, " mine").await;
    let mut theirs = snapshot(vec![para(1, "Hello theirs", 7), para(2, "Other", 1)], 8, true);
    theirs.notion_page = notion_page;
    rig.read(&page, notion_page, &theirs).await;
    let bid = rig.tree(&page).await.conflicts[0].of;
    (rig, page, bid)
}

fn uri(rig: &Rig, page: &str, block: Uuid) -> String {
    format!("{}/{page}/notion/conflicts/{block}", Kit::pages(rig.ws))
}

#[sqlx::test(migrations = "../../../migrations")]
async fn keep_mine_lets_the_block_go_to_notion(db: PgPool) {
    let (rig, page, bid) = conflicted(db).await;
    let done = rig
        .kit
        .post(&rig.token, &uri(&rig, &page, bid), Some(json!({"keep": "mine"})))
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    assert_eq!(done.json["conflicts"], 0);
    assert!(done.json["resolution"].is_string());
    assert_eq!(rig.texts(&page).await, ["Hello mine", "Other"]);
    assert_eq!(rig.link(&page).await["conflicts"], 0);
    // Now it differs from the base: the page asks for it to be written.
    rig.push_now().await;
    let requests = rig.requests(&page).await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].2[0]["action"], "update");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn use_notions_takes_its_words_and_sends_nothing(db: PgPool) {
    let (rig, page, bid) = conflicted(db).await;
    let done = rig
        .kit
        .post(&rig.token, &uri(&rig, &page, bid), Some(json!({"keep": "notion"})))
        .await;
    assert_eq!(done.status, StatusCode::OK);
    assert_eq!(rig.texts(&page).await, ["Hello theirs", "Other"]);
    rig.push_now().await;
    assert!(rig.requests(&page).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn keep_both_adds_notions_version_to_be_added_there(db: PgPool) {
    let (rig, page, bid) = conflicted(db).await;
    let done = rig
        .kit
        .post(&rig.token, &uri(&rig, &page, bid), Some(json!({"keep": "both"})))
        .await;
    assert_eq!(done.status, StatusCode::OK);
    assert_eq!(rig.texts(&page).await, ["Hello mine", "Hello theirs", "Other"]);
    rig.push_now().await;
    let items = rig.requests(&page).await[0].2.clone();
    let actions: Vec<&str> = items
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["action"].as_str().unwrap())
        .collect();
    assert_eq!(actions, ["update", "append"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_choice_can_be_taken_back_once(db: PgPool) {
    let (rig, page, bid) = conflicted(db).await;
    let done = rig
        .kit
        .post(&rig.token, &uri(&rig, &page, bid), Some(json!({"keep": "notion"})))
        .await;
    let resolution = done.json["resolution"].as_str().unwrap();
    let undo = format!(
        "{}/{page}/notion/resolutions/{resolution}/undo",
        Kit::pages(rig.ws)
    );
    let back = rig.kit.post(&rig.token, &undo, None).await;
    assert_eq!(back.status, StatusCode::OK, "{:?}", back.json);
    assert_eq!(back.json["conflicts"], 1);
    assert_eq!(rig.texts(&page).await, ["Hello mine", "Other"]);
    assert_eq!(rig.tree(&page).await.conflicts.len(), 1);
    assert_eq!(rig.link(&page).await["conflicts"], 1);
    // The conflict is back, so it can be settled another way.
    let again = rig.kit.post(&rig.token, &undo, None).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "cannot_undo");
    let other = rig
        .kit
        .post(&rig.token, &uri(&rig, &page, bid), Some(json!({"keep": "mine"})))
        .await;
    assert_eq!(other.status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_block_without_a_conflict_or_a_choice_nobody_made_is_refused(db: PgPool) {
    let (rig, page, bid) = conflicted(db).await;
    let none = rig
        .kit
        .post(&rig.token, &uri(&rig, &page, Uuid::now_v7()), Some(json!({"keep": "mine"})))
        .await;
    assert_eq!(none.status, StatusCode::CONFLICT);
    assert_eq!(none.json["code"], "no_conflict");
    let odd = rig
        .kit
        .post(&rig.token, &uri(&rig, &page, bid), Some(json!({"keep": "everything"})))
        .await;
    assert_eq!(odd.status, StatusCode::BAD_REQUEST);
    let nothing = rig
        .kit
        .post(
            &rig.token,
            &format!("{}/{page}/notion/resolutions/{}/undo", Kit::pages(rig.ws), Uuid::now_v7()),
            None,
        )
        .await;
    assert_eq!(nothing.status, StatusCode::CONFLICT);
    assert_eq!(rig.tree(&page).await.conflicts.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_someone_who_may_write_the_page_settles_it(db: PgPool) {
    let (rig, page, bid) = conflicted(db).await;
    let (rui, _) = rig.kit.member("rui", rig.ws);
    let hidden = rig
        .kit
        .post(&rui, &uri(&rig, &page, bid), Some(json!({"keep": "notion"})))
        .await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
    let client = rig.kit.client(rig.ws, "pages:write");
    let denied = rig
        .kit
        .post(&client, &uri(&rig, &page, bid), Some(json!({"keep": "notion"})))
        .await;
    assert!(matches!(denied.status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND));
    assert_eq!(rig.tree(&page).await.conflicts.len(), 1);
}
