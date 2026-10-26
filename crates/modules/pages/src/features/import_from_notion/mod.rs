//! `POST /api/v1/w/{workspace_id}/pages/notion/imports` — bring pages from a
//! member's Notion account into Pages, to be kept in step both ways
//! (campbooks-cloud#376).
//!
//! The member picks Notion pages their **own** Notion connection can see
//! (the connectors module lists them). Each becomes a page of the caller's,
//! private under a place of their own or under a page they may write, linked
//! to its Notion page, and filled by the first read of Notion; its sub-pages
//! follow when that read finds them. A connection that is not the caller's
//! is answered as one that does not exist (404). A Notion page already linked
//! through the connection is left as it is.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    domain::{
        MAX_DEPTH, PageId, Place, access, announce, clean_title, expect_live, position,
        resources::PAGE_RECORD, roles, tracked,
    },
    events::{NotionLinkRequested, PageChangeKind},
    scopes::PagesWrite,
};

/// Most pages one import takes.
pub const MAX_IMPORT: usize = 100;

/// The record type of a connection, which the connectors module owns.
const CONNECTION_RECORD: &str = "connectors.connection";

/// One Notion page to bring in.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NotionPageToImport {
    /// The page at Notion.
    pub notion_page: Uuid,
    /// The title to show until Notion's is read (0 to 200 characters).
    #[serde(default)]
    pub title: Option<String>,
}

/// What to import.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportFromNotion {
    /// Your Notion connection.
    pub connection: Uuid,
    /// The page they go under; none for the top level of your private place.
    #[serde(default)]
    pub parent: Option<PageId>,
    /// The Notion pages, 1 to 100.
    pub pages: Vec<NotionPageToImport>,
}

/// A page made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ImportedPage {
    /// The new page.
    pub page: PageId,
    /// Its page at Notion.
    pub notion_page: Uuid,
}

/// What was imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Imported {
    /// The pages made, in the order asked.
    pub pages: Vec<ImportedPage>,
    /// How many were left alone: already linked through the connection, or
    /// named twice.
    pub skipped: u32,
}

/// Import pages from Notion.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/notion/imports",
    operation_id = "pages.import_from_notion",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Create)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = ImportFromNotion,
    responses(
        (status = CREATED, description = "The pages made.", body = Imported),
        (status = BAD_REQUEST, description = "No page, or more than 100 (`invalid_import`), or a title over 200 characters (`invalid_title`).",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, a client token, or you may read the parent but not write in it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, a parent you cannot see, or a connection that is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The parent is in the trash (`page_trashed`), or pages would nest deeper than 12 levels (`page_too_deep`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
#[allow(
    clippy::too_many_lines,
    reason = "one transaction read top to bottom: check, parent, make, announce"
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Json(request): extract::Json<ImportFromNotion>,
) -> Result<(StatusCode, Json<Imported>)> {
    let access = &workspace.access;
    authz::require_type(&state, access, Action::Create, PAGE_RECORD, || {
        Error::Forbidden
    })?;
    let Some(creator) = access.user else {
        return Err(Error::Forbidden);
    };
    if request.pages.is_empty() || request.pages.len() > MAX_IMPORT {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_import")));
    }
    let mut titles = Vec::with_capacity(request.pages.len());
    for page in &request.pages {
        titles.push(clean_title(page.title.as_deref().unwrap_or_default())?);
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    // Only the member whose Notion account it is: the owner's action on the
    // connection; anyone else is told there is no such connection.
    authz::require(
        &state,
        &mut tx,
        access,
        Action::Disconnect,
        &RecordRef::new(CONNECTION_RECORD, &request.connection),
        || Error::NotFound,
    )
    .await?;
    let (parent, place) = match request.parent {
        Some(parent_id) => {
            let parent =
                access::require_page(&state, &mut tx, access, Action::Write, parent_id, false)
                    .await?;
            expect_live(&parent)?;
            let above = roles::chain(&mut tx, parent_id).await?;
            if above.len() >= MAX_DEPTH {
                return Err(Error::Conflict(crate::MESSAGES.reason("page_too_deep")));
            }
            (Some(parent_id), parent.place()?)
        }
        None => (None, Place::Private),
    };
    let mut made = Vec::new();
    let mut skipped = 0_u32;
    let mut seen: Vec<Uuid> = Vec::new();
    for (asked, title) in request.pages.iter().zip(&titles) {
        let linked: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pages.external_links
                             WHERE connection_id = $1 AND notion_page_id = $2
                               AND state <> 'stopped')",
        )
        .bind(request.connection)
        .bind(asked.notion_page)
        .fetch_one(&mut *tx)
        .await?;
        if linked || seen.contains(&asked.notion_page) {
            skipped += 1;
            continue;
        }
        seen.push(asked.notion_page);
        let id = PageId::new();
        let key = position::next(&mut tx, parent, place, creator).await?;
        sqlx::query(
            "INSERT INTO pages.pages
                 (id, workspace_id, parent_id, place, position, title, created_by, edited_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $7)",
        )
        .bind(id)
        .bind(access.workspace.as_uuid())
        .bind(parent)
        .bind(place.as_str())
        .bind(&key)
        .bind(title)
        .bind(creator)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO pages.external_links
                 (page_id, workspace_id, connection_id, notion_page_id, state, created_by,
                  last_title)
             VALUES ($1, $2, $3, $4, 'syncing', $5, $6)",
        )
        .bind(id)
        .bind(access.workspace.as_uuid())
        .bind(request.connection)
        .bind(asked.notion_page)
        .bind(creator)
        .bind(title)
        .execute(&mut *tx)
        .await?;
        state
            .events
            .publish(
                &mut tx,
                &NotionLinkRequested {
                    workspace: access.workspace.to_string(),
                    page: id,
                    connection: request.connection,
                    notion_page: asked.notion_page,
                },
            )
            .await?;
        announce(&state, &mut tx, access, id, vec![PageChangeKind::Meta]).await?;
        tracked::simple(
            &state,
            &mut tx,
            access.workspace,
            id,
            tracked::Change::Created,
        )
        .await?;
        made.push(ImportedPage {
            page: id,
            notion_page: asked.notion_page,
        });
    }
    if !made.is_empty() {
        tracked::notion_imported(
            &state,
            &mut tx,
            access.workspace,
            request.connection,
            made.len(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(Imported {
            pages: made,
            skipped,
        }),
    ))
}

#[cfg(test)]
mod tests;
