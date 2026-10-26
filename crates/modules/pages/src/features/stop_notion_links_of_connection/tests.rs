//! A Notion connection that goes takes its links with it, and nobody else's.

use sqlx::PgPool;
use uuid::Uuid;

use crate::{events::ConnectionRemoved, testkit_notion::Rig};

#[sqlx::test(migrations = "../../../migrations")]
async fn disconnecting_stops_every_page_linked_through_it(db: PgPool) {
    let rig = Rig::new(db).await;
    let (one, two) = (
        rig.import(Uuid::now_v7(), "One").await,
        rig.import(Uuid::now_v7(), "Two").await,
    );
    let other = rig.kit.connection_of(rig.user);
    let uri = format!("{}/notion/imports", crate::testkit::Kit::pages(rig.ws));
    let third = rig
        .kit
        .post(
            &rig.token,
            &uri,
            Some(serde_json::json!({"connection": other, "pages": [{"notion_page": Uuid::now_v7(), "title": "Three"}]})),
        )
        .await;
    let third = third.json["pages"][0]["page"].as_str().unwrap().to_owned();
    let mut tx = rig.kit.begin(rig.ws).await;
    rig.kit
        .state
        .events
        .publish(
            &mut tx,
            &ConnectionRemoved {
                workspace: rig.ws.to_string(),
                connection: rig.connection,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rig.kit.run_jobs().await;
    for page in [&one, &two] {
        let link = rig.link(page).await;
        assert_eq!(link["state"], "stopped");
        assert_eq!(link["stopped_reason"], "disconnected");
    }
    assert_eq!(rig.link(&third).await["state"], "syncing");
    assert_eq!(rig.kit.page_count(rig.ws).await, 3, "no page goes");
}
