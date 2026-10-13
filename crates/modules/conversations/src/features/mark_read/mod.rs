//! `POST …/conversations/{conversation_id}/read` and `/unread` — mark a
//! conversation read (every message) or unread (its newest message).

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

/// Mark a conversation read.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/read",
    operation_id = "conversations.mark_read",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.mark_unread")
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
pub async fn read(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    apply(&state, &workspace.access, id, Flag::Read)
        .await
        .map(Json)
}

/// Mark a conversation unread (its newest message).
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/unread",
    operation_id = "conversations.mark_unread",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.mark_read")
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
pub async fn unread(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    apply(&state, &workspace.access, id, Flag::Unread)
        .await
        .map(Json)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn read_and_unread_are_announced_and_requested_of_the_provider(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        let base = format!("/api/v1/w/{workspace}/conversations/{id}");

        let read = kit.post(&sam, &format!("{base}/read"), None).await;
        assert_eq!(read.status, StatusCode::OK, "{:?}", read.json);
        assert_eq!(read.json["unread_count"], 0);
        let requested = kit.events("conversations.changes_requested").await;
        assert_eq!(requested.len(), 1);
        assert_eq!(requested[0]["origin"], "user");
        assert_eq!(
            requested[0]["items"],
            json!([{"provider_message_id": "en-1", "change": {"kind": "mark_read"}}])
        );
        let changed = kit.events("conversations.conversation_changed").await;
        assert_eq!(changed.last().unwrap()["origin"], "member");
        assert_eq!(changed.last().unwrap()["changes"], json!(["read"]));

        // Already read: nothing to do, nothing announced.
        kit.post(&sam, &format!("{base}/read"), None).await;
        assert_eq!(kit.events("conversations.changes_requested").await.len(), 1);

        let unread = kit.post(&sam, &format!("{base}/unread"), None).await;
        assert_eq!(unread.json["unread_count"], 1);
        let requested = kit.events("conversations.changes_requested").await;
        assert_eq!(requested[1]["items"][0]["change"]["kind"], "mark_unread");
        assert_eq!(requested[1]["items"][0]["provider_message_id"], "en-1");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn reading_needs_the_write_scope(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, _) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        let reader = kit.client(workspace, "conversations:read");
        let response = kit
            .post(
                &reader,
                &format!("/api/v1/w/{workspace}/conversations/{id}/read"),
                None,
            )
            .await;
        assert_eq!(response.status, StatusCode::FORBIDDEN);
    }
}
