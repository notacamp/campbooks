//! Ending a link removes only the stream of that page, connection and
//! Notion page, and nothing at Notion.

use campbooks_kernel::{
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{domain::model::ConnectionId, events::NotionLinkEnded, testkit::Kit};

#[sqlx::test(migrations = "../../../migrations")]
async fn only_the_link_the_event_names_is_ended(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let id: ConnectionId = connection["id"].as_str().unwrap().parse().unwrap();
    let (one, two) = (
        kit.notion.add_page("One", None),
        kit.notion.add_page("Two", None),
    );
    let (page_one, page_two) = (Uuid::now_v7(), Uuid::now_v7());
    kit.link_notion(ws, &connection, page_one, one).await;
    kit.link_notion(ws, &connection, page_two, two).await;

    // Right Notion page, wrong page of Pages: nothing ends.
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &NotionLinkEnded {
                workspace: ws.to_string(),
                page: page_two,
                connection: id,
                notion_page: one,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    assert!(kit.notion_stream_opt(one).await.is_some());
    assert!(kit.notion_stream_opt(two).await.is_some());

    kit.unlink_notion(ws, &connection, page_one, one).await;
    assert!(kit.notion_stream_opt(one).await.is_none());
    assert!(kit.notion_stream_opt(two).await.is_some());
    assert!(kit.notion.deleted().is_empty());
}
