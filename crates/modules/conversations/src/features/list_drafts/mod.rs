//! `GET /api/v1/w/{workspace_id}/drafts` and `GET …/drafts/{draft_id}` —
//! the caller's own drafts (a composer reopens the one it left), newest
//! first, optionally only those answering one conversation.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        access::visible_inboxes,
        drafts::{self, DraftView},
        paths::DraftPath,
    },
    model::{ConversationId, DraftId, InboxId},
    scopes::ConversationsSend,
};
use campbooks_kernel::authz::Action;
use crate::domain::resources;
use crate::events::DRAFT_RECORD;

/// Most drafts listed.
pub const MAX_DRAFTS: i64 = 50;

/// Which drafts.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DraftFilter {
    /// Only drafts answering this conversation.
    #[serde(default)]
    pub conversation_id: Option<ConversationId>,
}

/// The caller's drafts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DraftList {
    /// Newest first, at most 50.
    pub drafts: Vec<DraftView>,
}

/// List your drafts.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/drafts",
    operation_id = "conversations.list_drafts",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant()
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        DraftFilter,
    ),
    responses(
        (status = OK, description = "Your drafts.", body = DraftList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not in it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Query(filter): extract::Query<DraftFilter>,
) -> Result<Json<DraftList>> {
    let access = &workspace.access;
    resources::authorize_type(&state, access, Action::Read, DRAFT_RECORD)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    let ids = sqlx::query_scalar!(
        r#"SELECT id AS "id: DraftId" FROM conversations.drafts
            WHERE author IS NOT DISTINCT FROM $1 AND inbox_id = ANY($2)
              AND ($3::uuid IS NULL OR conversation_id = $3)
            ORDER BY updated_at DESC, id DESC
            LIMIT $4"#,
        access.user.map(|u| *u.as_uuid()),
        &inboxes as &[InboxId],
        filter.conversation_id as Option<ConversationId>,
        MAX_DRAFTS,
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut list = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(row) = drafts::find(&mut tx, access, id, false).await? {
            list.push(row.view()?);
        }
    }
    tx.commit().await?;
    Ok(Json(DraftList { drafts: list }))
}

/// Read one of your drafts.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/drafts/{draft_id}",
    operation_id = "conversations.get_draft",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant()
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("draft_id" = DraftId, Path, description = "The draft."),
    ),
    responses(
        (status = OK, description = "The draft.", body = DraftView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read its inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<DraftPath>,
) -> Result<Json<DraftView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let draft = drafts::load(&state, &mut tx, access, path.draft_id, false, Action::Read)
        .await?
        .view()?;
    tx.commit().await?;
    Ok(Json(draft))
}
