//! What the module is composed with (constructor injection): settings, the
//! Google client, the credentials cipher and the clock. Handlers reach it as
//! an axum extension; jobs and consumers own a copy.

use std::sync::Arc;

use campbooks_kernel::{
    Config,
    clock::{Clock, SystemClock},
    crypto::{Cipher, SecretKey},
};
use figment::Figment;

use crate::{
    config::{ConnectorsConfig, GoogleConfig},
    google::{ClientSecret, Endpoints, Google},
};

/// The HKDF label of the credentials key. Never change it: every stored
/// credential was sealed under it.
pub const CREDENTIALS_PURPOSE: &[u8] = b"campbooks connectors credentials v1";

/// The path of the web app page Google redirects to after consent.
pub const GOOGLE_CALLBACK_PATH: &str = "/connections/google/callback";

/// The module's dependencies. `Default` is inert: no provider configured,
/// nothing can be connected (what the OpenAPI and events catalogues use).
#[derive(Clone)]
pub struct Deps {
    /// Engine and adapter settings.
    pub config: Arc<ConnectorsConfig>,
    /// The Google client, when `CAMPBOOKS_GOOGLE__*` is set.
    pub google: Option<Arc<Google>>,
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
            .finish_non_exhaustive()
    }
}

impl Default for Deps {
    fn default() -> Self {
        Self {
            config: Arc::new(ConnectorsConfig::default()),
            google: None,
            cipher: None,
            clock: Arc::new(SystemClock),
        }
    }
}

impl Deps {
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
        let settings = ConnectorsConfig::from_figment(figment)?;
        let google = GoogleConfig::from_figment(figment)?;
        let client_id = google
            .client_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let client_secret = match (google.client_secret_file, google.client_secret) {
            (Some(path), _) if !path.as_os_str().is_empty() => Some(ClientSecret::File(path)),
            (_, Some(secret)) => Some(ClientSecret::Inline(secret)),
            _ => None,
        };
        let mut deps = Self {
            config: Arc::new(settings),
            ..Self::default()
        };
        let (Some(client_id), Some(client_secret)) = (client_id, client_secret) else {
            return Ok(deps);
        };
        let Some(secret) = secret else {
            return Err(
                "CAMPBOOKS_GOOGLE__CLIENT_ID is set, so connections need CAMPBOOKS_SECRET_KEY_FILE \
                 or CAMPBOOKS_SECRET_KEY (32 random bytes, base64; e.g. `openssl rand -base64 32`) \
                 to encrypt the credentials they keep"
                    .to_owned(),
            );
        };
        let redirect_uri = format!(
            "{}{GOOGLE_CALLBACK_PATH}",
            config.public_url.trim_end_matches('/')
        );
        deps.google = Some(Arc::new(Google::new(
            client_id.to_owned(),
            client_secret,
            redirect_uri,
            Endpoints::google(),
        )?));
        deps.cipher = Some(Arc::new(Cipher::new(&secret, CREDENTIALS_PURPOSE)));
        Ok(deps)
    }
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
        let google = deps.google.unwrap();
        assert_eq!(
            google.redirect_uri(),
            "http://localhost:8080/connections/google/callback"
        );
        assert!(deps.cipher.is_some());
    }
}
