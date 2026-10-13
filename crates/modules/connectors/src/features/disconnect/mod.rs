//! `DELETE /api/v1/w/{workspace_id}/connections/{connection_id}?mail=` —
//! disconnect: revoke the grant at the provider, delete the credentials,
//! streams and index, and publish `connectors.connection_removed` with what
//! becomes of the mail (`delete`, the default, or `keep` read-only). Only
//! the member who connected it may (owners and admins see it but may not);
//! keeping its mail is for its grantor when it is their own mailbox (they
//! sign in with it), or when they are an owner or admin. Undoing it needs a
//! new consent.

use axum::{
    Extension,
    extract::{Path, Query, State},
    http::StatusCode,
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        connections,
        model::{ConnectionId, ConnectionPath},
        removal,
    },
    events::MailChoice,
    features::start_connection::acting_member,
    scopes::ConnectorsWrite,
};

/// What to do with the mailbox's mail in Campbooks.
#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DisconnectQuery {
    /// `delete` (the default): its messages, attachments, search index,
    /// decisions and drafts leave Campbooks. `keep`: they stay, read-only;
    /// only for the grantor's own mailbox, or a grantor who is an owner or
    /// admin. The mail at the provider is never touched.
    #[serde(default)]
    pub mail: MailChoice,
}

/// Disconnect a connection.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/connections/{connection_id}",
    operation_id = "connectors.disconnect",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    // Stops syncing for good; reconnecting needs the member's consent.
    extensions(("x-campbooks" = json!(Operation::command(Risk::High).records(["connectors.connection"])))),
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
            (`connection_needs_user`), another member's connection (`not_your_connection`), \
            or `mail=keep` where it is not allowed (`keep_mail_not_allowed`).", body = Problem,
            content_type = "application/problem+json"),
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
        query.mail,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Disconnect connection `id` on behalf of `access`, doing `mail` with its
/// mail.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    id: ConnectionId,
    mail: MailChoice,
) -> Result<()> {
    let user = acting_member(access)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let connection = connections::load(&mut tx, id)
        .await?
        .filter(|c| connections::visible(access, c.granted_by))
        .ok_or(Error::NotFound)?;
    tx.commit().await?;
    if connection.granted_by != user {
        return Err(Error::Denied(MESSAGES.reason("not_your_connection")));
    }
    let manages = access
        .role
        .is_some_and(campbooks_kernel::tenancy::Role::can_manage);
    if mail == MailChoice::Keep && !connection.grantor_signs_in && !manages {
        return Err(Error::Denied(MESSAGES.reason("keep_mail_not_allowed")));
    }
    removal::revoke(state, deps, access.workspace, id).await;
    let mut tx = state.db.begin_for(access.workspace).await?;
    removal::delete(&mut tx, state, access.workspace, id, mail).await?;
    tx.commit().await?;
    tracing::info!(workspace = %access.workspace, %user, connection = %id, mail = ?mail, "disconnected");
    Ok(())
}

#[cfg(test)]
mod tests;
