//! `POST …/conversations/{conversation_id}/archive` and `/unarchive` — take a
//! conversation out of the inbox, or put it back.

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

/// Archive a conversation.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/archive",
    operation_id = "conversations.archive",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.unarchive")
        .automation()
        .assistant()
        .records(["conversations.conversation"])))),
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
pub async fn archive(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    apply(&state, &workspace.access, id, Flag::Archive)
        .await
        .map(Json)
}

/// Move a conversation back to the inbox.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/unarchive",
    operation_id = "conversations.unarchive",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.archive")
        .automation()
        .assistant()
        .records(["conversations.conversation"])))),
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
pub async fn unarchive(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    apply(&state, &workspace.access, id, Flag::Unarchive)
        .await
        .map(Json)
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn archive_moves_it_out_of_the_inbox_and_unarchive_back(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        let base = format!("/api/v1/w/{workspace}/conversations/{id}");
        let list = format!("/api/v1/w/{workspace}/conversations");

        let archived = kit.post(&sam, &format!("{base}/archive"), None).await;
        assert_eq!(archived.json["archived"], true);
        assert_eq!(
            kit.get(&sam, &list).await.json["conversations"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let archive = kit.get(&sam, &format!("{list}?archived=true")).await;
        assert_eq!(archive.json["conversations"][0]["id"], id);
        let requested = kit.events("conversations.changes_requested").await;
        assert_eq!(requested[0]["items"].as_array().unwrap().len(), 2);

        let back = kit.post(&sam, &format!("{base}/unarchive"), None).await;
        assert_eq!(back.json["archived"], false);
        assert_eq!(
            kit.get(&sam, &list).await.json["conversations"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
}
