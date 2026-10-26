//! Making block changes at Notion: the base check that holds back a block
//! Notion edited first, appends in order, and a page that is never removed.

use std::collections::HashMap;

use campbooks_kernel::blobs::{BlobHash, BlobRef};
use sqlx::{PgPool, types::Json};
use uuid::Uuid;

use super::*;
use crate::{
    config::NotionAdapter,
    domain::model::{ConnectionId, StreamId},
    notion::{
        ClientSecret, Endpoints,
        blocks::{Block, Kind, Run},
    },
    testkit::Kit,
    testkit_notion::{CLIENT_ID, CLIENT_SECRET, FakeNotion},
};

struct Rig {
    pace: Pace,
    fake: FakeNotion,
    notion: Notion,
    token: String,
    page: Uuid,
}

async fn rig(db: PgPool) -> Rig {
    let kit = Kit::new(db).await;
    let fake = FakeNotion::start().await;
    let notion = Notion::new(
        CLIENT_ID.into(),
        ClientSecret::Inline(CLIENT_SECRET.into()),
        Endpoints::under(&fake.base()).unwrap(),
        "http://localhost:3000/auth/callback/notion".into(),
    )
    .unwrap();
    let (token, _) = fake.grant();
    let page = fake.add_page("Roadmap", None);
    let pace = Pace::new(
        kit.state.db.clone(),
        ConnectionId::new(),
        &NotionAdapter {
            requests_per_second: 1_000,
            requests_per_minute: 100_000,
            ..NotionAdapter::default()
        },
    );
    Rig {
        pace,
        fake,
        notion,
        token,
        page,
    }
}

fn paragraph(text: &str) -> Block {
    let mut block = Block::new(Uuid::now_v7(), Kind::Paragraph);
    block.runs = vec![Run {
        text: text.to_owned(),
        ..Run::default()
    }];
    block
}

fn row(kind: &str) -> Recorded {
    Recorded {
        id: Uuid::now_v7(),
        stream_id: StreamId::new(),
        request_id: Uuid::now_v7(),
        page_id: Uuid::now_v7(),
        seq: 0,
        kind: kind.to_owned(),
        block_id: Uuid::now_v7(),
        notion_block_id: None,
        parent_id: None,
        parent_block_id: None,
        after_notion_id: None,
        after_block_id: None,
        base_edited_at: None,
        body: Json(BlobRef {
            hash: BlobHash::of(b""),
            size: 0,
            content_type: String::new(),
        }),
        mac: None,
    }
}

async fn run(
    rig: &mut Rig,
    row: &Recorded,
    item: Option<&Block>,
    made: &mut HashMap<Uuid, Uuid>,
) -> Made {
    make(
        &rig.notion,
        &rig.token,
        &mut rig.pace,
        row,
        item,
        rig.page,
        made,
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_update_is_written_when_notion_has_not_edited_the_block_since(db: PgPool) {
    let mut rig = rig(db).await;
    let block = rig.fake.paragraph(rig.page, "Old");
    let mut change = row("update");
    change.notion_block_id = Some(block);
    change.base_edited_at = rig.fake.edited(block);
    let outcome = run(
        &mut rig,
        &change,
        Some(&paragraph("New")),
        &mut HashMap::new(),
    )
    .await;
    assert!(matches!(outcome, Made::Applied(w) if w.notion_block == block));
    assert_eq!(rig.fake.text(block).as_deref(), Some("New"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_update_is_held_back_when_notion_changed_the_block_first(db: PgPool) {
    let mut rig = rig(db).await;
    let block = rig.fake.paragraph(rig.page, "Old");
    let mut change = row("update");
    change.notion_block_id = Some(block);
    change.base_edited_at = rig.fake.edited(block);
    // Someone edits it in Notion after the last sync.
    rig.fake.set_text(block, "Theirs");
    let outcome = run(
        &mut rig,
        &change,
        Some(&paragraph("Mine")),
        &mut HashMap::new(),
    )
    .await;
    assert_eq!(outcome, Made::Skipped(NotionReason::ChangedInNotion));
    assert_eq!(rig.fake.text(block).as_deref(), Some("Theirs"));
    assert!(rig.fake.patches().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_block_without_a_base_time_is_not_written_over(db: PgPool) {
    let mut rig = rig(db).await;
    let block = rig.fake.paragraph(rig.page, "Old");
    let mut change = row("update");
    change.notion_block_id = Some(block);
    change.base_edited_at = None;
    let outcome = run(
        &mut rig,
        &change,
        Some(&paragraph("New")),
        &mut HashMap::new(),
    )
    .await;
    assert_eq!(outcome, Made::Skipped(NotionReason::ChangedInNotion));
    assert!(rig.fake.patches().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_update_of_a_block_that_is_gone_is_skipped(db: PgPool) {
    let mut rig = rig(db).await;
    let block = rig.fake.paragraph(rig.page, "Old");
    let mut change = row("update");
    change.notion_block_id = Some(block);
    change.base_edited_at = rig.fake.edited(block);
    rig.fake.remove_block(block);
    let outcome = run(
        &mut rig,
        &change,
        Some(&paragraph("New")),
        &mut HashMap::new(),
    )
    .await;
    assert_eq!(outcome, Made::Skipped(NotionReason::NotFound));
    let unknown = row("update");
    let outcome = run(
        &mut rig,
        &unknown,
        Some(&paragraph("New")),
        &mut HashMap::new(),
    )
    .await;
    assert_eq!(outcome, Made::Failed(NotionReason::Rejected));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn appends_land_in_order_under_the_block_they_follow(db: PgPool) {
    let mut rig = rig(db).await;
    let first = rig.fake.paragraph(rig.page, "First");
    let last = rig.fake.paragraph(rig.page, "Last");
    let mut made = HashMap::new();

    let mut one = row("append");
    one.after_notion_id = Some(first);
    let a = paragraph("A");
    assert!(matches!(
        run(&mut rig, &one, Some(&a), &mut made).await,
        Made::Applied(_)
    ));
    // The next one follows the block the first made, by its id in Pages.
    let mut two = row("append");
    two.after_block_id = Some(one.block_id);
    let b = paragraph("B");
    assert!(matches!(
        run(&mut rig, &two, Some(&b), &mut made).await,
        Made::Applied(_)
    ));
    let order: Vec<String> = rig
        .fake
        .children_of(rig.page)
        .into_iter()
        .map(|id| rig.fake.text(id).unwrap())
        .collect();
    assert_eq!(order, ["First", "A", "B", "Last"]);
    assert_eq!(made.len(), 2);
    assert!(rig.fake.children_of(rig.page).contains(&last));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_append_after_a_block_that_was_never_made_is_skipped(db: PgPool) {
    let mut rig = rig(db).await;
    let mut change = row("append");
    change.after_block_id = Some(Uuid::now_v7());
    let outcome = run(
        &mut rig,
        &change,
        Some(&paragraph("X")),
        &mut HashMap::new(),
    )
    .await;
    assert_eq!(outcome, Made::Skipped(NotionReason::NotFound));
    assert!(rig.fake.appends().is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_delete_removes_an_unedited_block_and_holds_back_an_edited_one(db: PgPool) {
    let mut rig = rig(db).await;
    let plain = rig.fake.paragraph(rig.page, "Plain");
    let touched = rig.fake.paragraph(rig.page, "Touched");
    let mut remove = row("delete");
    remove.notion_block_id = Some(plain);
    remove.base_edited_at = rig.fake.edited(plain);
    assert!(matches!(
        run(&mut rig, &remove, None, &mut HashMap::new()).await,
        Made::Applied(_)
    ));
    assert_eq!(rig.fake.deleted(), vec![plain]);

    let mut held = row("delete");
    held.notion_block_id = Some(touched);
    held.base_edited_at = rig.fake.edited(touched);
    rig.fake.set_text(touched, "Edited meanwhile");
    assert_eq!(
        run(&mut rig, &held, None, &mut HashMap::new()).await,
        Made::Skipped(NotionReason::ChangedInNotion)
    );
    assert_eq!(rig.fake.deleted(), vec![plain]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_is_never_removed_whatever_the_request_says(db: PgPool) {
    let mut rig = rig(db).await;
    let sub = rig.fake.add_page("A sub-page", Some(rig.page));
    rig.fake.add_page("Sibling", None);
    let mut change = row("delete");
    change.notion_block_id = Some(sub);
    change.base_edited_at = rig.fake.edited(sub);
    let outcome = run(&mut rig, &change, None, &mut HashMap::new()).await;
    assert_eq!(outcome, Made::Skipped(NotionReason::Unsupported));
    // Notion was never asked to remove anything.
    assert!(rig.fake.deleted().is_empty());
    assert!(
        rig.fake.calls().iter().all(|c| !c.starts_with("DELETE")),
        "{:?}",
        rig.fake.calls()
    );
    // And the client has no way to archive a page by its own: only blocks.
    let again = run(&mut rig, &change, None, &mut HashMap::new()).await;
    assert_eq!(again, Made::Skipped(NotionReason::Unsupported));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_kind_pages_does_not_write_is_unsupported(db: PgPool) {
    let mut rig = rig(db).await;
    let block = rig.fake.paragraph(rig.page, "Text");
    let mut change = row("update");
    change.notion_block_id = Some(block);
    change.base_edited_at = rig.fake.edited(block);
    let external = Block::new(Uuid::now_v7(), Kind::External);
    assert_eq!(
        run(&mut rig, &change, Some(&external), &mut HashMap::new()).await,
        Made::Skipped(NotionReason::Unsupported)
    );
    let odd = row("rearrange");
    assert_eq!(
        run(&mut rig, &odd, None, &mut HashMap::new()).await,
        Made::Skipped(NotionReason::Unsupported)
    );
}
