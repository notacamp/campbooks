//! Reading a page's blocks: nesting, sub-pages, blocks Pages lacks, files
//! kept and refused, and the limits that make a read partial.

use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;

use super::*;
use crate::{
    config::NotionAdapter,
    domain::model::ConnectionId,
    notion::{ClientSecret, Endpoints, Notion},
    provider::ProviderError,
    testkit::Kit,
    testkit_notion::{CLIENT_ID, CLIENT_SECRET, FakeNotion, rich},
};

struct Rig {
    kit: Kit,
    fake: FakeNotion,
    notion: Notion,
    token: String,
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
    Rig {
        kit,
        fake,
        notion,
        token,
    }
}

async fn read(rig: &Rig, page: uuid::Uuid, limits: Limits) -> Result<Read, Stop> {
    let mut budget = Pace::new(
        rig.kit.state.db.clone(),
        ConnectionId::new(),
        &NotionAdapter {
            requests_per_second: 1_000,
            requests_per_minute: 100_000,
            ..NotionAdapter::default()
        },
    );
    let info = rig.notion.page(&rig.token, page).await.unwrap();
    read_page(
        &rig.notion,
        &rig.token,
        &mut budget,
        &rig.kit.state.blobs,
        "workspace-1",
        &info,
        Files::default(),
        limits,
        Utc::now(),
    )
    .await
}

const WIDE: Limits = Limits {
    max_blocks: 2000,
    max_depth: 12,
};

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_is_read_in_order_with_its_nesting(db: PgPool) {
    let rig = rig(db).await;
    let page = rig.fake.add_page("Product roadmap", None);
    rig.fake.paragraph(page, "First");
    let toggle = rig
        .fake
        .add_block(page, "toggle", &json!({"rich_text": [rich("More")]}));
    rig.fake.paragraph(toggle, "Inside");
    rig.fake.paragraph(page, "Last");
    let read = read(&rig, page, WIDE).await.unwrap();
    assert!(read.snapshot.complete);
    assert_eq!(read.snapshot.title, "Product roadmap");
    assert_eq!(read.snapshot.notion_page, page);
    let kinds: Vec<Kind> = read.snapshot.blocks.iter().map(|b| b.kind).collect();
    assert_eq!(kinds, vec![Kind::Paragraph, Kind::Toggle, Kind::Paragraph]);
    assert_eq!(read.snapshot.blocks[1].children.len(), 1);
    assert_eq!(read.snapshot.blocks[1].children[0].runs[0].text, "Inside");
    assert!(read.snapshot.blocks.iter().all(|b| b.edited.is_some()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_sub_page_is_a_link_not_a_read_of_its_blocks(db: PgPool) {
    let rig = rig(db).await;
    let page = rig.fake.add_page("Parent", None);
    let sub = rig.fake.add_page("Child", Some(page));
    rig.fake.paragraph(sub, "Not here");
    let before = rig.fake.calls().len();
    let read = read(&rig, page, WIDE).await.unwrap();
    assert_eq!(read.snapshot.blocks.len(), 1);
    assert_eq!(read.snapshot.blocks[0].kind, Kind::PageLink);
    assert_eq!(read.snapshot.blocks[0].page, Some(sub));
    // The sub-page's own blocks were never asked for.
    let asked: Vec<String> = rig.fake.calls()[before..].to_vec();
    assert!(
        asked.iter().all(|call| !call.contains(&sub.to_string())),
        "{asked:?}"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn blocks_pages_lacks_are_kept_in_notion_without_their_content(db: PgPool) {
    let rig = rig(db).await;
    let page = rig.fake.add_page("Parent", None);
    let columns = rig.fake.add_block(page, "column_list", &json!({}));
    rig.fake.paragraph(columns, "Secret column text");
    let read = read(&rig, page, WIDE).await.unwrap();
    assert_eq!(read.snapshot.blocks.len(), 1);
    let block = &read.snapshot.blocks[0];
    assert_eq!(block.kind, Kind::External);
    assert!(block.label.is_some());
    assert!(block.children.is_empty());
    let text = serde_json::to_string(&read.snapshot).unwrap();
    assert!(!text.contains("Secret column text"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_hosted_file_is_downloaded_once_and_a_refused_one_stays_in_notion(db: PgPool) {
    let rig = rig(db).await;
    let page = rig.fake.add_page("Files", None);
    let png = rig.fake.add_file("a.png", b"\x89PNG....", "image/png");
    let html = rig.fake.add_file("b.png", b"<script>", "text/html");
    let good = rig.fake.add_block(
        page,
        "image",
        &json!({"type": "file", "file": {"url": png}, "caption": []}),
    );
    rig.fake.add_block(
        page,
        "image",
        &json!({"type": "file", "file": {"url": html}, "caption": []}),
    );
    let first = read(&rig, page, WIDE).await.unwrap();
    assert_eq!(first.snapshot.blocks[0].kind, Kind::Image);
    let file = first.snapshot.blocks[0].file.as_ref().unwrap();
    assert_eq!(file.content_type, "image/png");
    assert_eq!(first.held.len(), 1);
    // The one of a type that is not allowed is not kept.
    assert_eq!(first.snapshot.blocks[1].kind, Kind::External);
    assert!(first.snapshot.blocks[1].file.is_none());
    assert!(first.files.known.contains_key(&good.to_string()));
    // Remembered: the unchanged file is not fetched again.
    let mut budget = Pace::new(
        rig.kit.state.db.clone(),
        ConnectionId::new(),
        &NotionAdapter::default(),
    );
    let info = rig.notion.page(&rig.token, page).await.unwrap();
    rig.fake.forget_calls();
    let again = read_page(
        &rig.notion,
        &rig.token,
        &mut budget,
        &rig.kit.state.blobs,
        "workspace-1",
        &info,
        first.files.clone(),
        WIDE,
        Utc::now(),
    )
    .await
    .unwrap();
    assert!(again.snapshot.blocks[0].file.is_some());
    assert!(
        rig.fake
            .calls()
            .iter()
            .all(|c| !c.starts_with("GET /files/a.png")),
        "{:?}",
        rig.fake.calls()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_over_the_limits_is_read_in_part_and_says_so(db: PgPool) {
    let rig = rig(db).await;
    let page = rig.fake.add_page("Long", None);
    for n in 0..6 {
        rig.fake.paragraph(page, &format!("Line {n}"));
    }
    let few = read(
        &rig,
        page,
        Limits {
            max_blocks: 3,
            max_depth: 12,
        },
    )
    .await
    .unwrap();
    assert!(!few.snapshot.complete);
    assert_eq!(few.snapshot.blocks.len(), 3);

    let deep_page = rig.fake.add_page("Deep", None);
    let mut at = deep_page;
    for _ in 0..4 {
        at = rig
            .fake
            .add_block(at, "toggle", &json!({"rich_text": [rich("down")]}));
    }
    let shallow = read(
        &rig,
        deep_page,
        Limits {
            max_blocks: 2000,
            max_depth: 2,
        },
    )
    .await
    .unwrap();
    assert!(!shallow.snapshot.complete);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn listings_in_small_pages_are_followed_to_the_end(db: PgPool) {
    let rig = rig(db).await;
    let page = rig.fake.add_page("Long", None);
    for n in 0..7 {
        rig.fake.paragraph(page, &format!("Line {n}"));
    }
    rig.fake.page_size(2);
    let read = read(&rig, page, WIDE).await.unwrap();
    assert!(read.snapshot.complete);
    assert_eq!(read.snapshot.blocks.len(), 7);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_that_is_not_shared_any_more_reads_as_not_found(db: PgPool) {
    let rig = rig(db).await;
    let page = rig.fake.add_page("Gone", None);
    rig.fake.paragraph(page, "x");
    let info = rig.notion.page(&rig.token, page).await.unwrap();
    rig.fake.unshare(page);
    let mut budget = Pace::new(
        rig.kit.state.db.clone(),
        ConnectionId::new(),
        &NotionAdapter::default(),
    );
    let outcome = read_page(
        &rig.notion,
        &rig.token,
        &mut budget,
        &rig.kit.state.blobs,
        "workspace-1",
        &info,
        Files::default(),
        WIDE,
        Utc::now(),
    )
    .await;
    assert!(matches!(
        outcome,
        Err(Stop::Provider(ProviderError::NotFound))
    ));
}
