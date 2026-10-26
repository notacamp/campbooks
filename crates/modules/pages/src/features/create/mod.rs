//! `POST /api/v1/w/{workspace_id}/pages` — a member makes a page, at the top
//! level of a place or under a page they may write, optionally with a body.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;
use utoipa::ToSchema;
use yrs::Doc;

use crate::{
    domain::{
        MAX_DEPTH, PageId, PageRow, PageView, Place, SELECT_ONE, access, announce, body,
        clean_icon, clean_title, expect_live, position, resources::PAGE_RECORD, roles,
        store::Loaded, tracked, view,
    },
    events::PageChangeKind,
    scopes::PagesWrite,
};

/// A new page.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewPage {
    /// Its title: 0 to 200 characters. A page may be made before it has one.
    #[serde(default)]
    pub title: Option<String>,
    /// Its icon, one of the page icons.
    #[serde(default)]
    pub icon: Option<String>,
    /// The page it goes under; none for the top level of `place`.
    #[serde(default)]
    pub parent: Option<PageId>,
    /// `private` (the default) or `workspace`, for a top-level page. A
    /// sub-page is always in its parent's place.
    #[serde(default)]
    pub place: Option<Place>,
    /// Where it sits among its siblings: a fractional-indexing key. Default:
    /// after the last one.
    #[serde(default)]
    pub position: Option<String>,
    /// Its body as one Yjs update (v1), in base64. Default: empty.
    #[serde(default)]
    pub document: Option<String>,
}

/// Make a page.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages",
    operation_id = "pages.create",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.trash")
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = NewPage,
    responses(
        (status = CREATED, description = "The page.", body = PageView),
        (status = BAD_REQUEST, description = "A title over 200 characters (`invalid_title`), an unknown icon, a bad position, or a body that is not valid (`invalid_update`, `unknown_block`, `link_scheme`, `document_too_large`, `foreign_blob`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the parent but not write in it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or a parent you cannot see.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The parent is in the trash (`page_trashed`), or pages would nest deeper than 12 levels (`page_too_deep`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Json(request): extract::Json<NewPage>,
) -> Result<(StatusCode, Json<PageView>)> {
    let access = &workspace.access;
    authz::require_type(&state, access, Action::Create, PAGE_RECORD, || {
        Error::Forbidden
    })?;
    let Some(creator) = access.user else {
        return Err(Error::Forbidden);
    };
    let title = clean_title(request.title.as_deref().unwrap_or_default())?;
    let icon = clean_icon(request.icon.as_deref())?;
    if let Some(key) = &request.position {
        position::check(key)?;
    }
    let update = request
        .document
        .as_deref()
        .map(body::decode_update)
        .transpose()?;

    let mut tx = state.db.begin_for(access.workspace).await?;
    let (parent, place) = match request.parent {
        Some(parent_id) => {
            let parent =
                access::require_page(&state, &mut tx, access, Action::Write, parent_id, false)
                    .await?;
            expect_live(&parent)?;
            if parent.kind()? != crate::domain::Kind::Page {
                return Err(Error::Conflict(crate::MESSAGES.reason("table_parent")));
            }
            let place = parent.place()?;
            if request.place.is_some_and(|asked| asked != place) {
                return Err(Error::BadRequest(crate::MESSAGES.reason("place_mismatch")));
            }
            let above = roles::chain(&mut tx, parent_id).await?;
            if above.len() >= MAX_DEPTH {
                return Err(Error::Conflict(crate::MESSAGES.reason("page_too_deep")));
            }
            (Some(parent_id), place)
        }
        None => (None, request.place.unwrap_or(Place::Private)),
    };
    let key = match request.position {
        Some(key) => key,
        None => position::next(&mut tx, parent, place, creator).await?,
    };
    let id = PageId::new();
    sqlx::query(
        "INSERT INTO pages.pages
             (id, workspace_id, parent_id, place, position, title, icon, created_by, edited_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)",
    )
    .bind(id)
    .bind(access.workspace.as_uuid())
    .bind(parent)
    .bind(place.as_str())
    .bind(&key)
    .bind(&title)
    .bind(icon)
    .bind(creator)
    .execute(&mut *tx)
    .await?;
    let mut kinds = vec![PageChangeKind::Meta];
    if let Some(update) = update {
        let loaded = Loaded {
            doc: Doc::new(),
            last_seq: 0,
            tail: 0,
        };
        let accepted = body::accept(&state, &mut tx, access, id, &loaded.doc, &update).await?;
        if accepted.changed {
            body::store(&mut tx, access, id, &loaded, &update, &accepted.inspection).await?;
            kinds.push(PageChangeKind::Body);
        }
    }
    announce(&state, &mut tx, access, id, kinds).await?;
    tracked::simple(
        &state,
        &mut tx,
        access.workspace,
        id,
        tracked::Change::Created,
    )
    .await?;
    let row: PageRow = sqlx::query_as(SELECT_ONE)
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    let page = view::page_view(&mut tx, access, &row).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(page)))
}

#[cfg(test)]
mod tests;
