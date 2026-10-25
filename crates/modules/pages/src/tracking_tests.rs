//! The module's tracked events (ADR 0020, campbooks-cloud#363), read back
//! from the outbox: one per change, with the actor, the operation and a
//! snapshot of ids and enums, and nothing for a change that changes nothing.
//! No event holds a title, an icon or a body: every such text in these tests
//! says `Weekly plan`, `Secret` or `Notes`.

use axum::http::StatusCode;
use campbooks_kernel::{
    tenancy::WorkspaceId,
    testing::{self, TrackedRow},
};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::testkit::Kit;

async fn tracked(kit: &Kit, name: &str) -> Vec<TrackedRow> {
    testing::tracked_events(&kit.db, name).await
}

/// No title, icon or body anywhere in the event.
fn nothing_written(event: &TrackedRow) {
    let body = event.body.to_string();
    for text in ["Weekly plan", "Secret", "Notes", "\"title\":", "\"icon\":"] {
        assert!(
            !body.contains(text),
            "{} holds what the member wrote ({text}): {body}",
            event.name
        );
    }
}

/// `event` is version 1 of `name`, in `workspace`, from a member's own
/// request to `operation`.
fn by_user(event: &TrackedRow, name: &str, workspace: WorkspaceId, operation: &str) {
    assert_eq!(event.name, name);
    assert_eq!(event.version, 1, "{name}");
    assert_eq!(event.workspace_id, Some(*workspace.as_uuid()), "{name}");
    assert_eq!(event.envelope["actor"]["type"], "user", "{name}");
    assert_eq!(event.envelope["operation"], operation, "{name}");
    assert_eq!(
        event.envelope["plan_version"],
        campbooks_kernel::tracking::PLAN_VERSION
    );
    nothing_written(event);
}

fn uri(workspace: WorkspaceId, page: &Value) -> String {
    format!("{}/{}", Kit::pages(workspace), page["id"].as_str().unwrap())
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_made_is_tracked_with_ids_and_enums_only(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let page = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Weekly plan", "icon": "star"}),
        )
        .await;

    let made = tracked(&kit, "pages.page_created").await;
    assert_eq!(made.len(), 1);
    by_user(&made[0], "pages.page_created", workspace, "pages.create");
    assert_eq!(made[0].body["page"]["id"], page["id"]);
    assert_eq!(made[0].body["page"]["place"], "private");
    assert_eq!(made[0].body["page"]["created_by"], ana_id.to_string());
    assert!(made[0].body["page"].get("parent").is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_rename_tracks_the_fields_and_the_same_values_again_track_nothing(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Weekly plan"}))
        .await;
    let page_uri = uri(workspace, &page);

    let renamed = kit
        .patch(
            &ana,
            &page_uri,
            json!({"meta_revision": 1, "title": "Secret plan", "icon": "star"}),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK, "{:?}", renamed.json);
    let edited = tracked(&kit, "pages.page_edited").await;
    assert_eq!(edited.len(), 1);
    by_user(&edited[0], "pages.page_edited", workspace, "pages.update");
    assert_eq!(edited[0].body["fields"], json!(["icon", "title"]));

    let again = kit
        .patch(
            &ana,
            &page_uri,
            json!({"meta_revision": 2, "title": "Secret plan", "icon": "star"}),
        )
        .await;
    assert_eq!(again.status, StatusCode::OK, "{:?}", again.json);
    assert_eq!(tracked(&kit, "pages.page_edited").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_move_tracks_where_the_page_was(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let a = kit.add(&ana, workspace, json!({"title": "A"})).await;
    let b = kit.add(&ana, workspace, json!({"title": "B"})).await;

    let moved = kit
        .post(
            &ana,
            &format!("{}/move", uri(workspace, &b)),
            Some(json!({"meta_revision": 1, "parent": a["id"]})),
        )
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    let events = tracked(&kit, "pages.page_moved").await;
    assert_eq!(events.len(), 1);
    by_user(&events[0], "pages.page_moved", workspace, "pages.move");
    assert_eq!(events[0].body["page"]["parent"], a["id"]);
    assert_eq!(events[0].body["before"]["place"], "private");
    assert!(events[0].body["before"].get("parent").is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sharing_tracks_the_member_the_role_and_the_one_before(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (_, rui_id) = kit.member("rui", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Weekly plan"}))
        .await;
    let grant = format!("{}/grants/{rui_id}", uri(workspace, &page));

    kit.put(&ana, &grant, json!({"role": "reader"})).await;
    kit.put(&ana, &grant, json!({"role": "writer"})).await;
    let shared = tracked(&kit, "pages.page_shared").await;
    assert_eq!(shared.len(), 2);
    by_user(&shared[0], "pages.page_shared", workspace, "pages.share");
    assert_eq!(shared[0].body["member"], rui_id.to_string());
    assert_eq!(shared[0].body["role"], "reader");
    assert!(shared[0].body.get("previous_role").is_none());
    assert_eq!(shared[1].body["role"], "writer");
    assert_eq!(shared[1].body["previous_role"], "reader");

    assert_eq!(
        kit.delete(&ana, &grant).await.status,
        StatusCode::NO_CONTENT
    );
    let unshared = tracked(&kit, "pages.page_unshared").await;
    assert_eq!(unshared.len(), 1);
    by_user(
        &unshared[0],
        "pages.page_unshared",
        workspace,
        "pages.unshare",
    );
    assert_eq!(unshared[0].body["member"], rui_id.to_string());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn trash_restore_and_delete_for_good_are_tracked_once_each(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Weekly plan"}))
        .await;
    let page_uri = uri(workspace, &page);

    kit.post(&ana, &format!("{page_uri}/trash"), None).await;
    // Again is a conflict and tracks nothing.
    kit.post(&ana, &format!("{page_uri}/trash"), None).await;
    let trashed = tracked(&kit, "pages.page_trashed").await;
    assert_eq!(trashed.len(), 1);
    by_user(&trashed[0], "pages.page_trashed", workspace, "pages.trash");

    kit.post(&ana, &format!("{page_uri}/restore"), None).await;
    let restored = tracked(&kit, "pages.page_restored").await;
    assert_eq!(restored.len(), 1);
    by_user(
        &restored[0],
        "pages.page_restored",
        workspace,
        "pages.restore",
    );

    kit.post(&ana, &format!("{page_uri}/trash"), None).await;
    let gone = kit.delete(&ana, &page_uri).await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT, "{:?}", gone.json);
    let purged = tracked(&kit, "pages.page_purged").await;
    assert_eq!(purged.len(), 1);
    by_user(&purged[0], "pages.page_purged", workspace, "pages.purge");
    assert_eq!(purged[0].body["page"]["id"], page["id"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_refused_change_tracks_nothing(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let page = kit
        .add(&ana, workspace, json!({"title": "Weekly plan"}))
        .await;

    // Rui cannot see it: the answer is a 404 and nothing is tracked.
    let page_uri = uri(workspace, &page);
    let refused = kit.post(&rui, &format!("{page_uri}/trash"), None).await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND);
    assert!(tracked(&kit, "pages.page_trashed").await.is_empty());
}
