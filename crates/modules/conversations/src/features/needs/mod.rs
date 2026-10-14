//! `POST …/conversations/{conversation_id}/need/resolve`, `/need/confirm` and
//! `/need/reopen` — a member's verdict on what a conversation needs.
//!
//! Done clears the need for everyone who can see the inbox (owner,
//! 2026-10-13); Not needed also stores a correction, the labelled example the
//! decide evals learn from (campbooks-cloud#59); confirming a question turns
//! it into a need Campbooks stands behind, and is stored as a correction too.
//! Reopen undoes either, withdrawing the correction.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;

use crate::{
    domain::paths::ConversationPath,
    domain::{
        access::lock_conversation,
        conversation::{ConversationSummary, summary},
        needs::{self, NeedSettings},
    },
    model::{ConversationId, CorrectionId, DecisionId, MessageId, NeedVerdict},
    scopes::ConversationsWrite,
};

#[cfg(test)]
mod tests;

/// How a member resolves a need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// It is done (replied by phone, paid, signed…).
    Done,
    /// It was never needed: Campbooks got it wrong.
    NotNeeded,
}

/// A member's verdict on a need.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Resolve {
    /// Done or Not needed.
    pub outcome: Outcome,
    /// The message the need the member saw comes from (`NeedView.message_id`):
    /// when the open need comes from another one, nothing is recorded (409
    /// `need_changed`).
    pub message_id: MessageId,
}

/// Yes to a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Confirm {
    /// The message the question the member saw comes from.
    pub message_id: MessageId,
}

/// Mark what a conversation needs as done, or as not needed.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/need/resolve",
    operation_id = "conversations.resolve_need",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.reopen_need")
        .assistant()
        .records(["conversations.conversation", "conversations.correction"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    request_body = Resolve,
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = BAD_REQUEST, description = "Not an outcome.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "It needs nothing (`no_open_need`), or not what you saw any more (`need_changed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn resolve(
    State(state): State<AppState>,
    Extension(settings): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Json(body): extract::Json<Resolve>,
) -> Result<Json<ConversationSummary>> {
    let verdict = match body.outcome {
        Outcome::Done => NeedVerdict::Done,
        Outcome::NotNeeded => NeedVerdict::NotNeeded,
    };
    judge(
        &state,
        &settings,
        &workspace.access,
        path.conversation_id,
        verdict,
        body.message_id,
    )
    .await
    .map(Json)
}

/// Answer yes to a question Campbooks put (a Review need it was unsure of).
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/need/confirm",
    operation_id = "conversations.confirm_need",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.reopen_need")
        .assistant()
        .records(["conversations.conversation", "conversations.correction"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    request_body = Confirm,
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = BAD_REQUEST, description = "No message id.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "No question to answer (`no_open_need`), or not the one you saw (`need_changed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn confirm(
    State(state): State<AppState>,
    Extension(settings): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Json(body): extract::Json<Confirm>,
) -> Result<Json<ConversationSummary>> {
    judge(
        &state,
        &settings,
        &workspace.access,
        path.conversation_id,
        NeedVerdict::Confirmed,
        body.message_id,
    )
    .await
    .map(Json)
}

/// Take back Done, Not needed or a confirmation: the need is as it was.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/need/reopen",
    operation_id = "conversations.reopen_need",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["conversations.conversation", "conversations.correction"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Nothing to bring back (`nothing_to_reopen`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn reopen(
    State(state): State<AppState>,
    Extension(settings): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let access = &workspace.access;
    let id = path.conversation_id;
    let mut tx = state.db.begin_for(access.workspace).await?;
    lock_conversation(&mut tx, access, id).await?;
    let withdrawn = sqlx::query_scalar!(
        r#"
        UPDATE conversations.needs n
           SET verdict = NULL, verdict_message_id = NULL, verdict_by = NULL,
               verdict_correction_id = NULL, updated_at = now()
          FROM (SELECT conversation_id, verdict_correction_id FROM conversations.needs
                 WHERE conversation_id = $1 AND verdict IS NOT NULL
                   AND verdict_message_id = message_id FOR UPDATE) old
         WHERE n.conversation_id = old.conversation_id
        RETURNING old.verdict_correction_id AS "correction: CorrectionId"
        "#,
        id as ConversationId,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("nothing_to_reopen")))?;
    if let Some(correction) = withdrawn {
        sqlx::query!(
            "UPDATE conversations.corrections SET withdrawn_at = now() WHERE id = $1",
            correction as CorrectionId,
        )
        .execute(&mut *tx)
        .await?;
    }
    needs::refresh(
        &mut tx,
        &state.events,
        &access.workspace.to_string(),
        id,
        &settings,
    )
    .await?;
    let summary = summary(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(summary))
}

/// Record `verdict` on conversation `id`'s open need.
async fn judge(
    state: &AppState,
    settings: &NeedSettings,
    access: &Access,
    id: ConversationId,
    verdict: NeedVerdict,
    seen: MessageId,
) -> Result<ConversationSummary> {
    let mut tx = state.db.begin_for(access.workspace).await?;
    lock_conversation(&mut tx, access, id).await?;
    let need = sqlx::query!(
        r#"
        SELECT message_id AS "message: MessageId", question, basis, trust,
               decision_id AS "decision: DecisionId"
          FROM conversations.needs
         WHERE conversation_id = $1 AND state = 'open' FOR UPDATE
        "#,
        id as ConversationId,
    )
    .fetch_optional(&mut *tx)
    .await?
    .filter(|need| verdict != NeedVerdict::Confirmed || need.trust == "check")
    .ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("no_open_need")))?;
    // Not the need the member saw (a newer message replaced it meanwhile):
    // record nothing rather than a wrong correction.
    if need.message != seen {
        return Err(Error::Conflict(crate::MESSAGES.reason("need_changed")));
    }
    let correction = match verdict {
        NeedVerdict::Done => None,
        NeedVerdict::NotNeeded => Some(
            correct(
                &mut tx,
                access,
                id,
                need.message,
                Correction {
                    question: &need.question,
                    to: "false",
                    overridden: &need.basis,
                    decision: need.decision,
                },
            )
            .await?,
        ),
        NeedVerdict::Confirmed => Some(
            correct(
                &mut tx,
                access,
                id,
                need.message,
                Correction {
                    question: &need.question,
                    to: "true",
                    overridden: &need.basis,
                    decision: need.decision,
                },
            )
            .await?,
        ),
    };
    sqlx::query!(
        r#"
        UPDATE conversations.needs
           SET verdict = $2, verdict_message_id = message_id, verdict_by = $3,
               verdict_correction_id = $4, updated_at = now()
         WHERE conversation_id = $1
        "#,
        id as ConversationId,
        verdict.as_str(),
        access.user.map(|u| *u.as_uuid()),
        correction as Option<CorrectionId>,
    )
    .execute(&mut *tx)
    .await?;
    needs::refresh(
        &mut tx,
        &state.events,
        &access.workspace.to_string(),
        id,
        settings,
    )
    .await?;
    let summary = summary(&mut tx, id).await?;
    tx.commit().await?;
    Ok(summary)
}

/// A correction of the answer behind a need.
struct Correction<'a> {
    question: &'a str,
    to: &'a str,
    overridden: &'a str,
    decision: Option<DecisionId>,
}

/// Store a correction of the answer behind a need: the labelled example.
async fn correct(
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    conversation: ConversationId,
    message: MessageId,
    correction: Correction<'_>,
) -> Result<CorrectionId> {
    let id = CorrectionId::new();
    let decisions: Vec<DecisionId> = correction.decision.into_iter().collect();
    sqlx::query!(
        r#"
        INSERT INTO conversations.corrections
            (id, workspace_id, conversation_id, message_id, question, from_value, to_value,
             overridden_source, decision_ids, corrected_by)
        VALUES ($1, $2, $3, $4, $5, 'true', $6, $7, $8, $9)
        "#,
        id as CorrectionId,
        access.workspace.as_uuid(),
        conversation as ConversationId,
        message as MessageId,
        correction.question,
        correction.to,
        correction.overridden,
        &decisions as &[DecisionId],
        access.user.map(|u| *u.as_uuid()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(id)
}
