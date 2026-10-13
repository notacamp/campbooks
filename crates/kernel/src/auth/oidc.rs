//! OIDC tokens a relying party receives from the identity
//! provider: ID tokens (sign-in) and back-channel logout tokens. They are
//! checked with the same signing-key cache, issuer and leeway as access
//! tokens, but for the relying party's own client id as audience.
//!
//! The server's web BFF is the relying party (ADR 0004); the API itself never
//! accepts these tokens.

use serde::Deserialize;

use super::{AuthFailure, TokenVerifier};
use crate::Error;

/// Header `typ` values accepted on ID tokens when present.
const ID_TOKEN_TYPES: [&str; 1] = ["jwt"];

/// Header `typ` values accepted on logout tokens when present
/// (OIDC Back-Channel Logout 1.0 §2.4 recommends `logout+jwt`).
const LOGOUT_TOKEN_TYPES: [&str; 2] = ["jwt", "logout+jwt"];

/// The `events` member every logout token carries.
pub const BACKCHANNEL_LOGOUT_EVENT: &str = "http://schemas.openid.net/event/backchannel-logout";

/// A validated ID token: who signed in, as the identity provider says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdToken {
    /// The user's stable id at the IdP (`sub`).
    pub subject: String,
    /// The IdP's session id (`sid`), used to match back-channel logouts.
    pub session_id: Option<String>,
    /// A display name: `name`, else `given_name family_name`, else
    /// `preferred_username`.
    pub name: Option<String>,
    /// The user's email address, if the IdP shares it.
    pub email: Option<String>,
}

/// A validated back-channel logout token: end the sessions it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoutToken {
    /// End every session of this user (when no `session_id` is given).
    pub subject: Option<String>,
    /// End the sessions started in this IdP session.
    pub session_id: Option<String>,
}

#[derive(Deserialize)]
struct IdClaims {
    sub: Option<String>,
    /// A string or an array; its membership is checked by `decode`.
    aud: serde_json::Value,
    azp: Option<String>,
    nonce: Option<String>,
    sid: Option<String>,
    name: Option<String>,
    given_name: Option<String>,
    family_name: Option<String>,
    preferred_username: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize)]
struct LogoutClaims {
    sub: Option<String>,
    sid: Option<String>,
    nonce: Option<serde_json::Value>,
    events: Option<serde_json::Map<String, serde_json::Value>>,
}

fn non_blank(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

impl TokenVerifier {
    /// Validate an ID token returned to `client_id` for the sign-in that sent
    /// `nonce` (OIDC Core §3.1.3.7): signature, issuer, `aud`
    /// containing `client_id` (and `azp` = `client_id` when present or when
    /// there are several audiences), `exp`, `iat`, and the exact `nonce`.
    ///
    /// Errors are [`Error::Unauthorized`] for any problem with the token, and
    /// [`Error::Unavailable`] when the IdP's signing keys were never fetched.
    pub async fn verify_id_token(
        &self,
        token: &str,
        client_id: &str,
        nonce: &str,
    ) -> Result<IdToken, Error> {
        let invalid = || Error::Unauthorized(AuthFailure::InvalidToken);
        let inner = self
            .inner
            .as_ref()
            .ok_or(Error::Unauthorized(AuthFailure::NotConfigured))?;
        let claims: IdClaims = inner
            .decode(token, client_id, &ID_TOKEN_TYPES, true)
            .await
            .inspect_err(|err| tracing::debug!(error = %err, "ID token rejected"))?;
        if claims.nonce.as_deref() != Some(nonce) {
            return Err(invalid());
        }
        match claims.azp.as_deref() {
            Some(azp) if azp != client_id => return Err(invalid()),
            None if claims.aud.as_array().is_some_and(|aud| aud.len() > 1) => return Err(invalid()),
            _ => {}
        }
        let subject = non_blank(claims.sub).ok_or_else(invalid)?;
        let full_name = match (non_blank(claims.given_name), non_blank(claims.family_name)) {
            (Some(given), Some(family)) => Some(format!("{given} {family}")),
            (given, family) => given.or(family),
        };
        Ok(IdToken {
            subject,
            session_id: non_blank(claims.sid),
            name: non_blank(claims.name)
                .or(full_name)
                .or_else(|| non_blank(claims.preferred_username)),
            email: non_blank(claims.email),
        })
    }

    /// Validate a back-channel logout token sent to `client_id`
    /// (OIDC Back-Channel Logout 1.0 §2.6): signature, issuer,
    /// `aud`, `iat`, `exp`, the logout event, no `nonce`, and a `sub`, a
    /// `sid` or both.
    pub async fn verify_logout_token(
        &self,
        token: &str,
        client_id: &str,
    ) -> Result<LogoutToken, Error> {
        let invalid = || Error::Unauthorized(AuthFailure::InvalidToken);
        let inner = self
            .inner
            .as_ref()
            .ok_or(Error::Unauthorized(AuthFailure::NotConfigured))?;
        let claims: LogoutClaims = inner
            .decode(token, client_id, &LOGOUT_TOKEN_TYPES, true)
            .await
            .inspect_err(|err| tracing::debug!(error = %err, "logout token rejected"))?;
        let is_logout_event = claims.events.as_ref().is_some_and(|events| {
            events
                .get(BACKCHANNEL_LOGOUT_EVENT)
                .is_some_and(serde_json::Value::is_object)
        });
        if claims.nonce.is_some() || !is_logout_event {
            return Err(invalid());
        }
        let token = LogoutToken {
            subject: non_blank(claims.sub),
            session_id: non_blank(claims.sid),
        };
        if token.subject.is_none() && token.session_id.is_none() {
            return Err(invalid());
        }
        Ok(token)
    }
}
