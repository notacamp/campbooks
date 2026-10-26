//! Which pages are kept in step with Notion: only those the caller may open.

use axum::http::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::notion::tests::{para, snapshot},
    testkit::Kit,
    testkit_notion::Rig,
};

#[sqlx::test(migrations = "../../../migrations")]
async fn the_links_of_the_pages_the_caller_may_open_say_how_they_are_going(db: PgPool) {
    let rig = Rig::new(db).await;
    let (one, two) = (Uuid::now_v7(), Uuid::now_v7());
    let first = rig.import(one, "One").await;
    let second = rig.import(two, "Two").await;
    let mut snap = snapshot(vec![para(1, "Hello", 1)], 5, true);
    snap.notion_page = one;
    rig.read(&first, one, &snap).await;
    let uri = format!("{}/notion/links", Kit::pages(rig.ws));
    let listed = rig.kit.get(&rig.token, &uri).await;
    assert_eq!(listed.status, StatusCode::OK, "{:?}", listed.json);
    let links = listed.json["links"].as_array().unwrap();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0]["page"], first);
    assert_eq!(links[0]["state"], "syncing");
    assert_eq!(links[0]["conflicts"], 0);
    assert!(links[0]["last_inbound_at"].is_string());
    assert!(links[1]["last_inbound_at"].is_null());
    assert_eq!(links[1]["page"], second);
    assert_eq!(links[0]["owner"], rig.user.to_string());
    // Rui, who cannot open her private pages, sees none.
    let (rui, _) = rig.kit.member("rui", rig.ws);
    let theirs = rig.kit.get(&rui, &uri).await;
    assert_eq!(theirs.json["links"].as_array().unwrap().len(), 0);
    // A stopped link says why.
    rig.kit
        .post(&rig.token, &format!("{}/{second}/notion/stop", Kit::pages(rig.ws)), None)
        .await;
    let listed = rig.kit.get(&rig.token, &uri).await;
    assert_eq!(listed.json["links"][1]["stopped_reason"], "stopped_by_member");
    // A client acting for nobody is refused.
    let client = rig.kit.client(rig.ws, "pages:read");
    assert_eq!(rig.kit.get(&client, &uri).await.status, StatusCode::FORBIDDEN);
}
