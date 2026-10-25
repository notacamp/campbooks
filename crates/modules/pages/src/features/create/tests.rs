//! Making a page.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::{Kit, docs};

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_page_is_private_and_its_creators_and_its_event_has_no_title(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let made = kit
        .post(
            &ana,
            &Kit::pages(workspace),
            Some(json!({"title": "  Weekly\tplan ", "icon": "star"})),
        )
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{:?}", made.json);
    let page = &made.json;
    assert_eq!(page["title"], "Weekly plan");
    assert_eq!(page["icon"], "star");
    assert_eq!(page["place"], "private");
    assert_eq!(page["parent"], serde_json::Value::Null);
    assert_eq!(page["role"], "manager");
    assert_eq!(page["created_by"], ana_id.to_string());
    assert_eq!(page["revision"], 1);
    assert_eq!(page["trashed_at"], serde_json::Value::Null);
    assert_eq!(page["grants"], json!([]));

    let events = kit.events("pages.page_changed").await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["kinds"], json!(["meta"]));
    assert_eq!(events[0]["place"], "private");
    assert!(!events[0].to_string().contains("Weekly"));
    assert!(events[0].get("title").is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_bad_title_icon_or_position_is_refused_and_nothing_is_stored(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let uri = Kit::pages(workspace);
    let long = kit
        .post(&ana, &uri, Some(json!({"title": "x".repeat(201)})))
        .await;
    assert_eq!(long.status, StatusCode::BAD_REQUEST);
    assert_eq!(long.json["code"], "invalid_title");
    let icon = kit.post(&ana, &uri, Some(json!({"icon": "rocket"}))).await;
    assert_eq!(icon.json["code"], "invalid_icon");
    let position = kit
        .post(&ana, &uri, Some(json!({"position": "not a key"})))
        .await;
    assert_eq!(position.json["code"], "invalid_position");
    assert_eq!(kit.page_count(workspace).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_sub_page_goes_in_its_parents_place_after_the_last_one(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let top = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Handbook", "place": "workspace"}),
        )
        .await;
    let id = top["id"].as_str().unwrap();
    // Any member may write in a workspace page.
    let first = kit
        .add(&rui, workspace, json!({"title": "Holidays", "parent": id}))
        .await;
    let second = kit
        .add(&ana, workspace, json!({"title": "Expenses", "parent": id}))
        .await;
    assert_eq!(first["place"], "workspace");
    assert_eq!(first["parent"], id);
    assert!(first["position"].as_str().unwrap() < second["position"].as_str().unwrap());
    // A place that is not the parent's is refused.
    let wrong = kit
        .post(
            &ana,
            &Kit::pages(workspace),
            Some(json!({"parent": id, "place": "private"})),
        )
        .await;
    assert_eq!(wrong.json["code"], "place_mismatch");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn nobody_makes_a_page_under_one_they_cannot_see_or_cannot_write(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let mine = kit.add(&ana, workspace, json!({"title": "Mine"})).await;
    let id = mine["id"].as_str().unwrap();
    let uri = Kit::pages(workspace);
    // Not shared with Rui: it is not there.
    let unseen = kit.post(&rui, &uri, Some(json!({"parent": id}))).await;
    assert_eq!(unseen.status, StatusCode::NOT_FOUND);
    // Shared as a reader: he sees it, and may not write in it.
    let shared = kit
        .put(
            &ana,
            &format!("{uri}/{id}/grants/{rui_id}"),
            json!({"role": "reader"}),
        )
        .await;
    assert_eq!(shared.status, StatusCode::OK, "{:?}", shared.json);
    let denied = kit.post(&rui, &uri, Some(json!({"parent": id}))).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_eq!(kit.page_count(workspace).await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn pages_nest_twelve_levels_deep_and_no_more(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let mut parent: Option<String> = None;
    for level in 1..=12 {
        let body = match &parent {
            Some(parent) => json!({"title": format!("Level {level}"), "parent": parent}),
            None => json!({"title": "Level 1"}),
        };
        let made = kit.add(&ana, workspace, body).await;
        parent = Some(made["id"].as_str().unwrap().to_owned());
    }
    let too_deep = kit
        .post(
            &ana,
            &Kit::pages(workspace),
            Some(json!({"parent": parent.unwrap()})),
        )
        .await;
    assert_eq!(too_deep.status, StatusCode::CONFLICT);
    assert_eq!(too_deep.json["code"], "page_too_deep");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_body_made_with_the_page_is_checked_like_any_update(db: PgPool) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let uri = Kit::pages(workspace);
    let client = docs::client(1);
    let good = docs::paragraph(&client, "Welcome");
    let made = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Onboarding", "document": STANDARD.encode(&good)}),
        )
        .await;
    assert_eq!(made["revision"], 2);
    let forged = docs::element(&docs::client(2), "iframe", &[], None);
    let refused = kit
        .post(
            &ana,
            &uri,
            Some(json!({"title": "Forged", "document": STANDARD.encode(forged)})),
        )
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json["code"], "unknown_block");
    // Only the first was stored.
    assert_eq!(kit.page_count(workspace).await, 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn writing_needs_the_write_scope_a_member_and_the_flag(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, _) = kit.member("ana", workspace);
    let uri = Kit::pages(workspace);
    let body = Some(json!({"title": "x"}));
    assert_eq!(
        kit.post("", &uri, body.clone()).await.status,
        StatusCode::UNAUTHORIZED
    );
    let reader = kit.token_with("ana", "pages:read");
    let denied = kit.post(&reader, &uri, body.clone()).await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_eq!(
        denied.content_type.as_deref(),
        Some("application/problem+json")
    );
    // A client acting for nobody has no pages of its own.
    let client = kit.client(workspace, "pages:read pages:write");
    assert_eq!(
        kit.post(&client, &uri, body.clone()).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(kit.page_count(workspace).await, 0);

    // Off, every endpoint is a 404, for everyone, and nothing was stored.
    let kit = kit.switched_off();
    let (ana, _) = kit.member("ana", workspace);
    assert_eq!(
        kit.post(&ana, &uri, body).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(kit.get(&ana, &uri).await.status, StatusCode::NOT_FOUND);
    assert_eq!(kit.page_count(workspace).await, 0);
}
