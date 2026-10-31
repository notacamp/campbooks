//! `GET /api/v1/w/{workspace_id}/conversations/{conversation_id}/explanation`
//! — the member's own explanation of a thread (campbooks-cloud#407).
//!
//! Served only to the member who asked for it, and only when, at this
//! moment, they may read the conversation and every message it was made from
//! is still a message of it, in an inbox they can see. Otherwise the answer
//! is 404 and the explanation is deleted on the spot. No other member, role or
//! owner reads it. A thread with a newer message, or a member who reads
//! another language now, gets it marked `stale`; it is never written again on
//! its own. The client calls this only after the member asked for one (the
//! conversation detail says whether they may).

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    i18n::Locale,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::{
        explanation::{self, ExplanationView, Row, SELECT},
        paths::ConversationPath,
        resources,
    },
    events::CONVERSATION_RECORD,
    model::ConversationId,
    scopes::ConversationsRead,
};

/// The member's explanation of a thread.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/explanation",
    operation_id = "conversations.get_explanation",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant()
        .records(["conversations.conversation"]).access("conversations.conversation", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "Your explanation of this thread.", body = ExplanationView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "You have none for this thread, or you cannot see the thread, or a message it was made from is gone from it (it is deleted).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    locale: Locale,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ExplanationView>> {
    let conversation = path.conversation_id;
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let allowed = resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &RecordRef::new(CONVERSATION_RECORD, &conversation),
    )
    .await;
    let (member, inboxes) =
        match explanation::member_sees(&mut tx, access, conversation, allowed).await {
            Ok(seen) => seen,
            Err(Error::NotFound) => {
                tx.commit().await?;
                return Err(Error::NotFound);
            }
            Err(other) => return Err(other),
        };
    let row: Option<Row> = sqlx::query_as(SELECT)
        .bind(access.workspace.as_uuid())
        .bind(member.as_uuid())
        .bind(conversation)
        .fetch_optional(&mut *tx)
        .await?;
    let row = row.ok_or(Error::NotFound)?;
    if !explanation::messages_still_there(&mut tx, &row.message_ids, &inboxes, conversation).await?
    {
        explanation::forget(&mut tx, access, member, conversation).await?;
        tx.commit().await?;
        return Err(Error::NotFound);
    }
    let current = explanation::current(&mut tx, conversation).await?;
    let view = explanation::view(
        &mut tx,
        &row,
        current.as_ref(),
        locale.tag(),
        &inboxes,
        conversation,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(view))
}
