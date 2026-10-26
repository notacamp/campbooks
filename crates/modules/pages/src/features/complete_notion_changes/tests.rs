//! A request that ended moves the bases the last sync agreed: what was
//! written is in step now, what was not is still a difference.

use campbooks_kernel::tenancy::WorkspaceId;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    domain::notion::{
        model::hash,
        tests::{at, id, para, snapshot},
    },
    events::{NotionChangesCompleted, NotionItemResult, NotionOutcome},
    testkit_notion::Rig,
};

async fn completed(rig: &Rig, request: Uuid, page: &str, results: Vec<NotionItemResult>) {
    completed_in(rig, rig.ws, request, page, results).await;
}

async fn completed_in(rig: &Rig, ws: WorkspaceId, request: Uuid, page: &str, results: Vec<NotionItemResult>) {
    let mut tx = rig.kit.begin(ws).await;
    rig.kit
        .state
        .events
        .publish(
            &mut tx,
            &NotionChangesCompleted {
                workspace: ws.to_string(),
                request,
                page: page.parse().unwrap(),
                results,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    rig.kit.run_jobs().await;
}

async fn base_of(rig: &Rig, notion_block: Uuid) -> (Vec<u8>, chrono::DateTime<chrono::Utc>) {
    sqlx::query_as(
        "SELECT base_hash, base_edited_at FROM pages.external_blocks WHERE notion_block_id = $1",
    )
    .bind(notion_block)
    .fetch_one(&rig.kit.db)
    .await
    .unwrap()
}

async fn ready(db: PgPool) -> (Rig, String, Uuid, Uuid, Uuid) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "Roadmap").await;
    let mut snap = snapshot(vec![para(1, "Plan", 1), para(2, "Notes", 1)], 5, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    rig.type_into(&page, 1, " more").await;
    rig.push_now().await;
    let (request, _, items) = rig.requests(&page).await.remove(0);
    let bid: Uuid = items[0]["bid"].as_str().unwrap().parse().unwrap();
    (rig, page, request, bid, notion_page)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_applied_update_moves_the_base_to_what_was_written(db: PgPool) {
    let (rig, page, request, bid, _) = ready(db).await;
    completed(
        &rig,
        request,
        &page,
        vec![NotionItemResult {
            block: bid,
            outcome: NotionOutcome::Applied,
            notion_block: Some(id(2)),
            edited_at: Some(at(20)),
            reason: None,
        }],
    )
    .await;
    let (written, edited) = base_of(&rig, id(2)).await;
    assert_eq!(written, hash(&para(2, "Notes more", 1)).to_vec());
    assert_eq!(edited, at(20));
    assert_eq!(rig.requests(&page).await[0].1, "done");
    assert!(rig.link(&page).await["last_outbound_at"].is_string());
    // The next read finds Notion as we left it: nothing to do, no conflict.
    let mut snap = snapshot(vec![para(1, "Plan", 1), para(2, "Notes more", 20)], 21, true);
    snap.notion_page = id(0xAA);
    rig.read(&page, id(0xAA), &snap).await;
    assert_eq!(rig.texts(&page).await, ["Plan", "Notes more"]);
    assert_eq!(rig.link(&page).await["conflicts"], 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_skipped_change_leaves_the_base_so_the_difference_comes_back_as_a_conflict(db: PgPool) {
    let (rig, page, request, bid, _) = ready(db).await;
    let before = base_of(&rig, id(2)).await;
    completed(
        &rig,
        request,
        &page,
        vec![NotionItemResult {
            block: bid,
            outcome: NotionOutcome::Skipped,
            notion_block: None,
            edited_at: None,
            reason: Some(crate::events::NotionReason::ChangedInNotion),
        }],
    )
    .await;
    assert_eq!(base_of(&rig, id(2)).await, before, "the base did not move");
    assert_eq!(rig.requests(&page).await[0].1, "done");
    // Notion's own edit arrives: both sides changed the block.
    let mut snap = snapshot(vec![para(1, "Plan", 1), para(2, "Notes, their way", 15)], 16, true);
    snap.notion_page = id(0xAA);
    rig.read(&page, id(0xAA), &snap).await;
    assert_eq!(rig.texts(&page).await, ["Plan", "Notes more"]);
    assert_eq!(rig.tree(&page).await.conflicts.len(), 1);
    assert_eq!(rig.link(&page).await["conflicts"], 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_applied_append_makes_the_new_block_the_same_block_as_notions(db: PgPool) {
    let rig = Rig::new(db).await;
    let notion_page = id(0xAA);
    let page = rig.import(notion_page, "Roadmap").await;
    let mut snap = snapshot(vec![para(1, "Plan", 1)], 5, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    rig.edit_client(&page, |txn, fragment| {
        use yrs::{XmlElementPrelim, XmlFragment, XmlTextPrelim};
        let line = fragment.push_back(txn, XmlElementPrelim::empty("paragraph"));
        line.push_back(txn, XmlTextPrelim::new("Added here"));
    })
    .await;
    rig.push_now().await;
    let (request, _, items) = rig.requests(&page).await.remove(0);
    let bid: Uuid = items[0]["bid"].as_str().unwrap().parse().unwrap();
    completed(
        &rig,
        request,
        &page,
        vec![NotionItemResult {
            block: bid,
            outcome: NotionOutcome::Applied,
            notion_block: Some(id(9)),
            edited_at: Some(at(12)),
            reason: None,
        }],
    )
    .await;
    assert!(rig.bases(&page).await.contains(&(bid, id(9))));
    // The read that follows has Notion's block 9: the same block, no copy.
    let mut snap = snapshot(vec![para(1, "Plan", 1), para(9, "Added here", 12)], 13, true);
    snap.notion_page = notion_page;
    rig.read(&page, notion_page, &snap).await;
    assert_eq!(rig.texts(&page).await, ["Plan", "Added here"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_report_for_another_page_or_workspace_or_a_second_report_changes_nothing(db: PgPool) {
    let (rig, page, request, bid, _) = ready(db).await;
    let before = base_of(&rig, id(2)).await;
    let result = || NotionItemResult {
        block: bid,
        outcome: NotionOutcome::Applied,
        notion_block: Some(id(2)),
        edited_at: Some(at(20)),
        reason: None,
    };
    // Naming another page than the request's.
    completed(&rig, request, &Uuid::now_v7().to_string(), vec![result()]).await;
    assert_eq!(base_of(&rig, id(2)).await, before);
    assert_eq!(rig.requests(&page).await[0].1, "waiting");
    // In another workspace.
    let elsewhere = WorkspaceId::new();
    completed_in(&rig, elsewhere, request, &page, vec![result()]).await;
    assert_eq!(base_of(&rig, id(2)).await, before);
    // For a block the request did not name.
    let mut stray = result();
    stray.block = Uuid::now_v7();
    completed(&rig, request, &page, vec![stray]).await;
    assert_eq!(base_of(&rig, id(2)).await, before);
    // Once for real, then again with other times.
    completed(&rig, request, &page, vec![result()]).await;
    let after = base_of(&rig, id(2)).await;
    let mut later = result();
    later.edited_at = Some(at(40));
    completed(&rig, request, &page, vec![later]).await;
    assert_eq!(base_of(&rig, id(2)).await, after, "a request ends once");
}
