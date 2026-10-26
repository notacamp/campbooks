//! Linking a page to Notion makes the stream that reads it, but only for a
//! link the pages module confirms and a Notion connection of the workspace.

use campbooks_kernel::{
    db::BeginFor as _,
    tenancy::{Role, WorkspaceId},
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::model::ConnectionId,
    events::NotionLinkRequested,
    testkit::Kit,
};

async fn request(kit: &Kit, ws: WorkspaceId, page: Uuid, connection: ConnectionId, notion_page: Uuid) {
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state
        .events
        .publish(
            &mut tx,
            &NotionLinkRequested {
                workspace: ws.to_string(),
                page,
                connection,
                notion_page,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

fn id_of(connection: &serde_json::Value) -> ConnectionId {
    connection["id"].as_str().unwrap().parse().unwrap()
}

async fn streams(kit: &Kit) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM connectors.streams")
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_confirmed_link_makes_one_stream_that_runs_at_once(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let notion_page = kit.notion.add_page("Roadmap", None);
    let page = Uuid::now_v7();
    let stream = kit.link_notion(ws, &connection, page, notion_page).await;
    let (capability, key, paused): (String, String, bool) = sqlx::query_as(
        "SELECT capability, mailbox_key, paused FROM connectors.streams WHERE id = $1",
    )
    .bind(stream)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(capability, "pages");
    assert_eq!(key, notion_page.to_string());
    assert!(!paused);
    // The same request again changes nothing.
    request(&kit, ws, page, id_of(&connection), notion_page).await;
    assert_eq!(streams(&kit).await, 1);
    // It has read the page already.
    assert_eq!(kit.payloads(ws, "connectors.notion_page_read").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_link_the_pages_module_does_not_confirm_makes_nothing(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    let notion_page = kit.notion.add_page("Roadmap", None);
    // The event, with no link standing in the pages module.
    request(&kit, ws, Uuid::now_v7(), id_of(&connection), notion_page).await;
    assert_eq!(streams(&kit).await, 0);
    assert!(kit.payloads(ws, "connectors.notion_page_read").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_connection_that_is_not_notions_ends_the_link(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let google = kit.connect(&marta, ws).await;
    let page = Uuid::now_v7();
    let notion_page = Uuid::now_v7();
    kit.pages.link(page, *id_of(&google).as_uuid(), notion_page);
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.streams")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    request(&kit, ws, page, id_of(&google), notion_page).await;
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM connectors.streams")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(before, after, "no stream reads Notion through a Gmail grant");
    let stopped = kit.payloads(ws, "connectors.notion_link_stopped").await;
    assert_eq!(stopped.len(), 1);
    assert_eq!(stopped[0]["reason"], "notion_unavailable");
    assert_eq!(stopped[0]["page"], page.to_string());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_workspaces_connection_is_not_one_to_read_through(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws_a = WorkspaceId::new();
    let ws_b = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws_a, Role::Member);
    let rui = kit.user("rui");
    kit.join(&rui, ws_b, Role::Member);
    let connection = kit.connect_notion(&marta, ws_a).await;
    let notion_page = kit.notion.add_page("Roadmap", None);
    let page = Uuid::now_v7();
    kit.pages.link(page, *id_of(&connection).as_uuid(), notion_page);
    request(&kit, ws_b, page, id_of(&connection), notion_page).await;
    assert_eq!(streams(&kit).await, 0);
    assert!(kit.notion_stream_opt(notion_page).await.is_none());
    let stopped = kit.payloads(ws_b, "connectors.notion_link_stopped").await;
    assert_eq!(stopped.len(), 1);
    assert!(kit.payloads(ws_a, "connectors.notion_link_stopped").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_who_must_connect_again_gets_a_paused_stream(db: PgPool) {
    let kit = Kit::with_notion(db).await;
    let ws = WorkspaceId::new();
    let marta = kit.user("marta");
    kit.join(&marta, ws, Role::Member);
    let connection = kit.connect_notion(&marta, ws).await;
    sqlx::query("UPDATE connectors.connections SET health = 'needs_reauth'")
        .execute(&kit.db)
        .await
        .unwrap();
    let notion_page = kit.notion.add_page("Roadmap", None);
    let stream = kit
        .link_notion(ws, &connection, Uuid::now_v7(), notion_page)
        .await;
    let paused: bool = sqlx::query_scalar("SELECT paused FROM connectors.streams WHERE id = $1")
        .bind(stream)
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert!(paused);
    assert!(kit.payloads(ws, "connectors.notion_page_read").await.is_empty());
}
