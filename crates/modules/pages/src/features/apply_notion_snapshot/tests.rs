//! A Notion snapshot applied to the page linked to it: only the page the
//! event is about, only a newer read, block by block, and nothing of a
//! page's words in an event.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::notion::{
        model::Kind,
        tests::{block, id, para, snapshot},
    },
    testkit_notion::Rig,
};

#[sqlx::test(migrations = "../../../migrations")]
async fn the_first_read_fills_the_page_and_names_it(db: PgPool) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "Draft title").await;
    assert!(rig.texts(&page).await.is_empty());
    let mut snap = snapshot(vec![para(1, "Plan the launch", 1), para(2, "Write the notes", 1)], 5, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    assert_eq!(rig.texts(&page).await, ["Plan the launch", "Write the notes"]);
    // The title is Notion's, as the page had not been renamed here.
    let shown = rig
        .kit
        .get(&rig.token, &format!("{}/{page}", crate::testkit::Kit::pages(rig.ws)))
        .await;
    assert_eq!(shown.json["title"], "Product roadmap");
    let link = rig.link(&page).await;
    assert_eq!(link["conflicts"], 0);
    assert!(link["last_inbound_at"].is_string());
    assert!(link["last_read_at"].is_string());
    assert_eq!(rig.bases(&page).await.len(), 2);
    // No event carries a word of the page.
    let all: Vec<String> = sqlx::query_scalar("SELECT payload::text FROM kernel.events")
        .fetch_all(&rig.kit.db)
        .await
        .unwrap();
    assert!(all.iter().all(|p| !p.contains("launch") && !p.contains("notes")), "{all:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_title_changed_here_is_not_overwritten(db: PgPool) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "Imported").await;
    let uri = format!("{}/{page}", crate::testkit::Kit::pages(rig.ws));
    let shown = rig.kit.get(&rig.token, &uri).await;
    let renamed = rig
        .kit
        .patch(
            &rig.token,
            &uri,
            serde_json::json!({"title": "My own name", "meta_revision": shown.json["meta_revision"]}),
        )
        .await;
    assert_eq!(renamed.status.as_u16(), 200, "{:?}", renamed.json);
    let mut snap = snapshot(vec![para(1, "x", 1)], 5, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    let shown = rig.kit.get(&rig.token, &uri).await;
    assert_eq!(shown.json["title"], "My own name");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_older_or_repeated_read_changes_nothing(db: PgPool) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "t").await;
    let mut newer = snapshot(vec![para(1, "Newer words", 9)], 10, true);
    newer.notion_page = notion_page;
    rig.read(&page, notion_page, &newer).await;
    let mut older = snapshot(vec![para(1, "Older words", 2)], 3, true);
    older.notion_page = notion_page;
    rig.read(&page, notion_page, &older).await;
    assert_eq!(rig.texts(&page).await, ["Newer words"]);
    let updates: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.updates")
        .fetch_one(&rig.kit.db)
        .await
        .unwrap();
    rig.read(&page, notion_page, &newer).await;
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.updates")
        .fetch_one(&rig.kit.db)
        .await
        .unwrap();
    assert_eq!(updates, after, "the same read again stores nothing");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_event_for_another_connection_or_notion_page_or_workspace_is_ignored(db: PgPool) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "t").await;
    let mut snap = snapshot(vec![para(1, "Forged", 1)], 5, true);
    snap.notion_page = notion_page;
    // Another connection claims the page.
    rig.read_as(&page, Uuid::now_v7(), notion_page, &snap).await;
    assert!(rig.texts(&page).await.is_empty());
    // The snapshot is of another Notion page than the link's.
    let mut other = snap.clone();
    other.notion_page = id(0xBB);
    rig.read(&page, notion_page, &other).await;
    assert!(rig.texts(&page).await.is_empty());
    // A page of nobody's.
    rig.read(&Uuid::now_v7().to_string(), notion_page, &snap).await;
    // A snapshot of a format this module does not read.
    let mut odd = snap.clone();
    odd.version = 2;
    rig.read(&page, notion_page, &odd).await;
    assert!(rig.texts(&page).await.is_empty());
    // And the real one still applies.
    rig.read(&page, notion_page, &snap).await;
    assert_eq!(rig.texts(&page).await, ["Forged"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_stopped_link_or_a_trashed_page_takes_no_more_reads(db: PgPool) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "t").await;
    let uri = format!("{}/{page}/notion/stop", crate::testkit::Kit::pages(rig.ws));
    assert_eq!(rig.kit.post(&rig.token, &uri, None).await.status.as_u16(), 200);
    let mut snap = snapshot(vec![para(1, "After stop", 1)], 5, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    assert!(rig.texts(&page).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_corrupt_or_oversize_snapshot_is_not_applied(db: PgPool) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "t").await;
    let junk = rig
        .kit
        .state
        .blobs
        .put(&rig.ws.to_string(), b"not json".to_vec(), "application/json")
        .await
        .unwrap();
    let mut tx = rig.kit.begin(rig.ws).await;
    rig.kit
        .state
        .events
        .publish(
            &mut tx,
            &crate::events::NotionPageRead {
                workspace: rig.ws.to_string(),
                connection: rig.connection,
                notion_page,
                page: page.parse().unwrap(),
                snapshot: junk,
                read_at: crate::domain::notion::tests::at(9),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rig.kit.run_jobs().await;
    assert!(rig.texts(&page).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sub_pages_become_linked_pages_beneath_it(db: PgPool) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "Parent").await;
    let mut link = block(7, Kind::PageLink, "Q4 plan", 1);
    link.page = Some(id(0x70));
    let mut snap = snapshot(vec![para(1, "See below", 1), link], 5, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    assert_eq!(rig.kit.page_count(rig.ws).await, 2);
    let sub: Uuid = sqlx::query_scalar(
        "SELECT page_id FROM pages.external_links WHERE notion_page_id = $1",
    )
    .bind(id(0x70))
    .fetch_one(&rig.kit.db)
    .await
    .unwrap();
    let shown = rig
        .kit
        .get(&rig.token, &format!("{}/{sub}", crate::testkit::Kit::pages(rig.ws)))
        .await;
    assert_eq!(shown.json["title"], "Q4 plan");
    assert_eq!(shown.json["parent"], page);
    // The connectors module is asked to read it.
    let requested = rig.kit.events("pages.notion_link_requested").await;
    assert!(requested.iter().any(|e| e["notion_page"] == id(0x70).to_string()));
    // The link in the parent points at the sub-page, and a second read makes
    // no second page.
    snap.read_at = crate::domain::notion::tests::at(9);
    rig.read(&page, notion_page, &snap).await;
    assert_eq!(rig.kit.page_count(rig.ws).await, 2);
    let body = rig.tree(&page).await;
    assert!(body.all().iter().any(|n| n.block.kind == Kind::PageLink));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn both_sides_changing_a_block_raises_one_conflict_and_counts_it(db: PgPool) {
    let rig = Rig::tracking(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "t").await;
    let mut first = snapshot(vec![para(1, "Hello", 1)], 5, true);
    first.notion_page = notion_page;
    rig.read(&page, notion_page, &first).await;
    rig.type_into(&page, 0, " from Marta").await;
    let mut theirs = snapshot(vec![para(1, "Hello from Notion", 7)], 8, true);
    theirs.notion_page = notion_page;
    rig.read(&page, notion_page, &theirs).await;
    assert_eq!(rig.texts(&page).await, ["Hello from Marta"]);
    assert_eq!(rig.tree(&page).await.conflicts.len(), 1);
    assert_eq!(rig.link(&page).await["conflicts"], 1);
    let raised = campbooks_kernel::testing::tracked_events(&rig.kit.db, "pages.notion_conflict_raised").await;
    assert_eq!(raised.len(), 1);
    let applied = campbooks_kernel::testing::tracked_events(&rig.kit.db, "pages.notion_sync_applied").await;
    assert!(!applied.is_empty());
    // Nothing of the words is in what is tracked.
    let body: Value = serde_json::to_value(applied.iter().map(|r| r.body.clone()).collect::<Vec<_>>()).unwrap();
    assert!(!body.to_string().contains("Hello"));
}
