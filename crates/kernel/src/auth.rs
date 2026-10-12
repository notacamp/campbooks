//! Authentication and authorization types — placeholder.
//!
//! The API is an **OAuth 2.1 resource server** (see `docs/adr/0004`): every
//! request to `/api/v1/*` carries a JWT issued by the instance's OIDC identity
//! provider, validated against the IdP's JWKS (issuer, audience, expiry,
//! signature). Handlers then require a [`Scope`] through an extractor
//! (`kernel::auth::Scoped<S>`), and `cargo xtask arch` will reject endpoints
//! without one.
//!
//! **Not implemented yet.** Only the shapes the rest of the code can already
//! name live here; JWT validation lands with the IdP spike.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// An OAuth scope, e.g. `mail:read`.
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

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The authenticated caller of a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// The token's `sub` claim: a user, or a client for `client_credentials`.
    pub subject: String,
    /// Scopes granted to the token.
    pub scopes: BTreeSet<Scope>,
}

impl Principal {
    /// Whether the token grants `scope`.
    pub fn has_scope(&self, scope: &Scope) -> bool {
        self.scopes.contains(scope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_scope() {
        let principal = Principal {
            subject: "user-1".into(),
            scopes: [Scope::new("mail:read")].into_iter().collect(),
        };
        assert!(principal.has_scope(&Scope::new("mail:read")));
        assert!(!principal.has_scope(&Scope::new("mail:send")));
    }
}
