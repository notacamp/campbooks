//! `GET` and `PUT /api/v1/w/{workspace_id}/inboxes/{inbox_id}/signature` — a
//! member's own signature for an inbox they send from (campbooks-cloud#393).
//!
//! The signature is text with the marks a message may carry (bold, italic,
//! links), at most 2000 characters. It is its member's alone, per inbox:
//! every statement is keyed by the caller's own id, so in a shared inbox
//! each member signs with their own name and nobody reads or changes
//! another's. A new draft copies it ([`own`]); editing it never changes a
//! draft already started.
//!
//! Neither operation is the assistant's: a signature goes out under a
//! member's name, so only the member writes it. A client token acting for
//! nobody has none (403).

#[cfg(test)]
mod tests;

use axum::Json;
use axum::extract::State;
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::{
    domain::{paths::InboxPath, person::kept_char, resources},
    model::InboxId,
    scopes::{ConversationsRead, ConversationsSend},
};

/// Longest signature kept, in characters.
pub const MAX_SIGNATURE_CHARS: usize = 2000;

/// A member's signature for an inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Signature {
    /// Text with bold (`**x**`), italic (`*x*`) and links (`[words](url)`);
    /// empty when there is none.
    #[schema(example = "**Sam Rivers**\nRivers Lettings")]
    pub text: String,
}

/// The author's signature for `inbox` (empty when they have none).
pub async fn own(conn: &mut PgConnection, inbox: InboxId, user: Option<UserId>) -> Result<String> {
    let Some(user) = user else {
        return Ok(String::new());
    };
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT text FROM conversations.signatures WHERE inbox_id = $1 AND user_id = $2",
    )
    .bind(inbox)
    .bind(user.as_uuid())
    .fetch_optional(conn)
    .await?
    .unwrap_or_default())
}

/// The caller, when they may keep a signature in `inbox`: a member (a client
/// token acting for nobody has none) who may send from it. 404 for someone
/// outside the inbox, 403 `inbox_read_only` for a reader.
async fn member_of(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    inbox: InboxId,
) -> Result<UserId> {
    // The inbox first: a client token sees none (404), as everywhere.
    resources::authorize(state, conn, access, Action::Write, &resources::inbox(inbox)).await?;
    access
        .user
        .ok_or_else(|| Error::Denied(crate::MESSAGES.reason("signature_needs_member")))
}

/// Your signature for an inbox.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/inboxes/{inbox_id}/signature",
    operation_id = "conversations.get_signature",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query()
        .records(["conversations.inbox"])
        .access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("inbox_id" = InboxId, Path, description = "The inbox."),
    ),
    responses(
        (status = OK, description = "Your signature for this inbox; empty when you have none.",
            body = Signature),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, not a member (`signature_needs_member`), or you only read the inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such inbox, or you are not in it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<InboxPath>,
) -> Result<Json<Signature>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let user = member_of(&state, &mut tx, access, path.inbox_id).await?;
    let text = own(&mut tx, path.inbox_id, Some(user)).await?;
    tx.commit().await?;
    Ok(Json(Signature { text }))
}

/// Set your signature for an inbox.
#[utoipa::path(
    put,
    path = "/api/v1/w/{workspace_id}/inboxes/{inbox_id}/signature",
    operation_id = "conversations.set_signature",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    // Not the assistant's: a signature goes out under the member's name.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["conversations.inbox"])
        .access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("inbox_id" = InboxId, Path, description = "The inbox."),
    ),
    request_body = Signature,
    responses(
        (status = OK, description = "Your signature as kept.", body = Signature),
        (status = BAD_REQUEST, description = "Over 2000 characters (`signature_too_long`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, not a member (`signature_needs_member`), or you only read the inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such inbox, or you are not in it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn set(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<InboxPath>,
    extract::Json(body): extract::Json<Signature>,
) -> Result<Json<Signature>> {
    let access = &workspace.access;
    // Not the assistant's or automation's, even if the catalogue listed it
    // by mistake.
    if workspace.invoked_by.is_some() {
        return Err(Error::Forbidden);
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let user = member_of(&state, &mut tx, access, path.inbox_id).await?;
    let text = kept(&body.text);
    if text.chars().count() > MAX_SIGNATURE_CHARS {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("signature_too_long")
                .arg("max", MAX_SIGNATURE_CHARS),
        ));
    }
    if text.is_empty() {
        sqlx::query("DELETE FROM conversations.signatures WHERE inbox_id = $1 AND user_id = $2")
            .bind(path.inbox_id)
            .bind(user.as_uuid())
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query(
            "INSERT INTO conversations.signatures (workspace_id, inbox_id, user_id, text)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (inbox_id, user_id) DO UPDATE SET text = EXCLUDED.text, updated_at = now()",
        )
        .bind(access.workspace.as_uuid())
        .bind(path.inbox_id)
        .bind(user.as_uuid())
        .bind(&text)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    // The text is the member's own words: only that it was kept is logged.
    tracing::info!(cleared = text.is_empty(), "a signature kept");
    Ok(Json(Signature { text }))
}

/// The text as kept: line breaks normalised, no control characters but line
/// breaks, no invisible direction or zero-width marks, trimmed.
fn kept(text: &str) -> String {
    let text: String = text
        .replace("\r\n", "\n")
        .chars()
        .filter(|c| *c == '\n' || (*c != '\t' && kept_char(*c)))
        .collect();
    text.trim().to_owned()
}
