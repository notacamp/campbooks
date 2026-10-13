//! Authentication and authorization: the API is an **OAuth 2.1 resource
//! server** (ADR 0004).
//!
//! - [`TokenVerifier`] validates JWT access tokens issued by the instance's
//!   identity provider: signature against the IdP's JWKS (cached, refreshed on
//!   rotation), `alg` allowlist, issuer, audience, expiry. It is configured by
//!   [`AuthConfig`](crate::config::AuthConfig) and lives in
//!   [`AppState`](crate::AppState).
//! - A scope is declared **once**, as a type implementing [`ScopeName`]; a
//!   module lists its scopes in its [`Module`](crate::Module) descriptor and
//!   the server builds the catalogue (`campbooks-server scopes`) from them.
//! - Handlers require a scope with the [`Scoped<S>`] extractor, which yields
//!   the caller's [`Principal`]:
//!
//! ```text
//! pub async fn handler(Scoped { principal, .. }: Scoped<MailRead>) -> Result<Json<…>> { … }
//! ```
//!
//! The extractor answers *"may this token call this endpoint?"* only. Whether
//! the principal may see a particular record is the module's job, and a read
//! it may not see answers 404 ([`Error::NotFound`](crate::Error::NotFound)),
//! never 403.
//!
//! Tokens are never logged.

mod challenge;
mod jwks;
mod oidc;
mod scoped;
mod verifier;

use std::{borrow::Borrow, collections::BTreeSet};

use serde::{Deserialize, Serialize};

pub use challenge::challenge_with_resource_metadata;
pub use jwks::{KeySource, KeySourceFuture};
pub use oidc::{BACKCHANNEL_LOGOUT_EVENT, IdToken, LogoutToken};
pub use scoped::Scoped;
pub(crate) use scoped::bearer_token;
pub use verifier::TokenVerifier;

/// An OAuth scope a token carries, e.g. `mail:read`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Scope(String);

impl Scope {
    /// A scope from its wire name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The wire name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Scope {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A scope an endpoint can require, declared once as a type:
///
/// ```
/// use campbooks_kernel::auth::{ScopeInfo, ScopeName};
///
/// /// Read mail.
/// pub struct MailRead;
/// impl ScopeName for MailRead {
///     const NAME: &'static str = "mail:read";
///     const DESCRIPTION: &'static str = "Read your mail.";
/// }
///
/// // Listed once per module and handed to the server in its `Module`.
/// pub const ALL: &[ScopeInfo] = &[ScopeInfo::of::<MailRead>()];
/// assert_eq!(ALL[0].name, "mail:read");
/// ```
///
/// `NAME` is `<module>:<action>` (checked by `cargo xtask arch`) and is a
/// wire contract with the IdP and every client: never rename it.
pub trait ScopeName: Send + Sync + 'static {
    /// Wire name, `<module>:<action>`.
    const NAME: &'static str;
    /// One sentence a person granting the scope understands.
    const DESCRIPTION: &'static str;
}

/// One entry of the scope catalogue, as modules declare it and
/// `campbooks-server scopes` prints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ScopeInfo {
    /// Wire name.
    pub name: &'static str,
    /// What granting it allows.
    pub description: &'static str,
}

impl ScopeInfo {
    /// The catalogue entry of scope `S`.
    pub const fn of<S: ScopeName>() -> Self {
        Self {
            name: S::NAME,
            description: S::DESCRIPTION,
        }
    }
}

/// Who a token was issued to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    /// A person, signed in through a client (authorization code flow).
    User,
    /// A client acting on its own behalf (`client_credentials`).
    Client,
}

impl PrincipalKind {
    /// `user` or `client`, as telemetry records it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Client => "client",
        }
    }
}

/// The authenticated caller of a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// Stable id of the caller: the token's `sub`, or for `client_credentials`
    /// tokens (which carry no `sub`) the client id from `azp`.
    pub subject: String,
    /// Whether the caller is a user or a client.
    pub kind: PrincipalKind,
    /// The client the token was issued to (`azp`), when the token names it.
    pub client_id: Option<String>,
    /// Scopes granted to the token (its space-separated `scope` claim).
    pub scopes: BTreeSet<Scope>,
    /// The workspace an external client is bound to (`workspace_id`, at the
    /// token root or under `custom`). `None` for users: their workspaces are
    /// resolved by Campbooks from `subject`.
    pub workspace: Option<String>,
    /// The user's email address (`email`), when the token carries one.
    /// Always `None` for clients.
    pub email: Option<String>,
    /// Whether the identity provider verified [`email`](Self::email)
    /// (`email_verified`). `false` when absent.
    pub email_verified: bool,
    /// The user's display name (`name`, else `preferred_username`), when the
    /// token carries one. Always `None` for clients.
    pub name: Option<String>,
    /// When the access token expires (`exp`); `None` for a principal
    /// authenticated in-process without a token. Long-lived responses (the
    /// change feed) end by then, so the caller is authorized again.
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// When the user last authenticated (`auth_time`), when the token says.
    /// Operations that change how someone signs in ask for a recent one
    /// (step-up), and admin operations require one
    /// ([`Admin`](crate::admin::Admin)). Always `None` for clients and
    /// in-process principals.
    pub auth_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl Principal {
    /// Whether the token grants the scope named `scope`.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.contains(scope)
    }
}

/// A caller already authenticated in-process, attached to a request's
/// extensions by the kernel only: by [`invoke`](crate::operations::invoke)
/// (automation and the assistant act for a principal without a token) and
/// by the idempotency layer (which verifies the token once, before the
/// handler). [`Scoped`] uses it instead of the `Authorization` header and
/// still checks the scope. Nothing outside the process can set request
/// extensions, and the type is private to the kernel, so a module cannot
/// forge one either.
#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedPrincipal(pub(crate) Principal);

/// Why a request carries no usable access token. Each reason's
/// [`code`](Self::code) is shown to the client as the problem `detail`; it
/// never says more than that (no claim values, no key ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFailure {
    /// The server has no identity provider configured
    /// (`CAMPBOOKS_AUTH__ISSUER` unset), so no token can be accepted.
    NotConfigured,
    /// No `Authorization: Bearer` header.
    MissingToken,
    /// The token is malformed, wrongly signed, or for another issuer or
    /// audience.
    InvalidToken,
    /// The token was valid but has expired.
    ExpiredToken,
    /// The token is valid, but the person signed in too long ago for an
    /// admin operation ([`Admin`](crate::admin::Admin)): sign in again
    /// (RFC 9470 step-up, `max_age` 12 hours).
    AdminReauthRequired,
}

impl AuthFailure {
    /// Machine-readable reason, sent as the problem `detail`.
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotConfigured => "auth_not_configured",
            Self::MissingToken => "missing_token",
            Self::InvalidToken => "invalid_token",
            Self::ExpiredToken => "token_expired",
            Self::AdminReauthRequired => "admin_reauth_required",
        }
    }

    /// The `WWW-Authenticate` value. RFC 6750 §3.1: a request with no
    /// credentials gets a bare challenge; a bad token gets `invalid_token`.
    pub const fn challenge(self) -> &'static str {
        match self {
            Self::NotConfigured | Self::MissingToken => "Bearer",
            Self::InvalidToken | Self::ExpiredToken => r#"Bearer error="invalid_token""#,
            Self::AdminReauthRequired => {
                r#"Bearer error="insufficient_user_authentication", max_age=43200"#
            }
        }
    }
}

#[cfg(test)]
mod tests;
