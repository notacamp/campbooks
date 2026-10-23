//! What the module is composed with (constructor injection): settings, the
//! Google and Zoho clients (refresh, revoke, mail; the consent itself is the
//! identity module's), the IMAP adapter (its resolvers and trust), the
//! credentials cipher and the clock. Handlers reach it as
//! an axum extension; jobs and consumers own a copy.

use std::sync::Arc;

use campbooks_kernel::{
    Config,
    clock::{Clock, SystemClock},
    config::Environment,
    crypto::{Cipher, SecretKey},
};
use figment::Figment;

use crate::{
    config::{ConnectorsConfig, OAuthClientConfig},
    google::{Endpoints, Google},
    imap::Imap,
    provider::ClientSecret,
    zoho::{Centres, Zoho},
};

/// The HKDF label of the credentials key. Never change it: every stored
/// credential was sealed under it.
pub const CREDENTIALS_PURPOSE: &[u8] = b"campbooks connectors credentials v1";

/// The module's dependencies. `Default` is inert: no provider configured,
/// nothing can be connected (what the OpenAPI and events catalogues use).
#[derive(Clone)]
pub struct Deps {
    /// Engine and adapter settings.
    pub config: Arc<ConnectorsConfig>,
    /// The Google client, when `CAMPBOOKS_GOOGLE__*` is set.
    pub google: Option<Arc<Google>>,
    /// The Zoho client, when `CAMPBOOKS_ZOHO__*` is set.
    pub zoho: Option<Arc<Zoho>>,
    /// The IMAP adapter (`CAMPBOOKS_CONNECTORS__IMAP__*`); offered only
    /// when enabled ([`imap_enabled`](Self::imap_enabled)).
    pub imap: Arc<Imap>,
    /// Seals credentials; present whenever a provider is configured.
    pub cipher: Option<Arc<Cipher>>,
    /// The time.
    pub clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for Deps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Deps")
            .field("config", &self.config)
            .field("google", &self.google.is_some())
            .field("zoho", &self.zoho.is_some())
            .field("imap", &self.imap_enabled())
            .finish_non_exhaustive()
    }
}

impl Default for Deps {
    fn default() -> Self {
        Self {
            config: Arc::new(ConnectorsConfig::default()),
            google: None,
            zoho: None,
            imap: Arc::new(Imap::default()),
            cipher: None,
            clock: Arc::new(SystemClock),
        }
    }
}

impl Deps {
    /// Whether IMAP mailboxes may be connected: turned on, and credentials
    /// can be sealed.
    pub fn imap_enabled(&self) -> bool {
        self.imap.config.enabled && self.cipher.is_some()
    }

    /// Read `CAMPBOOKS_CONNECTORS__*`, `CAMPBOOKS_GOOGLE__*` and the
    /// instance secret. A provider configured without the instance secret,
    /// or a broken setting, is an error naming the variable.
    pub fn from_env(config: &Config) -> Result<Self, String> {
        Self::from_figment(
            &campbooks_kernel::config::env_source(),
            config,
            SecretKey::from_env()?,
        )
    }

    /// [`from_env`](Self::from_env) over any source (tests).
    pub fn from_figment(
        figment: &Figment,
        config: &Config,
        secret: Option<SecretKey>,
    ) -> Result<Self, String> {
        let config = ConnectorsConfig::from_figment(figment)?;
        let mut deps = Self {
            imap: Arc::new(Imap::new(config.imap.clone())),
            config: Arc::new(config),
            ..Self::default()
        };
        let development = config.env == Environment::Development;
        if let Some((client_id, client_secret, base)) = client(
            &OAuthClientConfig::from_figment(figment, "google")?,
            "GOOGLE",
            development,
        )? {
            let endpoints = match base {
                None => Endpoints::google(),
                Some(base) => Endpoints::under(&base),
            };
            deps.google = Some(Arc::new(Google::new(client_id, client_secret, endpoints)?));
        }
        if let Some((client_id, client_secret, base)) = client(
            &OAuthClientConfig::from_figment(figment, "zoho")?,
            "ZOHO",
            development,
        )? {
            let centres = base.map_or(Centres::Zoho, Centres::Fake);
            deps.zoho = Some(Arc::new(Zoho::new(client_id, client_secret, centres)?));
        }
        if deps.google.is_none() && deps.zoho.is_none() && !deps.config.imap.enabled {
            return Ok(deps);
        }
        let Some(secret) = secret else {
            let setting = if deps.google.is_some() {
                "CAMPBOOKS_GOOGLE__CLIENT_ID"
            } else if deps.zoho.is_some() {
                "CAMPBOOKS_ZOHO__CLIENT_ID"
            } else {
                "CAMPBOOKS_CONNECTORS__IMAP__ENABLED"
            };
            return Err(format!(
                "{setting} is set, so connections need \
                 CAMPBOOKS_SECRET_KEY_FILE or CAMPBOOKS_SECRET_KEY (32 random bytes, base64; \
                 e.g. `openssl rand -base64 32`) to encrypt the credentials they keep"
            ));
        };
        deps.cipher = Some(Arc::new(Cipher::new(&secret, CREDENTIALS_PURPOSE)));
        Ok(deps)
    }
}

/// A provider's client id, secret and (development only) fake base, when
/// its `CAMPBOOKS_<PREFIX>__CLIENT_ID` and a secret are set.
fn client(
    settings: &OAuthClientConfig,
    prefix: &str,
    development: bool,
) -> Result<Option<(String, ClientSecret, Option<url::Url>)>, String> {
    let client_id = settings
        .client_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let client_secret = match (&settings.client_secret_file, &settings.client_secret) {
        (Some(path), _) if !path.as_os_str().is_empty() => Some(ClientSecret::File(path.clone())),
        (_, Some(secret)) => Some(ClientSecret::Inline(secret.clone())),
        _ => None,
    };
    let (Some(client_id), Some(client_secret)) = (client_id, client_secret) else {
        return Ok(None);
    };
    let base = match settings
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        None => None,
        Some(_) if !development => {
            return Err(format!(
                "CAMPBOOKS_{prefix}__BASE_URL points {} at another server: it is for \
                 development only (CAMPBOOKS_ENV=development)",
                prefix.to_ascii_lowercase()
            ));
        }
        Some(base) => Some(
            url::Url::parse(&format!("{}/", base.trim_end_matches('/')))
                .map_err(|e| format!("CAMPBOOKS_{prefix}__BASE_URL={base}: {e}"))?,
        ),
    };
    Ok(Some((client_id.to_owned(), client_secret, base)))
}

#[cfg(test)]
mod tests {
    use figment::providers::Serialized;

    use super::*;

    fn kernel_config() -> Config {
        campbooks_kernel::testing::config("postgres://test@localhost/test")
    }

    #[test]
    fn inert_without_a_google_client() {
        let deps = Deps::from_figment(&Figment::new(), &kernel_config(), None).unwrap();
        assert!(deps.google.is_none() && deps.cipher.is_none());
    }

    #[test]
    fn google_needs_the_instance_secret() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "google": {"client_id": "client-1", "client_secret": "s3cret"}
        })));
        let err = Deps::from_figment(&figment, &kernel_config(), None).unwrap_err();
        assert!(err.contains("CAMPBOOKS_SECRET_KEY"), "{err}");
        let deps = Deps::from_figment(
            &figment,
            &kernel_config(),
            Some(SecretKey::from_bytes([1; 32])),
        )
        .unwrap();
        assert!(deps.google.is_some());
        assert!(deps.cipher.is_some());
    }

    #[test]
    fn zoho_is_offered_with_its_client_and_the_instance_secret() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "zoho": {"client_id": "zoho-1", "client_secret": "s3cret",
                     "redirect_path": "/oauth/zoho/callback"}
        })));
        let err = Deps::from_figment(&figment, &kernel_config(), None).unwrap_err();
        assert!(err.contains("CAMPBOOKS_ZOHO__CLIENT_ID"), "{err}");
        let deps = Deps::from_figment(
            &figment,
            &kernel_config(),
            Some(SecretKey::from_bytes([1; 32])),
        )
        .unwrap();
        let zoho = deps.zoho.unwrap();
        assert!(deps.google.is_none() && deps.cipher.is_some());
        assert!(zoho.centre("https://accounts.zoho.eu").is_some());
        // A fake Zoho is for development only.
        let fake = Figment::from(Serialized::defaults(serde_json::json!({
            "zoho": {"client_id": "zoho-1", "client_secret": "s3cret",
                     "base_url": "http://127.0.0.1:9052"}
        })));
        let key = || Some(SecretKey::from_bytes([1; 32]));
        let err = Deps::from_figment(&fake, &kernel_config(), key()).unwrap_err();
        assert!(err.contains("development only"), "{err}");
        let mut dev = kernel_config();
        dev.env = Environment::Development;
        let zoho = Deps::from_figment(&fake, &dev, key())
            .unwrap()
            .zoho
            .unwrap();
        assert!(zoho.centre("http://127.0.0.1:9052").is_some());
        assert!(zoho.centre("https://accounts.zoho.eu").is_none());
    }

    #[test]
    fn imap_is_offered_when_enabled_with_the_instance_secret() {
        let off = Deps::from_figment(&Figment::new(), &kernel_config(), None).unwrap();
        assert!(!off.imap_enabled());
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {"imap": {"enabled": true}}
        })));
        let err = Deps::from_figment(&figment, &kernel_config(), None).unwrap_err();
        assert!(err.contains("CAMPBOOKS_CONNECTORS__IMAP__ENABLED"), "{err}");
        let deps = Deps::from_figment(
            &figment,
            &kernel_config(),
            Some(SecretKey::from_bytes([1; 32])),
        )
        .unwrap();
        assert!(deps.imap_enabled());
        assert!(deps.google.is_none() && deps.zoho.is_none());
    }

    #[test]
    fn a_fake_google_is_for_development_only() {
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "google": {"client_id": "client-1", "client_secret": "s3cret",
                       "base_url": "http://localhost:9999"}
        })));
        let key = || Some(SecretKey::from_bytes([1; 32]));
        let err = Deps::from_figment(&figment, &kernel_config(), key()).unwrap_err();
        assert!(err.contains("development only"), "{err}");
        let mut dev = kernel_config();
        dev.env = Environment::Development;
        assert!(
            Deps::from_figment(&figment, &dev, key())
                .unwrap()
                .google
                .is_some()
        );
    }
}
