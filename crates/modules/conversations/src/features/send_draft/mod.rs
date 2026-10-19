//! `POST …/drafts/{draft_id}/send` — send a draft, after an undo window;
//! `POST …/drafts/{draft_id}/cancel_send` — take it back within the window.
//!
//! Sending checks the draft, then schedules it: `conversations.release_send`
//! runs when the window ends and hands it to the connector, unless it was
//! cancelled, deleted or sent again meanwhile (each send has a generation,
//! and a release acts only at its own).
//!
//! Both are the member's own request only (campbooks-cloud#242): neither is
//! the assistant's or automation's (`x-campbooks` opts neither in), and a
//! request run in-process for the member that reaches them anyway is
//! refused. The assistant may write and save drafts; the member sends them.

use axum::{Extension, Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::ScopeName,
    db::BeginFor as _,
    extract,
    jobs::EnqueueOptions,
    operations::RateLimit,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{TimeDelta, Utc};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    domain::{
        binding,
        drafts::{self, DraftView, MAX_ATTACHMENT_BYTES, attachment_bytes, valid_address},
        paths::DraftPath,
    },
    features::release_send::{ReleaseSend, ReleaseSendPayload},
    model::{DraftId, DraftState},
    scopes::ConversationsSend,
};
use campbooks_kernel::authz::Action;

/// Which version of the draft to send: the one the member last saw saved.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SendDraft {
    /// The draft's version as last saved by the caller; any other answers
    /// 409 `draft_changed`, so what is sent is what the member saw.
    pub version: u32,
    /// The `content_token` that came with the draft on screen (its last
    /// save's answer, or the draft as read). Missing, another member's, or
    /// not the draft as it is now: 409 `content_changed`, and nothing is
    /// sent (campbooks-cloud#242).
    #[serde(default)]
    pub content_token: Option<String>,
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
    // Mail leaves in the person's name: the send class of the rate limits.
    // The member's own request only (campbooks-cloud#242): neither the
    // assistant nor automation may send, whatever a call says it confirmed.
    extensions(("x-campbooks" = json!(Operation::command(Risk::High)
        .undo("conversations.cancel_send")
        .rate_limit(RateLimit::Send)
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
        (status = FORBIDDEN, description = "Missing scope, you only read its inbox, or not your own request (`send_needs_you`): the assistant and automation never send.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Already on its way (`draft_not_editable`), saved since that version (`draft_changed`), or not what you were shown (`content_changed`): read it again, check it, and send again.",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "`rate_limited`: you sent too much mail in a short time; `Retry-After` says when to try again.",
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
    let row = drafts::load(&state, &mut tx, access, id, true, Action::Send).await?;
    members_own_send(&workspace, id)?;
    if !row.state()?.editable() {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("draft_not_editable"),
        ));
    }
    if i64::from(body.version) != i64::from(row.version) {
        return Err(Error::Conflict(crate::MESSAGES.reason("draft_changed")));
    }
    if !state.binding.is_enabled() {
        tracing::warn!(
            workspace = %access.workspace,
            draft = %row.id,
            "a send was refused: there is no key to bind it with (CAMPBOOKS_SECRET_KEY is unset)"
        );
        return Err(Error::internal("sends are not bound: no instance secret"));
    }
    let content = drafts::shown_to_author(&state, access, &row, body.content_token.as_deref())?;
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
    bind_request(&state, &mut tx, access, (&row, &content), generation).await?;
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
    let draft = drafts::load(&state, &mut tx, access, id, false, Action::Read)
        .await?
        .shown(&state.binding, access)?;
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(draft)))
}

/// 403 `send_needs_you` unless the request is the member's own
/// (campbooks-cloud#242). Mail is untrusted input to the assistant, so a
/// message could ask it to send something, or to take back a send the
/// member made; and the assistant can read a draft's `content_token` like
/// any client. A `confirmed` flag is set by the code making the call, so it
/// proves nothing: only a request that arrived over HTTP with the member's
/// token sends mail or cancels a send. Only the kernel says who invoked a
/// request ([`Workspace::invoked_by`]), from a request extension no client
/// can send. Checked after the access checks, before anything changes.
fn members_own_send<S: ScopeName>(workspace: &Workspace<S>, draft: DraftId) -> Result<()> {
    let Some(caller) = workspace.invoked_by else {
        return Ok(());
    };
    tracing::warn!(
        workspace = %workspace.id,
        draft = %draft,
        caller = %caller,
        "a send was refused: only a member's own request sends mail or takes a send back"
    );
    Err(Error::Denied(crate::MESSAGES.reason("send_needs_you")))
}

/// Bind the author's request to send `row` (with `content`, its
/// [`binding::content_hash`] as checked by [`drafts::shown_to_author`]) at
/// `generation` (scheduled in `tx`) with a MAC the database cannot forge
/// (campbooks-cloud#227): `release_send` releases only a draft whose
/// content, recipients, inbox, author and generation are still the ones
/// signed here. Without the instance secret there is no key, and nothing
/// is scheduled.
async fn bind_request(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    access: &campbooks_kernel::tenancy::Access,
    draft: (&drafts::DraftRow, &[u8; 32]),
    generation: i32,
) -> Result<()> {
    let (row, content) = draft;
    let request = binding::SendRequest {
        workspace: access.workspace,
        draft: row.id,
        generation,
        author: access.user,
        inbox: row.inbox_id,
        content,
    };
    let Some(mac) = request.sign(&state.binding) else {
        tracing::warn!(
            workspace = %access.workspace,
            draft = %row.id,
            "a send was refused: there is no key to bind it with (CAMPBOOKS_SECRET_KEY is unset)"
        );
        return Err(Error::internal("sends are not bound: no instance secret"));
    };
    sqlx::query("UPDATE conversations.drafts SET request_mac = $2 WHERE id = $1")
        .bind(row.id)
        .bind(mac)
        .execute(&mut *tx)
        .await?;
    Ok(())
}

/// Take a send back within its undo window.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/drafts/{draft_id}/cancel_send",
    operation_id = "conversations.cancel_send",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    // The member's own request only (campbooks-cloud#242): the assistant
    // may not take back a send the member made.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("draft_id" = DraftId, Path, description = "The draft."),
    ),
    responses(
        (status = OK, description = "A draft again.", body = DraftView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, you only read its inbox, or not your own request (`send_needs_you`).",
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
    let row = drafts::load(&state, &mut tx, access, id, true, Action::Write).await?;
    members_own_send(&workspace, id)?;
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
            // The request it was sent with no longer stands.
            sqlx::query("UPDATE conversations.drafts SET request_mac = NULL WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        DraftState::Sending => return Err(Error::Conflict(crate::MESSAGES.reason("too_late"))),
        // Not sending: nothing to take back.
        DraftState::Draft | DraftState::Failed => {}
    }
    let draft = drafts::load(&state, &mut tx, access, id, false, Action::Read)
        .await?
        .shown(&state.binding, access)?;
    tx.commit().await?;
    Ok(Json(draft))
}

#[cfg(test)]
mod tests;
