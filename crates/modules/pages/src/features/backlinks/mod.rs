//! `GET /api/v1/w/{workspace_id}/pages/backlinks` — where a record is linked
//! from: the pages that mention or embed it, which the caller may open, the
//! most recently edited first.
//!
//! The record has to be one the caller may read: otherwise it is not there
//! (404). A person is addressed by their cleaned address; it names no record
//! of its own, so it is asked about as the pages it is on.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{PageId, Place, document, resources::PAGE_RECORD, roles},
    scopes::PagesRead,
};

/// A page that links to the record, as the query reads it: id, title, icon,
/// place, edited at.
type Candidate = (PageId, String, Option<String>, String, DateTime<Utc>);

/// The most pages answered.
const LIMIT: i64 = 100;

/// The record to find the links to.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BacklinkParams {
    /// The record type: `workspaces.member`, `conversations.conversation`,
    /// `tasks.task`, `schedule.event`, `pages.page` or `person`.
    #[serde(rename = "type")]
    pub record_type: String,
    /// The record's id (for `person`, the cleaned address).
    pub id: String,
}

/// A page that links to the record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Backlink {
    /// The page.
    pub page: PageId,
    /// Its title.
    pub title: String,
    /// Its icon.
    pub icon: Option<String>,
    /// Private or workspace.
    pub place: Place,
    /// When it was last changed.
    pub edited_at: DateTime<Utc>,
}

/// The pages that link to a record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Backlinks {
    /// The pages, most recently edited first.
    pub pages: Vec<Backlink>,
}

/// Where a record is linked from.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/pages/backlinks",
    operation_id = "pages.backlinks",
    tag = "pages",
    security(("bearer" = ["pages:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), BacklinkParams),
    responses(
        (status = OK, description = "The pages the caller may open that link to the record.", body = Backlinks),
        (status = BAD_REQUEST, description = "A record type or id a page cannot link to (`invalid_ref`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or a token acting for nobody.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or a record you cannot read.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesRead>,
    extract::Query(params): extract::Query<BacklinkParams>,
) -> Result<Json<Backlinks>> {
    let access = &workspace.access;
    authz::require_type(&state, access, Action::Read, PAGE_RECORD, || {
        Error::Forbidden
    })?;
    let invalid = || Error::BadRequest(crate::MESSAGES.reason("invalid_ref"));
    if !document::REF_TYPES.contains(&params.record_type.as_str()) {
        return Err(invalid());
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    if params.record_type == "person" {
        if document::clean_address(&params.id).as_deref() != Some(params.id.as_str()) {
            return Err(invalid());
        }
    } else {
        // The record has to be one the caller may read, as its own module
        // answers: otherwise it does not exist for them.
        authz::authorize(
            &state,
            &mut tx,
            access,
            Action::Read,
            &RecordRef::new(params.record_type.clone(), &params.id),
        )
        .await?
        .or_else(|| Error::NotFound)?;
    }
    let candidates: Vec<Candidate> = sqlx::query_as(
        "SELECT p.id, p.title, p.icon, p.place, p.edited_at
           FROM pages.pages p
          WHERE p.trashed_at IS NULL
            AND p.id IN (SELECT page_id FROM pages.links WHERE ref_type = $1 AND ref_id = $2)
          ORDER BY p.edited_at DESC, p.id
          LIMIT 500",
    )
    .bind(&params.record_type)
    .bind(&params.id)
    .fetch_all(&mut *tx)
    .await?;
    let readable = roles::readable(&mut tx, access).await?;
    tx.commit().await?;
    let mut pages = Vec::new();
    for (page, title, icon, place, edited_at) in candidates {
        let itself = params.record_type == "pages.page" && params.id == page.to_string();
        if itself || !readable.contains_key(&page) {
            continue;
        }
        pages.push(Backlink {
            page,
            title,
            icon,
            place: place.parse()?,
            edited_at,
        });
        if i64::try_from(pages.len()).unwrap_or(i64::MAX) >= LIMIT {
            break;
        }
    }
    Ok(Json(Backlinks { pages }))
}

#[cfg(test)]
mod tests;
