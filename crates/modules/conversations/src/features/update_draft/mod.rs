//! `PATCH /api/v1/w/{workspace_id}/drafts/{draft_id}` — save a draft as the
//! member writes (only the fields given change; the `version` read must
//! still be current), and `DELETE` — discard it.
//!
//! Saving publishes no event: drafts are private, and autosave would fill
//! the retained event log. Dropping a forwarded attachment releases its blob.

use std::collections::HashSet;

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    blobs::BlobOwner,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;
use serde_json::json;
use utoipa::ToSchema;

use crate::{
    domain::{
        drafts::{self, DraftView, check_limits, tidy},
        paths::DraftPath,
    },
    events::DRAFT_RECORD,
    model::{Address, AttachmentId, DraftId, DraftState},
    scopes::ConversationsSend,
};
use campbooks_kernel::authz::Action;

/// The fields to change; the others stay.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DraftChanges {
    /// The version this change was made on (from the draft last read).
    pub version: u32,
    /// Recipients.
    #[serde(default)]
    pub to: Option<Vec<Address>>,
    /// Copied recipients.
    #[serde(default)]
    pub cc: Option<Vec<Address>>,
    /// Blind-copied recipients.
    #[serde(default)]
    pub bcc: Option<Vec<Address>>,
    /// The subject.
    #[serde(default)]
    pub subject: Option<String>,
    /// The text.
    #[serde(default)]
    pub body: Option<String>,
    /// The quoted history.
    #[serde(default)]
    pub quoted: Option<String>,
    /// The forwarded attachments to keep (the others are dropped).
    #[serde(default)]
    pub attachments: Option<Vec<AttachmentId>>,
}

/// Save a draft.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/drafts/{draft_id}",
    operation_id = "conversations.update_draft",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("draft_id" = DraftId, Path, description = "The draft."),
    ),
    request_body = DraftChanges,
    responses(
        (status = OK, description = "The draft, with its new version.", body = DraftView),
        (status = BAD_REQUEST, description = "A field over its limit.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read its inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Changed since that version (`draft_changed`), or on its way (`draft_not_editable`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<DraftPath>,
    extract::Json(changes): extract::Json<DraftChanges>,
) -> Result<Json<DraftView>> {
    let access = &workspace.access;
    let id = path.draft_id;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = drafts::load(&state, &mut tx, access, id, true, Action::Write).await?;
    if !row.state()?.editable() {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("draft_not_editable"),
        ));
    }
    if i64::from(changes.version) != i64::from(row.version) {
        return Err(Error::Conflict(crate::MESSAGES.reason("draft_changed")));
    }
    let (to, cc, bcc) = row.recipients()?;
    let [to, cc, bcc] = tidy([
        changes.to.unwrap_or(to),
        changes.cc.unwrap_or(cc),
        changes.bcc.unwrap_or(bcc),
    ]);
    let subject = changes.subject.unwrap_or_else(|| row.subject.clone());
    let body = changes.body.unwrap_or_else(|| row.body.clone());
    let quoted = changes.quoted.unwrap_or_else(|| row.quoted.clone());
    check_limits(to.len() + cc.len() + bcc.len(), &subject, &body, &quoted)?;

    let mut attachments = row.stored_attachments()?;
    if let Some(keep) = changes.attachments {
        let keep: HashSet<AttachmentId> = keep.into_iter().collect();
        let (kept, dropped): (Vec<_>, Vec<_>) =
            attachments.into_iter().partition(|a| keep.contains(&a.id));
        let still: HashSet<_> = kept.iter().map(|a| a.blob.hash).collect();
        let ws = access.workspace.to_string();
        let owner = BlobOwner::new(DRAFT_RECORD, id);
        for gone in dropped.iter().filter(|a| !still.contains(&a.blob.hash)) {
            state
                .blobs
                .detach(&mut tx, &ws, &gone.blob.hash, &owner)
                .await?;
        }
        attachments = kept;
    }

    sqlx::query!(
        r#"
        UPDATE conversations.drafts
           SET to_recipients = $2, cc_recipients = $3, bcc_recipients = $4, subject = $5,
               body = $6, quoted = $7, attachments = $8, version = version + 1,
               state = $9, failure_reason = NULL, updated_at = now()
         WHERE id = $1
        "#,
        id as DraftId,
        json!(to),
        json!(cc),
        json!(bcc),
        subject,
        body,
        quoted,
        json!(attachments),
        DraftState::Draft.as_str(),
    )
    .execute(&mut *tx)
    .await?;
    let draft = drafts::load(&state, &mut tx, access, id, false, Action::Read)
        .await?
        .shown(&state.binding, access)?;
    tx.commit().await?;
    Ok(Json(draft))
}

/// Discard a draft (also a scheduled one, which then never leaves).
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/drafts/{draft_id}",
    operation_id = "conversations.delete_draft",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("draft_id" = DraftId, Path, description = "The draft."),
    ),
    responses(
        (status = NO_CONTENT, description = "Discarded."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read its inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "On its way (`draft_not_editable`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<DraftPath>,
) -> Result<StatusCode> {
    let access = &workspace.access;
    let id = path.draft_id;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = drafts::load(&state, &mut tx, access, id, true, Action::Delete).await?;
    if row.state()? == DraftState::Sending {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("draft_not_editable"),
        ));
    }
    discard(&state, &mut tx, &access.workspace.to_string(), id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Delete draft `id` and release every blob it holds.
pub async fn discard(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: &str,
    id: DraftId,
) -> Result<()> {
    sqlx::query!(
        "DELETE FROM conversations.drafts WHERE id = $1",
        id as DraftId
    )
    .execute(&mut **tx)
    .await?;
    state
        .blobs
        .detach_owner(tx, workspace, &BlobOwner::new(DRAFT_RECORD, id))
        .await?;
    Ok(())
}
