//! Signed-in round trips: adding a way in, connecting a mailbox, and
//! reconnecting one, through the same provider round trip as sign-in.
//!
//! 1. `identity.start_link` (the API) issues a ticket for the signed-in
//!    account; the browser opens `/auth/start/{provider}?link=<ticket>`.
//! 2. [`start`] spends the ticket and goes on only when this browser's issuer
//!    session belongs to the ticket's account. The provider is asked for
//!    consent again (so a refresh token comes back when offline access is
//!    asked), for the connection's account when reconnecting.
//! 3. [`finish`], at the callback, checks the browser is still signed in as
//!    that account. A link or a connect links the provider identity (or finds
//!    it already there) and refuses one that is another account's way in; a
//!    connect asked not to add a way in (a shared mailbox) links nothing. A
//!    reconnect accepts only the connection's own provider account and never
//!    links anything. For a connect or a reconnect, the grant then goes to the
//!    connectors module through the grant-intake port.
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
use uuid::Uuid;

use super::{
    Ctx,
    sign_in::{MailGrant, Prompt, drop_refresh, redirect_to_provider},
};
use crate::{
    deps::Issuer,
    domain::{
        accounts::{self, LinkOutcome, ProviderIdentity},
        links,
        mailbox_links::{self, Recorded},
        sessions,
    },
    events::WayInAdded,
    features::forget_connected_way_in::announce as announce_removed,
    upstream::{
        Flow, Provider, Purpose,
        client::{SignedIn, Tokens, Upstream},
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
    if !signed_in_as(ctx, issuer, headers, ticket.account_id, now).await? {
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
        expected_subject: ticket.expected_subject,
        request_id: None,
        return_to: ticket.return_to,
        nonce: campbooks_kernel::crypto::random_token(),
        code_verifier: campbooks_kernel::crypto::random_token(),
        consent_retry: false,
        adds_way_in: ticket.adds_way_in,
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

/// Whether this browser's issuer session is a live one of `account`.
async fn signed_in_as(
    ctx: &Ctx,
    issuer: &Issuer,
    headers: &HeaderMap,
    account: Uuid,
    now: DateTime<Utc>,
) -> Result<bool> {
    let session = match issuer.session_cookie(headers) {
        Some(token) => sessions::current(&ctx.state.db, &token, now).await?,
        None => None,
    };
    Ok(session.is_some_and(|s| s.account_id == account))
}

/// Finish the signed-in round trip `flow` the provider answered with
/// `signed_in`, in the browser that sent `headers`.
pub(super) async fn finish(
    ctx: &Ctx,
    issuer: &Issuer,
    upstream: &Upstream,
    headers: &HeaderMap,
    flow: Flow,
    signed_in: SignedIn,
    now: DateTime<Utc>,
) -> Result<Response> {
    let Some(account) = flow.account_id else {
        return Ok(back(&flow.return_to, "link_error", "link_expired"));
    };
    // Still signed in as that account (not signed out, or in as someone
    // else, during the round trip), and the account still exists.
    if !signed_in_as(ctx, issuer, headers, account, now).await? {
        return Ok(back(&flow.return_to, "link_error", "sign_in_again"));
    }
    let SignedIn { identity, tokens } = signed_in;
    let provider = flow.provider;
    if flow.purpose == Purpose::Reconnect {
        // Only the connection's own account may come back, and a reconnect
        // never adds a way in: nothing is written for any other.
        if flow.expected_subject.as_deref() != Some(identity.subject.as_str()) {
            tracing::info!(
                provider = provider.as_str(),
                "refused a reconnect by another provider account"
            );
            drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
            return Ok(back(&flow.return_to, "link_error", "account_mismatch"));
        }
        let way_in = own_way_in(ctx, &identity, account).await?;
        let way_in = track(ctx, way_in, flow.workspace_id, now).await?;
        return hand_over_detached(ctx, flow, identity, tokens, way_in).await;
    }
    if flow.purpose == Purpose::Connect && !flow.adds_way_in {
        return finish_shared(ctx, upstream, flow, identity, tokens, account, now).await;
    }
    // A way in added to an account must be an address the allow-list lets in.
    if !ctx.permits_identity(&identity)? {
        drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
        tracing::info!(
            provider = provider.as_str(),
            purpose = flow.purpose.as_str(),
            "refused a way in the allow-list does not let in"
        );
        return Ok(back(&flow.return_to, "link_error", "sign_in_not_allowed"));
    }
    let via = if flow.purpose == Purpose::Link {
        "link"
    } else {
        "connect"
    };
    let mut tx = ctx.state.db.begin().await?;
    let way_in = match accounts::link(&mut tx, account, &identity, via, now).await? {
        LinkOutcome::Another => {
            tx.commit().await?;
            drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
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
        LinkOutcome::Already(id) => {
            let recorded = match (flow.purpose, flow.workspace_id) {
                (Purpose::Connect, Some(workspace)) => {
                    mailbox_links::record(&mut tx, id, workspace, false, now).await?
                }
                _ => Recorded::Untracked,
            };
            WayInLink {
                id,
                recorded,
                created: false,
            }
        }
        LinkOutcome::Added(id) => {
            announce_added(ctx, &mut tx, account, provider, via, id, now).await?;
            let recorded = match (flow.purpose, flow.workspace_id) {
                (Purpose::Connect, Some(workspace)) => {
                    mailbox_links::record(&mut tx, id, workspace, true, now).await?
                }
                _ => Recorded::Untracked,
            };
            WayInLink {
                id,
                recorded,
                created: true,
            }
        }
    };
    tx.commit().await?;
    if !flow.purpose.wants_mailbox() {
        drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
        return Ok(back(&flow.return_to, "linked", provider.as_str()));
    }
    hand_over_detached(ctx, flow, identity, tokens, Some(way_in)).await
}

/// Audit and announce way in `id`, just linked to `account` by `via`.
async fn announce_added(
    ctx: &Ctx,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    account: Uuid,
    provider: Provider,
    via: &str,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<()> {
    accounts::audit(
        tx,
        Some(account),
        "way_in_added",
        serde_json::json!({ "method": "provider", "provider": provider.as_str(), "via": via, "way_in": id }),
        now,
    )
    .await?;
    ctx.state
        .events
        .publish(
            tx,
            &WayInAdded {
                account,
                method: "provider".to_owned(),
                provider: Some(provider.as_str().to_owned()),
                via: via.to_owned(),
                way_in: Some(id),
            },
        )
        .await?;
    Ok(())
}

/// A connect asked not to add a way in (a shared mailbox): connected, never
/// made a way in, still refused when it is another account's way in.
async fn finish_shared(
    ctx: &Ctx,
    upstream: &Upstream,
    flow: Flow,
    identity: ProviderIdentity,
    tokens: Tokens,
    account: Uuid,
    now: DateTime<Utc>,
) -> Result<Response> {
    let provider = flow.provider;
    let linked = accounts::find(&ctx.state.db, &identity).await?;
    if linked.as_ref().is_some_and(|l| l.account_id != account) {
        drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
        tracing::info!(
            provider = provider.as_str(),
            "refused a mailbox that is another account's way in"
        );
        return Ok(back(
            &flow.return_to,
            "link_error",
            "way_in_belongs_to_another_account",
        ));
    }
    let way_in = linked.map(|l| l.identity_id);
    let way_in = track(ctx, way_in, flow.workspace_id, now).await?;
    hand_over_detached(ctx, flow, identity, tokens, way_in).await
}

/// The way in `identity` is for `account`, if it is one of theirs.
async fn own_way_in(ctx: &Ctx, identity: &ProviderIdentity, account: Uuid) -> Result<Option<Uuid>> {
    Ok(accounts::find(&ctx.state.db, identity)
        .await?
        .filter(|l| l.account_id == account)
        .map(|l| l.identity_id))
}

/// Record, before the grant is handed over, that the mailbox of way in
/// `way_in` is connected in `workspace` (when that way in is tracked). If it
/// cannot be recorded, the round trip fails rather than leave a mailbox the
/// way in's removal would not know about.
async fn track(
    ctx: &Ctx,
    way_in: Option<Uuid>,
    workspace: Option<Uuid>,
    now: DateTime<Utc>,
) -> Result<Option<WayInLink>> {
    let Some(id) = way_in else {
        return Ok(None);
    };
    let recorded = match workspace {
        Some(workspace) => {
            let mut conn = ctx.state.db.acquire().await?;
            mailbox_links::record(&mut conn, id, workspace, false, now).await?
        }
        None => Recorded::Untracked,
    };
    Ok(Some(WayInLink {
        id,
        recorded,
        created: false,
    }))
}

/// The person's way in of the account a grant is handed over for: whether
/// this round trip recorded its mailbox, and whether it created the way in.
#[derive(Debug, Clone, Copy)]
struct WayInLink {
    id: Uuid,
    recorded: Recorded,
    created: bool,
}

/// [`hand_over`] in a task of its own: once started it runs to the end
/// (connected and named, or refused and taken back) even if the browser
/// drops the request meanwhile.
async fn hand_over_detached(
    ctx: &Ctx,
    flow: Flow,
    identity: ProviderIdentity,
    tokens: Tokens,
    way_in: Option<WayInLink>,
) -> Result<Response> {
    let ctx = ctx.clone();
    tokio::spawn(async move {
        let issuer = ctx.issuer()?.clone();
        let upstream = super::sign_in::configured(&issuer, flow.provider)?;
        Ok(hand_over(&ctx, upstream, flow, identity, tokens, way_in).await)
    })
    .await
    .map_err(campbooks_kernel::Error::internal)?
}

/// Hand the mailbox grant of a connect or reconnect to the connectors
/// module; where the browser goes next. `way_in` is the person's way in of
/// that account: on success its record is told which connection the
/// mailbox became; on any failure a record this round trip wrote is taken
/// back, and a way in it created goes with it, since nothing came of it.
async fn hand_over(
    ctx: &Ctx,
    upstream: &Upstream,
    flow: Flow,
    identity: ProviderIdentity,
    tokens: Tokens,
    way_in: Option<WayInLink>,
) -> Response {
    let provider = flow.provider;
    let (Some(account), Some(workspace)) = (flow.account_id, flow.workspace_id) else {
        drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
        return back(&flow.return_to, "link_error", "link_expired");
    };
    let mail = MailGrant::of(ctx, provider, &tokens, true);
    let (true, Some(refresh), Some(address)) = (
        mail.granted,
        tokens.refresh_token.clone(),
        identity.email.clone(),
    ) else {
        // Nothing connected: a record or way in this round trip made goes.
        drop_refresh(upstream, &identity.issuer, tokens.refresh_token.as_deref()).await;
        take_back(ctx, way_in, workspace).await;
        return back(&flow.return_to, "link_error", "consent_incomplete");
    };
    let issuer = identity.issuer.clone();
    let grant = ProviderGrant {
        workspace: WorkspaceId::from_uuid(workspace),
        subject: account.to_string(),
        provider: provider.as_str().to_owned(),
        account_subject: identity.subject,
        issuer: identity.issuer,
        address,
        address_verified: identity.email_verified,
        display_name: identity.name,
        scopes: mail.scopes,
        access_token: SecretString::from(tokens.access_token),
        refresh_token: Some(SecretString::from(refresh.clone())),
        expires_in: tokens.expires_in,
        extending: flow.connection_id,
    };
    match ctx.state.grants.adopt(&ctx.state, grant).await {
        Ok(connection) => {
            if let Some(way_in) = way_in.filter(|w| w.recorded.tracked()) {
                name_connection(ctx, way_in.id, workspace, &connection).await;
            }
            back(&flow.return_to, "connected", &connection)
        }
        Err(error) => {
            // Refused: the token is not kept anywhere, and no connection
            // came of it, so neither does a mailbox record.
            drop_refresh(upstream, &issuer, Some(&refresh)).await;
            take_back(ctx, way_in, workspace).await;
            tracing::info!(
                provider = provider.as_str(),
                purpose = flow.purpose.as_str(),
                code = error.code(),
                "the connectors module refused a mailbox grant"
            );
            back(&flow.return_to, "link_error", error.code())
        }
    }
}

/// The hand-over failed: take back the mailbox record this round trip
/// wrote, and the way in it created (announced as removed). If that fails,
/// the row stays unnamed, and the daily sweep does the same.
async fn take_back(ctx: &Ctx, way_in: Option<WayInLink>, workspace: Uuid) {
    let Some(way_in) = way_in.filter(|w| w.recorded == Recorded::Inserted) else {
        return;
    };
    let now = Utc::now();
    let taken = async {
        let mut tx = ctx.state.db.begin().await?;
        let gone =
            mailbox_links::take_back(&mut tx, way_in.id, workspace, way_in.created, now).await?;
        if let Some(gone) = &gone {
            announce_removed(&ctx.state, &mut tx, gone, "connect_failed", now).await?;
        }
        tx.commit().await?;
        Ok::<(), campbooks_kernel::Error>(())
    }
    .await;
    if let Err(error) = taken {
        tracing::warn!(%error, "could not take back what a failed hand-over wrote");
    }
}

/// Name the connection the mailbox became (the fast path; the connectors
/// module's `connection_added` does it durably). Written if the row went
/// missing meanwhile.
async fn name_connection(ctx: &Ctx, way_in: Uuid, workspace: Uuid, connection: &str) {
    let named = async {
        let mut conn = ctx.state.db.acquire().await?;
        mailbox_links::name_connection(&mut conn, way_in, workspace, connection, Utc::now()).await
    }
    .await;
    if let Err(error) = named {
        tracing::warn!(%error, "could not note which connection a mailbox became");
    }
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
