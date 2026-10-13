//! What the module is composed with (constructor injection), read from the
//! environment at startup.
//!
//! The issuer runs **in this process** when the instance has a secret key
//! (`CAMPBOOKS_SECRET_KEY[_FILE]`) and no other identity provider is
//! configured (`CAMPBOOKS_AUTH__ISSUER` unset, or set to this instance's
//! public origin). Until Rauthy is removed (ADR 0014, slice S5), an install
//! whose `CAMPBOOKS_AUTH__ISSUER` names Rauthy keeps using it, and every
//! identity endpoint answers 404.

use std::{path::PathBuf, sync::Arc};

use campbooks_kernel::{
    Config,
    clock::{Clock, SystemClock},
    config::Environment,
    crypto::{Cipher, SecretKey, derive_secret},
};
use figment::Figment;
use secrecy::SecretString;
use serde::Deserialize;

use crate::{
    domain::{clients::Clients, keys::Keys},
    upstream::google::{Google, GoogleEndpoints},
    web::Cookies,
};

/// The HKDF label of the key sealing what the issuer keeps (signing keys,
/// PKCE verifiers sent upstream, held provider grants). Never change it.
pub const SEALING_PURPOSE: &[u8] = b"campbooks identity sealing v1";

/// The HKDF label the web client's secret is derived from. The web BFF
/// derives the same value; never change it.
pub const WEB_CLIENT_SECRET_PURPOSE: &[u8] = b"campbooks identity client campbooks-web v1";

/// The module's dependencies. `Default` is inert: no issuer in this process
/// (what the OpenAPI and events catalogues are built from).
#[derive(Clone, Default)]
pub struct Deps {
    /// The in-process issuer, when it is the instance's identity provider.
    pub issuer: Option<Arc<Issuer>>,
}

impl std::fmt::Debug for Deps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Deps")
            .field("issuer", &self.issuer.as_ref().map(|i| i.url.as_str()))
            .finish()
    }
}

/// The issuer and everything it needs.
pub struct Issuer {
    /// The issuer identifier: this instance's public origin, no trailing
    /// slash (e.g. `https://app.example.com`).
    pub url: String,
    /// Cookie names and attributes.
    pub cookies: Cookies,
    /// Seals what the issuer must read back.
    pub cipher: Cipher,
    /// The clients that may ask for tokens.
    pub clients: Clients,
    /// The signing keys.
    pub keys: Arc<Keys>,
    /// Google, when `CAMPBOOKS_GOOGLE__*` is set.
    pub google: Option<Google>,
    /// The audience of access tokens (`CAMPBOOKS_AUTH__AUDIENCE`).
    pub audience: String,
    /// The API's scope catalogue: what first-party clients may ask for.
    pub api_scopes: Vec<String>,
    /// The time.
    pub clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for Issuer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Issuer")
            .field("url", &self.url)
            .field("google", &self.google.is_some())
            .finish_non_exhaustive()
    }
}

/// `CAMPBOOKS_GOOGLE__*`, the same settings the connectors module reads (one
/// Google OAuth app serves sign-in and mail). Unknown keys are ignored: they
/// belong to the connectors module.
#[derive(Debug, Default, Deserialize)]
struct GoogleConfig {
    client_id: Option<String>,
    client_secret: Option<SecretString>,
    client_secret_file: Option<PathBuf>,
    base_url: Option<String>,
}

/// Where a provider's client secret comes from.
#[derive(Debug, Clone)]
pub enum ClientSecret {
    /// Inline (`…__CLIENT_SECRET`).
    Inline(SecretString),
    /// A file (`…__CLIENT_SECRET_FILE`), read when needed.
    File(PathBuf),
}

impl Deps {
    /// Read the settings from the environment. `api_scopes` is the scope
    /// catalogue. A broken setting is an error naming it.
    pub fn from_env(config: &Config, api_scopes: &[&str]) -> Result<Self, String> {
        Self::from_figment(
            &campbooks_kernel::config::env_source(),
            config,
            SecretKey::from_env()?,
            api_scopes,
            Arc::new(SystemClock),
        )
    }

    /// [`from_env`](Self::from_env) over any source and clock (tests).
    pub fn from_figment(
        figment: &Figment,
        config: &Config,
        secret: Option<SecretKey>,
        api_scopes: &[&str],
        clock: Arc<dyn Clock>,
    ) -> Result<Self, String> {
        let (url, secure) = public_origin(&config.public_url)?;
        if config
            .auth
            .issuer()
            .is_some_and(|issuer| issuer.trim_end_matches('/') != url)
        {
            // Another identity provider (Rauthy) is configured.
            return Ok(Self::default());
        }
        let Some(secret) = secret else {
            return Ok(Self::default());
        };
        let google: GoogleConfig = if figment.contains("google") {
            figment
                .extract_inner("google")
                .map_err(|e| format!("CAMPBOOKS_GOOGLE__*: {e}"))?
        } else {
            GoogleConfig::default()
        };
        let google = google_client(google, config, &url)?;
        let cipher = Cipher::new(&secret, SEALING_PURPOSE);
        let issuer = Issuer {
            clients: Clients::first_party(&url, derive_secret(&secret, WEB_CLIENT_SECRET_PURPOSE)),
            url,
            cookies: Cookies::new(secure),
            keys: Arc::new(Keys::new(
                Cipher::new(&secret, SEALING_PURPOSE),
                Arc::clone(&clock),
            )),
            cipher,
            google,
            audience: config.auth.audience.trim().to_owned(),
            api_scopes: api_scopes.iter().map(|s| (*s).to_owned()).collect(),
            clock,
        };
        Ok(Self {
            issuer: Some(Arc::new(issuer)),
        })
    }
}

/// The Google client, when its client id and secret are set.
fn google_client(
    google: GoogleConfig,
    config: &Config,
    issuer: &str,
) -> Result<Option<Google>, String> {
    let client_id = google
        .client_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let secret = match (google.client_secret_file, google.client_secret) {
        (Some(path), _) if !path.as_os_str().is_empty() => Some(ClientSecret::File(path)),
        (_, Some(secret)) => Some(ClientSecret::Inline(secret)),
        _ => None,
    };
    let (Some(client_id), Some(secret)) = (client_id, secret) else {
        return Ok(None);
    };
    let endpoints = match google
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        None => GoogleEndpoints::google(),
        Some(_) if config.env != Environment::Development => {
            return Err(
                "CAMPBOOKS_GOOGLE__BASE_URL points Google at another server: it is for \
                 development only (CAMPBOOKS_ENV=development)"
                    .to_owned(),
            );
        }
        Some(base) => GoogleEndpoints::under(base)
            .map_err(|e| format!("CAMPBOOKS_GOOGLE__BASE_URL={base}: {e}"))?,
    };
    Google::new(client_id.to_owned(), secret, issuer, endpoints).map(Some)
}

/// `scheme://host[:port]` of the public URL and whether it is `https`. The
/// app must sit at the root of its origin.
fn public_origin(public_url: &str) -> Result<(String, bool), String> {
    let url = url::Url::parse(public_url.trim())
        .map_err(|e| format!("CAMPBOOKS_PUBLIC_URL={public_url} is not a URL: {e}"))?;
    let secure = match url.scheme() {
        "https" => true,
        "http" => false,
        _ => return Err(format!("CAMPBOOKS_PUBLIC_URL={public_url} must be http(s)")),
    };
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(format!(
            "CAMPBOOKS_PUBLIC_URL={public_url} must be an origin with no path: sign-in cookies \
             need the app at the root of its origin"
        ));
    }
    Ok((url.origin().ascii_serialization(), secure))
}

#[cfg(test)]
mod tests {
    use figment::providers::Serialized;

    use super::*;

    fn config(public_url: &str, issuer: Option<&str>) -> Config {
        let mut config = campbooks_kernel::testing::config("postgres://test@localhost/test");
        config.public_url = public_url.to_owned();
        config.auth.issuer = issuer.map(ToOwned::to_owned);
        config
    }

    fn deps(
        config: &Config,
        secret: Option<SecretKey>,
        google: &serde_json::Value,
    ) -> Result<Deps, String> {
        Deps::from_figment(
            &Figment::from(Serialized::defaults(
                serde_json::json!({ "google": google }),
            )),
            config,
            secret,
            &["system:read"],
            Arc::new(SystemClock),
        )
    }

    const KEY: Option<SecretKey> = Some(SecretKey::from_bytes([7; 32]));

    #[test]
    fn the_issuer_runs_here_unless_another_is_configured() {
        let here = deps(
            &config("https://app.example.com/", None),
            KEY,
            &serde_json::json!({}),
        )
        .unwrap();
        let issuer = here.issuer.unwrap();
        assert_eq!(issuer.url, "https://app.example.com");
        assert!(issuer.google.is_none());

        let same = deps(
            &config("https://app.example.com", Some("https://app.example.com/")),
            KEY,
            &serde_json::json!({}),
        )
        .unwrap();
        assert!(same.issuer.is_some());

        let rauthy = deps(
            &config(
                "https://app.example.com",
                Some("https://auth.example.com/auth/v1/"),
            ),
            KEY,
            &serde_json::json!({}),
        )
        .unwrap();
        assert!(rauthy.issuer.is_none());

        let no_secret = deps(
            &config("https://app.example.com", None),
            None,
            &serde_json::json!({}),
        )
        .unwrap();
        assert!(no_secret.issuer.is_none());
    }

    #[test]
    fn google_is_configured_by_id_and_secret_and_its_base_url_is_for_development() {
        let config = config("http://localhost:8080", None);
        let with_google = deps(
            &config,
            KEY,
            &serde_json::json!({ "client_id": "g-id", "client_secret": "g-secret", "unknown": 1 }),
        )
        .unwrap();
        assert!(with_google.issuer.unwrap().google.is_some());

        let fake = deps(
            &config,
            KEY,
            &serde_json::json!({ "client_id": "g", "client_secret": "s", "base_url": "http://127.0.0.1:9/" }),
        )
        .unwrap_err();
        assert!(fake.contains("development only"), "{fake}");
    }

    #[test]
    fn the_public_url_must_be_an_origin() {
        let err = deps(
            &config("https://app.example.com/app", None),
            KEY,
            &serde_json::json!({}),
        )
        .unwrap_err();
        assert!(err.contains("CAMPBOOKS_PUBLIC_URL"), "{err}");
    }
}
