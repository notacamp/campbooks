//! The connector stopped reading a page: the link ends with the reason, the
//! page stays, and only the link the event names.

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::notion::tests::{id, para, snapshot},
    events::{NotionLinkStopped, NotionStop},
    testkit_notion::Rig,
};

async fn stopped(rig: &Rig, page: &str, connection: Uuid, notion_page: Uuid, reason: NotionStop) {
    let mut tx = rig.kit.begin(rig.ws).await;
    rig.kit
        .state
        .events
        .publish(
            &mut tx,
            &NotionLinkStopped {
                workspace: rig.ws.to_string(),
                connection,
                notion_page,
                page: page.parse().unwrap(),
                reason,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rig.kit.run_jobs().await;
}

#[sqlx::test(migrations = "../../../migrations")]
async fn each_reason_ends_the_link_and_keeps_the_page(db: PgPool) {
    let rig = Rig::new(db).await;
    for (reason, name) in [
        (NotionStop::NotShared, "not_shared"),
        (NotionStop::TrashedInNotion, "trashed_in_notion"),
        (NotionStop::NotionUnavailable, "notion_unavailable"),
    ] {
        let notion_page = Uuid::now_v7();
        let page = rig.import(notion_page, "Kept").await;
        let mut snap = snapshot(vec![para(1, "Words stay", 1)], 5, true);
        snap.notion_page = notion_page;
        rig.read(&page, notion_page, &snap).await;
        stopped(&rig, &page, rig.connection, notion_page, reason).await;
        let link = rig.link(&page).await;
        assert_eq!(link["state"], "stopped", "{name}");
        assert_eq!(link["stopped_reason"], name);
        assert_eq!(rig.texts(&page).await, ["Words stay"], "{name}");
        let ended = rig.kit.events("pages.notion_link_ended").await;
        assert!(
            ended.iter().any(|e| e["page"] == page),
            "the connector is told to forget the stream ({name})"
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_event_naming_another_connection_or_notion_page_ends_nothing(db: PgPool) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "t").await;
    stopped(
        &rig,
        &page,
        Uuid::now_v7(),
        notion_page,
        NotionStop::NotShared,
    )
    .await;
    stopped(&rig, &page, rig.connection, id(0xBB), NotionStop::NotShared).await;
    assert_eq!(rig.link(&page).await["state"], "syncing");
}
