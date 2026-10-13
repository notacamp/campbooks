//! `DELETE /api/v1/w/{workspace_id}/connections/{connection_id}` —
//! disconnect: revoke the grant at the provider, delete the credentials,
//! streams and index, and publish `connectors.connection_removed`. Only the
//! member who connected it may (the access catalogue's `disconnect`;
//! owners and admins see it but get 403 `disconnect_owner_only`). `?mail=`
//! says what becomes of the mail it brought in: `delete` (the default) or
//! `keep` as read-only history. Undoing it needs a new consent.

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    extract::Query,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        model::{ConnectionId, ConnectionPath, MailChoice},
        removal,
    },
    events::CONNECTION_RECORD,
    features::start_connection::acting_member,
    scopes::ConnectorsWrite,
};

/// What to do with the mail.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DisconnectQuery {
    /// `delete` (the default): the inboxes it fed are deleted, with their
    /// messages, conversations, drafts, decisions, search entries and
    /// files. `keep`: they stay as read-only history, no longer synced.
    pub mail: Option<MailChoice>,
}

/// Disconnect a connection.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/connections/{connection_id}",
    operation_id = "connectors.disconnect",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    // Stops syncing for good; reconnecting needs the member's consent.
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["connectors.connection"]).access("connectors.connection", campbooks_kernel::authz::Action::Disconnect)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("connection_id" = ConnectionId, Path, description = "The connection."),
        DisconnectQuery,
    ),
    responses(
        (status = NO_CONTENT, description = "Disconnected."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, a client token \
            (`connection_needs_user`), or someone other than the member who connected it \
            (`disconnect_owner_only`).", body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such connection, or you may not see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    workspace: Workspace<ConnectorsWrite>,
    Path(path): Path<ConnectionPath>,
    Query(query): Query<DisconnectQuery>,
) -> Result<StatusCode> {
    execute(
        &state,
        &deps,
        &workspace.access,
        path.connection_id,
        query.mail.unwrap_or_default(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Disconnect connection `id` on behalf of `access`, deleting or keeping
/// its mail as `mail` says.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    id: ConnectionId,
    mail: MailChoice,
) -> Result<()> {
    let user = acting_member(access)?;
    let resource = RecordRef::new(CONNECTION_RECORD, &id);
    let forbidden = || Error::Denied(MESSAGES.reason("disconnect_owner_only"));
    let mut tx = state.db.begin_for(access.workspace).await?;
    authz::require(state, &mut tx, access, Action::Disconnect, &resource, forbidden).await?;
    tx.commit().await?;
    removal::revoke(state, deps, access.workspace, id).await;
    let mut tx = state.db.begin_for(access.workspace).await?;
    // Asked again in the transaction that deletes: the grant may have moved.
    authz::require(state, &mut tx, access, Action::Disconnect, &resource, forbidden).await?;
    removal::delete(&mut tx, state, access.workspace, id, mail).await?;
    tx.commit().await?;
    tracing::info!(workspace = %access.workspace, %user, connection = %id, ?mail, "disconnected");
    Ok(())
}

#[cfg(test)]
mod tests;
