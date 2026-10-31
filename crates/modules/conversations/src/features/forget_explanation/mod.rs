//! `DELETE /api/v1/w/{workspace_id}/conversations/{conversation_id}/explanation`
//! — forget the member's own explanation of a thread (campbooks-cloud#407).
//!
//! "Hide" in the reader does not call this (it only folds the card); this
//! exists for privacy and for tests. It removes only the caller's own row.

use axum::{extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::{explanation, paths::ConversationPath, resources},
    events::CONVERSATION_RECORD,
    model::ConversationId,
    scopes::ConversationsRead,
};

/// Forget your explanation of a thread.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/explanation",
    operation_id = "conversations.forget_explanation",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::command(campbooks_kernel::Risk::Low).records(["conversations.conversation"]).access("conversations.conversation", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = NO_CONTENT, description = "Your explanation of this thread is gone."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<StatusCode> {
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
    let member = match explanation::member_sees(&mut tx, access, conversation, allowed).await {
        Ok((member, _)) => member,
        Err(Error::NotFound) => {
            tx.commit().await?;
            return Err(Error::NotFound);
        }
        Err(other) => return Err(other),
    };
    explanation::forget(&mut tx, access, member, conversation).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
