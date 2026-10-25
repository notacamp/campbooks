//! A member who leaves.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use serde_json::json;
use sqlx::PgPool;

use crate::{events::MemberLeft, testkit::Kit};

async fn leaves(kit: &Kit, workspace: WorkspaceId, user: UserId) {
    let mut tx = kit.begin(workspace).await;
    kit.state
        .events
        .publish(
            &mut tx,
            &MemberLeft {
                workspace: workspace.to_string(),
                user,
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

async fn grants(kit: &Kit) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM pages.grants")
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn their_grants_go_and_their_pages_stay(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let (carol, carol_id) = kit.member("carol", workspace);
    let shared = kit.add(&ana, workspace, json!({"title": "Plan"})).await;
    let hers = kit
        .add(&rui, workspace, json!({"title": "Rui private"}))
        .await;
    let open = kit
        .add(
            &rui,
            workspace,
            json!({"title": "Handbook", "place": "workspace"}),
        )
        .await;
    let uri = Kit::pages(workspace);
    for member in [rui_id, carol_id] {
        let granted = kit
            .put(
                &ana,
                &format!("{uri}/{}/grants/{member}", shared["id"].as_str().unwrap()),
                json!({"role": "writer"}),
            )
            .await;
        assert_eq!(granted.status, StatusCode::OK);
    }
    assert_eq!(grants(&kit).await, 2);

    kit.remove_member("rui", workspace);
    leaves(&kit, workspace, rui_id).await;

    // Only Rui's grant went.
    assert_eq!(grants(&kit).await, 1);
    assert_eq!(
        kit.get(&carol, &format!("{uri}/{}", shared["id"].as_str().unwrap()))
            .await
            .status,
        StatusCode::OK
    );
    // His pages are where they were: nothing was deleted.
    assert_eq!(kit.page_count(workspace).await, 3);
    let _ = (hers, open, rui);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_who_is_back_keeps_what_they_have(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Plan"})).await;
    kit.put(
        &ana,
        &format!(
            "{}/{}/grants/{rui_id}",
            Kit::pages(workspace),
            page["id"].as_str().unwrap()
        ),
        json!({"role": "reader"}),
    )
    .await;
    // The event is old news: he is a member again.
    leaves(&kit, workspace, rui_id).await;
    assert_eq!(grants(&kit).await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn nobody_is_shared_a_page_after_leaving(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Plan"})).await;
    kit.remove_member("rui", workspace);
    let refused = kit
        .put(
            &ana,
            &format!(
                "{}/{}/grants/{rui_id}",
                Kit::pages(workspace),
                page["id"].as_str().unwrap()
            ),
            json!({"role": "reader"}),
        )
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(grants(&kit).await, 0);
}
