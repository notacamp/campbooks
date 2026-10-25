//! `POST /api/v1/w/{workspace_id}/connections/{connection_id}/address-check`
//! — check an IMAP mailbox's address again (campbooks-cloud#314): a new
//! one-time code sent to the address through the mailbox's own SMTP server,
//! looked for in its Inbox by the stream's runs. For a mailbox whose last
//! check did not find its code (or went with its job); one still under way
//! is answered as it is.

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, RecordRef, Result, Risk,
    authz::{self, Action},
    db::BeginFor as _,
    i18n::Locale,
    tenancy::{Access, Workspace, WorkspaceId},
};

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        address_checks, connections,
        model::{AddressCheck, ConnectionId, ConnectionPath, ConnectionView, Health, Provider},
    },
    events::CONNECTION_RECORD,
    scopes::ConnectorsWrite,
};

/// Check a mailbox's address again.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/connections/{connection_id}/address-check",
    operation_id = "connectors.check_address",
    tag = "connectors",
    security(("bearer" = ["connectors:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low).records(["connectors.connection"]).access("connectors.connection", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("connection_id" = ConnectionId, Path, description = "The connection."),
    ),
    responses(
        (status = ACCEPTED, description = "A new check is on its way (or the one under way \
            goes on): the connection, its `address_check` `checking`.", body = ConnectionView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such connection, or you may not see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Not an IMAP mailbox, or its address is verified \
            (`address_check_not_needed`), it must be connected again first (`needs_reauth`), \
            or IMAP is not turned on on this instance (`provider_not_configured`).",
            body = Problem, content_type = "application/problem+json"),
        (status = TOO_MANY_REQUESTS, description = "Its address was checked 5 times today \
            (`rate_limited`).", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    Extension(deps): Extension<Deps>,
    locale: Locale,
    workspace: Workspace<ConnectorsWrite>,
    Path(path): Path<ConnectionPath>,
) -> Result<(StatusCode, Json<ConnectionView>)> {
    let view = execute(&state, &deps, &workspace.access, path.connection_id, locale).await?;
    Ok((StatusCode::ACCEPTED, Json(view)))
}

/// Check connection `id`'s address again, writing in `locale`.
pub async fn execute(
    state: &AppState,
    deps: &Deps,
    access: &Access,
    id: ConnectionId,
    locale: Locale,
) -> Result<ConnectionView> {
    let mut tx = state.db.begin_for(access.workspace).await?;
    authz::require(
        state,
        &mut tx,
        access,
        Action::Write,
        &RecordRef::new(CONNECTION_RECORD, &id),
        || Error::Forbidden,
    )
    .await?;
    let connection = connections::load(&mut tx, id)
        .await?
        .ok_or(Error::NotFound)?;
    if connection.provider != Provider::Imap || connection.address_verified {
        return Err(Error::Conflict(MESSAGES.reason("address_check_not_needed")));
    }
    if !deps.imap_enabled() {
        return Err(Error::Conflict(MESSAGES.reason("provider_not_configured")));
    }
    if connection.health == Health::NeedsReauth {
        return Err(Error::Conflict(MESSAGES.reason("needs_reauth")));
    }
    let under_way = address_checks::view(&mut tx, id, false).await? == Some(AddressCheck::Checking);
    tx.commit().await?;
    if !under_way {
        if !address_checks::take_budget(&state.db, id).await? {
            return Err(Error::RateLimited {
                retry_after: 60 * 60,
            });
        }
        let mut tx = state.db.begin_for(access.workspace).await?;
        address_checks::start(&mut tx, access.workspace, id, locale, deps.clock.now()).await?;
        tx.commit().await?;
        tracing::info!(workspace = %access.workspace, connection = %id, "an address check was asked for again");
    }
    let mut tx = state.db.begin_for(access.workspace).await?;
    let connection = connections::load(&mut tx, id)
        .await?
        .ok_or(Error::NotFound)?;
    let view = connections::view(&mut tx, connection, access.user, deps).await?;
    tx.commit().await?;
    Ok(view)
}

#[cfg(test)]
mod tests;
