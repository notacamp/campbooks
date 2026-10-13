//! `GET /api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/body`
//! — one message's text and HTML bodies, as JSON.
//!
//! The HTML is untrusted and never served as `text/html`: it is sanitised
//! here (`domain::sanitize`: no scripts, style sheets, forms or frames; links
//! open in a new tab; remote images held back unless asked for; tracking
//! pixels removed), then reaches the client as a string that the client
//! renders only in its sandboxed frame (`lib/ui/safe-html`, AGENTS.md).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    blobs::BlobRef,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::paths::MessagePath,
    domain::{access::visible_inboxes, blobs::read_text, sanitize::sanitize},
    features::get_conversation::Bodies,
    model::{ConversationId, InboxId, MessageId},
    scopes::ConversationsRead,
};

/// Longest body returned, in bytes, per part.
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

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
    extensions(("x-campbooks" = json!(Operation::query().records(["conversations.message"])))),
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
    tx.commit().await?;
    let bodies: Bodies = serde_json::from_value(bodies)
        .map_err(|e| Error::internal(crate::domain::redact::json(&e)))?;
    let ws = access.workspace.to_string();
    let (text, text_cut) = read(&state, &ws, bodies.text.as_ref()).await?;
    let (html, html_cut) = read(&state, &ws, bodies.html.as_ref()).await?;
    let truncated = text_cut || html_cut;
    let sanitized = html
        .as_deref()
        .map(|html| sanitize(html, query.show_images));
    Ok(Json(MessageBody {
        text,
        remote_images: sanitized.as_ref().map_or(0, |s| s.remote_images),
        images_shown: query.show_images,
        html: sanitized.map(|s| s.html),
        truncated,
    }))
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
    use sqlx::PgPool;

    use crate::testkit::Kit;

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
        // Sanitised: the remote image is held back and counted.
        assert!(
            !response.json["html"]
                .as_str()
                .unwrap()
                .contains("paperkite.example")
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
