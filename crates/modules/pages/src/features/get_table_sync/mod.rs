//! `GET /api/v1/w/{workspace_id}/tables/{table_id}/notion` — how a table is
//! kept in step with its Notion database, and the conflicts waiting for a
//! choice (campbooks-cloud#377): whether it is syncing, paused or ended (and
//! why), when it last moved either way, and for each column or cell changed
//! on both sides the two versions.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    domain::{
        PageId,
        notion::{
            LinkState, StopReason,
            tables::{
                self as synced,
                conflicts::{self, ConflictView},
            },
        },
        tables::{self, TablePath},
    },
    scopes::PagesRead,
};

/// A table's link to its Notion database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(as = NotionTableLink)]
pub struct TableLink {
    /// The Notion connection it is read through.
    pub connection: Uuid,
    /// The database at Notion (its data source).
    pub data_source: Uuid,
    /// Where the link is.
    pub state: LinkState,
    /// Why it ended, when it did.
    pub stopped_reason: Option<StopReason>,
    /// The member whose Notion account it is.
    pub owner: UserId,
    /// How many conflicts wait for a choice.
    pub conflicts: i32,
    /// When a change from Notion was last applied.
    pub last_inbound_at: Option<DateTime<Utc>>,
    /// When a change here was last written to Notion.
    pub last_outbound_at: Option<DateTime<Utc>>,
}

/// A table's sync with Notion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[schema(as = NotionTableSync)]
pub struct TableSync {
    /// The link; none for a table that is not kept in step with Notion.
    pub link: Option<TableLink>,
    /// The conflicts waiting for a choice, oldest first.
    pub conflicts: Vec<ConflictView>,
}

impl synced::ExternalTable {
    /// The link as the API shows it.
    pub fn view(&self) -> TableLink {
        TableLink {
            connection: self.connection_id,
            data_source: self.notion_data_source_id,
            state: match self.state.as_str() {
                "paused" => LinkState::Paused,
                "stopped" => LinkState::Stopped,
                _ => LinkState::Syncing,
            },
            stopped_reason: self.reason(),
            owner: self.created_by,
            conflicts: self.conflicts,
            last_inbound_at: self.last_inbound_at,
            last_outbound_at: self.last_outbound_at,
        }
    }
}

/// How a table is kept in step with Notion.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/tables/{table_id}/notion",
    operation_id = "pages.get_table_sync",
    tag = "pages",
    security(("bearer" = ["pages:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["pages.page"]).access("pages.page", campbooks_kernel::authz::Action::Read)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("table_id" = PageId, Path, description = "The table."),
    ),
    responses(
        (status = OK, description = "The sync and its conflicts.", body = TableSync),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such table, Notion databases are off, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not a table (`not_a_table`).", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<PagesRead>,
    extract::Path(path): extract::Path<TablePath>,
) -> Result<Json<TableSync>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    tables::require_table(&state, &mut tx, access, Action::Read, path.table_id, false).await?;
    let link = synced::external_of(&mut tx, path.table_id).await?;
    let conflicts = conflicts::open(&mut tx, path.table_id)
        .await?
        .iter()
        .map(conflicts::Row::view)
        .collect();
    tx.commit().await?;
    Ok(Json(TableSync {
        link: link.as_ref().map(synced::ExternalTable::view),
        conflicts,
    }))
}
