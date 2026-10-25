//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/sync` — the one call that
//! keeps a page's body in step between the server and a client.
//!
//! The client sends the state vector of the document it holds, and an update
//! when it has changes the server has not seen. The server checks the update
//! (`domain::document`), merges it into the page's document, stores it, and
//! answers what the client is missing with the document's new state vector.
//! Two people editing one page at once both keep what they wrote.
//!
//! Without an update the call only reads (any role may); with one it writes
//! (needs `writer`). An update that is not valid is refused with 400 and
//! nothing is stored.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::{PageId, access, announce, body, document, expect_live, store},
    events::PageChangeKind,
    features::get::PagePath,
    scopes::PagesWrite,
};

/// Body changes are announced at most this often per page, in seconds;
/// later ones in the window are folded into the next announcement (editors
/// pull on the first).
const ANNOUNCE_EVERY_SECONDS: i32 = 5;

/// What the client holds, and what it wants to add.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SyncRequest {
    /// The client's document's state vector (v1), in base64.
    pub state_vector: String,
    /// Changes the server has not seen, as one Yjs update (v1) of at most
    /// 256 KiB, in base64.
    #[serde(default)]
    pub update: Option<String>,
}

/// What the server holds that the client does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncResponse {
    /// What the client is missing, as one Yjs update (v1), in base64.
    pub update: String,
    /// The server's document's state vector (v1), in base64.
    pub state_vector: String,
    /// The page's revision after this call.
    pub revision: i64,
}

/// Merge a client's changes into a page's body, and get back what it lacks.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/sync",
    operation_id = "pages.sync",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    request_body = SyncRequest,
    responses(
        (status = OK, description = "What the client is missing, and the new state vector.", body = SyncResponse),
        (status = BAD_REQUEST, description = "The update is not valid: not a Yjs update or one that cannot be applied (`invalid_update`), over 256 KiB (`update_too_large`), uses a block or attribute the page cannot hold (`unknown_block`), links outside https, http and mailto (`link_scheme`), names a file that was not uploaded to this page (`foreign_blob`), or would make the page too large (`document_too_large`). Nothing is stored.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is in the trash (`page_trashed`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
    extract::Json(request): extract::Json<SyncRequest>,
) -> Result<Json<SyncResponse>> {
    let access = &workspace.access;
    let vector = body::decode_vector(&request.state_vector)?;
    let update = request
        .update
        .as_deref()
        .map(body::decode_update)
        .transpose()?;
    let action = if update.is_some() {
        Action::Write
    } else {
        Action::Read
    };
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = access::require_page(&state, &mut tx, access, action, path.page_id, true).await?;
    let loaded = store::load(&mut tx, path.page_id).await?;
    let mut announce_body = false;
    if let Some(update) = &update {
        expect_live(&row)?;
        let accepted =
            body::accept(&state, &mut tx, access, path.page_id, &loaded.doc, update).await?;
        if accepted.changed {
            body::store(
                &mut tx,
                access,
                path.page_id,
                &loaded,
                update,
                &accepted.inspection,
            )
            .await?;
            // At most one announcement every few seconds per page.
            announce_body = sqlx::query_scalar::<_, bool>(
                "UPDATE pages.pages SET last_body_event_at = now()
                  WHERE id = $1
                    AND (last_body_event_at IS NULL
                         OR last_body_event_at < now() - make_interval(secs => $2::int))
              RETURNING true",
            )
            .bind(path.page_id)
            .bind(ANNOUNCE_EVERY_SECONDS)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
        }
    }
    if announce_body {
        announce(
            &state,
            &mut tx,
            access,
            path.page_id,
            vec![PageChangeKind::Body],
        )
        .await?;
    }
    let missing = document::diff_since(&loaded.doc, &vector)?;
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM pages.pages WHERE id = $1")
        .bind(path.page_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(SyncResponse {
        update: body::encode(&missing),
        state_vector: body::encode(&document::state_vector(&loaded.doc)),
        revision,
    }))
}

#[cfg(test)]
mod tests;
