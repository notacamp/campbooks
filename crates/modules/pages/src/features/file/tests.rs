//! Downloading files and images of a page.

use axum::http::{StatusCode, header};
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::Kit;

#[sqlx::test(migrations = "../../../migrations")]
async fn a_raster_image_is_shown_inline_and_anything_else_is_opaque(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let page = kit.add(&ana, workspace, json!({"title": "Files"})).await;
    let base = format!(
        "{}/{}/files",
        Kit::pages(workspace),
        page["id"].as_str().unwrap()
    );

    let png = b"\x89PNG\r\n\x1a\n some pixels".to_vec();
    let up = kit.upload(&ana, &base, "image/png", png.clone()).await;
    let (status, headers, bytes) = kit
        .download(
            &ana,
            &format!("{base}/{}", up.json["blob"].as_str().unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(headers[header::CONTENT_DISPOSITION], "inline");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(
        headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .starts_with("sandbox")
    );
    assert_eq!(bytes, png);

    // HTML sent as an image is opaque bytes, whatever it says it is.
    let html = b"<script>alert(1)</script>".to_vec();
    let up = kit.upload(&ana, &base, "image/png", html.clone()).await;
    let (status, headers, bytes) = kit
        .download(
            &ana,
            &format!("{base}/{}", up.json["blob"].as_str().unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/octet-stream");
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment")
    );
    assert_eq!(bytes, html);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_file_is_only_served_with_its_own_page_to_someone_who_may_read_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, _) = kit.member("ana", workspace);
    let (rui, _) = kit.member("rui", workspace);
    let mine = kit.add(&ana, workspace, json!({"title": "Mine"})).await;
    let theirs = kit
        .add(
            &ana,
            workspace,
            json!({"title": "Shared", "place": "workspace"}),
        )
        .await;
    let uri = Kit::pages(workspace);
    let mine_id = mine["id"].as_str().unwrap();
    let theirs_id = theirs["id"].as_str().unwrap();
    let up = kit
        .upload(
            &ana,
            &format!("{uri}/{mine_id}/files"),
            "text/plain",
            b"secret".to_vec(),
        )
        .await;
    let blob = up.json["blob"].as_str().unwrap();

    // Rui cannot open the page the file is on.
    let (status, _, _) = kit
        .download(&rui, &format!("{uri}/{mine_id}/files/{blob}"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // The hash on another page he can open is not a file of that page.
    let (status, _, _) = kit
        .download(&rui, &format!("{uri}/{theirs_id}/files/{blob}"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // And a hash that is not one.
    let (status, _, _) = kit
        .download(&ana, &format!("{uri}/{mine_id}/files/../../etc"))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
