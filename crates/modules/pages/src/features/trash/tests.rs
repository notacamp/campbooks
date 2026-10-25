//! The trash: delete, restore, delete for good.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::testkit::Kit;

async fn make_tree(kit: &Kit, token: &str, workspace: WorkspaceId) -> (Value, Value, Value) {
    let handbook = kit
        .add(token, workspace, json!({"title": "Team handbook"}))
        .await;
    let holidays = kit
        .add(
            token,
            workspace,
            json!({"title": "Holidays", "parent": handbook["id"]}),
        )
        .await;
    let policy = kit
        .add(
            token,
            workspace,
            json!({"title": "Policy", "parent": holidays["id"]}),
        )
        .await;
    (handbook, holidays, policy)
}

async fn trashed_count(kit: &Kit) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM pages.pages WHERE trashed_at IS NOT NULL")
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_and_its_sub_pages_go_to_the_trash_together_and_come_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (handbook, _, _) = make_tree(&kit, &ana, workspace).await;
    let uri = format!(
        "{}/{}",
        Kit::pages(workspace),
        handbook["id"].as_str().unwrap()
    );

    let trashed = kit.post(&ana, &format!("{uri}/trash"), None).await;
    assert_eq!(trashed.status, StatusCode::OK, "{:?}", trashed.json);
    assert!(trashed.json["trashed_at"].is_string());
    assert_eq!(trashed_count(&kit).await, 3);
    // Out of the list, into the trash list.
    let list = kit.get(&ana, &Kit::pages(workspace)).await.json;
    assert_eq!(list["pages"], json!([]));
    // Again is a conflict.
    assert_eq!(
        kit.post(&ana, &format!("{uri}/trash"), None).await.status,
        StatusCode::CONFLICT
    );

    let restored = kit.post(&ana, &format!("{uri}/restore"), None).await;
    assert_eq!(restored.status, StatusCode::OK, "{:?}", restored.json);
    assert_eq!(restored.json["trashed_at"], Value::Null);
    assert_eq!(trashed_count(&kit).await, 0);
    let list = kit.get(&ana, &Kit::pages(workspace)).await.json;
    assert_eq!(list["pages"].as_array().unwrap().len(), 3);
    // Restoring what is not in the trash is a conflict.
    assert_eq!(
        kit.post(&ana, &format!("{uri}/restore"), None).await.json["code"],
        "page_not_trashed"
    );
    // Every change was announced without a title.
    for event in kit.events("pages.page_changed").await {
        assert!(!event.to_string().contains("handbook"));
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn restoring_after_the_parent_was_deleted_for_good_puts_the_page_at_the_top(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (handbook, holidays, _) = make_tree(&kit, &ana, workspace).await;
    let uri = Kit::pages(workspace);
    let holidays_uri = format!("{uri}/{}", holidays["id"].as_str().unwrap());
    let handbook_uri = format!("{uri}/{}", handbook["id"].as_str().unwrap());

    // "Holidays" is deleted on its own, then "Team handbook" and what is
    // left of it, which is then deleted for good.
    assert_eq!(
        kit.post(&ana, &format!("{holidays_uri}/trash"), None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        kit.post(&ana, &format!("{handbook_uri}/trash"), None)
            .await
            .status,
        StatusCode::OK
    );
    let purged = kit.delete(&ana, &handbook_uri).await;
    assert_eq!(purged.status, StatusCode::NO_CONTENT, "{:?}", purged.json);
    // "Holidays" (and what went with it) is still in the trash.
    let rest = kit.get(&ana, &format!("{uri}?trashed=true")).await.json;
    assert_eq!(rest["pages"].as_array().unwrap().len(), 1);

    let restored = kit
        .post(&ana, &format!("{holidays_uri}/restore"), None)
        .await;
    assert_eq!(restored.status, StatusCode::OK, "{:?}", restored.json);
    assert_eq!(restored.json["parent"], Value::Null);
    assert_eq!(restored.json["place"], "private");
    let list = kit.get(&ana, &uri).await.json;
    assert_eq!(list["pages"].as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_a_page_deleted_itself_can_be_restored_or_deleted_for_good(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (handbook, _, policy) = make_tree(&kit, &ana, workspace).await;
    let uri = Kit::pages(workspace);
    let handbook_uri = format!("{uri}/{}", handbook["id"].as_str().unwrap());
    let policy_uri = format!("{uri}/{}", policy["id"].as_str().unwrap());

    // Not in the trash: nothing to delete for good.
    let early = kit.delete(&ana, &handbook_uri).await;
    assert_eq!(early.status, StatusCode::CONFLICT);
    assert_eq!(early.json["code"], "page_not_trashed");

    kit.post(&ana, &format!("{handbook_uri}/trash"), None).await;
    // The sub-page went with its parent: act on the parent.
    let inner = kit.post(&ana, &format!("{policy_uri}/restore"), None).await;
    assert_eq!(inner.status, StatusCode::CONFLICT);
    assert_eq!(inner.json["code"], "page_not_trash_root");
    assert_eq!(
        kit.delete(&ana, &policy_uri).await.status,
        StatusCode::CONFLICT
    );
    assert_eq!(trashed_count(&kit).await, 3);

    assert_eq!(
        kit.delete(&ana, &handbook_uri).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(kit.page_count(workspace).await, 0);
    let purged: Vec<Value> = kit.events("pages.page_changed").await;
    assert!(purged.iter().any(|e| e["kinds"] == json!(["purge"])));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn deleting_needs_the_right_to_delete(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let (wes, _) = kit.owner("wes", workspace);
    let shared = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Plan", "place": "workspace"}),
        )
        .await;
    let private = kit.add(&ana, workspace, json!({"title": "Notes"})).await;
    let uri = Kit::pages(workspace);
    let shared_uri = format!("{uri}/{}", shared["id"].as_str().unwrap());
    let private_uri = format!("{uri}/{}", private["id"].as_str().unwrap());

    // Everyone writes a workspace page; only its creator, a workspace owner
    // and a manager delete it.
    assert_eq!(
        kit.post(&rui, &format!("{shared_uri}/trash"), None)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    // A private page that was never shared is not there at all, not even for
    // the workspace's owner.
    assert_eq!(
        kit.post(&wes, &format!("{private_uri}/trash"), None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    // As a manager, Rui deletes Ana's private page.
    kit.put(
        &ana,
        &format!("{private_uri}/grants/{rui_id}"),
        json!({"role": "manager"}),
    )
    .await;
    assert_eq!(
        kit.post(&rui, &format!("{private_uri}/trash"), None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        kit.post(&wes, &format!("{shared_uri}/trash"), None)
            .await
            .status,
        StatusCode::OK
    );
}
