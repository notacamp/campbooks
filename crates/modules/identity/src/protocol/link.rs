//! Signed-in round trips: adding a way in, connecting a mailbox, and
//! reconnecting one, through the same provider round trip as sign-in.
//!
//! 1. `identity.start_link` (the API) issues a ticket for the signed-in
//!    account; the browser opens `/auth/start/{provider}?link=<ticket>`.
//! 2. [`start`] spends the ticket and goes on only when this browser's issuer
//!    session belongs to the ticket's account. The provider is asked for
//!    consent again (so a refresh token comes back when offline access is
//!    asked), for the connection's account when reconnecting.
//! 3. [`finish`], at the callback, links the provider identity to the
//!    account (or finds it already there) and refuses one that is another
//!    account's way in. For a connect or a reconnect, the grant then goes to
//!    the connectors module through the grant-intake port.
//!
//! The browser always lands back on the ticket's `return_to`, with
//! `linked=<provider>`, `connected=<connection id>` or `link_error=<code>`.
//! Tokens of a refused round trip are dropped, never revoked: revoking a
//! Google token revokes the person's whole grant to this app, including a
//! mailbox another account connected with it.

use axum::{http::HeaderMap, response::Response};
use campbooks_kernel::{Result, connections::ProviderGrant, tenancy::WorkspaceId};
use chrono::{DateTime, Utc};
use secrecy::SecretString;

use super::{
    Ctx,
    sign_in::{MailGrant, Prompt, redirect_to_provider},
};
use crate::{
    deps::Issuer,
    domain::{
        accounts::{self, LinkOutcome},
        links, sessions,
    },
    events::WayInAdded,
    upstream::{
        Flow, Purpose,
        client::{SignedIn, Upstream},
    },
    web::see_other,
};

/// Where the browser lands when even the ticket's `return_to` is unknown.
const HOME: &str = "/";

/// `return_to` with `name=value` added to its query.
pub(super) fn back(return_to: &str, name: &str, value: &str) -> Response {
    let separator = if return_to.contains('?') { '&' } else { '?' };
    see_other(&format!(
        "{return_to}{separator}{name}={}",
        super::sign_in::encode(value)
    ))
}

/// Start the signed-in round trip `ticket` names, from this browser.
pub(super) async fn start(
    ctx: &Ctx,
    issuer: &Issuer,
    headers: &HeaderMap,
    upstream: &Upstream,
    ticket: &str,
) -> Result<Response> {
    let now = issuer.now();
    let Some(ticket) = links::take(&ctx.state.db, ticket, now)
        .await?
        .filter(|t| t.provider == upstream.provider())
    else {
        return Ok(back(HOME, "link_error", "link_expired"));
    };
    let session = match issuer.session_cookie(headers) {
        Some(token) => sessions::current(&ctx.state.db, &token, now).await?,
        None => None,
    };
    if session.is_none_or(|s| s.account_id != ticket.account_id) {
        tracing::info!(
            purpose = ticket.purpose.as_str(),
            "a link ticket was opened by a browser not signed in to its account"
        );
        return Ok(back(&ticket.return_to, "link_error", "sign_in_again"));
    }
    let flow = Flow {
        provider: ticket.provider,
        purpose: ticket.purpose,
        account_id: Some(ticket.account_id),
        workspace_id: ticket.workspace_id,
        connection_id: ticket.connection_id,
        request_id: None,
        return_to: ticket.return_to,
        nonce: campbooks_kernel::crypto::random_token(),
        code_verifier: campbooks_kernel::crypto::random_token(),
        consent_retry: false,
    };
    redirect_to_provider(
        ctx,
        issuer,
        upstream,
        flow,
        Prompt {
            consent: true,
            login_hint: ticket.login_hint.as_deref(),
        },
    )
    .await
}

/// Finish the signed-in round trip `flow` the provider answered with
/// `signed_in`.
pub(super) async fn finish(
    ctx: &Ctx,
    flow: Flow,
    signed_in: SignedIn,
    now: DateTime<Utc>,
) -> Result<Response> {
    let Some(account) = flow.account_id else {
        return Ok(back(&flow.return_to, "link_error", "link_expired"));
    };
    if accounts::profile(&ctx.state.db, account).await?.is_none() {
        return Ok(back(&flow.return_to, "link_error", "sign_in_again"));
    }
    let SignedIn { identity, tokens } = signed_in;
    let provider = flow.provider;
    let via = if flow.purpose == Purpose::Link {
        "link"
    } else {
        "connect"
    };
    let mut tx = ctx.state.db.begin().await?;
    match accounts::link(&mut tx, account, &identity, via, now).await? {
        LinkOutcome::Another => {
            tx.commit().await?;
            tracing::info!(
                provider = provider.as_str(),
                purpose = flow.purpose.as_str(),
                "refused a way in that belongs to another account"
            );
            return Ok(back(
                &flow.return_to,
                "link_error",
                "way_in_belongs_to_another_account",
            ));
        }
        LinkOutcome::Already(_) => {}
        LinkOutcome::Added(id) => {
            accounts::audit(
                &mut tx,
                Some(account),
                "way_in_added",
                serde_json::json!({ "method": "provider", "provider": provider.as_str(), "via": via, "way_in": id }),
                now,
            )
            .await?;
            ctx.state
                .events
                .publish(
                    &mut tx,
                    &WayInAdded {
                        account,
                        method: "provider".to_owned(),
                        provider: Some(provider.as_str().to_owned()),
                        via: via.to_owned(),
                    },
                )
                .await?;
        }
    }
    tx.commit().await?;
    if !flow.purpose.wants_mailbox() {
        return Ok(back(&flow.return_to, "linked", provider.as_str()));
    }
    let Some(workspace) = flow.workspace_id else {
        return Ok(back(&flow.return_to, "link_error", "link_expired"));
    };
    let mail = MailGrant::of(ctx, provider, &tokens);
    let (true, Some(refresh), Some(address)) =
        (mail.granted, tokens.refresh_token, identity.email.clone())
    else {
        // The way in stays: the round trip proved the account is theirs.
        return Ok(back(&flow.return_to, "link_error", "consent_incomplete"));
    };
    let grant = ProviderGrant {
        workspace: WorkspaceId::from_uuid(workspace),
        subject: account.to_string(),
        provider: provider.as_str().to_owned(),
        account_subject: identity.subject,
        address,
        address_verified: identity.email_verified,
        display_name: identity.name,
        scopes: mail.scopes,
        access_token: SecretString::from(tokens.access_token),
        refresh_token: Some(SecretString::from(refresh)),
        expires_in: tokens.expires_in,
        extending: flow.connection_id,
    };
    Ok(match ctx.state.grants.adopt(&ctx.state, grant).await {
        Ok(connection) => back(&flow.return_to, "connected", &connection),
        Err(error) => {
            tracing::info!(
                provider = provider.as_str(),
                purpose = flow.purpose.as_str(),
                code = error.code(),
                "the connectors module refused a mailbox grant"
            );
            back(&flow.return_to, "link_error", error.code())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_are_added_to_the_return_path() {
        let location = |r: Response| r.headers()["location"].to_str().unwrap().to_owned();
        assert_eq!(
            location(back("/w/1/settings/sign-in", "linked", "zoho")),
            "/w/1/settings/sign-in?linked=zoho"
        );
        assert_eq!(
            location(back("/w/1?tab=a", "link_error", "a b")),
            "/w/1?tab=a&link_error=a+b"
        );
    }
}
