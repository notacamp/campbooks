//! `POST /api/v1/w/{workspace_id}/pages/{page_id}/append` — add a section of
//! Markdown at the end of a page (campbooks-cloud#364).
//!
//! The Markdown is read **on the server** into the blocks the schema has
//! (`domain::markdown`): headings, paragraphs, lists, to-dos, quotes, code
//! and dividers, with bold, italic, strike, code and links to `https`,
//! `http` and `mailto` only. Anything else, raw HTML included, is plain
//! text. The blocks become a Yjs update, which goes through the same
//! checker as any `pages.sync` update before it is stored, so what a client
//! could not write by hand cannot be written this way either. Sources are
//! records the section draws on; they end the section as a line of mentions
//! (and so show up as backlinks).
//!
//! The change is announced and, on a page kept in step with Notion, pushed
//! there like any edit. The answer names the append (`append_id`) so
//! `pages.unappend` can take it back. Nothing the member wrote is changed.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    domain::{
        Kind, PageId, access, announce, append, body, document, expect_live, markdown, notion,
        store, tracked,
    },
    events::PageChangeKind,
    features::get::PagePath,
    scopes::PagesWrite,
};

/// The most sources one append names.
pub const MAX_SOURCES: usize = 10;

/// A section to add.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AppendRequest {
    /// The section, as Markdown of at most 16 KiB.
    pub markdown: String,
    /// Up to 10 records the section draws on, shown as mentions after it.
    #[serde(default)]
    pub sources: Vec<RecordRef>,
}

/// What was added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Appended {
    /// This append, for `pages.unappend`.
    pub append_id: Uuid,
    /// How many blocks it added (a list item counts one).
    pub blocks: u32,
    /// The page's revision after it.
    pub revision: i64,
}

/// The sources, each once, every one a record a page may point at.
pub fn clean_sources(sources: &[RecordRef]) -> Result<Vec<RecordRef>> {
    let mut clean: Vec<RecordRef> = Vec::new();
    for source in sources {
        if !document::is_reference(&source.record_type, &source.id) {
            return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_ref")));
        }
        if !clean.contains(source) {
            clean.push(source.clone());
        }
    }
    if clean.len() > MAX_SOURCES {
        return Err(Error::BadRequest(
            crate::MESSAGES.reason("too_many_sources"),
        ));
    }
    Ok(clean)
}

/// The Markdown, within its cap and with something in it.
pub fn clean_markdown(text: &str) -> Result<Vec<crate::domain::notion::model::Block>> {
    if text.len() > markdown::MAX_MARKDOWN_BYTES {
        return Err(Error::PayloadTooLarge(
            crate::MESSAGES.reason("markdown_too_large"),
        ));
    }
    let blocks = markdown::parse(text);
    if blocks.is_empty() {
        return Err(Error::BadRequest(crate::MESSAGES.reason("markdown_empty")));
    }
    Ok(blocks)
}

/// Add a section of Markdown at the end of a page.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/pages/{page_id}/append",
    operation_id = "pages.append",
    tag = "pages",
    security(("bearer" = ["pages:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("pages.unappend")
        .assistant()
        .records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("page_id" = PageId, Path, description = "The page."),
    ),
    request_body = AppendRequest,
    responses(
        (status = OK, description = "The section was added at the end of the page.", body = Appended),
        (status = BAD_REQUEST, description = "Nothing to add (`markdown_empty`), a source a page cannot point at (`invalid_ref`), more than 10 sources (`too_many_sources`), or a page that would be too large (`document_too_large`). Nothing is stored.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may read the page but not change it.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such page, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "The page is in the trash (`page_trashed`), or is a table (`not_a_page`).",
            body = Problem, content_type = "application/problem+json"),
        (status = PAYLOAD_TOO_LARGE, description = "More than 16 KiB of Markdown (`markdown_too_large`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesWrite>,
    extract::Path(path): extract::Path<PagePath>,
    extract::Json(request): extract::Json<AppendRequest>,
) -> Result<Json<Appended>> {
    let access = &workspace.access;
    let blocks = clean_markdown(&request.markdown)?;
    let sources = clean_sources(&request.sources)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row =
        access::require_page(&state, &mut tx, access, Action::Write, path.page_id, true).await?;
    // Only a member writes (a client acting for nobody never gets this far:
    // it cannot see the page).
    let Some(author) = access.user else {
        return Err(Error::Forbidden);
    };
    expect_live(&row)?;
    if row.kind()? == Kind::Table {
        return Err(Error::Conflict(crate::MESSAGES.reason("not_a_page")));
    }
    let loaded = store::load(&mut tx, path.page_id).await?;
    let added = append::append(&loaded.doc, &blocks, &sources)?;
    // The one checker every update passes: a section cannot be anything a
    // client's own update could not be.
    let accepted = body::accept(
        &state,
        &mut tx,
        access,
        path.page_id,
        &loaded.doc,
        &added.update,
    )
    .await?;
    let append_id = Uuid::now_v7();
    if accepted.changed {
        body::store(
            &mut tx,
            access,
            path.page_id,
            &loaded,
            &added.update,
            &accepted.inspection,
        )
        .await?;
        sqlx::query(
            "INSERT INTO pages.appends (id, page_id, workspace_id, appended_by, units)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(append_id)
        .bind(path.page_id)
        .bind(access.workspace.as_uuid())
        .bind(author)
        .bind(sqlx::types::Json(&added.units))
        .execute(&mut *tx)
        .await?;
        // A page kept in step with Notion sends this like any other edit.
        notion::schedule_push(&mut tx, access.workspace, path.page_id, chrono::Utc::now()).await?;
        announce(
            &state,
            &mut tx,
            access,
            path.page_id,
            vec![PageChangeKind::Body],
        )
        .await?;
        tracked::section_appended(
            &state,
            &mut tx,
            access.workspace,
            path.page_id,
            added.units.len(),
            sources.len(),
        )
        .await?;
    }
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM pages.pages WHERE id = $1")
        .bind(path.page_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(Appended {
        append_id,
        blocks: u32::try_from(added.units.len()).unwrap_or(u32::MAX),
        revision,
    }))
}

#[cfg(test)]
mod tests;
