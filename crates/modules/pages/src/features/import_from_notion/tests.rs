//! Importing pages from Notion: only through the caller's own connection,
//! each page private and linked, and nothing of a page's words in an event.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{testkit::Kit, testkit_notion::Rig};

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_imports_pages_that_are_private_and_linked(db: PgPool) {
    let rig = Rig::new(db).await;
    let (roadmap, notes) = (Uuid::now_v7(), Uuid::now_v7());
    let made = rig
        .import_response(&[(roadmap, "Product roadmap"), (notes, "Notes")])
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{:?}", made.json);
    assert_eq!(made.json["skipped"], 0);
    let pages = made.json["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0]["notion_page"], roadmap.to_string());
    let page = pages[0]["page"].as_str().unwrap();
    // A private page of hers, titled as asked, linked and syncing.
    let shown = rig
        .kit
        .get(&rig.token, &format!("{}/{page}", Kit::pages(rig.ws)))
        .await;
    assert_eq!(shown.status, StatusCode::OK);
    assert_eq!(shown.json["title"], "Product roadmap");
    assert_eq!(shown.json["place"], "private");
    let link = rig.link(page).await;
    assert_eq!(link["state"], "syncing");
    assert_eq!(link["connection_id"], rig.connection.to_string());
    assert_eq!(link["notion_page_id"], roadmap.to_string());
    // The connectors module is asked to read it, with ids only.
    let requested = rig.kit.events("pages.notion_link_requested").await;
    assert_eq!(requested.len(), 2);
    assert_eq!(requested[0]["page"], page);
    assert_eq!(requested[0]["notion_page"], roadmap.to_string());
    assert!(!requested.iter().any(|e| e.to_string().contains("Product roadmap")));
    // Another member does not see her page.
    let (rui, _) = rig.kit.member("rui", rig.ws);
    let hidden = rig
        .kit
        .get(&rui, &format!("{}/{page}", Kit::pages(rig.ws)))
        .await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_page_linked_already_or_named_twice_is_left_alone(db: PgPool) {
    let rig = Rig::new(db).await;
    let roadmap = Uuid::now_v7();
    rig.import(roadmap, "Product roadmap").await;
    let again = rig
        .import_response(&[(roadmap, "Product roadmap"), (roadmap, "Twice")])
        .await;
    assert_eq!(again.status, StatusCode::CREATED);
    assert_eq!(again.json["pages"], json!([]));
    assert_eq!(again.json["skipped"], 2);
    assert_eq!(rig.kit.page_count(rig.ws).await, 1);
    let fresh = Uuid::now_v7();
    let both = rig
        .import_response(&[(fresh, "New"), (fresh, "New again")])
        .await;
    assert_eq!(both.json["pages"].as_array().unwrap().len(), 1);
    assert_eq!(both.json["skipped"], 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_connection_that_is_not_hers_is_a_connection_that_does_not_exist(db: PgPool) {
    let rig = Rig::new(db).await;
    let (_, rui_id) = rig.kit.member("rui", rig.ws);
    let ruis = rig.kit.connection_of(rui_id);
    let theirs = rig
        .kit
        .post(
            &rig.token,
            &format!("{}/notion/imports", Kit::pages(rig.ws)),
            Some(json!({"connection": ruis, "pages": [{"notion_page": Uuid::now_v7()}]})),
        )
        .await;
    assert_eq!(theirs.status, StatusCode::NOT_FOUND, "{:?}", theirs.json);
    let nowhere = rig
        .kit
        .post(
            &rig.token,
            &format!("{}/notion/imports", Kit::pages(rig.ws)),
            Some(json!({"connection": Uuid::now_v7(), "pages": [{"notion_page": Uuid::now_v7()}]})),
        )
        .await;
    assert_eq!(nowhere.status, StatusCode::NOT_FOUND);
    assert_eq!(rig.kit.page_count(rig.ws).await, 0);
    assert!(rig.kit.events("pages.notion_link_requested").await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_connection_of_another_workspace_is_not_usable_here(db: PgPool) {
    let rig = Rig::new(db).await;
    let elsewhere = WorkspaceId::new();
    let (token, _) = rig.kit.member("marta-elsewhere", elsewhere);
    // Marta's own connection, from a workspace she shares with nobody: the
    // other workspace's member is not its owner.
    let crossed = rig
        .kit
        .post(
            &token,
            &format!("{}/notion/imports", Kit::pages(elsewhere)),
            Some(json!({"connection": rig.connection, "pages": [{"notion_page": Uuid::now_v7()}]})),
        )
        .await;
    assert_eq!(crossed.status, StatusCode::NOT_FOUND);
    assert_eq!(rig.kit.page_count(elsewhere).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_request_is_checked_before_anything_is_made(db: PgPool) {
    let rig = Rig::new(db).await;
    let uri = format!("{}/notion/imports", Kit::pages(rig.ws));
    let none = rig
        .kit
        .post(&rig.token, &uri, Some(json!({"connection": rig.connection, "pages": []})))
        .await;
    assert_eq!(none.status, StatusCode::BAD_REQUEST);
    assert_eq!(none.json["code"], "invalid_import");
    let many: Vec<_> = (0..101)
        .map(|_| json!({"notion_page": Uuid::now_v7()}))
        .collect();
    let too_many = rig
        .kit
        .post(&rig.token, &uri, Some(json!({"connection": rig.connection, "pages": many})))
        .await;
    assert_eq!(too_many.json["code"], "invalid_import");
    let long = rig
        .kit
        .post(
            &rig.token,
            &uri,
            Some(json!({"connection": rig.connection,
                "pages": [{"notion_page": Uuid::now_v7(), "title": "t".repeat(201)}]})),
        )
        .await;
    assert_eq!(long.status, StatusCode::BAD_REQUEST);
    let unknown = rig
        .kit
        .post(
            &rig.token,
            &uri,
            Some(json!({"connection": rig.connection, "pages": [], "extra": 1})),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
    assert_eq!(rig.kit.page_count(rig.ws).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_client_token_and_a_read_only_token_cannot_import(db: PgPool) {
    let rig = Rig::new(db).await;
    let uri = format!("{}/notion/imports", Kit::pages(rig.ws));
    let body = json!({"connection": rig.connection, "pages": [{"notion_page": Uuid::now_v7()}]});
    let client = rig.kit.client(rig.ws, "pages:write");
    assert_eq!(
        rig.kit.post(&client, &uri, Some(body.clone())).await.status,
        StatusCode::FORBIDDEN
    );
    let read_only = rig.kit.token_with("marta", "pages:read");
    assert_eq!(
        rig.kit.post(&read_only, &uri, Some(body)).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(rig.kit.page_count(rig.ws).await, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn pages_go_under_a_parent_the_member_may_write(db: PgPool) {
    let rig = Rig::new(db).await;
    let parent = rig
        .kit
        .add(&rig.token, rig.ws, json!({"title": "Wiki", "place": "private"}))
        .await;
    let parent_id = parent["id"].as_str().unwrap();
    let uri = format!("{}/notion/imports", Kit::pages(rig.ws));
    let made = rig
        .kit
        .post(
            &rig.token,
            &uri,
            Some(json!({"connection": rig.connection, "parent": parent_id,
                "pages": [{"notion_page": Uuid::now_v7(), "title": "Child"}]})),
        )
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{:?}", made.json);
    let child = made.json["pages"][0]["page"].as_str().unwrap();
    let shown = rig
        .kit
        .get(&rig.token, &format!("{}/{child}", Kit::pages(rig.ws)))
        .await;
    assert_eq!(shown.json["parent"], parent_id);
    // A parent nobody can see.
    let hidden = rig
        .kit
        .post(
            &rig.token,
            &uri,
            Some(json!({"connection": rig.connection, "parent": Uuid::now_v7(),
                "pages": [{"notion_page": Uuid::now_v7()}]})),
        )
        .await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
}
