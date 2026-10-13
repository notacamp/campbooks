//! `POST …/conversations/{conversation_id}/star` and `/unstar` — star a
//! conversation (its newest message) or take every star off.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk, extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::paths::ConversationPath,
    domain::{
        conversation::ConversationSummary,
        flags::{Flag, apply},
    },
    model::ConversationId,
    scopes::ConversationsWrite,
};

/// Star a conversation.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/star",
    operation_id = "conversations.star",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.unstar")
        .automation()
        .assistant()
        .records(["conversations.conversation"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn star(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    apply(&state, &workspace.access, id, Flag::Star)
        .await
        .map(Json)
}

/// Unstar a conversation.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/unstar",
    operation_id = "conversations.unstar",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.star")
        .automation()
        .assistant()
        .records(["conversations.conversation"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn unstar(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    apply(&state, &workspace.access, id, Flag::Unstar)
        .await
        .map(Json)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn star_then_unstar(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        let base = format!("/api/v1/w/{workspace}/conversations/{id}");
        assert_eq!(
            kit.post(&sam, &format!("{base}/star"), None).await.json["starred"],
            true
        );
        let requested = kit.events("conversations.changes_requested").await;
        // The newest message is Sam's own reply.
        assert_eq!(requested[0]["items"][0]["provider_message_id"], "en-2");
        assert_eq!(
            kit.post(&sam, &format!("{base}/unstar"), None).await.json["starred"],
            false
        );
        assert_eq!(kit.events("conversations.changes_requested").await.len(), 2);
    }
}
