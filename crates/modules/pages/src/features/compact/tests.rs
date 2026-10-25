//! Folding updates into a snapshot.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::{jobs, jobs::EnqueueOptions, tenancy::WorkspaceId};
use serde_json::json;
use sqlx::PgPool;

use super::Compact;
use crate::{
    domain::PageId,
    testkit::{Kit, docs},
};

async fn compact(kit: &Kit, workspace: WorkspaceId, page: &str) {
    let mut tx = kit.begin(workspace).await;
    jobs::enqueue(
        &mut tx,
        &Compact {
            page: page.parse::<PageId>().unwrap(),
        },
        EnqueueOptions::for_workspace(workspace.to_string()),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

async fn counts(kit: &Kit, page: &str) -> (i64, i64) {
    let page: sqlx::types::Uuid = page.parse().unwrap();
    let updates = sqlx::query_scalar("SELECT count(*) FROM pages.updates WHERE page_id = $1")
        .bind(page)
        .fetch_one(&kit.db)
        .await
        .unwrap();
    let snapshots = sqlx::query_scalar("SELECT count(*) FROM pages.snapshots WHERE page_id = $1")
        .bind(page)
        .fetch_one(&kit.db)
        .await
        .unwrap();
    (updates, snapshots)
}

async fn text_of(kit: &Kit, token: &str, workspace: WorkspaceId, page: &str) -> String {
    let got = kit
        .get(token, &format!("{}/{page}", Kit::pages(workspace)))
        .await;
    let reader = docs::client(1000);
    docs::apply(
        &reader,
        &STANDARD
            .decode(got.json["document"]["update"].as_str().unwrap())
            .unwrap(),
    );
    docs::text(&reader)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn updates_fold_into_a_snapshot_and_the_page_reads_the_same(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Log"})).await;
    let page = page["id"].as_str().unwrap().to_owned();
    let client = docs::client(1);
    for line in ["one", "two", "three"] {
        let update = docs::paragraph(&client, line);
        assert_eq!(
            kit.sync(&ana, workspace, &page, &client, Some(&update))
                .await
                .status,
            axum::http::StatusCode::OK
        );
    }
    assert_eq!(counts(&kit, &page).await, (3, 0));
    let before = text_of(&kit, &ana, workspace, &page).await;

    compact(&kit, workspace, &page).await;
    assert_eq!(counts(&kit, &page).await, (0, 1));
    assert_eq!(text_of(&kit, &ana, workspace, &page).await, before);
    assert_eq!(before, "one\ntwo\nthree\n");

    // The next update goes after the snapshot, and compacting again folds it.
    let update = docs::paragraph(&client, "four");
    kit.sync(&ana, workspace, &page, &client, Some(&update))
        .await;
    assert_eq!(counts(&kit, &page).await, (1, 1));
    assert_eq!(
        text_of(&kit, &ana, workspace, &page).await,
        "one\ntwo\nthree\nfour\n"
    );
    compact(&kit, workspace, &page).await;
    assert_eq!(counts(&kit, &page).await, (0, 1));
    // Nothing to fold is not an error.
    compact(&kit, workspace, &page).await;
    assert_eq!(
        text_of(&kit, &ana, workspace, &page).await,
        "one\ntwo\nthree\nfour\n"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_compaction_racing_syncs_loses_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let made = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Log", "place": "workspace"}),
        )
        .await;
    let page = made["id"].as_str().unwrap().to_owned();
    let (ana_doc, rui_doc) = (docs::client(1), docs::client(2));
    for round in 0..4 {
        let a = docs::paragraph(&ana_doc, &format!("ana {round}"));
        let r = docs::paragraph(&rui_doc, &format!("rui {round}"));
        let mut tx = kit.begin(workspace).await;
        jobs::enqueue(
            &mut tx,
            &Compact {
                page: page.parse::<PageId>().unwrap(),
            },
            EnqueueOptions::for_workspace(workspace.to_string()),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let (ana_sync, rui_sync, _) = tokio::join!(
            kit.sync(&ana, workspace, &page, &ana_doc, Some(&a)),
            kit.sync(&rui, workspace, &page, &rui_doc, Some(&r)),
            kit.run_jobs(),
        );
        assert_eq!(
            ana_sync.status,
            axum::http::StatusCode::OK,
            "{:?}",
            ana_sync.json
        );
        assert_eq!(
            rui_sync.status,
            axum::http::StatusCode::OK,
            "{:?}",
            rui_sync.json
        );
    }
    let text = text_of(&kit, &ana, workspace, &page).await;
    for round in 0..4 {
        assert!(text.contains(&format!("ana {round}\n")), "{text}");
        assert!(text.contains(&format!("rui {round}\n")), "{text}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_long_tail_asks_for_a_compaction_at_once_and_a_short_one_in_ten_minutes(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let made = kit.add(&ana, workspace, json!({"title": "Log"})).await;
    let page = made["id"].as_str().unwrap().to_owned();
    let client = docs::client(1);
    let update = docs::paragraph(&client, "first");
    kit.sync(&ana, workspace, &page, &client, Some(&update))
        .await;
    let soon: (i64, bool) = sqlx::query_as(
        "SELECT count(*), bool_and(run_at > now() + interval '9 minutes') FROM kernel.jobs
          WHERE kind = 'pages.compact' AND unique_key LIKE '%:idle'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(soon, (1, true));
    for n in 1..200 {
        let update = docs::paragraph(&client, &format!("line {n}"));
        kit.sync(&ana, workspace, &page, &client, Some(&update))
            .await;
    }
    let now: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'pages.compact' AND unique_key LIKE '%:now'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(now, 1);
}
