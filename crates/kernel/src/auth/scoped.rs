//! The `Scoped<S>` handler extractor.

use std::marker::PhantomData;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{HeaderMap, header, request::Parts},
};

use super::{AuthFailure, Principal, ScopeName, TokenVerifier};
use crate::Error;

/// Requires a valid access token that grants scope `S`, and yields its
/// [`Principal`].
///
/// - no identity provider configured → 401 `auth_not_configured`
/// - no `Authorization: Bearer` header → 401 `missing_token`
/// - malformed, wrongly signed, foreign or expired token → 401 with
///   `WWW-Authenticate: Bearer error="invalid_token"`
/// - valid token without `S` → 403 with
///   `WWW-Authenticate: Bearer error="insufficient_scope", scope="<S>"`
///
/// Every operation using it declares `security(("bearer" = ["<S>"]))` in its
/// `#[utoipa::path]`; `cargo xtask arch` rejects operations that declare no
/// security at all.
pub struct Scoped<S: ScopeName> {
    /// The authenticated caller.
    pub principal: Principal,
    scope: PhantomData<fn() -> S>,
}

impl<S: ScopeName> std::fmt::Debug for Scoped<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scoped")
            .field("scope", &S::NAME)
            .field("principal", &self.principal)
            .finish()
    }
}

impl<S, St> FromRequestParts<St> for Scoped<S>
where
    S: ScopeName,
    St: Send + Sync,
    TokenVerifier: FromRef<St>,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &St) -> Result<Self, Self::Rejection> {
        let principal = authorize(&parts.headers, &TokenVerifier::from_ref(state), S::NAME).await?;
        Ok(Self {
            principal,
            scope: PhantomData,
        })
    }
}

/// What [`Scoped<S>`] does, for a scope known only at run time (the action
/// registry's generated endpoints): authenticate the request's bearer token
/// and require `scope`, with the same 401/403 answers.
pub(crate) async fn authorize(
    headers: &HeaderMap,
    verifier: &TokenVerifier,
    scope: &'static str,
) -> Result<Principal, Error> {
    let principal = authenticate(headers, verifier).await?;
    if !principal.has_scope(scope) {
        return Err(Error::InsufficientScope(scope));
    }
    Ok(principal)
}

/// The principal of the request's valid bearer token, whatever its scopes.
pub(crate) async fn authenticate(
    headers: &HeaderMap,
    verifier: &TokenVerifier,
) -> Result<Principal, Error> {
    if !verifier.is_configured() {
        return Err(Error::Unauthorized(AuthFailure::NotConfigured));
    }
    let token = bearer_token(headers)?;
    verifier.verify(token).await
}

/// The token of the request's single `Authorization: Bearer <token>` header.
fn bearer_token(headers: &HeaderMap) -> Result<&str, Error> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let Some(value) = values.next() else {
        return Err(Error::Unauthorized(AuthFailure::MissingToken));
    };
    if values.next().is_some() {
        return Err(Error::Unauthorized(AuthFailure::InvalidToken));
    }
    let value = value
        .to_str()
        .map_err(|_| Error::Unauthorized(AuthFailure::InvalidToken))?;
    let (scheme, token) = value.split_once(' ').unwrap_or((value, ""));
    if !scheme.eq_ignore_ascii_case("bearer") {
        // Some other scheme (Basic, …): no bearer credentials at all.
        return Err(Error::Unauthorized(AuthFailure::MissingToken));
    }
    let token = token.trim();
    if token.is_empty() {
        return Err(Error::Unauthorized(AuthFailure::InvalidToken));
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(values: &[&'static str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(header::AUTHORIZATION, HeaderValue::from_static(value));
        }
        headers
    }

    fn failure(result: Result<&str, Error>) -> AuthFailure {
        match result {
            Err(Error::Unauthorized(failure)) => failure,
            other => panic!("expected an auth failure, got {other:?}"),
        }
    }

    #[test]
    fn extracts_bearer_tokens() {
        assert_eq!(
            bearer_token(&headers(&["Bearer abc.def.ghi"])).unwrap(),
            "abc.def.ghi"
        );
        assert_eq!(bearer_token(&headers(&["bearer  abc "])).unwrap(), "abc");
    }

    #[test]
    fn missing_or_foreign_scheme_is_missing_token() {
        assert_eq!(
            failure(bearer_token(&headers(&[]))),
            AuthFailure::MissingToken
        );
        assert_eq!(
            failure(bearer_token(&headers(&["Basic dXNlcjpwYXNz"]))),
            AuthFailure::MissingToken
        );
    }

    #[test]
    fn empty_or_repeated_bearer_is_invalid() {
        assert_eq!(
            failure(bearer_token(&headers(&["Bearer"]))),
            AuthFailure::InvalidToken
        );
        assert_eq!(
            failure(bearer_token(&headers(&["Bearer   "]))),
            AuthFailure::InvalidToken
        );
        assert_eq!(
            failure(bearer_token(&headers(&["Bearer a", "Bearer b"]))),
            AuthFailure::InvalidToken
        );
    }
}
