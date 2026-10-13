//! `POST …/drafts/{draft_id}/send` — send a draft, after an undo window;
//! `POST …/drafts/{draft_id}/cancel_send` — take it back within the window.
//!
//! Sending checks the draft, then schedules it: `conversations.release_send`
//! runs when the window ends and hands it to the connector, unless it was
//! cancelled, deleted or sent again meanwhile (each send has a generation,
//! and a release acts only at its own).

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    jobs::EnqueueOptions,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{TimeDelta, Utc};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{
        drafts::{self, DraftView, MAX_ATTACHMENT_BYTES, attachment_bytes, valid_address},
        paths::DraftPath,
    },
    features::release_send::{ReleaseSend, ReleaseSendPayload},
    model::{DraftId, DraftState},
    scopes::ConversationsSend,
};

/// Which version of the draft to send: the one the member last saw saved.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
pub struct SendDraft {
    /// The draft's version as last saved by the caller; any other answers
    /// 409 `draft_changed`, so what is sent is what the member saw.
    pub version: u32,
}

/// How sending behaves, from the module's settings.
#[derive(Debug, Clone, Copy)]
pub struct SendSettings {
    /// How long a send waits before it leaves.
    pub undo: TimeDelta,
}

/// Send a draft.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/drafts/{draft_id}/send",
    operation_id = "conversations.send_draft",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::High)
        .undo("conversations.cancel_send")
        .assistant()
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Send)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("draft_id" = DraftId, Path, description = "The draft."),
    ),
    request_body = SendDraft,
    responses(
        (status = ACCEPTED, description = "Scheduled: it leaves at `send_at` unless cancelled.",
            body = DraftView),
        (status = BAD_REQUEST, description = "No recipient (`no_recipients`), a malformed address (`invalid_address`) or too many attachments (`attachments_too_large`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read its inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Already on its way (`draft_not_editable`), or saved since that version (`draft_changed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn send(
    State(state): State<AppState>,
    Extension(settings): Extension<SendSettings>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<DraftPath>,
    extract::Json(body): extract::Json<SendDraft>,
) -> Result<(StatusCode, Json<DraftView>)> {
    let access = &workspace.access;
    let id = path.draft_id;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = drafts::load(&mut tx, access, id, true).await?;
    if !row.state()?.editable() {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("draft_not_editable"),
        ));
    }
    if i64::from(body.version) != i64::from(row.version) {
        return Err(Error::Conflict(crate::MESSAGES.reason("draft_changed")));
    }
    let (to, cc, bcc) = row.recipients()?;
    if to.len() + cc.len() + bcc.len() == 0 {
        return Err(Error::BadRequest(crate::MESSAGES.reason("no_recipients")));
    }
    if let Some(bad) = to
        .iter()
        .chain(&cc)
        .chain(&bcc)
        .find(|a| !valid_address(&a.address))
    {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("invalid_address")
                .arg("address", bad.address.clone()),
        ));
    }
    if attachment_bytes(&row.stored_attachments()?) > MAX_ATTACHMENT_BYTES {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("attachments_too_large")
                .arg("max", MAX_ATTACHMENT_BYTES / (1024 * 1024)),
        ));
    }
    let send_at = Utc::now() + settings.undo;
    let generation = sqlx::query_scalar!(
        r#"
        UPDATE conversations.drafts
           SET state = $2, send_generation = send_generation + 1, send_at = $3,
               failure_reason = NULL, updated_at = now()
         WHERE id = $1
        RETURNING send_generation
        "#,
        id as DraftId,
        DraftState::Scheduled.as_str(),
        send_at,
    )
    .fetch_one(&mut *tx)
    .await?;
    campbooks_kernel::jobs::enqueue_to::<ReleaseSend>(
        &mut tx,
        &ReleaseSendPayload {
            draft: id,
            generation,
        },
        EnqueueOptions::for_workspace(access.workspace.to_string())
            .unique(format!("release:{id}:{generation}"))
            .run_at(send_at),
    )
    .await?;
    let draft = drafts::load(&mut tx, access, id, false).await?.view()?;
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(draft)))
}

/// Take a send back within its undo window.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/drafts/{draft_id}/cancel_send",
    operation_id = "conversations.cancel_send",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("draft_id" = DraftId, Path, description = "The draft."),
    ),
    responses(
        (status = OK, description = "A draft again.", body = DraftView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read its inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours (a sent draft is gone).",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "It has already left (`too_late`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn cancel(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<DraftPath>,
) -> Result<Json<DraftView>> {
    let access = &workspace.access;
    let id = path.draft_id;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = drafts::load(&mut tx, access, id, true).await?;
    match row.state()? {
        DraftState::Scheduled => {
            sqlx::query!(
                r#"UPDATE conversations.drafts
                      SET state = $2, send_at = NULL, version = version + 1, updated_at = now()
                    WHERE id = $1"#,
                id as DraftId,
                DraftState::Draft.as_str(),
            )
            .execute(&mut *tx)
            .await?;
        }
        DraftState::Sending => return Err(Error::Conflict(crate::MESSAGES.reason("too_late"))),
        // Not sending: nothing to take back.
        DraftState::Draft | DraftState::Failed => {}
    }
    let draft = drafts::load(&mut tx, access, id, false).await?.view()?;
    tx.commit().await?;
    Ok(Json(draft))
}

#[cfg(test)]
mod tests;
