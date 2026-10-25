//! Adding files and images to a page.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_file_is_stored_held_by_the_page_and_named_by_its_hash(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Files"})).await;
    let uri = format!(
        "{}/{}/files",
        Kit::pages(workspace),
        page["id"].as_str().unwrap()
    );
    let bytes = b"%PDF-1.7 an offer".to_vec();
    let up = kit
        .upload(&ana, &uri, "Application/PDF", bytes.clone())
        .await;
    assert_eq!(up.status, StatusCode::CREATED, "{:?}", up.json);
    assert_eq!(up.json["size"], bytes.len());
    assert_eq!(up.json["content_type"], "application/pdf");
    let blob = up.json["blob"].as_str().unwrap();
    assert_eq!(blob.len(), 64);
    // The page holds the blob, so the sweeper keeps it.
    let held: i64 =
        sqlx::query_scalar("SELECT count(*) FROM kernel.blob_refs WHERE owner_type = 'pages.page'")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(held, 1);
    // The same bytes again are the same blob.
    let again = kit.upload(&ana, &uri, "application/pdf", bytes).await;
    assert_eq!(again.json["blob"], blob);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_empty_file_a_reader_a_stranger_and_a_trashed_page_are_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, rui_id) = kit.member("rui", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Files"})).await;
    let page_uri = format!("{}/{}", Kit::pages(workspace), page["id"].as_str().unwrap());
    let uri = format!("{page_uri}/files");

    let empty = kit.upload(&ana, &uri, "text/plain", Vec::new()).await;
    assert_eq!(empty.status, StatusCode::BAD_REQUEST);
    assert_eq!(empty.json["code"], "file_empty");
    assert_eq!(
        kit.upload(&rui, &uri, "text/plain", b"x".to_vec())
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    kit.put(
        &ana,
        &format!("{page_uri}/grants/{rui_id}"),
        json!({"role": "reader"}),
    )
    .await;
    assert_eq!(
        kit.upload(&rui, &uri, "text/plain", b"x".to_vec())
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    kit.post(&ana, &format!("{page_uri}/trash"), None).await;
    assert_eq!(
        kit.upload(&ana, &uri, "text/plain", b"x".to_vec())
            .await
            .status,
        StatusCode::CONFLICT
    );
    let held: i64 = sqlx::query_scalar("SELECT count(*) FROM pages.files")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(held, 0);
}
