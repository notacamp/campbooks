//! Connecting Notion: the browser's round trip to Notion and back
//! (campbooks-cloud#376).
//!
//! Notion's OAuth returns no ID token, so it is never a way in and does not
//! go through the identity module's front door: this module runs the
//! authorization-code flow itself.
//!
//! 1. `connectors.start_notion` (a member, `connectors:write`) records a
//!    consent and answers `/auth/start/notion?ticket=…`: the ticket is the
//!    consent's id sealed with the instance key.
//! 2. `GET /auth/start/notion` opens the ticket, arms the consent once with a
//!    fresh `state`, a PKCE verifier (sealed with the credentials key,
//!    bound to the consent) and a cookie that binds the round trip to **this
//!    browser**, and sends the browser to Notion. Only hashes of the state
//!    and the cookie are kept.
//! 3. `GET /auth/callback/notion` is where Notion sends the browser back.
//!    It needs the `state`, the same browser's cookie and an unexpired
//!    consent, deletes the consent in the same statement that checks them
//!    (single use: a replayed callback finds nothing), exchanges the code
//!    with the integration's credentials and hands the grant to the one seam
//!    every connection goes through (`grants::create_or_refresh_connection`).
//!    The browser then goes back to the page that started it with
//!    `connected=<connection>` or `link_error=<code>`.
//!
//! Public and outside the operation catalogue, like the other callbacks:
//! absent unless Notion is offered. Tokens, codes and the state are never
//! logged, and a failure sends the browser back with a short code only.

use std::time::Duration;

use axum::{
    Router,
    extract::{FromRef, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{AppendHeaders, IntoResponse, Redirect, Response},
    routing::get,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{
    AppState, Error,
    crypto::{hash, pkce_challenge, random_token},
    db::BeginFor as _,
    rate_limit::{ClientIp, Limit, RateKey},
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::{
    deps::Deps,
    domain::{
        grants::{ProviderGrant, Secret, create_or_refresh_connection},
        model::{AuthorizationId, Capability, ConnectionId, Provider},
        notion as notion_db,
    },
    notion::{CALLBACK_PATH, START_PATH},
    provider::ProviderError,
};

/// What a sealed ticket or state names: the workspace and the consent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Pointer {
    w: WorkspaceId,
    a: AuthorizationId,
}

/// The context a ticket is sealed for.
const TICKET_CONTEXT: &[u8] = b"connectors.notion_ticket";

/// The context a `state` is sealed for.
const STATE_CONTEXT: &[u8] = b"connectors.notion_state";

/// The cookie that binds a round trip to the browser that started it.
const COOKIE: &str = "cb_notion_flow";

/// Rounds trips one address may start.
const START_LIMIT: Limit = Limit::new("connectors.notion_start", 30, Duration::from_mins(1));

/// Callbacks one address may finish.
const CALLBACK_LIMIT: Limit = Limit::new("connectors.notion_callback", 30, Duration::from_mins(1));

/// The associated data of a consent's sealed PKCE verifier.
fn pkce_context(id: AuthorizationId) -> Vec<u8> {
    format!("connectors.notion_authorizations:pkce_verifier:{id}").into_bytes()
}

/// The ticket for consent `id` of `workspace`: what `/auth/start/notion`
/// takes. Sealed, so it names nothing to the browser that holds it.
pub fn ticket(
    state: &AppState,
    workspace: WorkspaceId,
    id: AuthorizationId,
) -> Result<String, Error> {
    let pointer = Pointer {
        w: workspace,
        a: id,
    };
    let bytes = serde_json::to_vec(&pointer).map_err(Error::internal)?;
    Ok(URL_SAFE_NO_PAD.encode(state.signer.seal(TICKET_CONTEXT, &bytes)))
}

/// What `sealed` (a ticket or a state) names, when it was sealed for
/// `context` by this instance.
fn open(state: &AppState, context: &[u8], sealed: &str) -> Option<Pointer> {
    let bytes = URL_SAFE_NO_PAD.decode(sealed.as_bytes()).ok()?;
    let opened = state.signer.open(context, &bytes)?;
    serde_json::from_slice(&opened).ok()
}

/// Whether `path` is a path of this instance a browser may be sent back to:
/// one `/`, no scheme, no second `/`, no backslash and no control character.
pub fn safe_return_to(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && path.len() <= 256
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
}

/// The routes over `state` and `deps`, ready to merge into the server's
/// application: none (the paths fall through to the application's
/// fallback) unless Notion is offered.
pub fn router(state: AppState, deps: Deps) -> Router {
    let Some(notion) = deps.notion.clone() else {
        return Router::new();
    };
    if deps.cipher.is_none() {
        return Router::new();
    }
    let callback = url::Url::parse(notion.redirect_uri())
        .map_or_else(|_| CALLBACK_PATH.to_owned(), |url| url.path().to_owned());
    Router::new()
        .route(START_PATH, get(start))
        .route(&callback, get(finish))
        .with_state(Flow { state, deps })
}

/// What the routes reach.
#[derive(Clone)]
struct Flow {
    state: AppState,
    deps: Deps,
}

impl FromRef<Flow> for AppState {
    fn from_ref(flow: &Flow) -> Self {
        flow.state.clone()
    }
}

impl Flow {
    /// Whether this address is within `limit`.
    async fn within(&self, limit: &Limit, ip: ClientIp) -> bool {
        self.state
            .rate_limits
            .check(&self.state.db, limit, &RateKey::ip(ip))
            .await
            .is_ok()
    }

    /// The cookie that sets `value`, or ends the round trip's.
    fn cookie(&self, value: &str, max_age: u64) -> String {
        let secure = if self.state.config.public_url.starts_with("https://") {
            "; Secure"
        } else {
            ""
        };
        // Lax: the browser comes back from Notion on a top-level GET, which
        // Lax sends and Strict does not.
        format!("{COOKIE}={value}; Max-Age={max_age}; Path=/auth; HttpOnly; SameSite=Lax{secure}")
    }
}

/// The browser goes back to `path` with `query` (`connected=` or
/// `link_error=`), the round trip's cookie ended.
fn back(flow: &Flow, path: Option<&str>, query: &str) -> Response {
    let path = path.filter(|path| safe_return_to(path)).unwrap_or("/");
    let separator = if path.contains('?') { '&' } else { '?' };
    (
        AppendHeaders([
            (header::SET_COOKIE, flow.cookie("", 0)),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ]),
        Redirect::to(&format!("{path}{separator}{query}")),
    )
        .into_response()
}

/// `/auth/start/notion` parameters.
#[derive(Debug, Deserialize)]
struct StartQuery {
    ticket: Option<String>,
}

/// The browser arrives with its ticket: arm the consent, bind this browser
/// to it, and send it to Notion.
async fn start(
    State(flow): State<Flow>,
    ip: ClientIp,
    Query(query): Query<StartQuery>,
) -> Response {
    if !flow.within(&START_LIMIT, ip).await {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let outcome = flow.arm(query.ticket.as_deref().unwrap_or_default()).await;
    match outcome {
        Ok(Some((url, browser))) => (
            AppendHeaders([
                (header::SET_COOKIE, flow.cookie(&browser, 600)),
                (header::CACHE_CONTROL, "no-store".to_owned()),
            ]),
            Redirect::to(url.as_str()),
        )
            .into_response(),
        Ok(None) => back(&flow, None, "link_error=link_expired"),
        Err(err) => {
            tracing::warn!(
                error = %campbooks_kernel::redact::error(&err),
                "could not start a Notion consent"
            );
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

impl Flow {
    /// Arm the consent `ticket` names; the address to send the browser to
    /// and the value of its binding cookie. `None` for a ticket that is not
    /// ours, was used or has expired.
    async fn arm(&self, ticket: &str) -> Result<Option<(url::Url, String)>, Error> {
        let (Some(notion), Some(cipher)) = (self.deps.notion.as_ref(), self.deps.cipher.as_ref())
        else {
            return Ok(None);
        };
        let Some(pointer) = open(&self.state, TICKET_CONTEXT, ticket) else {
            return Ok(None);
        };
        let bytes = serde_json::to_vec(&pointer).map_err(Error::internal)?;
        let state_token = URL_SAFE_NO_PAD.encode(self.state.signer.seal(STATE_CONTEXT, &bytes));
        let browser = random_token();
        let verifier = random_token();
        let sealed = cipher.seal(&verifier, &pkce_context(pointer.a));
        let (state_hash, browser_hash) = (hash(&state_token), hash(&browser));
        let mut tx = self.state.db.begin_for(pointer.w).await?;
        let armed = notion_db::arm(
            &mut tx,
            pointer.a,
            (state_hash.as_slice(), browser_hash.as_slice()),
            &sealed,
            self.deps.clock.now(),
        )
        .await?;
        tx.commit().await?;
        if !armed {
            return Ok(None);
        }
        let url = notion.authorize_url(&state_token, &pkce_challenge(&verifier));
        Ok(Some((url, browser)))
    }
}

/// What Notion sends the browser back with.
#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// The browser's cookie, if it sent ours.
fn browser_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_owned())
        .filter(|value| !value.is_empty())
}

/// Notion sent the browser back.
async fn finish(
    State(flow): State<Flow>,
    ip: ClientIp,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    if !flow.within(&CALLBACK_LIMIT, ip).await {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    match flow.complete(&headers, &query).await {
        Ok(done) => match done {
            Done::Connected { connection, to } => {
                back(&flow, to.as_deref(), &format!("connected={connection}"))
            }
            Done::Refused { code, to } => back(&flow, to.as_deref(), &format!("link_error={code}")),
        },
        Err(err) => {
            tracing::warn!(
                error = %campbooks_kernel::redact::error(&err),
                "could not finish a Notion consent"
            );
            back(&flow, None, "link_error=provider_unavailable")
        }
    }
}

/// How a callback ended.
enum Done {
    Connected {
        connection: ConnectionId,
        to: Option<String>,
    },
    Refused {
        code: &'static str,
        to: Option<String>,
    },
}

impl Flow {
    /// Finish the consent the browser brings back.
    async fn complete(&self, headers: &HeaderMap, query: &CallbackQuery) -> Result<Done, Error> {
        let refused = |code| Ok(Done::Refused { code, to: None });
        let (Some(notion), Some(cipher)) = (self.deps.notion.as_ref(), self.deps.cipher.as_ref())
        else {
            return refused("provider_not_configured");
        };
        let (Some(state_token), Some(browser)) = (query.state.as_deref(), browser_cookie(headers))
        else {
            return refused("link_expired");
        };
        let Some(pointer) = open(&self.state, STATE_CONTEXT, state_token) else {
            return refused("link_expired");
        };
        let now = self.deps.clock.now();
        let (state_hash, browser_hash) = (hash(state_token), hash(&browser));
        let mut tx = self.state.db.begin_for(pointer.w).await?;
        let found = notion_db::finish(
            &mut tx,
            pointer.a,
            (state_hash.as_slice(), browser_hash.as_slice()),
            now,
        )
        .await?;
        tx.commit().await?;
        let Some(found) = found else {
            return refused("link_expired");
        };
        let to = found.return_to.clone();
        // The member said no at Notion, or Notion sent no code.
        let Some(code) = query.code.as_deref().filter(|_| query.error.is_none()) else {
            return Ok(Done::Refused {
                code: "cancelled_at_provider",
                to,
            });
        };
        let Ok(verifier) = cipher.open(&found.pkce_verifier, &pkce_context(pointer.a)) else {
            return Ok(Done::Refused {
                code: "link_expired",
                to,
            });
        };
        let granted = match notion.exchange(code, &verifier).await {
            Ok(granted) => granted,
            Err(ProviderError::Unavailable(_) | ProviderError::RateLimited { .. }) => {
                return Ok(Done::Refused {
                    code: "provider_unavailable",
                    to,
                });
            }
            Err(_) => {
                return Ok(Done::Refused {
                    code: "login_failed",
                    to,
                });
            }
        };
        let grant = ProviderGrant {
            workspace: pointer.w,
            user: found.user_id,
            provider: Provider::Notion,
            account_subject: granted.bot_id,
            // Notion gives no address: the connection is its Notion
            // workspace, named by `display_name`.
            address: String::new(),
            address_verified: false,
            display_name: granted.workspace_name,
            capabilities: vec![Capability::Pages],
            secret: Secret::Tokens(granted.tokens),
            imap: None,
            extending: found.connection_id,
            grantor_subject: None,
            provider_server: None,
            mailboxes: Vec::new(),
        };
        match create_or_refresh_connection(&self.state, &self.deps, &grant).await {
            Ok(view) => Ok(Done::Connected {
                connection: view.id,
                to,
            }),
            Err(Error::Conflict(reason) | Error::Denied(reason)) => Ok(Done::Refused {
                code: refusal_code(reason.code()),
                to,
            }),
            Err(err) => Err(err),
        }
    }
}

/// The codes the page knows how to word (`link_error=`); anything else is
/// `login_failed`.
fn refusal_code(code: &str) -> &'static str {
    match code {
        "account_mismatch" => "account_mismatch",
        "account_connected_by_another_member" => "account_connected_by_another_member",
        "no_offline_access" => "no_offline_access",
        "provider_not_configured" => "provider_not_configured",
        _ => "login_failed",
    }
}

#[cfg(test)]
mod tests;
