//! The identity provider's signing keys (JWKS), fetched lazily and cached.
//!
//! Rules (see `openspec/changes/api-resource-server/design.md`):
//!
//! - Keys are fetched on first use, from `CAMPBOOKS_AUTH__JWKS_URL` or else
//!   the `jwks_uri` of the issuer's discovery document.
//! - The cache lives as long as the response's `Cache-Control: max-age`
//!   (default [`DEFAULT_TTL`], at most [`MAX_TTL`]).
//! - A token whose `kid` is not in the cache triggers a refresh, so a key
//!   rotation at the IdP needs no restart.
//! - Refreshes are single-flight, and while keys are cached at most one fetch
//!   starts per [`MIN_REFRESH_INTERVAL`], so a flood of tokens with made-up
//!   `kid`s cannot hammer the IdP.
//! - A failed refresh keeps serving the last good keys.

use std::{
    sync::{Arc, PoisonError, RwLock},
    time::Duration,
};

use axum::http::{HeaderMap, header};
use chrono::{DateTime, TimeDelta, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey,
    jwk::{AlgorithmParameters, EllipticCurve, Jwk, KeyAlgorithm, PublicKeyUse},
};
use serde::Deserialize;
use tokio::sync::{Mutex, OnceCell};
use tracing::Instrument as _;

use super::verifier::ALLOWED_ALGORITHMS;
use crate::{Error, clock::Clock, telemetry};

/// Cache lifetime when the JWKS response sets no `max-age`.
pub const DEFAULT_TTL: Duration = Duration::from_hours(1);
/// Longest the keys are cached, whatever the IdP says.
pub const MAX_TTL: Duration = Duration::from_hours(24);
/// Least time between two fetches while keys are cached.
pub const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
/// Timeout for one request to the IdP.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest discovery or JWKS document accepted.
const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;

/// The signing-key cache of one issuer.
pub(super) struct Jwks {
    issuer: String,
    configured_url: Option<String>,
    clock: Arc<dyn Clock>,
    http: OnceCell<reqwest::Client>,
    cache: RwLock<Arc<KeySet>>,
    refresh: Mutex<RefreshState>,
}

/// Bookkeeping of the single refresh in flight; held while fetching.
#[derive(Default)]
struct RefreshState {
    /// When the last fetch started, successful or not.
    last_attempt: Option<DateTime<Utc>>,
    /// JWKS URL learned from the discovery document.
    discovered_url: Option<String>,
}

/// One successfully fetched JWKS, or nothing yet.
#[derive(Default)]
struct KeySet {
    keys: Vec<CachedKey>,
    /// `None` until the first successful fetch.
    expires_at: Option<DateTime<Utc>>,
}

struct CachedKey {
    kid: String,
    alg: Algorithm,
    key: DecodingKey,
}

impl KeySet {
    fn loaded(&self) -> bool {
        self.expires_at.is_some()
    }

    fn find(&self, kid: &str, alg: Algorithm) -> Option<DecodingKey> {
        self.keys
            .iter()
            .find(|key| key.kid == kid && key.alg == alg)
            .map(|key| key.key.clone())
    }
}

impl Jwks {
    pub(super) fn new(issuer: &str, configured_url: Option<&str>, clock: Arc<dyn Clock>) -> Self {
        Self {
            issuer: issuer.to_owned(),
            configured_url: configured_url.map(ToOwned::to_owned),
            clock,
            http: OnceCell::new(),
            cache: RwLock::default(),
            refresh: Mutex::default(),
        }
    }

    /// The key with id `kid` usable for `alg`, refreshing the cache when it is
    /// stale or does not know `kid`. `Ok(None)` means no such key;
    /// [`Error::Unavailable`] means no keys could ever be fetched.
    pub(super) async fn key(
        &self,
        kid: &str,
        alg: Algorithm,
    ) -> Result<Option<DecodingKey>, Error> {
        let requested_at = self.clock.now();
        let mut keys = self.current();
        let stale = keys
            .expires_at
            .is_none_or(|expires| requested_at >= expires);
        if stale || keys.find(kid, alg).is_none() {
            keys = self.refresh(requested_at).await;
        }
        if !keys.loaded() {
            return Err(Error::Unavailable(
                crate::i18n::MESSAGES.reason("idp_unavailable"),
            ));
        }
        Ok(keys.find(kid, alg))
    }

    fn current(&self) -> Arc<KeySet> {
        Arc::clone(&self.cache.read().unwrap_or_else(PoisonError::into_inner))
    }

    async fn refresh(&self, requested_at: DateTime<Utc>) -> Arc<KeySet> {
        let mut state = self.refresh.lock().await;
        let current = self.current();
        // Another request fetched while this one waited for the lock: use
        // its result rather than fetching again.
        if state.last_attempt.is_some_and(|at| at >= requested_at) {
            return current;
        }
        let now = self.clock.now();
        let cooled_down = state
            .last_attempt
            .is_none_or(|at| now - at >= delta(MIN_REFRESH_INTERVAL));
        if current.loaded() && !cooled_down {
            return current;
        }
        state.last_attempt = Some(now);
        match self.fetch(&mut state, now).await {
            Ok(keys) => {
                let keys = Arc::new(keys);
                *self.cache.write().unwrap_or_else(PoisonError::into_inner) = Arc::clone(&keys);
                tracing::info!(issuer = %self.issuer, keys = keys.keys.len(), "identity provider signing keys refreshed");
                keys
            }
            Err(error) => {
                // Rediscover next time in case the IdP moved its JWKS.
                state.discovered_url = None;
                tracing::warn!(
                    issuer = %self.issuer,
                    %error,
                    kept_keys = current.keys.len(),
                    "could not refresh the identity provider's signing keys"
                );
                current
            }
        }
    }

    async fn fetch(
        &self,
        state: &mut RefreshState,
        now: DateTime<Utc>,
    ) -> Result<KeySet, FetchError> {
        let http = self.http.get_or_try_init(build_client).await?;
        let url = match (&self.configured_url, &state.discovered_url) {
            (Some(url), _) | (None, Some(url)) => url.clone(),
            (None, None) => {
                let url = self.discover(http).await?;
                state.discovered_url = Some(url.clone());
                url
            }
        };
        let (headers, body) = get(http, &url).await?;
        let document: JwksDocument = serde_json::from_slice(&body)?;
        let keys: Vec<CachedKey> = document.keys.into_iter().filter_map(usable_key).collect();
        if keys.is_empty() {
            return Err(FetchError::NoUsableKeys);
        }
        Ok(KeySet {
            keys,
            expires_at: Some(now + delta(cache_ttl(&headers))),
        })
    }

    /// The `jwks_uri` of the issuer's OIDC discovery document,
    /// which must name the configured issuer (OIDC Discovery §4.3).
    async fn discover(&self, http: &reqwest::Client) -> Result<String, FetchError> {
        let url = format!(
            "{}/.well-known/openid-configuration",
            self.issuer.trim_end_matches('/')
        );
        let (_, body) = get(http, &url).await?;
        let document: DiscoveryDocument = serde_json::from_slice(&body)?;
        if document.issuer != self.issuer {
            return Err(FetchError::IssuerMismatch(document.issuer));
        }
        Ok(document.jwks_uri)
    }
}

#[derive(Debug, thiserror::Error)]
enum FetchError {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("response larger than {MAX_DOCUMENT_BYTES} bytes")]
    TooLarge,
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("discovery document names issuer {0:?}, not the configured one")]
    IssuerMismatch(String),
    #[error("the JWKS holds no usable EdDSA or RS256 signing key with a `kid`")]
    NoUsableKeys,
}

impl FetchError {
    /// The `error.type` telemetry records.
    fn kind(&self) -> &'static str {
        match self {
            Self::Http(e) => telemetry::http_error_type(e),
            Self::TooLarge | Self::Json(_) | Self::IssuerMismatch(_) | Self::NoUsableKeys => {
                "invalid_response"
            }
        }
    }
}

async fn build_client() -> Result<reqwest::Client, FetchError> {
    Ok(reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("campbooks-server/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

/// GET a document from the IdP, in a client span carrying the trace on.
async fn get(http: &reqwest::Client, url: &str) -> Result<(HeaderMap, Vec<u8>), FetchError> {
    async {
        let result = fetch_document(http, url, telemetry::trace_headers()).await;
        if let Err(e) = &result {
            telemetry::record_error(e.kind());
        }
        result
    }
    .instrument(telemetry::client_span("GET idp", url))
    .await
}

async fn fetch_document(
    http: &reqwest::Client,
    url: &str,
    trace: HeaderMap,
) -> Result<(HeaderMap, Vec<u8>), FetchError> {
    let response = http.get(url).headers(trace).send().await?;
    tracing::Span::current().record("http.response.status_code", response.status().as_u16());
    let response = response.error_for_status()?;
    let headers = response.headers().clone();
    let body = response.bytes().await?;
    if body.len() > MAX_DOCUMENT_BYTES {
        return Err(FetchError::TooLarge);
    }
    Ok((headers, body.to_vec()))
}

#[derive(Deserialize)]
struct DiscoveryDocument {
    issuer: String,
    jwks_uri: String,
}

/// Keys are parsed one by one so a key type this crate does not know (an
/// encryption key, a new curve) is skipped instead of failing the whole set.
#[derive(Deserialize)]
struct JwksDocument {
    keys: Vec<serde_json::Value>,
}

/// A JWKS entry we can verify access tokens with: a signing key with a
/// `kid`, whose type fits one allowed algorithm and whose own `alg` (when
/// given) is that algorithm.
fn usable_key(raw: serde_json::Value) -> Option<CachedKey> {
    let jwk: Jwk = serde_json::from_value(raw).ok()?;
    if jwk
        .common
        .public_key_use
        .as_ref()
        .is_some_and(|key_use| *key_use != PublicKeyUse::Signature)
    {
        return None;
    }
    let alg = match &jwk.algorithm {
        AlgorithmParameters::OctetKeyPair(params) if params.curve == EllipticCurve::Ed25519 => {
            Algorithm::EdDSA
        }
        AlgorithmParameters::RSA(_) => Algorithm::RS256,
        _ => return None,
    };
    let declared_matches = match jwk.common.key_algorithm {
        None => true,
        Some(KeyAlgorithm::EdDSA) => alg == Algorithm::EdDSA,
        Some(KeyAlgorithm::RS256) => alg == Algorithm::RS256,
        Some(_) => false,
    };
    if !declared_matches || !ALLOWED_ALGORITHMS.contains(&alg) {
        return None;
    }
    let kid = jwk.common.key_id.clone()?;
    let key = DecodingKey::from_jwk(&jwk).ok()?;
    Some(CachedKey { kid, alg, key })
}

/// How long a JWKS response may be cached: its `Cache-Control: max-age`,
/// else [`DEFAULT_TTL`]; never more than [`MAX_TTL`].
fn cache_ttl(headers: &HeaderMap) -> Duration {
    headers
        .get_all(header::CACHE_CONTROL)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .find_map(|directive| {
            let (name, value) = directive.trim().split_once('=')?;
            name.trim()
                .eq_ignore_ascii_case("max-age")
                .then(|| value.trim().trim_matches('"').parse::<u64>().ok())
                .flatten()
        })
        .map_or(DEFAULT_TTL, Duration::from_secs)
        .min(MAX_TTL)
}

fn delta(duration: Duration) -> TimeDelta {
    TimeDelta::from_std(duration).unwrap_or(TimeDelta::MAX)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn ttl(values: &[&'static str]) -> Duration {
        let mut headers = HeaderMap::new();
        for value in values {
            headers.append(header::CACHE_CONTROL, HeaderValue::from_static(value));
        }
        cache_ttl(&headers)
    }

    #[test]
    fn ttl_follows_max_age_within_bounds() {
        assert_eq!(ttl(&[]), DEFAULT_TTL);
        assert_eq!(ttl(&["no-transform"]), DEFAULT_TTL);
        assert_eq!(ttl(&["public, max-age=300"]), Duration::from_mins(5));
        assert_eq!(ttl(&["no-cache", "Max-Age = 120"]), Duration::from_mins(2));
        assert_eq!(ttl(&["max-age=0"]), Duration::ZERO);
        assert_eq!(ttl(&["max-age=999999999"]), MAX_TTL);
        assert_eq!(ttl(&["max-age=soon"]), DEFAULT_TTL);
    }

    fn okp(extra: &serde_json::Value) -> serde_json::Value {
        let mut jwk = serde_json::json!({
            "kty": "OKP", "crv": "Ed25519", "kid": "k1",
            "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo"
        });
        if let (Some(jwk), Some(extra)) = (jwk.as_object_mut(), extra.as_object()) {
            jwk.extend(extra.clone());
        }
        jwk
    }

    #[test]
    fn accepts_signing_keys_of_allowed_algorithms() {
        let key = usable_key(okp(&serde_json::json!({}))).unwrap();
        assert_eq!((key.kid.as_str(), key.alg), ("k1", Algorithm::EdDSA));
        assert!(usable_key(okp(&serde_json::json!({ "alg": "EdDSA", "use": "sig" }))).is_some());

        let rsa = serde_json::json!({
            "kty": "RSA", "kid": "r1", "alg": "RS256", "n": "sXch", "e": "AQAB"
        });
        assert_eq!(usable_key(rsa).unwrap().alg, Algorithm::RS256);
    }

    #[test]
    fn skips_keys_it_must_not_use() {
        for jwk in [
            okp(&serde_json::json!({ "use": "enc" })),
            okp(&serde_json::json!({ "alg": "RS256" })),
            okp(&serde_json::json!({ "kid": null })),
            serde_json::json!({ "kty": "RSA", "kid": "r", "alg": "RS512", "n": "sXch", "e": "AQAB" }),
            serde_json::json!({ "kty": "oct", "kid": "h", "k": "c2VjcmV0" }),
            serde_json::json!({ "kty": "EC", "kid": "e", "crv": "P-256", "x": "AA", "y": "AA" }),
            serde_json::json!({ "kty": "unknown" }),
        ] {
            assert!(usable_key(jwk.clone()).is_none(), "{jwk}");
        }
    }
}
