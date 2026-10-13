//! `GET /api/v1/w/{workspace_id}/changes` — the client change feed
//! (ADR 0011), served by the server as infrastructure owner `changes`.
//!
//! The machinery (recording, sequencing, the stream) is
//! [`campbooks_kernel::changes`]; this is its HTTP face. The web app reaches
//! it through the BFF unchanged (`GET /bff/api/v1/w/{workspace_id}/changes`):
//! the BFF runs the request in-process and returns the streaming body as
//! is. The request timeout covers the response head only, and the
//! compression layer skips `text/event-stream`.

#[cfg(test)]
mod tests;

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue},
    response::{IntoResponse, Response, Sse},
};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    changes::{self, Subscription},
    extract::Query,
    tenancy::{Workspace, WorkspaceId},
};
use campbooks_workspaces::scopes::WorkspacesRead;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Owner of the change feed's operation (`changes.stream`); no module may
/// take the name.
pub const OWNER: &str = "changes";

/// The SSE resume header.
const LAST_EVENT_ID: &str = "last-event-id";

/// The change feed's routes, merged into the API.
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(stream))
}

/// Where to resume.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StreamQuery {
    /// The last cursor received, for clients that cannot send
    /// `Last-Event-ID` (the header wins when both are present). Omit it to
    /// start from now.
    pub after: Option<String>,
}

/// Follow a workspace's changes.
///
/// A Server-Sent Events stream of what changed in the workspace, for clients
/// that keep a cache: each `change` frame names a record and an operation
/// (`{"cursor","record":{"type","id"},"op":"upsert"|"delete","version"}`),
/// never its contents; refetch what you show. Frames: `ready` (where the
/// stream starts), `change`, `heartbeat` (an SSE comment, or the cursor when
/// it moved past changes you may not see), `reset` (your cursor is unknown,
/// expired or too far behind: refetch everything, continue from the
/// cursor given), `revoked` (you no longer belong to the workspace) and
/// `closing` (the stream ends; reconnect with your cursor). Every frame
/// that carries a cursor sets the SSE `id`, so `EventSource` resumes on its
/// own. You only receive changes of records a read operation you hold a
/// scope for returns.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/changes",
    operation_id = "changes.stream",
    tag = "changes",
    security(("bearer" = ["workspaces:read"])),
    extensions(("x-campbooks" = json!(Operation::query()))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("Last-Event-ID" = Option<String>, Header,
            description = "The last cursor received; resumes after it."),
        StreamQuery,
    ),
    responses(
        (status = OK, description = "The change stream.", content_type = "text/event-stream",
            body = String),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "`too_many_streams`: you hold the maximum \
            number of open streams.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn stream(
    State(state): State<AppState>,
    workspace: Workspace<WorkspacesRead>,
    Query(query): Query<StreamQuery>,
    headers: HeaderMap,
) -> Result<Response> {
    let after = headers
        .get(LAST_EVENT_ID)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
        .or(query.after);
    let frames = changes::open(
        &state,
        Subscription {
            workspace: workspace.id,
            access: workspace.access,
            principal: workspace.principal,
            after,
        },
    )
    .await?;
    let mut response = Sse::new(frames).into_response();
    // Tell buffering reverse proxies (nginx) to pass frames through at once.
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}
