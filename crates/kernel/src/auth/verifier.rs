//! JWT access-token validation.

use std::{collections::BTreeSet, sync::Arc};

use axum::extract::FromRef;
use jsonwebtoken::{Algorithm, Validation, errors::ErrorKind};
use serde::{Deserialize, de::DeserializeOwned};

use super::{
    AuthFailure, Principal, PrincipalKind, Scope,
    jwks::{Jwks, KeySource},
};
use crate::{
    AppState, Error,
    clock::{Clock, SystemClock},
    config::AuthConfig,
};

/// Signature algorithms accepted on access tokens. Everything else (`none`,
/// HMAC, …) is rejected before any key is looked up.
pub(super) const ALLOWED_ALGORITHMS: [Algorithm; 2] = [Algorithm::EdDSA, Algorithm::RS256];

/// Header `typ` values accepted when present (RFC 9068 §2.1; compared without
/// regard to case, with or without the `application/` prefix). An outside
/// identity provider may still label access tokens `JWT` (Rauthy 0.36).
const ALLOWED_TYPES: [&str; 2] = ["jwt", "at+jwt"];

/// The only header `typ` accepted, and required, on access tokens of the
/// in-process issuer: its ID tokens (`JWT`) can never pass as access tokens.
const STRICT_TYPES: [&str; 1] = ["at+jwt"];

/// Validates access tokens against the configured identity provider. Cheap to
/// clone; clones share the signing-key cache.
#[derive(Clone)]
pub struct TokenVerifier {
    pub(super) inner: Option<Arc<Inner>>,
}

pub(super) struct Inner {
    pub(super) issuer: String,
    audience: String,
    leeway_seconds: u64,
    keys: Jwks,
    /// Access tokens must say `typ: at+jwt` (the in-process issuer).
    strict_type: bool,
}

impl std::fmt::Debug for TokenVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("TokenVerifier");
        match &self.inner {
            Some(inner) => debug
                .field("issuer", &inner.issuer)
                .field("audience", &inner.audience)
                .finish_non_exhaustive(),
            None => debug.field("issuer", &"<not configured>").finish(),
        }
    }
}

impl TokenVerifier {
    /// A verifier for `config`; with no issuer configured it rejects every
    /// token with [`AuthFailure::NotConfigured`].
    pub fn new(config: &AuthConfig) -> Self {
        Self::with_clock(config, Arc::new(SystemClock))
    }

    /// Like [`new`](Self::new), timing the signing-key cache by `clock` (tests
    /// move a `FixedClock` to expire it). Token `exp`/`nbf`/`iat` are always
    /// checked against the system clock.
    pub fn with_clock(config: &AuthConfig, clock: Arc<dyn Clock>) -> Self {
        let inner = config.issuer().map(|issuer| {
            Arc::new(Inner {
                issuer: issuer.to_owned(),
                audience: config.audience.trim().to_owned(),
                leeway_seconds: config.leeway_seconds,
                keys: Jwks::new(issuer, config.jwks_url(), clock),
                strict_type: false,
            })
        });
        Self { inner }
    }

    /// A verifier for tokens of `issuer`, an issuer running in this process
    /// whose signing keys `keys` hands over directly. Audience and leeway
    /// come from `config`; its issuer and JWKS URL are not used. Access
    /// tokens must carry `typ: at+jwt`.
    pub fn in_process(config: &AuthConfig, issuer: &str, keys: Arc<dyn KeySource>) -> Self {
        Self::in_process_with_clock(config, issuer, keys, Arc::new(SystemClock))
    }

    /// Like [`in_process`](Self::in_process), timing the key cache by `clock`.
    pub fn in_process_with_clock(
        config: &AuthConfig,
        issuer: &str,
        keys: Arc<dyn KeySource>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            inner: Some(Arc::new(Inner {
                issuer: issuer.to_owned(),
                audience: config.audience.trim().to_owned(),
                leeway_seconds: config.leeway_seconds,
                keys: Jwks::local(issuer, keys, clock),
                strict_type: true,
            })),
        }
    }

    /// The issuer this verifier trusts, when one is configured.
    pub fn issuer(&self) -> Option<&str> {
        self.inner.as_ref().map(|inner| inner.issuer.as_str())
    }

    /// Whether an identity provider is configured.
    pub fn is_configured(&self) -> bool {
        self.inner.is_some()
    }

    /// Validate `token` and return who it was issued to.
    ///
    /// Errors are [`Error::Unauthorized`] for any problem with the token, and
    /// [`Error::Unavailable`] when the IdP's signing keys have never been
    /// fetched successfully (the token may well be fine).
    pub async fn verify(&self, token: &str) -> Result<Principal, Error> {
        let Some(inner) = &self.inner else {
            return Err(Error::Unauthorized(AuthFailure::NotConfigured));
        };
        inner.verify(token).await.inspect_err(|err| {
            // The reason only; never the token or its claims.
            tracing::debug!(error = %err, "access token rejected");
        })
    }
}

impl FromRef<AppState> for TokenVerifier {
    fn from_ref(state: &AppState) -> Self {
        state.auth.clone()
    }
}

impl Inner {
    async fn verify(&self, token: &str) -> Result<Principal, Error> {
        if self.strict_type {
            let typ = jsonwebtoken::decode_header(token)
                .ok()
                .and_then(|header| header.typ);
            if !typ.is_some_and(|typ| is_allowed_type(&typ, &STRICT_TYPES)) {
                return Err(Error::Unauthorized(AuthFailure::InvalidToken));
            }
        }
        let claims: Claims = self
            .decode(token, &self.audience, &ALLOWED_TYPES, false)
            .await?;
        claims
            .into_principal()
            .ok_or(Error::Unauthorized(AuthFailure::InvalidToken))
    }

    /// Decode `token` signed by one of the IdP's keys, issued by the
    /// configured issuer for `audience`, with header `typ` (when present) one
    /// of `allowed_types`, unexpired (`exp` required), and not issued in the
    /// future (`iat` required when `require_iat`). Errors are
    /// [`Error::Unauthorized`], or [`Error::Unavailable`] when the signing
    /// keys could never be fetched.
    pub(super) async fn decode<T: DeserializeOwned>(
        &self,
        token: &str,
        audience: &str,
        allowed_types: &[&str],
        require_iat: bool,
    ) -> Result<T, Error> {
        self.decode_from(token, &self.issuer, audience, allowed_types, require_iat)
            .await
    }

    /// [`decode`](Self::decode) for tokens of `issuer` instead of the
    /// configured one, still signed by one of this verifier's keys (a
    /// multi-tenant provider publishes one key set for many issuers).
    pub(super) async fn decode_from<T: DeserializeOwned>(
        &self,
        token: &str,
        issuer: &str,
        audience: &str,
        allowed_types: &[&str],
        require_iat: bool,
    ) -> Result<T, Error> {
        let invalid = || Error::Unauthorized(AuthFailure::InvalidToken);

        let header = jsonwebtoken::decode_header(token).map_err(|_| invalid())?;
        if !ALLOWED_ALGORITHMS.contains(&header.alg) {
            return Err(invalid());
        }
        if let Some(typ) = &header.typ
            && !is_allowed_type(typ, allowed_types)
        {
            return Err(invalid());
        }
        let kid = header.kid.as_deref().ok_or_else(invalid)?;
        let key = self.keys.key(kid, header.alg).await?.ok_or_else(invalid)?;

        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[issuer]);
        validation.set_audience(&[audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        validation.validate_nbf = true;
        validation.leeway = self.leeway_seconds;

        let claims = jsonwebtoken::decode::<IssuedAt<T>>(token, &key, &validation)
            .map_err(|err| match err.kind() {
                ErrorKind::ExpiredSignature => Error::Unauthorized(AuthFailure::ExpiredToken),
                _ => invalid(),
            })?
            .claims;

        match claims.iat {
            Some(iat) if iat > jsonwebtoken::get_current_timestamp() + self.leeway_seconds => {
                Err(invalid())
            }
            None if require_iat => Err(invalid()),
            _ => Ok(claims.rest),
        }
    }
}

/// Every token's `iat`, checked against the clock, next to the claims the
/// caller reads.
#[derive(Debug, Deserialize)]
struct IssuedAt<T> {
    iat: Option<u64>,
    #[serde(flatten)]
    rest: T,
}

/// Whether header `typ` is one of `allowed` (compared without regard to case,
/// with or without the `application/` prefix).
pub(super) fn is_allowed_type(typ: &str, allowed: &[&str]) -> bool {
    let typ = typ.to_ascii_lowercase();
    let typ = typ.strip_prefix("application/").unwrap_or(&typ);
    allowed.contains(&typ)
}

/// The claims Campbooks reads; `exp`, `nbf`, `iss` and `aud` are checked by
/// `jsonwebtoken` itself.
#[derive(Debug, Deserialize)]
struct Claims {
    exp: Option<i64>,
    auth_time: Option<i64>,
    sub: Option<String>,
    azp: Option<String>,
    scope: Option<String>,
    workspace_id: Option<String>,
    custom: Option<CustomClaims>,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
    preferred_username: Option<String>,
}

/// Per-client claims an IdP may nest under `custom` instead of the root.
#[derive(Debug, Deserialize)]
struct CustomClaims {
    workspace_id: Option<String>,
}

impl Claims {
    /// `None` when the token names no subject at all, or a client token names
    /// two different workspaces.
    fn into_principal(self) -> Option<Principal> {
        let azp = self.azp.filter(|azp| !azp.is_empty());
        let sub = self.sub.filter(|sub| !sub.is_empty());
        let kind = match (&sub, &azp) {
            (Some(sub), Some(azp)) if sub == azp => PrincipalKind::Client,
            (Some(_), _) => PrincipalKind::User,
            (None, _) => PrincipalKind::Client,
        };
        // Only a client's workspace comes from its token: an admin sets a
        // client's claims. A user's custom claims can come from attributes the
        // user may edit, so a user's workspaces always come from membership.
        let workspace = match kind {
            PrincipalKind::User => None,
            PrincipalKind::Client => {
                let custom = self.custom.and_then(|custom| custom.workspace_id);
                match (self.workspace_id, custom) {
                    (Some(root), Some(custom)) if root != custom => return None,
                    (root, custom) => root.or(custom),
                }
            }
        };
        // Identity claims describe a person; a client has none.
        let (email, email_verified, name) = match kind {
            PrincipalKind::User => (
                self.email
                    .map(|e| e.trim().to_owned())
                    .filter(|e| !e.is_empty()),
                self.email_verified.unwrap_or(false),
                self.name
                    .or(self.preferred_username)
                    .map(|n| n.trim().to_owned())
                    .filter(|n| !n.is_empty()),
            ),
            PrincipalKind::Client => (None, false, None),
        };
        let subject = sub.or_else(|| azp.clone())?;
        let scopes: BTreeSet<Scope> = self
            .scope
            .unwrap_or_default()
            .split_whitespace()
            .map(Scope::new)
            .collect();
        Some(Principal {
            subject,
            kind,
            client_id: azp,
            scopes,
            workspace,
            email,
            email_verified,
            name,
            expires_at: self
                .exp
                .and_then(|exp| chrono::DateTime::from_timestamp(exp, 0)),
            auth_time: match kind {
                PrincipalKind::User => self
                    .auth_time
                    .and_then(|at| chrono::DateTime::from_timestamp(at, 0)),
                PrincipalKind::Client => None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(json: serde_json::Value) -> Claims {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn user_tokens_use_sub() {
        let principal = claims(serde_json::json!({
            "sub": "user-1", "azp": "campbooks-web", "scope": "openid system:read"
        }))
        .into_principal()
        .unwrap();
        assert_eq!(principal.subject, "user-1");
        assert_eq!(principal.kind, PrincipalKind::User);
        assert_eq!(principal.client_id.as_deref(), Some("campbooks-web"));
        assert!(principal.has_scope("system:read"));
        assert!(principal.has_scope("openid"));
        assert!(!principal.has_scope("mail:read"));
        assert_eq!(principal.workspace, None);
    }

    #[test]
    fn a_users_auth_time_is_kept_and_a_clients_is_not() {
        let user = claims(serde_json::json!({
            "sub": "user-1", "azp": "campbooks-web", "auth_time": 1_700_000_000
        }))
        .into_principal()
        .unwrap();
        assert_eq!(
            user.auth_time,
            chrono::DateTime::from_timestamp(1_700_000_000, 0)
        );
        let none = claims(serde_json::json!({ "sub": "user-1" }))
            .into_principal()
            .unwrap();
        assert_eq!(none.auth_time, None);
        let client = claims(serde_json::json!({ "azp": "acme-sync", "auth_time": 1_700_000_000 }))
            .into_principal()
            .unwrap();
        assert_eq!(client.auth_time, None);
    }

    #[test]
    fn client_credentials_use_azp() {
        let principal = claims(serde_json::json!({ "azp": "acme-sync", "scope": "" }))
            .into_principal()
            .unwrap();
        assert_eq!(principal.subject, "acme-sync");
        assert_eq!(principal.kind, PrincipalKind::Client);
        assert!(principal.scopes.is_empty());

        let same = claims(serde_json::json!({ "sub": "acme-sync", "azp": "acme-sync" }))
            .into_principal()
            .unwrap();
        assert_eq!(same.kind, PrincipalKind::Client);
    }

    #[test]
    fn no_subject_at_all_is_rejected() {
        assert!(
            claims(serde_json::json!({ "scope": "x" }))
                .into_principal()
                .is_none()
        );
        assert!(
            claims(serde_json::json!({ "sub": "", "azp": "" }))
                .into_principal()
                .is_none()
        );
    }

    #[test]
    fn workspace_from_root_or_custom() {
        let root = claims(serde_json::json!({ "azp": "c", "workspace_id": "w1" }));
        assert_eq!(
            root.into_principal().unwrap().workspace.as_deref(),
            Some("w1")
        );
        let custom = claims(serde_json::json!({ "azp": "c", "custom": { "workspace_id": "w2" } }));
        assert_eq!(
            custom.into_principal().unwrap().workspace.as_deref(),
            Some("w2")
        );
        let both = claims(serde_json::json!({
            "azp": "c", "workspace_id": "w1", "custom": { "workspace_id": "w1" }
        }));
        assert_eq!(
            both.into_principal().unwrap().workspace.as_deref(),
            Some("w1")
        );
        let conflicting = claims(serde_json::json!({
            "azp": "c", "workspace_id": "w1", "custom": { "workspace_id": "w2" }
        }));
        assert!(conflicting.into_principal().is_none());
    }

    #[test]
    fn users_never_get_a_workspace_from_their_token() {
        let user = claims(serde_json::json!({
            "sub": "user-1", "azp": "campbooks-web",
            "workspace_id": "w1", "custom": { "workspace_id": "w2" }
        }))
        .into_principal()
        .unwrap();
        assert_eq!(user.kind, PrincipalKind::User);
        assert_eq!(user.workspace, None);
    }

    #[test]
    fn identity_claims_only_for_users() {
        let user = claims(serde_json::json!({
            "sub": "user-1", "azp": "campbooks-web", "email": " ana@example.com ",
            "email_verified": true, "preferred_username": "ana"
        }))
        .into_principal()
        .unwrap();
        assert_eq!(user.email.as_deref(), Some("ana@example.com"));
        assert!(user.email_verified);
        assert_eq!(user.name.as_deref(), Some("ana"));

        let named = claims(serde_json::json!({
            "sub": "user-1", "name": "Ana Lima", "preferred_username": "ana", "email": ""
        }))
        .into_principal()
        .unwrap();
        assert_eq!(named.name.as_deref(), Some("Ana Lima"));
        assert_eq!(named.email, None);
        assert!(!named.email_verified);

        let client = claims(serde_json::json!({
            "azp": "c", "email": "bot@example.com", "email_verified": true, "name": "Bot"
        }))
        .into_principal()
        .unwrap();
        assert_eq!(
            (client.email, client.email_verified, client.name),
            (None, false, None)
        );
    }

    #[test]
    fn token_types() {
        for typ in ["JWT", "jwt", "at+jwt", "AT+JWT", "application/at+jwt"] {
            assert!(is_allowed_type(typ, &ALLOWED_TYPES), "{typ}");
        }
        for typ in ["", "JOSE", "id_token+jwt", "application/json"] {
            assert!(!is_allowed_type(typ, &ALLOWED_TYPES), "{typ}");
        }
    }

    #[tokio::test]
    async fn unconfigured_rejects_everything() {
        let verifier = TokenVerifier::new(&AuthConfig::default());
        assert!(!verifier.is_configured());
        assert!(matches!(
            verifier.verify("a.b.c").await,
            Err(Error::Unauthorized(AuthFailure::NotConfigured))
        ));
        assert!(format!("{verifier:?}").contains("not configured"));
    }
}
