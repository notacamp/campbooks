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
    auth::ScopeInfo,
    clock::{Clock, SystemClock},
    config::Environment,
    crypto::{Cipher, SecretKey, derive_secret},
};
use figment::Figment;
use secrecy::SecretString;
use serde::Deserialize;

use crate::{
    domain::{clients::Clients, keys::Keys},
    metadata::{Failures, FetcherPort, Guarded},
    upstream::{
        Provider,
        client::{Endpoints, Upstream, ZOHO_DEFAULT_ACCOUNTS_SERVER},
        redirect_uri,
    },
    web::Cookies,
};

/// The HKDF label of the key sealing what the issuer keeps (signing keys,
/// PKCE verifiers sent upstream, held provider grants). Never change it.
pub const SEALING_PURPOSE: &[u8] = b"campbooks identity sealing v1";

/// The HKDF label the web client's secret is derived from. The web BFF
/// derives the same value; never change it.
pub const WEB_CLIENT_SECRET_PURPOSE: &[u8] = b"campbooks identity client campbooks-web v1";

/// The path of the API's resource identifier under the public origin.
pub const API_RESOURCE_PATH: &str = "/api";

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
    /// The providers configured (`CAMPBOOKS_GOOGLE__*`,
    /// `CAMPBOOKS_MICROSOFT__*`, `CAMPBOOKS_ZOHO__*`), in display order.
    pub upstreams: Vec<Upstream>,
    /// The API's resource identifier (RFC 8707, RFC 9728):
    /// `<public origin>/api`, the `aud` of every access token.
    pub resource: String,
    /// The API's scope catalogue: what first-party clients may ask for
    /// (third parties: all but `identity:*`).
    pub api_scopes: Vec<ScopeInfo>,
    /// Fetches Client ID Metadata Documents (through the SSRF guard).
    pub fetcher: FetcherPort,
    /// Metadata documents that recently failed, not asked for again yet.
    pub document_failures: Failures,
    /// The time.
    pub clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for Issuer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Issuer")
            .field("url", &self.url)
            .field(
                "upstreams",
                &self
                    .upstreams
                    .iter()
                    .map(Upstream::provider)
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// `CAMPBOOKS_<PROVIDER>__*`. Google's are the same settings the connectors
/// module reads (one OAuth app per provider serves sign-in and mail).
/// Unknown keys are ignored: they belong to the connectors module.
#[derive(Debug, Default, Deserialize)]
struct ProviderConfig {
    client_id: Option<String>,
    client_secret: Option<SecretString>,
    client_secret_file: Option<PathBuf>,
    /// A fake laid out like the provider (development only).
    base_url: Option<String>,
    /// The path sent as `redirect_uri`, when the provider's app is
    /// registered on another path the proxy rewrites to ours.
    redirect_path: Option<String>,
    /// Microsoft only: `common`, `organizations`, `consumers` or a tenant id.
    tenant: Option<String>,
    /// Zoho only: the data centre's accounts server the sign-in request goes
    /// to (one of Zoho's; default `https://accounts.zoho.com`).
    accounts_server: Option<String>,
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
    pub fn from_env(config: &Config, api_scopes: &[ScopeInfo]) -> Result<Self, String> {
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
        api_scopes: &[ScopeInfo],
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
        if !secure && config.env != Environment::Development {
            return Err(format!(
                "CAMPBOOKS_PUBLIC_URL={} is plain http: Campbooks' own sign-in needs https, so \
                 its cookies are Secure and __Host- prefixed (plain http is for \
                 CAMPBOOKS_ENV=development only)",
                config.public_url
            ));
        }
        let mut upstreams = Vec::new();
        for provider in Provider::ALL {
            let key = provider.as_str();
            let settings: ProviderConfig = if figment.contains(key) {
                figment
                    .extract_inner(key)
                    .map_err(|e| format!("{}*: {e}", env_prefix(provider)))?
            } else {
                ProviderConfig::default()
            };
            upstreams.extend(upstream_client(provider, settings, config, &url)?);
        }
        let cipher = Cipher::new(&secret, SEALING_PURPOSE);
        let resource = format!("{url}{API_RESOURCE_PATH}");
        let issuer = Issuer {
            clients: Clients::first_party(&url, derive_secret(&secret, WEB_CLIENT_SECRET_PURPOSE)),
            url,
            cookies: Cookies::new(secure),
            keys: Arc::new(Keys::new(
                Cipher::new(&secret, SEALING_PURPOSE),
                Arc::clone(&clock),
            )),
            cipher,
            upstreams,
            resource,
            api_scopes: api_scopes.to_vec(),
            fetcher: Arc::new(Guarded),
            document_failures: Failures::default(),
            clock,
        };
        Ok(Self {
            issuer: Some(Arc::new(issuer)),
        })
    }
}

/// `CAMPBOOKS_<PROVIDER>__`.
fn env_prefix(provider: Provider) -> String {
    format!("CAMPBOOKS_{}__", provider.as_str().to_uppercase())
}

/// `provider`'s client, when its client id and secret are set.
fn upstream_client(
    provider: Provider,
    settings: ProviderConfig,
    config: &Config,
    issuer: &str,
) -> Result<Option<Upstream>, String> {
    let prefix = env_prefix(provider);
    let present =
        |value: Option<String>| value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    let client_id = present(settings.client_id);
    let secret = match (settings.client_secret_file, settings.client_secret) {
        (Some(path), _) if !path.as_os_str().is_empty() => Some(ClientSecret::File(path)),
        (_, Some(secret)) => Some(ClientSecret::Inline(secret)),
        _ => None,
    };
    let accounts_server = present(settings.accounts_server);
    if provider != Provider::Zoho && accounts_server.is_some() {
        return Err(format!("{prefix}ACCOUNTS_SERVER is a Zoho setting"));
    }
    let (Some(client_id), Some(secret)) = (client_id, secret) else {
        return Ok(None);
    };
    let tenant = present(settings.tenant).unwrap_or_else(|| "common".to_owned());
    let endpoints = match present(settings.base_url) {
        None => match provider {
            Provider::Google => Endpoints::google(),
            Provider::Microsoft => Endpoints::microsoft(&tenant)?,
            Provider::Zoho => Endpoints::zoho(
                accounts_server
                    .as_deref()
                    .unwrap_or(ZOHO_DEFAULT_ACCOUNTS_SERVER),
            )
            .map_err(|e| format!("{prefix}ACCOUNTS_SERVER={e}"))?,
        },
        Some(_) if config.env != Environment::Development => {
            return Err(format!(
                "{prefix}BASE_URL points {} at another server: it is for development only \
                 (CAMPBOOKS_ENV=development)",
                provider.as_str()
            ));
        }
        Some(base) => Endpoints::under(provider, &base, &tenant)
            .map_err(|e| format!("{prefix}BASE_URL={base}: {e}"))?,
    };
    let redirect_uri = redirect_uri(provider, issuer, settings.redirect_path.as_deref())?;
    Upstream::new(provider, client_id, secret, redirect_uri, endpoints).map(Some)
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
        config.env = Environment::Development;
        config.public_url = public_url.to_owned();
        config.auth.issuer = issuer.map(ToOwned::to_owned);
        config
    }

    fn deps(
        config: &Config,
        secret: Option<SecretKey>,
        google: &serde_json::Value,
    ) -> Result<Deps, String> {
        deps_with(config, secret, &serde_json::json!({ "google": google }))
    }

    fn deps_with(
        config: &Config,
        secret: Option<SecretKey>,
        settings: &serde_json::Value,
    ) -> Result<Deps, String> {
        Deps::from_figment(
            &Figment::from(Serialized::defaults(settings.clone())),
            config,
            secret,
            &[ScopeInfo {
                name: "system:read",
                description: "See the instance's health.",
            }],
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
        assert!(issuer.upstreams.is_empty());

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
        let providers: Vec<Provider> = with_google
            .issuer
            .unwrap()
            .upstreams
            .iter()
            .map(Upstream::provider)
            .collect();
        assert_eq!(providers, [Provider::Google]);

        let mut config = config;
        config.env = Environment::Production;
        config.public_url = "https://app.example.com".to_owned();
        let fake = deps(
            &config,
            KEY,
            &serde_json::json!({ "client_id": "g", "client_secret": "s", "base_url": "http://127.0.0.1:9/" }),
        )
        .unwrap_err();
        assert!(fake.contains("development only"), "{fake}");
    }

    #[test]
    fn microsoft_and_zoho_are_configured_like_google_in_display_order() {
        let config = config("http://localhost:8080", None);
        let all = deps_with(
            &config,
            KEY,
            &serde_json::json!({
                "zoho": { "client_id": "z", "client_secret": "s" },
                "microsoft": { "client_id": "m", "client_secret": "s", "tenant": "organizations" },
                "google": { "client_id": "g", "client_secret": "s" },
            }),
        )
        .unwrap();
        let providers: Vec<Provider> = all
            .issuer
            .unwrap()
            .upstreams
            .iter()
            .map(Upstream::provider)
            .collect();
        assert_eq!(
            providers,
            [Provider::Google, Provider::Microsoft, Provider::Zoho]
        );

        let bad_tenant = deps_with(
            &config,
            KEY,
            &serde_json::json!({ "microsoft": { "client_id": "m", "client_secret": "s", "tenant": "../x" } }),
        )
        .unwrap_err();
        assert!(bad_tenant.contains("TENANT"), "{bad_tenant}");
    }

    #[test]
    fn microsoft_and_zoho_take_a_redirect_path_too() {
        let config = config("http://localhost:8080", None);
        let rails = deps_with(
            &config,
            KEY,
            &serde_json::json!({
                "microsoft": { "client_id": "m", "client_secret": "s", "redirect_path": "/oauth/microsoft/callback" },
                "zoho": { "client_id": "z", "client_secret": "s", "redirect_path": "/oauth/zoho/callback" },
            }),
        )
        .unwrap();
        let issuer = rails.issuer.unwrap();
        let uris: Vec<&str> = issuer
            .upstreams
            .iter()
            .map(Upstream::redirect_uri)
            .collect();
        assert_eq!(
            uris,
            [
                "http://localhost:8080/oauth/microsoft/callback",
                "http://localhost:8080/oauth/zoho/callback"
            ]
        );
        let err = deps_with(
            &config,
            KEY,
            &serde_json::json!({ "zoho": { "client_id": "z", "client_secret": "s", "redirect_path": "//evil.example.com" } }),
        )
        .unwrap_err();
        assert!(err.contains("CAMPBOOKS_ZOHO__REDIRECT_PATH"), "{err}");
    }

    #[test]
    fn zoho_accounts_server_is_validated_at_load() {
        let config = config("http://localhost:8080", None);
        let zoho = |server: &str| {
            deps_with(
                &config,
                KEY,
                &serde_json::json!({ "zoho": { "client_id": "z", "client_secret": "s", "accounts_server": server } }),
            )
        };
        assert!(zoho("https://accounts.zoho.eu").is_ok());
        assert!(zoho("").is_ok(), "blank means the default");
        for bad in [
            "https://accounts.example.com",
            "http://accounts.zoho.eu",
            "https://accounts.zoho.eu/oauth",
        ] {
            let err = zoho(bad).unwrap_err();
            assert!(err.contains("CAMPBOOKS_ZOHO__ACCOUNTS_SERVER"), "{err}");
        }
    }

    #[test]
    fn the_google_redirect_path_defaults_and_is_validated_at_load() {
        let config = config("https://app.example.com", None);
        let redirect = |path: serde_json::Value| {
            deps(
                &config,
                KEY,
                &serde_json::json!({ "client_id": "g", "client_secret": "s", "redirect_path": path }),
            )
            .map(|deps| deps.issuer.unwrap().upstreams[0].redirect_uri().to_owned())
        };
        assert_eq!(
            redirect(serde_json::Value::Null).unwrap(),
            "https://app.example.com/auth/callback/google"
        );
        assert_eq!(
            redirect(serde_json::json!("")).unwrap(),
            "https://app.example.com/auth/callback/google"
        );
        assert_eq!(
            redirect(serde_json::json!("/oauth/gmail/callback")).unwrap(),
            "https://app.example.com/oauth/gmail/callback"
        );
        for bad in [
            "oauth/gmail/callback",
            "https://evil.example.com/cb",
            "//evil.example.com/cb",
            "/cb?x=1",
            "/cb#frag",
            "/a/../cb",
            "/a b",
            "/a\\b",
        ] {
            let err = redirect(serde_json::json!(bad)).unwrap_err();
            assert!(
                err.contains("CAMPBOOKS_GOOGLE__REDIRECT_PATH"),
                "{bad}: {err}"
            );
        }
    }

    #[test]
    fn plain_http_is_for_development_only() {
        let mut production = config("http://app.example.com", None);
        production.env = Environment::Production;
        let err = deps(&production, KEY, &serde_json::json!({})).unwrap_err();
        assert!(err.contains("https"), "{err}");
        production.public_url = "https://app.example.com".to_owned();
        assert!(
            deps(&production, KEY, &serde_json::json!({}))
                .unwrap()
                .issuer
                .is_some()
        );
        // Another identity provider: this check is not ours to make.
        let mut rauthy = config("http://app.example.com", Some("http://auth.example.com/"));
        rauthy.env = Environment::Production;
        assert!(
            deps(&rauthy, KEY, &serde_json::json!({}))
                .unwrap()
                .issuer
                .is_none()
        );
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
