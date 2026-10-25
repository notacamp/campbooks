//! What the module is composed with (constructor injection): settings, the
//! Google, Zoho and Microsoft clients (refresh, revoke, mail; the consent
//! itself is the identity module's), the IMAP adapter (its resolvers and trust), the
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
    microsoft::{self, Microsoft},
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
    /// The Microsoft client, only while Microsoft mail is offered
    /// (`CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED`, `CAMPBOOKS_MICROSOFT__*`
    /// and the instance secret): `None` means not offered.
    pub microsoft: Option<Arc<Microsoft>>,
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
            .field("microsoft", &self.microsoft.is_some())
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
            microsoft: None,
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
        let settings = ConnectorsConfig::from_figment(figment)?;
        let mut deps = Self {
            imap: Arc::new(Imap::new(settings.imap.clone())),
            config: Arc::new(settings),
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
        deps.microsoft = microsoft_client(figment, &deps.config, development, secret.is_some())?;
        // IMAP is on by default (campbooks-cloud#314) but needs the instance
        // secret to seal passwords: without one it is simply not offered,
        // unless an operator asked for it by name.
        let imap_asked =
            deps.config.imap.enabled && figment.find_value("connectors.imap.enabled").is_ok();
        if deps.google.is_none() && deps.zoho.is_none() && !imap_asked {
            if deps.config.imap.enabled && secret.is_none() {
                tracing::warn!(
                    "CAMPBOOKS_SECRET_KEY is not set: IMAP mailboxes cannot be connected \
                     (their passwords are sealed with it); set it to offer them"
                );
            }
            let Some(secret) = secret else {
                return Ok(deps);
            };
            deps.cipher = Some(Arc::new(Cipher::new(&secret, CREDENTIALS_PURPOSE)));
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

/// The Microsoft mail client, only while Microsoft mail is offered: the
/// flag on, the Microsoft OAuth client configured and the instance secret
/// set (`has_secret`). With the flag off nothing of Microsoft's is read, so
/// no setting of it can stop the server; with it on, a missing client or
/// secret is a warning. Only a broken setting the flag asked to use (a
/// development base outside development, a tenant `check` let through) is
/// an error.
fn microsoft_client(
    figment: &Figment,
    settings: &ConnectorsConfig,
    development: bool,
    has_secret: bool,
) -> Result<Option<Arc<Microsoft>>, String> {
    if !settings.microsoft.enabled {
        return Ok(None);
    }
    let Ok(client_settings) = OAuthClientConfig::from_figment(figment, "microsoft") else {
        tracing::warn!(
            "CAMPBOOKS_MICROSOFT__* could not be read: Microsoft mailboxes are not offered \
             although CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED is on"
        );
        return Ok(None);
    };
    let Some((client_id, client_secret, base)) =
        client(&client_settings, "MICROSOFT", development)?
    else {
        tracing::warn!(
            "CAMPBOOKS_CONNECTORS__MICROSOFT__ENABLED is on but CAMPBOOKS_MICROSOFT__CLIENT_ID and \
             a secret are not set: Microsoft mailboxes are not offered"
        );
        return Ok(None);
    };
    if !has_secret {
        tracing::warn!(
            "CAMPBOOKS_SECRET_KEY is not set: Microsoft mailboxes cannot be connected (their \
             tokens are sealed with it); set it to offer them"
        );
        return Ok(None);
    }
    let tenant = settings.microsoft.tenant.trim();
    let endpoints = match base {
        None => microsoft::Endpoints::microsoft(tenant)?,
        Some(base) => microsoft::Endpoints::under(&base, tenant)?,
    };
    Ok(Some(Arc::new(Microsoft::new(
        client_id,
        client_secret,
        endpoints,
    )?)))
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
    fn imap_is_offered_by_default_with_the_instance_secret() {
        // On by default: offered once the instance can seal passwords.
        let deps = Deps::from_figment(
            &Figment::new(),
            &kernel_config(),
            Some(SecretKey::from_bytes([1; 32])),
        )
        .unwrap();
        assert!(deps.imap_enabled());
        // Without the secret, the server still starts; IMAP is not offered.
        let off = Deps::from_figment(&Figment::new(), &kernel_config(), None).unwrap();
        assert!(!off.imap_enabled() && off.cipher.is_none());
        // Turned off by name: not offered, even with the secret.
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {"imap": {"enabled": false}}
        })));
        let deps = Deps::from_figment(
            &figment,
            &kernel_config(),
            Some(SecretKey::from_bytes([1; 32])),
        )
        .unwrap();
        assert!(!deps.imap_enabled());
    }

    #[test]
    fn imap_asked_for_by_name_needs_the_instance_secret() {
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
    fn the_module_starts_with_production_poll_bounds_and_imap_send_settings() {
        // What the server loads at startup (the 2026-10-23 outage): IMAP off
        // with stray send values, and IMAP on with the send defaults, both
        // under production's narrowed poll bounds.
        let key = || Some(SecretKey::from_bytes([1; 32]));
        let off = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {
                "poll": {"min_seconds": 60, "max_seconds": 120},
                "imap": {"enabled": false, "send_per_hour": 0, "send_timeout_seconds": 1,
                         "send_max_message_bytes": 1}
            }
        })));
        let deps = Deps::from_figment(&off, &kernel_config(), None).unwrap();
        assert!(!deps.imap_enabled());
        let on = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {
                "poll": {"min_seconds": 60, "max_seconds": 120},
                "imap": {"enabled": true}
            }
        })));
        let deps = Deps::from_figment(&on, &kernel_config(), key()).unwrap();
        assert!(deps.imap_enabled());
        assert_eq!(deps.imap.config.send_per_hour, 50);
        // A broken send setting stops only an instance that reads IMAP.
        let broken = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {"imap": {"enabled": true, "send_per_hour": 0}}
        })));
        let err = Deps::from_figment(&broken, &kernel_config(), key()).unwrap_err();
        assert!(err.contains("SEND_PER_HOUR"), "{err}");
    }

    #[test]
    fn the_module_starts_with_production_settings_and_imap_on_by_default() {
        // Production's settings exactly (campbooks-cloud#314): its poll
        // bounds, the instance secret, Google, and no IMAP variable
        // at all. IMAP is on with its defaults, IDLE off.
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}},
            "google": {"client_id": "client-1", "client_secret": "s3cret"}
        })));
        let deps = Deps::from_figment(
            &figment,
            &kernel_config(),
            Some(SecretKey::from_bytes([1; 32])),
        )
        .unwrap();
        assert!(deps.imap_enabled());
        assert!(!deps.imap.config.idle_enabled);
        assert_eq!(
            (deps.config.poll.min_seconds, deps.config.poll.max_seconds),
            (60, 120)
        );
        // The same bounds with nothing else set, with or without the secret.
        let bare = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}}
        })));
        assert!(
            Deps::from_figment(
                &bare,
                &kernel_config(),
                Some(SecretKey::from_bytes([1; 32]))
            )
            .unwrap()
            .imap_enabled()
        );
        assert!(
            !Deps::from_figment(&bare, &kernel_config(), None)
                .unwrap()
                .imap_enabled()
        );
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

    #[test]
    fn microsoft_mail_is_not_offered_while_its_flag_is_off() {
        let key = || Some(SecretKey::from_bytes([1; 32]));
        // The sign-in client alone (production may have it): not offered.
        let sign_in_only = Figment::from(Serialized::defaults(serde_json::json!({
            "microsoft": {"client_id": "m", "client_secret": "s", "tenant": "organizations"}
        })));
        let deps = Deps::from_figment(&sign_in_only, &kernel_config(), key()).unwrap();
        assert!(deps.microsoft.is_none());
        // No secret key either: it still starts (Microsoft is no provider
        // of mail here, so nothing asks for the key).
        let deps = Deps::from_figment(&sign_in_only, &kernel_config(), None).unwrap();
        assert!(deps.microsoft.is_none() && deps.cipher.is_none());
        // Off, with every other Microsoft setting broken: it starts.
        let broken = Figment::from(Serialized::defaults(serde_json::json!({
            "microsoft": {"client_id": "m", "client_secret": "s",
                          "base_url": "http://127.0.0.1:9051"},
            "connectors": {"microsoft": {"enabled": false, "tenant": "not a tenant"}}
        })));
        for secret in [None, key()] {
            let deps = Deps::from_figment(&broken, &kernel_config(), secret).unwrap();
            assert!(deps.microsoft.is_none());
        }
    }

    #[test]
    fn microsoft_mail_is_offered_with_its_flag_its_client_and_the_secret() {
        let key = || Some(SecretKey::from_bytes([1; 32]));
        let on = Figment::from(Serialized::defaults(serde_json::json!({
            "microsoft": {"client_id": "m", "client_secret": "s"},
            "connectors": {"microsoft": {"enabled": true}}
        })));
        let deps = Deps::from_figment(&on, &kernel_config(), key()).unwrap();
        assert!(deps.microsoft.is_some() && deps.cipher.is_some());
        // The client missing, or the secret key: a warning, not offered.
        let no_client = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {"microsoft": {"enabled": true}}
        })));
        let deps = Deps::from_figment(&no_client, &kernel_config(), key()).unwrap();
        assert!(deps.microsoft.is_none());
        let deps = Deps::from_figment(&on, &kernel_config(), None).unwrap();
        assert!(deps.microsoft.is_none());
        // On, the impossible combinations stop it: a fake outside
        // development, a tenant that is none.
        let fake = Figment::from(Serialized::defaults(serde_json::json!({
            "microsoft": {"client_id": "m", "client_secret": "s",
                          "base_url": "http://127.0.0.1:9051"},
            "connectors": {"microsoft": {"enabled": true}}
        })));
        let err = Deps::from_figment(&fake, &kernel_config(), key()).unwrap_err();
        assert!(err.contains("development only"), "{err}");
        let mut dev = kernel_config();
        dev.env = Environment::Development;
        assert!(
            Deps::from_figment(&fake, &dev, key())
                .unwrap()
                .microsoft
                .is_some()
        );
        let tenant = Figment::from(Serialized::defaults(serde_json::json!({
            "microsoft": {"client_id": "m", "client_secret": "s"},
            "connectors": {"microsoft": {"enabled": true, "tenant": "nope"}}
        })));
        let err = Deps::from_figment(&tenant, &kernel_config(), key()).unwrap_err();
        assert!(
            err.contains("CAMPBOOKS_CONNECTORS__MICROSOFT__TENANT"),
            "{err}"
        );
    }

    #[test]
    fn the_module_starts_with_production_settings_and_no_microsoft_mail_variable() {
        // Production passes an explicit environment list: no
        // CAMPBOOKS_CONNECTORS__MICROSOFT__* reaches it. Its poll bounds,
        // Google and the Microsoft sign-in client start, with Microsoft
        // mail not offered.
        let figment = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}},
            "google": {"client_id": "client-1", "client_secret": "s3cret"},
            "microsoft": {"client_id": "m", "client_secret_file": "/run/secrets/microsoft"}
        })));
        let deps = Deps::from_figment(
            &figment,
            &kernel_config(),
            Some(SecretKey::from_bytes([1; 32])),
        )
        .unwrap();
        assert!(deps.microsoft.is_none());
        assert!(deps.google.is_some() && deps.imap_enabled());
        assert_eq!(
            (deps.config.poll.min_seconds, deps.config.poll.max_seconds),
            (60, 120)
        );
        // Nothing set at all starts too.
        let bare = Deps::from_figment(&Figment::new(), &kernel_config(), None).unwrap();
        assert!(bare.microsoft.is_none());
    }
}
