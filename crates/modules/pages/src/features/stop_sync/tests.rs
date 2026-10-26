//! Stopping a page's sync, and the other ways a link ends: the page goes to
//! the trash, or its member leaves. Nothing is deleted on either side.

use axum::http::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::notion::tests::{para, snapshot},
    events::MemberLeft,
    testkit::Kit,
    testkit_notion::Rig,
};

async fn linked(rig: &Rig) -> (String, Uuid) {
    let notion_page = Uuid::now_v7();
    let page = rig.import(notion_page, "Roadmap").await;
    let mut snap = snapshot(vec![para(1, "Words stay", 1)], 5, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    (page, notion_page)
}

fn stop_uri(rig: &Rig, page: &str) -> String {
    format!("{}/{page}/notion/stop", Kit::pages(rig.ws))
}

#[sqlx::test(migrations = "../../../migrations")]
async fn stopping_ends_the_link_keeps_the_page_and_tells_the_connector(db: PgPool) {
    let rig = Rig::new(db).await;
    let (page, notion_page) = linked(&rig).await;
    rig.type_into(&page, 0, " more").await;
    rig.push_now().await;
    assert_eq!(rig.requests(&page).await[0].1, "waiting");
    let stopped = rig.kit.post(&rig.token, &stop_uri(&rig, &page), None).await;
    assert_eq!(stopped.status, StatusCode::OK, "{:?}", stopped.json);
    assert_eq!(stopped.json["state"], "stopped");
    assert_eq!(stopped.json["stopped_reason"], "stopped_by_member");
    // The page and its words stay; the waiting request ended with the link.
    assert_eq!(rig.texts(&page).await, ["Words stay more"]);
    assert_eq!(rig.requests(&page).await[0].1, "done");
    let ended = rig.kit.events("pages.notion_link_ended").await;
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0]["notion_page"], notion_page.to_string());
    assert_contract_ended(&ended[0]);
    // Stopping twice, or a page that was never linked: nothing to stop.
    let again = rig.kit.post(&rig.token, &stop_uri(&rig, &page), None).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "not_syncing");
    let plain = rig
        .kit
        .add(&rig.token, rig.ws, serde_json::json!({"title": "Plain"}))
        .await;
    let never = rig
        .kit
        .post(&rig.token, &stop_uri(&rig, plain["id"].as_str().unwrap()), None)
        .await;
    assert_eq!(never.status, StatusCode::CONFLICT);
    // Starting again is an import.
    let again = rig.import_response(&[(notion_page, "Roadmap again")]).await;
    assert_eq!(again.json["pages"].as_array().unwrap().len(), 1);
}

fn assert_contract_ended(event: &serde_json::Value) {
    crate::testkit_notion::assert_contract("pages.notion_link_ended.v1.schema.json", event);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_someone_who_may_write_the_page_stops_it(db: PgPool) {
    let rig = Rig::new(db).await;
    let (page, _) = linked(&rig).await;
    let (rui, _) = rig.kit.member("rui", rig.ws);
    // Rui cannot see Marta's private page.
    let hidden = rig.kit.post(&rui, &stop_uri(&rig, &page), None).await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
    let client = rig.kit.client(rig.ws, "pages:write");
    let denied = rig.kit.post(&client, &stop_uri(&rig, &page), None).await;
    assert!(matches!(denied.status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND));
    assert_eq!(rig.link(&page).await["state"], "syncing");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn putting_the_page_in_the_trash_ends_the_link(db: PgPool) {
    let rig = Rig::new(db).await;
    let (page, _) = linked(&rig).await;
    let trashed = rig
        .kit
        .post(&rig.token, &format!("{}/{page}/trash", Kit::pages(rig.ws)), None)
        .await;
    assert_eq!(trashed.status, StatusCode::OK, "{:?}", trashed.json);
    let link = rig.link(&page).await;
    assert_eq!(link["state"], "stopped");
    assert_eq!(link["stopped_reason"], "trashed_here");
    assert_eq!(rig.kit.events("pages.notion_link_ended").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_who_leaves_takes_their_links_but_not_their_pages(db: PgPool) {
    let rig = Rig::new(db).await;
    let (page, _) = linked(&rig).await;
    let mut tx = rig.kit.begin(rig.ws).await;
    rig.kit
        .state
        .events
        .publish(
            &mut tx,
            &MemberLeft {
                workspace: rig.ws.to_string(),
                user: rig.user,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rig.kit.run_jobs().await;
    let link = rig.link(&page).await;
    assert_eq!(link["state"], "stopped");
    assert_eq!(link["stopped_reason"], "member_left");
    assert_eq!(rig.kit.page_count(rig.ws).await, 1);
}
