//! `GET /api/v1/w/{workspace_id}/connections/{connection_id}/notion/pages` —
//! the pages a member's Notion connection can see, for "Import from
//! Notion". Only the member whose Notion account it is may ask (anyone else
//! is told there is no such connection), and only the pages the member
//! shared with Campbooks on Notion's own consent screen are there to list.
//!
//! The listing is Notion's search, newest edit first, read to the end up to
//! [`MAX_PAGES`] pages (so a sub-page count is a count), each call taking the
//! connection's request budget; a page that is a row of a database is left
//! out (they wait for Tables). Titles are the member's own words, returned to
//! them and logged nowhere.

use std::collections::{HashMap, HashSet};

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result,
    authz::{self, Action},
    db::BeginFor as _,
    tenancy::{Access, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        connections,
        credentials::{self, TokenError},
        model::{ConnectionId, ConnectionPath, Provider},
    },
    events::CONNECTION_RECORD,
    notion::{
        Parent,
        pace::{Pace, Stop},
    },
    provider::ProviderError,
    scopes::ConnectorsRead,
};

/// Most pages one listing reads (five calls of a hundred).
pub const MAX_PAGES: usize = 500;

/// How long a listing waits for the connection's request budget: a member is
/// waiting for it.
const PATIENCE_SECONDS: u32 = 10;

/// A page the connection can see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionPage {
    /// The page's id at Notion.
    pub notion_page: Uuid,
    /// Its title; empty when it has none.
    pub title: String,
    /// When Notion last edited it.
    pub edited_at: DateTime<Utc>,
    /// The page it is under, when that one is listed too.
    pub parent: Option<Uuid>,
    /// How many of the listed pages are under it.
    pub sub_pages: u32,
    /// It is linked to a page already, through this connection.
    pub linked: bool,
}

/// The pages of a Notion connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct NotionPages {
    /// The pages, newest edit first.
    pub pages: Vec<NotionPage>,
    /// There are more than were read.
    pub truncated: bool,
    /// How many requests the listing took of the connection's budget.
    pub requests: u32,
}

/// The pages a Notion connection can see.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/connections/{connection_id}/notion/pages",
    operation_id = "connectors.list_notion_pages",
    tag = "connectors",
    security(("bearer" = ["connectors:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["connectors.connection"]).access("connectors.connection", campbooks_kernel::authz::Action::Disconnect)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("connection_id" = ConnectionId, Path, description = "The Notion connection."),
    ),
    responses(
        (status = OK, description = "The pages the connection can see.", body = NotionPages),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or a connection you may see but that is not yours (an admin, a client token).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such Notion connection, or you may not see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Notion needs you to connect again (`needs_reauth`), or is not set up on this instance (`provider_not_configured`).",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "The connection's request budget with Notion is spent for now.",
            body = Problem, content_type = "application/problem+json"),
        (status = SERVICE_UNAVAILABLE, description = "Notion could not be reached (`provider_unavailable`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsRead>,
    Path(path): Path<ConnectionPath>,
) -> Result<Json<NotionPages>> {
    execute(&state, &deps, &workspace.access, path.connection_id)
        .await
        .map(Json)
}

/// The pages `connection` can see, for the member of `access` whose Notion
/// account it is.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    connection: ConnectionId,
) -> Result<NotionPages> {
    let mut tx = state.db.begin_for(access.workspace).await?;
    // Only the member whose account it is: the owner's action, which an
    // admin or a client token, who may see the connection, does not have
    // (403). Anyone who may not see it is told there is no such connection.
    authz::require(
        state,
        &mut tx,
        access,
        Action::Disconnect,
        &RecordRef::new(CONNECTION_RECORD, &connection),
        || Error::Forbidden,
    )
    .await?;
    let Some(notion) = deps.notion.as_ref() else {
        return Err(Error::Conflict(MESSAGES.reason("provider_not_configured")));
    };
    let found = connections::load(&mut tx, connection)
        .await?
        .filter(|c| c.provider == Provider::Notion)
        .ok_or(Error::NotFound)?;
    let linked: HashSet<Uuid> = sqlx::query_scalar(
        "SELECT notion_page_id FROM connectors.notion_links WHERE connection_id = $1",
    )
    .bind(connection)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .collect();
    tx.commit().await?;
    if found.health == crate::domain::model::Health::NeedsReauth {
        return Err(Error::Conflict(MESSAGES.reason("needs_reauth")));
    }
    let token = match credentials::access_token(&state.db, deps, access.workspace, connection).await
    {
        Ok(token) => token,
        Err(TokenError::NeedsReauth(_)) => {
            return Err(Error::Conflict(MESSAGES.reason("needs_reauth")));
        }
        Err(TokenError::Provider(err)) => return Err(unavailable(&err)),
        Err(TokenError::Database(err)) => return Err(err),
    };
    let mut adapter = deps.config.notion.adapter();
    adapter.patience_seconds = PATIENCE_SECONDS;
    let mut pace = Pace::new(state.db.clone(), connection, &adapter);
    let mut listed = Vec::new();
    let mut cursor: Option<String> = None;
    let mut requests = 0_u32;
    let mut truncated = false;
    loop {
        pace.take().await.map_err(stopped)?;
        requests += 1;
        let page = notion
            .search(&token, cursor.as_deref())
            .await
            .map_err(|err| unavailable(&err))?;
        listed.extend(page.pages.into_iter().filter(|p| !p.trashed));
        match page.next {
            Some(next) if listed.len() < MAX_PAGES => cursor = Some(next),
            Some(_) => {
                truncated = true;
                break;
            }
            None => break,
        }
    }
    listed.truncate(MAX_PAGES);
    let known: HashSet<Uuid> = listed.iter().map(|p| p.id).collect();
    let mut under: HashMap<Uuid, u32> = HashMap::new();
    for page in &listed {
        if let Parent::Page(parent) = page.parent
            && known.contains(&parent)
        {
            *under.entry(parent).or_default() += 1;
        }
    }
    let pages = listed
        .into_iter()
        .map(|page| NotionPage {
            notion_page: page.id,
            title: page.title,
            edited_at: page.edited,
            parent: match page.parent {
                Parent::Page(parent) if known.contains(&parent) => Some(parent),
                _ => None,
            },
            sub_pages: under.get(&page.id).copied().unwrap_or(0),
            linked: linked.contains(&page.id),
        })
        .collect();
    Ok(NotionPages {
        pages,
        truncated,
        requests,
    })
}

/// What Notion's trouble is to the member asking.
fn unavailable(err: &ProviderError) -> Error {
    match err {
        ProviderError::InvalidGrant | ProviderError::Unauthorized => {
            Error::Conflict(MESSAGES.reason("needs_reauth"))
        }
        ProviderError::RateLimited { retry_after } => Error::RateLimited {
            retry_after: retry_after.map_or(30, |wait| wait.as_secs().max(1)),
        },
        ProviderError::NotFound | ProviderError::Unavailable(_) | ProviderError::Unexpected(_) => {
            Error::Unavailable(MESSAGES.reason("provider_unavailable"))
        }
    }
}

/// What a stopped run is to the member asking.
fn stopped(stop: Stop) -> Error {
    match stop {
        Stop::Provider(err) => unavailable(&err),
        Stop::Database(err) => err,
        Stop::Waiting(wait) => Error::RateLimited {
            retry_after: wait.as_secs().max(1),
        },
    }
}

#[cfg(test)]
mod tests;
