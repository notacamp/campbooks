use axum::http::{HeaderMap, StatusCode, header};
use serde_json::json;
use sqlx::PgPool;

use super::{ATTACHMENT_POLICY, CROSS_ORIGIN_RESOURCE_POLICY};
use crate::{model::InboxRole, testkit::Kit};

/// The URI of attachment `attachment` of message `provider_message_id`.
async fn uri_of(kit: &Kit, workspace: &str, provider_message_id: &str, attachment: &str) -> String {
    let (conversation, message): (String, String) = sqlx::query_as(
        "SELECT conversation_id::text, id::text FROM conversations.messages
          WHERE provider_message_id = $1",
    )
    .bind(provider_message_id)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    format!(
        "/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/attachments/{attachment}"
    )
}

/// The id of the first attachment of `provider_message_id`.
async fn first_attachment(kit: &Kit, provider_message_id: &str) -> String {
    sqlx::query_scalar(
        "SELECT a.id::text FROM conversations.attachments a
           JOIN conversations.messages m ON m.id = a.message_id
          WHERE m.provider_message_id = $1 ORDER BY a.position LIMIT 1",
    )
    .bind(provider_message_id)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

/// Store `bytes` in the workspace and add them to `provider_message_id` as
/// an attachment named `filename` of type `content_type`; returns its id.
async fn add_attachment(
    kit: &Kit,
    workspace: campbooks_kernel::tenancy::WorkspaceId,
    provider_message_id: &str,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) -> String {
    let blob = kit
        .state
        .blobs
        .put(&workspace.to_string(), bytes.to_vec(), content_type)
        .await
        .unwrap();
    sqlx::query_scalar(
        "INSERT INTO conversations.attachments
             (id, workspace_id, message_id, position, filename, content_type, size, inline, blob)
         SELECT gen_random_uuid(), m.workspace_id, m.id, 9, $2, $3, $4, false, $5
           FROM conversations.messages m WHERE m.provider_message_id = $1
         RETURNING id::text",
    )
    .bind(provider_message_id)
    .bind(filename)
    .bind(content_type)
    .bind(i64::try_from(bytes.len()).unwrap())
    .bind(json!(blob))
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

fn header<'a>(headers: &'a HeaderMap, name: &header::HeaderName) -> &'a str {
    headers
        .get(name)
        .unwrap_or_else(|| panic!("no {name}"))
        .to_str()
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_downloads_the_stored_bytes_as_an_attachment(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let attachment = first_attachment(&kit, "en-1").await;
    let uri = uri_of(&kit, &workspace.to_string(), "en-1", &attachment).await;

    let (status, headers, body) = kit.get_raw(&sam, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"%PDF-1.7 fictional Q4 plan");
    assert_eq!(header(&headers, &header::CONTENT_TYPE), "application/pdf");
    assert_eq!(
        header(&headers, &header::CONTENT_DISPOSITION),
        "attachment; filename=\"q4-plan.pdf\"; filename*=UTF-8''q4-plan.pdf"
    );
    assert_eq!(header(&headers, &header::X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert_eq!(
        header(&headers, &header::CONTENT_SECURITY_POLICY),
        ATTACHMENT_POLICY
    );
    assert!(ATTACHMENT_POLICY.starts_with("sandbox;"));
    assert_eq!(
        header(&headers, &CROSS_ORIGIN_RESOURCE_POLICY),
        "same-origin"
    );
    assert_eq!(
        header(&headers, &header::CACHE_CONTROL),
        "private, no-store"
    );
    assert_eq!(header(&headers, &header::ACCEPT_RANGES), "none");
    assert_eq!(header(&headers, &header::CONTENT_LENGTH), "26");

    // A PDF may be shown inline when asked.
    let (status, headers, _) = kit.get_raw(&sam, &format!("{uri}?inline=true")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(header(&headers, &header::CONTENT_DISPOSITION).starts_with("inline; "));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn anything_the_caller_cannot_see_is_404(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let attachment = first_attachment(&kit, "en-1").await;
    let uri = uri_of(&kit, &workspace.to_string(), "en-1", &attachment).await;

    // Ana is a member of the workspace, not of Sam's inbox.
    let (ana, _) = kit.member("ana", workspace);
    let (status, _, body) = kit.get_raw(&ana, &uri).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!body.windows(4).any(|w| w == b"%PDF"));

    // Another inbox of the same workspace, with its own mail.
    let (lia_token, lia) = kit.member("lia", workspace);
    kit.inbox(workspace, "lia@example.com", &[(lia, InboxRole::Manager)])
        .await;
    assert_eq!(kit.get_raw(&lia_token, &uri).await.0, StatusCode::NOT_FOUND);

    // The right attachment under another message or conversation.
    let other_message = uri_of(&kit, &workspace.to_string(), "en-3", &attachment).await;
    assert_eq!(
        kit.get_raw(&sam, &other_message).await.0,
        StatusCode::NOT_FOUND
    );
    // An id that does not exist.
    let missing = uri.replace(&attachment, "00000000-0000-4000-8000-000000000000");
    assert_eq!(kit.get_raw(&sam, &missing).await.0, StatusCode::NOT_FOUND);
    // Another workspace's token.
    let elsewhere = campbooks_kernel::tenancy::WorkspaceId::new();
    let (eve, _) = kit.member("eve", elsewhere);
    let foreign = uri.replace(&workspace.to_string(), &elsewhere.to_string());
    assert_eq!(kit.get_raw(&eve, &foreign).await.0, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn html_and_svg_are_never_served_as_themselves(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let ws = workspace.to_string();
    for (name, content_type) in [
        ("invoice.html", "text/html"),
        ("page.xhtml", "application/xhtml+xml; charset=utf-8"),
        ("logo.svg", "image/svg+xml"),
        ("run.js", "text/javascript"),
    ] {
        let id = add_attachment(
            &kit,
            workspace,
            "en-1",
            name,
            content_type,
            b"<script>alert(document.domain)</script>",
        )
        .await;
        let uri = uri_of(&kit, &ws, "en-1", &id).await;
        for query in ["", "?inline=true"] {
            let (status, headers, _) = kit.get_raw(&sam, &format!("{uri}{query}")).await;
            assert_eq!(status, StatusCode::OK, "{name}");
            assert_eq!(
                header(&headers, &header::CONTENT_TYPE),
                "application/octet-stream",
                "{name}{query}"
            );
            assert!(
                header(&headers, &header::CONTENT_DISPOSITION).starts_with("attachment; "),
                "{name}{query}"
            );
            assert_eq!(header(&headers, &header::X_CONTENT_TYPE_OPTIONS), "nosniff");
        }
    }
    // Safe types keep theirs, and only raster images and PDFs go inline.
    let photo = add_attachment(
        &kit,
        workspace,
        "en-1",
        "photo.jpg",
        "image/jpeg",
        b"\xff\xd8\xff\xe0",
    )
    .await;
    let uri = uri_of(&kit, &ws, "en-1", &photo).await;
    let (_, headers, _) = kit.get_raw(&sam, &format!("{uri}?inline=true")).await;
    assert_eq!(header(&headers, &header::CONTENT_TYPE), "image/jpeg");
    assert!(header(&headers, &header::CONTENT_DISPOSITION).starts_with("inline; "));
    // Sent as a picture, but markup: an opaque download, never inline.
    let fake = add_attachment(
        &kit,
        workspace,
        "en-1",
        "photo.png",
        "image/png",
        b"<html><script>alert(1)</script></html>",
    )
    .await;
    let uri = uri_of(&kit, &ws, "en-1", &fake).await;
    let (_, headers, _) = kit.get_raw(&sam, &format!("{uri}?inline=true")).await;
    assert_eq!(
        header(&headers, &header::CONTENT_TYPE),
        "application/octet-stream"
    );
    assert!(header(&headers, &header::CONTENT_DISPOSITION).starts_with("attachment; "));
    let notes = add_attachment(&kit, workspace, "en-1", "notes.txt", "text/plain", b"hi").await;
    let uri = uri_of(&kit, &ws, "en-1", &notes).await;
    let (_, headers, _) = kit.get_raw(&sam, &format!("{uri}?inline=true")).await;
    assert_eq!(header(&headers, &header::CONTENT_TYPE), "text/plain");
    assert!(header(&headers, &header::CONTENT_DISPOSITION).starts_with("attachment; "));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_malicious_file_name_becomes_one_safe_header(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let id = add_attachment(
        &kit,
        workspace,
        "en-1",
        "../../etc/pass\r\nSet-Cookie: x=1\u{202E}gpj.exe",
        "application/pdf",
        b"%PDF-1.4",
    )
    .await;
    let uri = uri_of(&kit, &workspace.to_string(), "en-1", &id).await;
    let (status, headers, _) = kit.get_raw(&sam, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(header::SET_COOKIE).is_none());
    assert_eq!(
        header(&headers, &header::CONTENT_DISPOSITION),
        "attachment; filename=\"etcpassSet-Cookie: x=1gpj.exe\"; filename*=UTF-8''etcpassSet-Cookie%3A%20x%3D1gpj.exe"
    );
}
