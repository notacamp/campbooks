//! `GET /api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/body`
//! — one message's text and HTML bodies, as JSON.
//!
//! The HTML is untrusted and never served as `text/html`: it is sanitised
//! here (`domain::sanitize`: no scripts, style sheets, forms or frames; links
//! open in a new tab; remote images held back unless asked for; tracking
//! pixels removed), then reaches the client as a string that the client
//! renders only in its sandboxed frame (`lib/ui/safe-html`, AGENTS.md).
//! Inline (`cid:`) images are not remote: they show the message's own parts
//! as `data:` URLs, raster types only and within [`MAX_INLINE_IMAGE_BYTES`]
//! and [`MAX_INLINE_TOTAL_BYTES`]; anything else shows the placeholder.

use axum::{Json, extract::State};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    blobs::BlobRef,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::domain::resources;
use crate::events::MESSAGE_RECORD;
use crate::{
    domain::paths::MessagePath,
    domain::{
        access::visible_inboxes,
        attachments::{is_raster, sniff},
        blobs::{read_bytes, read_text},
        sanitize::{InlineImages, sanitize},
    },
    features::get_conversation::Bodies,
    model::{ConversationId, InboxId, MessageId},
    scopes::ConversationsRead,
};
use campbooks_kernel::RecordRef;
use campbooks_kernel::authz::Action;

/// Longest body returned, in bytes, per part.
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
/// Largest inline image shown, in bytes; a larger one shows the placeholder.
pub const MAX_INLINE_IMAGE_BYTES: u64 = 2 * 1024 * 1024;
/// Most inline-image bytes shown in one body; the rest show the placeholder.
pub const MAX_INLINE_TOTAL_BYTES: u64 = 6 * 1024 * 1024;

/// How to render the HTML part.
#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BodyQuery {
    /// `true` to keep remote images (the member chose "Show images");
    /// tracking pixels are removed either way.
    #[serde(default)]
    pub show_images: bool,
}

/// A message's bodies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessageBody {
    /// The plain-text part.
    pub text: Option<String>,
    /// The HTML part, sanitised; still to be shown only in a sandboxed frame.
    pub html: Option<String>,
    /// Whether either part was cut short.
    pub truncated: bool,
    /// Remote images in the HTML part: held back (their `src` removed)
    /// unless `show_images` was asked for.
    pub remote_images: u32,
    /// Whether the remote images were kept.
    pub images_shown: bool,
}

/// Read one message's text and HTML bodies.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/body",
    operation_id = "conversations.get_message_body",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["conversations.message"]).access("conversations.message", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
        ("message_id" = MessageId, Path, description = "The message."),
        BodyQuery,
    ),
    responses(
        (status = OK, description = "The bodies.", body = MessageBody),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such message, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<MessagePath>,
    extract::Query(query): extract::Query<BodyQuery>,
) -> Result<Json<MessageBody>> {
    let (conversation, message) = (path.conversation_id, path.message_id);
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &RecordRef::new(MESSAGE_RECORD, &message),
    )
    .await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    let bodies = sqlx::query_scalar!(
        r#"SELECT bodies FROM conversations.messages
            WHERE id = $1 AND conversation_id = $2 AND inbox_id = ANY($3)"#,
        message as MessageId,
        conversation as ConversationId,
        &inboxes as &[InboxId],
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    // Parts `cid:` images may name (read only if the HTML names any).
    let parts: Vec<InlinePart> = sqlx::query_as!(
        InlinePart,
        r#"SELECT content_id AS "content_id!", content_type, blob
             FROM conversations.attachments
            WHERE message_id = $1 AND content_id IS NOT NULL
            ORDER BY position"#,
        message as MessageId,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let bodies: Bodies = serde_json::from_value(bodies)
        .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
    let ws = access.workspace.to_string();
    let (text, text_cut) = read(&state, &ws, bodies.text.as_ref()).await?;
    let (html, html_cut) = read(&state, &ws, bodies.html.as_ref()).await?;
    let truncated = text_cut || html_cut;
    let inline = match html.as_deref() {
        Some(html) if html.to_ascii_lowercase().contains("cid:") => {
            inline_images(&state, &ws, parts).await?
        }
        _ => InlineImages::new(),
    };
    let sanitized = html
        .as_deref()
        .map(|html| sanitize(html, query.show_images, &inline));
    Ok(Json(MessageBody {
        text,
        remote_images: sanitized.as_ref().map_or(0, |s| s.remote_images),
        images_shown: query.show_images,
        html: sanitized.map(|s| s.html),
        truncated,
    }))
}

/// A part of the message with a content id.
struct InlinePart {
    content_id: String,
    content_type: String,
    blob: serde_json::Value,
}

/// `parts` of a raster type, as `data:` URLs by content id, within the size
/// limits.
async fn inline_images(state: &AppState, ws: &str, parts: Vec<InlinePart>) -> Result<InlineImages> {
    let mut images = InlineImages::new();
    let mut total = 0_u64;
    for part in parts {
        if !is_raster(&part.content_type) {
            continue;
        }
        let blob: BlobRef = serde_json::from_value(part.blob)
            .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
        if blob.size > MAX_INLINE_IMAGE_BYTES || total + blob.size > MAX_INLINE_TOTAL_BYTES {
            continue;
        }
        let limit = usize::try_from(MAX_INLINE_IMAGE_BYTES).unwrap_or(usize::MAX);
        let (bytes, cut) = read_bytes(&state.blobs, ws, &blob, limit).await?;
        if cut {
            continue;
        }
        // Shown only as what its bytes are, and only if that is a picture.
        let Some(kind) = sniff(&bytes).filter(|kind| is_raster(kind)) else {
            continue;
        };
        total += blob.size;
        let id = part
            .content_id
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>')
            .to_lowercase();
        let url = format!("data:{};base64,{}", kind, STANDARD.encode(bytes));
        images.entry(id).or_insert(url);
    }
    Ok(images)
}

async fn read(
    state: &AppState,
    ws: &str,
    blob: Option<&BlobRef>,
) -> Result<(Option<String>, bool)> {
    match blob {
        Some(blob) => {
            let (text, cut) = read_text(&state.blobs, ws, blob, MAX_BODY_BYTES).await?;
            Ok((Some(text), cut))
        }
        None => Ok((None, false)),
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;
    use sqlx::PgPool;

    use crate::{dev, domain::sanitize::PLACEHOLDER, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn inline_images_show_their_parts_and_the_rest_a_placeholder(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let recorded = json!({"changes": [{"op": "upsert", "message": {
            "provider_message_id": "cid-1",
            "from": {"name": "Ana Lopes", "address": "ana@brightloop.example"},
            "to": [{"address": "sam@example.com"}],
            "subject": "Our new logo",
            "sent_at": "2026-10-06T08:30:00Z",
            "direction": "inbound",
            "text": "Our new logo.",
            "html": "<p><img src=\"cid:logo@brightloop.example\" alt=\"Logo\"><img src=\"cid:badge@brightloop.example\" alt=\"Badge\"></p><p><img src=\"cid:vector@brightloop.example\" alt=\"Vector\"></p><img src=\"https://cdn.brightloop.example/banner.png\" alt=\"Banner\" width=\"560\" height=\"120\">",
            "attachments": [
                {"filename": "logo.gif", "content_type": "image/gif", "content": "GIF89a-bytes",
                 "inline": true, "content_id": "logo@brightloop.example"},
                {"filename": "badge.png", "content_type": "image/png",
                 "content": "<html><script>alert(1)</script></html>",
                 "inline": true, "content_id": "badge@brightloop.example"},
                {"filename": "vector.svg", "content_type": "image/svg+xml",
                 "content": "<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>",
                 "inline": true, "content_id": "vector@brightloop.example"}
            ],
            "mailbox": {"in_inbox": true},
            "read": false, "starred": false
        }}]});
        let batch = dev::record(&kit.state, workspace, inbox, &recorded)
            .await
            .unwrap();
        dev::submit(&kit.state, workspace, &batch).await.unwrap();
        kit.run_jobs().await;
        let (conversation, message): (String, String) = sqlx::query_as(
            "SELECT conversation_id::text, id::text FROM conversations.messages
              WHERE provider_message_id = 'cid-1'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        let stored: Option<String> = sqlx::query_scalar(
            "SELECT content_id FROM conversations.attachments WHERE filename = 'logo.gif'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(stored.as_deref(), Some("logo@brightloop.example"));

        let uri =
            format!("/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/body");
        let body = kit.get(&sam, &uri).await;
        assert_eq!(body.status, StatusCode::OK);
        let html = body.json["html"].as_str().unwrap();
        // The GIF part, without asking; never counted as remote.
        assert!(
            html.contains("src=\"data:image/gif;base64,R0lGODlhLWJ5dGVz\""),
            "{html}"
        );
        assert_eq!(body.json["remote_images"], 1);
        // The SVG part is not a raster image, the "PNG" is markup, and the
        // banner is held back: all show the placeholder; nothing of them
        // leaves.
        assert_eq!(html.matches(PLACEHOLDER).count(), 3, "{html}");
        assert!(!html.contains("script"), "{html}");
        assert!(!html.contains("cdn.brightloop.example"), "{html}");
        assert!(!html.contains("cid:"), "{html}");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn html_is_returned_as_json_data_never_as_a_page(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let (conversation, message): (String, String) = sqlx::query_as(
            "SELECT conversation_id::text, id::text FROM conversations.messages
              WHERE provider_message_id = 'en-3'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        let uri =
            format!("/api/v1/w/{workspace}/conversations/{conversation}/messages/{message}/body");
        let response = kit.get(&sam, &uri).await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.content_type.as_deref(), Some("application/json"));
        assert!(
            response.json["html"]
                .as_str()
                .unwrap()
                .starts_with("<h1>Issue 142")
        );
        assert!(
            response.json["text"]
                .as_str()
                .unwrap()
                .contains("calmer mornings")
        );
        assert_eq!(response.json["truncated"], false);
        // Sanitised: the remote image is held back (the placeholder shows
        // in its place) and counted.
        assert!(
            !response.json["html"]
                .as_str()
                .unwrap()
                .contains("paperkite.example")
        );
        assert!(
            response.json["html"]
                .as_str()
                .unwrap()
                .contains(PLACEHOLDER)
        );
        assert_eq!(response.json["remote_images"], 1);
        assert_eq!(response.json["images_shown"], false);
        let shown = kit.get(&sam, &format!("{uri}?show_images=true")).await;
        assert_eq!(shown.status, StatusCode::OK);
        assert!(
            shown.json["html"]
                .as_str()
                .unwrap()
                .contains(r#"src="https://paperkite.example/pixel.gif""#)
        );
        assert_eq!(shown.json["images_shown"], true);

        let other = kit.conversation_of("en-1").await;
        let mismatched =
            format!("/api/v1/w/{workspace}/conversations/{other}/messages/{message}/body");
        assert_eq!(
            kit.get(&sam, &mismatched).await.status,
            StatusCode::NOT_FOUND
        );
    }
}
