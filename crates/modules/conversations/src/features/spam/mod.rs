//! `POST …/conversations/{conversation_id}/spam` and `/not-spam` — report a
//! conversation as spam or phishing ("Report and delete"): every message
//! moves to the provider's spam folder, which reports it there, and its need
//! goes with it. Not spam puts it back in the inbox.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk, extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::paths::ConversationPath,
    domain::{
        conversation::ConversationSummary,
        flags::{self, Flag},
        needs::NeedSettings,
    },
    model::ConversationId,
    scopes::ConversationsWrite,
};

/// Report a conversation as spam: it moves to spam, at the provider too.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/spam",
    operation_id = "conversations.report_spam",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.not_spam")
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
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn report(
    State(state): State<AppState>,
    Extension(needs): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    flags::apply(&state, &workspace.access, id, Flag::Spam, Some(&needs))
        .await
        .map(Json)
}

/// Take a conversation out of spam, back in the inbox.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/not-spam",
    operation_id = "conversations.not_spam",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.report_spam")
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
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn not_spam(
    State(state): State<AppState>,
    Extension(needs): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    flags::apply(&state, &workspace.access, id, Flag::Unspam, Some(&needs))
        .await
        .map(Json)
}

#[cfg(test)]
mod tests;
