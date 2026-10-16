//! Client ID Metadata Documents (the MCP authorization spec's
//! recommended way for an agent to be a client without registering): a
//! `client_id` that is an `https` URL with a path names a JSON document,
//! served there, that describes the client. The issuer fetches it through
//! the SSRF [`guard`], checks that its `client_id` is that very URL, and
//! keeps it in `identity.clients` (kind `metadata_document`) for as long as
//! its `Cache-Control` allows, between [`MIN_CACHE`] and [`MAX_CACHE`].

pub mod guard;

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use campbooks_kernel::{
    Result,
    auth::ScopeInfo,
    rate_limit::{Limit, RateKey, RateLimits},
};
use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use url::Url;

use crate::domain::{
    client_names,
    clients::{
        self, Client, begin_as_server, default_third_party_scopes, is_registrable_redirect,
        third_party_may_hold, third_party_scopes,
    },
};

pub use guard::{Fetched, Refused};

/// The shortest time a document is cached.
pub const MIN_CACHE: Duration = Duration::from_mins(5);
/// The longest.
pub const MAX_CACHE: Duration = Duration::from_hours(24);
/// Most redirect URIs a document (or a registration) may list.
pub const MAX_REDIRECT_URIS: usize = 10;

/// How long a document that could not be fetched, or was refused, is not
/// asked for again.
pub const FAILURE_TTL: TimeDelta = TimeDelta::minutes(5);
/// Most failures remembered at once (then the oldest are forgotten).
const MAX_FAILURES: usize = 10_000;
/// Documents the whole instance fetches per hour, whoever asks.
const FETCHES: Limit = Limit::new("identity.metadata_fetch_all", 600, Duration::from_hours(1));

/// Document URLs that recently failed, so a stranger can't make the server
/// fetch the same bad URL again and again (per process).
#[derive(Debug, Default)]
pub struct Failures(Mutex<HashMap<String, DateTime<Utc>>>);

impl Failures {
    fn recent(&self, url: &Url, now: DateTime<Utc>) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(url.as_str())
            .is_some_and(|until| *until > now)
    }

    fn record(&self, url: &Url, now: DateTime<Utc>) {
        let mut failures = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if failures.len() >= MAX_FAILURES {
            failures.retain(|_, until| *until > now);
            if failures.len() >= MAX_FAILURES {
                failures.clear();
            }
        }
        failures.insert(url.as_str().to_owned(), now + FAILURE_TTL);
    }
}

/// A future returned by a [`Fetcher`].
pub type FetchFuture<'a> = Pin<Box<dyn Future<Output = Result<Fetched, Refused>> + Send + 'a>>;

/// What fetches documents: the SSRF [`guard`] in production, an in-memory
/// map in tests.
pub trait Fetcher: Send + Sync + 'static {
    /// Fetch `url`.
    fn fetch<'a>(&'a self, url: &'a Url) -> FetchFuture<'a>;
}

/// Fetches through the SSRF [`guard`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Guarded;

impl Fetcher for Guarded {
    fn fetch<'a>(&'a self, url: &'a Url) -> FetchFuture<'a> {
        Box::pin(guard::fetch(url))
    }
}

/// Shared handle to a [`Fetcher`].
pub type FetcherPort = Arc<dyn Fetcher>;

/// The URL a `client_id` names when it is a metadata document's: `https`,
/// a public-looking name, a path other than `/`.
pub fn document_url(client_id: &str) -> Option<Url> {
    let url = Url::parse(client_id).ok()?;
    (guard::check_url(&url).is_ok() && url.path() != "/" && url.as_str() == client_id)
        .then_some(url)
}

/// The fields of a document the issuer reads (RFC 7591 client metadata).
#[derive(Debug, Deserialize)]
struct Document {
    client_id: String,
    client_name: Option<String>,
    #[serde(default)]
    redirect_uris: Vec<String>,
    token_endpoint_auth_method: Option<String>,
    grant_types: Option<Vec<String>>,
    response_types: Option<Vec<String>>,
    scope: Option<String>,
    client_secret: Option<serde_json::Value>,
    jwks: Option<serde_json::Value>,
    jwks_uri: Option<serde_json::Value>,
}

/// Why a document is not acceptable. Logged by its name only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Invalid {
    /// Not JSON, or not an object of client metadata.
    #[error("not client metadata")]
    NotMetadata,
    /// Its `client_id` is not the URL it was fetched from.
    #[error("its client_id is not its URL")]
    WrongClientId,
    /// No redirect URI, too many, or one that is not `https` or loopback.
    #[error("unacceptable redirect URIs")]
    BadRedirects,
    /// It claims a secret, keys, or an authentication method other than
    /// `none`: a metadata-document client is public.
    #[error("it claims to be a confidential client")]
    NotPublic,
    /// It asks for a grant or response type other than the code flow's.
    #[error("unsupported grant or response type")]
    UnsupportedGrant,
}

/// What a valid document describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    /// A name to show (the document's, else the URL's host).
    pub name: String,
    /// Its redirect URIs.
    pub redirect_uris: Vec<String>,
    /// Its grant types.
    pub grant_types: Vec<String>,
    /// The scopes it may ask for.
    pub scopes: Vec<String>,
}

/// Check the client metadata `body` fetched from `url`.
pub fn validate(url: &Url, body: &[u8], catalogue: &[ScopeInfo]) -> Result<Described, Invalid> {
    let document: Document = serde_json::from_slice(body).map_err(|_| Invalid::NotMetadata)?;
    if document.client_id != url.as_str() {
        return Err(Invalid::WrongClientId);
    }
    if document.client_secret.is_some()
        || document.jwks.is_some()
        || document.jwks_uri.is_some()
        || document
            .token_endpoint_auth_method
            .as_deref()
            .is_some_and(|method| method != "none")
    {
        return Err(Invalid::NotPublic);
    }
    let described = Registration {
        redirect_uris: document.redirect_uris,
        grant_types: document.grant_types,
        response_types: document.response_types,
        scope: document.scope,
        client_name: document.client_name,
    }
    .check(url.host_str().unwrap_or_default(), catalogue)?;
    Ok(described)
}

/// Client metadata as a client sends it, for a document or a registration.
#[derive(Debug, Default)]
pub struct Registration {
    /// `redirect_uris`.
    pub redirect_uris: Vec<String>,
    /// `grant_types`; missing means `authorization_code`.
    pub grant_types: Option<Vec<String>>,
    /// `response_types`; missing means `code`.
    pub response_types: Option<Vec<String>>,
    /// `scope`, space-separated; missing means every scope a third party may
    /// hold.
    pub scope: Option<String>,
    /// `client_name`.
    pub client_name: Option<String>,
}

impl Registration {
    /// What the metadata describes, named `fallback_name` when it gives no
    /// usable name.
    pub fn check(self, fallback_name: &str, catalogue: &[ScopeInfo]) -> Result<Described, Invalid> {
        if self.redirect_uris.is_empty()
            || self.redirect_uris.len() > MAX_REDIRECT_URIS
            || !self
                .redirect_uris
                .iter()
                .all(|uri| is_registrable_redirect(uri))
        {
            return Err(Invalid::BadRedirects);
        }
        let grant_types = self
            .grant_types
            .unwrap_or_else(|| vec!["authorization_code".to_owned()]);
        let grants_ok = !grant_types.is_empty()
            && grant_types
                .iter()
                .all(|g| g == "authorization_code" || g == "refresh_token")
            && grant_types.iter().any(|g| g == "authorization_code");
        let responses_ok = self
            .response_types
            .is_none_or(|types| !types.is_empty() && types.iter().all(|t| t == "code"));
        if !grants_ok || !responses_ok {
            return Err(Invalid::UnsupportedGrant);
        }
        let allowed = third_party_scopes(catalogue);
        let scopes = match self.scope {
            // Reading only, unless it asks for more.
            None => default_third_party_scopes(catalogue),
            Some(scope) => scope
                .split_whitespace()
                .filter(|s| third_party_may_hold(s) && allowed.iter().any(|a| a == s))
                .map(ToOwned::to_owned)
                .collect(),
        };
        // A stranger's name, cleaned; never Campbooks' own, nor a word
        // mixing scripts (then its host).
        let name = self
            .client_name
            .map(|name| client_names::clean(&name))
            .filter(|name| {
                !name.is_empty()
                    && !client_names::impersonates_campbooks(name)
                    && !client_names::mixes_scripts(name)
            })
            .unwrap_or_else(|| client_names::clean(fallback_name));
        Ok(Described {
            name,
            redirect_uris: self.redirect_uris,
            grant_types,
            scopes,
        })
    }
}

/// The metadata-document client `url` names: from the cache while it is
/// fresh, else fetched again through `fetcher`. `None` when the document
/// cannot be fetched or is not acceptable (then nothing is cached).
pub async fn client(
    db: &PgPool,
    limits: &RateLimits,
    fetcher: &dyn Fetcher,
    failures: &Failures,
    url: &Url,
    catalogue: &[ScopeInfo],
    now: DateTime<Utc>,
) -> Result<Option<Client>> {
    let mut tx = begin_as_server(db).await?;
    let cached = clients::load_row(&mut tx, url.as_str()).await?;
    tx.commit().await?;
    if let Some(row) = cached {
        if row.kind != "metadata_document" {
            return Ok(None);
        }
        if row.metadata_expires_at.is_some_and(|at| at > now) {
            return Ok(row.into_client());
        }
    }
    if failures.recent(url, now) {
        return Ok(None);
    }
    limits
        .check(db, &FETCHES, &RateKey::text("instance"))
        .await?;
    let document = match fetcher.fetch(url).await {
        Ok(document) => document,
        Err(refused) => {
            tracing::info!(reason = %refused, "a client metadata document was not fetched");
            failures.record(url, now);
            return Ok(None);
        }
    };
    let described = match validate(url, &document.body, catalogue) {
        Ok(described) => described,
        Err(invalid) => {
            tracing::info!(reason = %invalid, "a client metadata document was refused");
            failures.record(url, now);
            return Ok(None);
        }
    };
    let cache = document
        .max_age
        .unwrap_or(MIN_CACHE)
        .clamp(MIN_CACHE, MAX_CACHE);
    let expires = now + TimeDelta::from_std(cache).unwrap_or(TimeDelta::minutes(5));
    let mut tx = begin_as_server(db).await?;
    sqlx::query(
        "INSERT INTO identity.clients
             (client_id, kind, name, redirect_uris, grant_types, auth_method, allowed_scopes,
              created_at, metadata_expires_at)
         VALUES ($1, 'metadata_document', $2, $3, $4, 'none', $5, $6, $7)
         ON CONFLICT (client_id) DO UPDATE SET
             name = EXCLUDED.name, redirect_uris = EXCLUDED.redirect_uris,
             grant_types = EXCLUDED.grant_types, allowed_scopes = EXCLUDED.allowed_scopes,
             metadata_expires_at = EXCLUDED.metadata_expires_at
         WHERE identity.clients.kind = 'metadata_document'",
    )
    .bind(url.as_str())
    .bind(&described.name)
    .bind(&described.redirect_uris)
    .bind(&described.grant_types)
    .bind(&described.scopes)
    .bind(now)
    .bind(expires)
    .execute(&mut *tx)
    .await?;
    let client = clients::load(&mut tx, url.as_str()).await?;
    tx.commit().await?;
    Ok(client.filter(|c| c.kind == clients::Kind::MetadataDocument))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOGUE: [ScopeInfo; 2] = [
        ScopeInfo {
            name: "system:read",
            description: "",
        },
        ScopeInfo {
            name: "identity:write",
            description: "",
        },
    ];

    fn url() -> Url {
        Url::parse("https://agent.example.com/oauth/client.json").unwrap()
    }

    fn check(document: &serde_json::Value) -> Result<Described, Invalid> {
        validate(&url(), document.to_string().as_bytes(), &CATALOGUE)
    }

    #[test]
    fn a_document_names_itself_and_lists_its_redirects() {
        let described = check(&serde_json::json!({
            "client_id": url().as_str(),
            "client_name": "  Agent  ",
            "redirect_uris": ["http://localhost:3334/callback", "https://agent.example.com/cb"],
            "scope": "system:read identity:write unknown:x",
            "grant_types": ["authorization_code", "refresh_token"],
        }))
        .unwrap();
        assert_eq!(described.name, "Agent");
        assert_eq!(described.scopes, ["system:read"], "never identity:*");
        let unnamed = check(&serde_json::json!({
            "client_id": url().as_str(), "redirect_uris": ["https://agent.example.com/cb"],
        }))
        .unwrap();
        assert_eq!(unnamed.name, "agent.example.com");
        assert_eq!(unnamed.scopes, ["system:read"]);
    }

    #[test]
    fn documents_that_are_not_what_they_claim_are_refused() {
        let id = url().to_string();
        let redirect = serde_json::json!(["https://agent.example.com/cb"]);
        for (document, why) in [
            (
                serde_json::json!({ "client_id": "https://other.example.com/c.json", "redirect_uris": redirect }),
                Invalid::WrongClientId,
            ),
            (
                serde_json::json!({ "client_id": id, "redirect_uris": [] }),
                Invalid::BadRedirects,
            ),
            (
                serde_json::json!({ "client_id": id, "redirect_uris": ["http://agent.example.com/cb"] }),
                Invalid::BadRedirects,
            ),
            (
                serde_json::json!({ "client_id": id, "redirect_uris": ["com.example.agent:/cb"] }),
                Invalid::BadRedirects,
            ),
            (
                serde_json::json!({ "client_id": id, "redirect_uris": redirect, "token_endpoint_auth_method": "client_secret_basic" }),
                Invalid::NotPublic,
            ),
            (
                serde_json::json!({ "client_id": id, "redirect_uris": redirect, "client_secret": "x" }),
                Invalid::NotPublic,
            ),
            (
                serde_json::json!({ "client_id": id, "redirect_uris": redirect, "jwks_uri": "https://agent.example.com/jwks" }),
                Invalid::NotPublic,
            ),
            (
                serde_json::json!({ "client_id": id, "redirect_uris": redirect, "grant_types": ["client_credentials"] }),
                Invalid::UnsupportedGrant,
            ),
            (
                serde_json::json!({ "client_id": id, "redirect_uris": redirect, "response_types": ["token"] }),
                Invalid::UnsupportedGrant,
            ),
            (
                serde_json::json!(["not", "an", "object"]),
                Invalid::NotMetadata,
            ),
        ] {
            assert_eq!(check(&document), Err(why), "{document}");
        }
        assert_eq!(
            validate(&url(), b"<html>", &CATALOGUE),
            Err(Invalid::NotMetadata)
        );
    }

    #[test]
    fn only_https_urls_with_a_path_are_metadata_client_ids() {
        assert!(document_url("https://agent.example.com/client.json").is_some());
        for not in [
            "https://agent.example.com",
            "https://agent.example.com/",
            "http://agent.example.com/client.json",
            "dcr_abc",
            "campbooks-native",
            "https://127.0.0.1/client.json",
            "https://agent.example.com/Client.json#x",
        ] {
            assert!(document_url(not).is_none(), "{not}");
        }
    }
}
