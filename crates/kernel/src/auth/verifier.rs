//! JWT access-token validation.

use std::{collections::BTreeSet, sync::Arc};

use axum::extract::FromRef;
use jsonwebtoken::{Algorithm, Validation, errors::ErrorKind};
use serde::Deserialize;

use super::{AuthFailure, Principal, PrincipalKind, Scope, jwks::Jwks};
use crate::{
    AppState, Error,
    clock::{Clock, SystemClock},
    config::AuthConfig,
};

/// Signature algorithms accepted on access tokens. Everything else (`none`,
/// HMAC, …) is rejected before any key is looked up.
pub(super) const ALLOWED_ALGORITHMS: [Algorithm; 2] = [Algorithm::EdDSA, Algorithm::RS256];

/// Header `typ` values accepted when present (RFC 9068 §2.1; compared without
/// regard to case, with or without the `application/` prefix).
const ALLOWED_TYPES: [&str; 2] = ["jwt", "at+jwt"];

/// Validates access tokens against the configured identity provider. Cheap to
/// clone; clones share the signing-key cache.
#[derive(Clone)]
pub struct TokenVerifier {
    inner: Option<Arc<Inner>>,
}

struct Inner {
    issuer: String,
    audience: String,
    leeway_seconds: u64,
    keys: Jwks,
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
            })
        });
        Self { inner }
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
        let invalid = || Error::Unauthorized(AuthFailure::InvalidToken);

        let header = jsonwebtoken::decode_header(token).map_err(|_| invalid())?;
        if !ALLOWED_ALGORITHMS.contains(&header.alg) {
            return Err(invalid());
        }
        if let Some(typ) = &header.typ
            && !is_allowed_type(typ)
        {
            return Err(invalid());
        }
        let kid = header.kid.as_deref().ok_or_else(invalid)?;
        let key = self.keys.key(kid, header.alg).await?.ok_or_else(invalid)?;

        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        validation.validate_nbf = true;
        validation.leeway = self.leeway_seconds;

        let claims = jsonwebtoken::decode::<Claims>(token, &key, &validation)
            .map_err(|err| match err.kind() {
                ErrorKind::ExpiredSignature => Error::Unauthorized(AuthFailure::ExpiredToken),
                _ => invalid(),
            })?
            .claims;

        if claims
            .iat
            .is_some_and(|iat| iat > jsonwebtoken::get_current_timestamp() + self.leeway_seconds)
        {
            return Err(invalid());
        }
        claims.into_principal().ok_or_else(invalid)
    }
}

fn is_allowed_type(typ: &str) -> bool {
    let typ = typ.to_ascii_lowercase();
    let typ = typ.strip_prefix("application/").unwrap_or(&typ);
    ALLOWED_TYPES.contains(&typ)
}

/// The claims Campbooks reads; `exp`, `nbf`, `iss` and `aud` are checked by
/// `jsonwebtoken` itself.
#[derive(Debug, Deserialize)]
struct Claims {
    sub: Option<String>,
    azp: Option<String>,
    scope: Option<String>,
    iat: Option<u64>,
    workspace_id: Option<String>,
    custom: Option<CustomClaims>,
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
    fn token_types() {
        for typ in ["JWT", "jwt", "at+jwt", "AT+JWT", "application/at+jwt"] {
            assert!(is_allowed_type(typ), "{typ}");
        }
        for typ in ["", "JOSE", "id_token+jwt", "application/json"] {
            assert!(!is_allowed_type(typ), "{typ}");
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
