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
    config::{ConnectorsConfig, NotionClientConfig, OAuthClientConfig},
    domain::model::{Capability, Provider},
    google::{Endpoints, Google},
    imap::Imap,
    microsoft::{self, Microsoft},
    notion::{self, Notion},
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
    /// The Notion client, only while Notion is offered
    /// (`CAMPBOOKS_CONNECTORS__NOTION__ENABLED`, `CAMPBOOKS_NOTION__*` and
    /// the instance secret): `None` means not offered.
    pub notion: Option<Arc<Notion>>,
    /// Where the secret of Notion's webhook subscription comes from
    /// (`CAMPBOOKS_NOTION__WEBHOOK_SECRET[_FILE]`), when it is set and Notion
    /// is offered: without it Notion's pages are polled and the webhook is
    /// not served.
    pub notion_webhook: Option<ClientSecret>,
    /// Seals the verification token Notion posts while no webhook secret is
    /// configured; present whenever Notion is on and the instance secret is
    /// set.
    pub notion_cipher: Option<Arc<Cipher>>,
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
            .field("notion", &self.notion.is_some())
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
            notion: None,
            notion_webhook: None,
            notion_cipher: None,
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

    /// What a `provider` connection holding `held` could still be given by
    /// consenting again: calendar access for Google, and for Zoho while its
    /// adapter is on (`CAMPBOOKS_CONNECTORS__ZOHO_CALENDAR`, as the sync and
    /// the consent check it). Microsoft and IMAP offer nothing yet.
    pub fn offers(&self, provider: Provider, held: &[Capability]) -> Vec<Capability> {
        let calendar = match provider {
            Provider::Google => self.google.is_some(),
            Provider::Zoho => self.zoho.is_some() && self.config.zoho_calendar,
            Provider::Microsoft | Provider::Imap | Provider::Notion => false,
        };
        if calendar && !held.contains(&Capability::Calendar) {
            vec![Capability::Calendar]
        } else {
            Vec::new()
        }
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
        deps.microsoft = microsoft_client(figment, &deps.config, development, secret.is_some())?
            .map(|client| with_notifications(client, &deps.config, &config.public_url));
        (deps.notion, deps.notion_webhook) = notion_client(
            figment,
            &deps.config,
            &config.public_url,
            development,
            secret.is_some(),
        )?;
        // The webhook's verification token is sealed whenever Notion is on,
        // before the integration's own settings are complete.
        deps.notion_cipher = secret
            .as_ref()
            .filter(|_| deps.config.notion.is_enabled())
            .map(|secret| Arc::new(crate::domain::notion_webhook::cipher(secret)));
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
    // Unset, sign-in's tenant (what the consent went to as well): an app
    // registered for one tenant never widens.
    let tenant = settings
        .microsoft
        .tenant
        .as_deref()
        .map(str::trim)
        .map(ToOwned::to_owned)
        .or_else(|| {
            figment
                .find_value("microsoft.tenant")
                .ok()
                .and_then(|value| value.as_str().map(|tenant| tenant.trim().to_owned()))
                .filter(|tenant| !tenant.is_empty())
        })
        .unwrap_or_else(|| microsoft::DEFAULT_TENANT.to_owned());
    let endpoints = match base {
        None => microsoft::Endpoints::microsoft(&tenant)?,
        Some(base) => microsoft::Endpoints::under(&base, &tenant)?,
    };
    Ok(Some(Arc::new(Microsoft::new(
        client_id,
        client_secret,
        endpoints,
    )?)))
}

/// The Notion client and its webhook secret, only while Notion is offered:
/// the flag on, the Notion integration configured and the instance secret
/// set (`has_secret`). With the flag off nothing of Notion's is read, so no
/// setting of it can stop the server; with it on, a missing client, secret
/// or unreadable section is a warning and Notion is not offered. Only a
/// development base outside development, which the flag asked to use, is an
/// error.
fn notion_client(
    figment: &Figment,
    settings: &ConnectorsConfig,
    public_url: &str,
    development: bool,
    has_secret: bool,
) -> Result<(Option<Arc<Notion>>, Option<ClientSecret>), String> {
    if !settings.notion.is_enabled() {
        return Ok((None, None));
    }
    let Ok(client) = NotionClientConfig::from_figment(figment) else {
        tracing::warn!(
            "CAMPBOOKS_NOTION__* could not be read: Notion is not offered although \
             CAMPBOOKS_CONNECTORS__NOTION__ENABLED is on"
        );
        return Ok((None, None));
    };
    let secret_of =
        |file: Option<&std::path::PathBuf>, inline: Option<&secrecy::SecretString>| match (
            file, inline,
        ) {
            (Some(path), _) => Some(ClientSecret::File(path.clone())),
            (_, Some(secret)) => Some(ClientSecret::Inline(secret.clone())),
            _ => None,
        };
    let (Some(client_id), Some(client_secret)) = (
        client.client_id.clone(),
        secret_of(
            client.client_secret_file.as_ref(),
            client.client_secret.as_ref(),
        ),
    ) else {
        tracing::warn!(
            "CAMPBOOKS_CONNECTORS__NOTION__ENABLED is on but CAMPBOOKS_NOTION__CLIENT_ID and a \
             secret are not set: Notion is not offered"
        );
        return Ok((None, None));
    };
    if !has_secret {
        tracing::warn!(
            "CAMPBOOKS_SECRET_KEY is not set: Notion cannot be connected (its tokens are sealed \
             with it); set it to offer Notion"
        );
        return Ok((None, None));
    }
    let endpoints = match client.base_url.as_deref() {
        None => notion::Endpoints::notion()?,
        Some(_) if !development => {
            return Err(
                "CAMPBOOKS_NOTION__BASE_URL points notion at another server: it is for \
                 development only (CAMPBOOKS_ENV=development)"
                    .to_owned(),
            );
        }
        Some(base) => {
            let base = url::Url::parse(&format!("{}/", base.trim_end_matches('/')))
                .map_err(|e| format!("CAMPBOOKS_NOTION__BASE_URL={base}: {e}"))?;
            notion::Endpoints::under(&base)?
        }
    };
    let path = client
        .redirect_path
        .as_deref()
        .filter(|path| path.starts_with('/') && !path.starts_with("//"))
        .unwrap_or(notion::CALLBACK_PATH);
    let redirect = format!("{}{path}", public_url.trim().trim_end_matches('/'));
    let notion = Notion::new(client_id, client_secret, endpoints, redirect)?;
    let webhook = secret_of(
        client.webhook_secret_file.as_ref(),
        client.webhook_secret.as_ref(),
    );
    Ok((Some(Arc::new(notion)), webhook))
}

/// `client` posting Graph's change notifications to this instance when
/// `CAMPBOOKS_CONNECTORS__MICROSOFT__NOTIFICATIONS` asks for them and
/// `public_url` can take them ([`microsoft::notification_url`]); otherwise
/// as it is, its mailboxes polled, with a warning when they were asked for.
/// Never an error: notifications never stop the server.
fn with_notifications(
    client: Arc<Microsoft>,
    settings: &ConnectorsConfig,
    public_url: &str,
) -> Arc<Microsoft> {
    if !settings.microsoft.adapter().notifications {
        return client;
    }
    let Some(url) = microsoft::notification_url(public_url) else {
        tracing::warn!(
            "CAMPBOOKS_CONNECTORS__MICROSOFT__NOTIFICATIONS is on but CAMPBOOKS_PUBLIC_URL is not \
             an https address Microsoft can reach: Microsoft mailboxes are polled instead"
        );
        return client;
    };
    Arc::new(Microsoft::clone(&client).with_notifications(url))
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

    fn notion_on(extra: serde_json::Value) -> Figment {
        let mut settings = serde_json::json!({
            "notion": {"client_id": "n", "client_secret": "s"},
            "connectors": {"notion": {"enabled": true}}
        });
        for (key, value) in extra.as_object().cloned().unwrap_or_default() {
            settings["notion"][key] = value;
        }
        Figment::from(Serialized::defaults(settings))
    }

    #[test]
    fn notion_is_not_offered_while_its_flag_is_off() {
        let key = || Some(SecretKey::from_bytes([1; 32]));
        // The integration's credentials alone, and every other setting
        // broken: it starts, Notion not offered.
        let off = Figment::from(Serialized::defaults(serde_json::json!({
            "notion": {"client_id": "n", "client_secret": "s",
                       "base_url": "http://127.0.0.1:9051"},
            "connectors": {"notion": {"enabled": false, "max_blocks": "lots"}}
        })));
        for secret in [None, key()] {
            let deps = Deps::from_figment(&off, &kernel_config(), secret).unwrap();
            assert!(deps.notion.is_none() && deps.notion_webhook.is_none());
            assert!(deps.notion_cipher.is_none(), "off: no hook, no sealing");
        }
        // Production's settings: its poll bounds, no Notion variable.
        let production = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {"poll": {"min_seconds": 60, "max_seconds": 120}},
            "google": {"client_id": "client-1", "client_secret": "s3cret"}
        })));
        let deps = Deps::from_figment(&production, &kernel_config(), key()).unwrap();
        assert!(deps.notion.is_none());
    }

    #[test]
    fn notion_is_offered_with_its_flag_its_integration_and_the_secret_key() {
        let key = || Some(SecretKey::from_bytes([1; 32]));
        let mut public = kernel_config();
        public.public_url = "https://campbooks.example.com/".to_owned();
        let deps = Deps::from_figment(&notion_on(serde_json::json!({})), &public, key()).unwrap();
        assert!(deps.notion_cipher.is_some());
        let notion = deps.notion.unwrap();
        assert_eq!(
            notion.redirect_uri(),
            "https://campbooks.example.com/auth/callback/notion"
        );
        assert!(deps.notion_webhook.is_none(), "no webhook without its secret");
        let hooked = notion_on(serde_json::json!({"webhook_secret": "w"}));
        assert!(
            Deps::from_figment(&hooked, &public, key())
                .unwrap()
                .notion_webhook
                .is_some()
        );
        // A redirect path must be a path of this instance.
        for (path, expected) in [
            ("/cb/notion", "https://campbooks.example.com/cb/notion"),
            ("//evil.example.com", "https://campbooks.example.com/auth/callback/notion"),
            ("evil", "https://campbooks.example.com/auth/callback/notion"),
        ] {
            let figment = notion_on(serde_json::json!({"redirect_path": path}));
            let deps = Deps::from_figment(&figment, &public, key()).unwrap();
            assert_eq!(deps.notion.unwrap().redirect_uri(), expected, "{path}");
        }
    }

    #[test]
    fn notion_without_its_integration_or_the_secret_key_is_a_warning_not_a_stop() {
        let key = || Some(SecretKey::from_bytes([1; 32]));
        let no_client = Figment::from(Serialized::defaults(serde_json::json!({
            "connectors": {"notion": {"enabled": true}}
        })));
        let deps = Deps::from_figment(&no_client, &kernel_config(), key()).unwrap();
        assert!(deps.notion.is_none());
        // The hook still takes Notion's verification token: it seals it.
        assert!(deps.notion_cipher.is_some());
        let no_secret = Figment::from(Serialized::defaults(serde_json::json!({
            "notion": {"client_id": "n"},
            "connectors": {"notion": {"enabled": true}}
        })));
        assert!(
            Deps::from_figment(&no_secret, &kernel_config(), key())
                .unwrap()
                .notion
                .is_none()
        );
        let deps = Deps::from_figment(&notion_on(serde_json::json!({})), &kernel_config(), None)
            .unwrap();
        assert!(deps.notion.is_none() && deps.notion_cipher.is_none());
    }

    #[test]
    fn a_notion_fake_outside_development_stops_the_server_but_not_in_development() {
        let key = || Some(SecretKey::from_bytes([1; 32]));
        let fake = notion_on(serde_json::json!({"base_url": "http://127.0.0.1:9051"}));
        let err = Deps::from_figment(&fake, &kernel_config(), key()).unwrap_err();
        assert!(err.contains("development only"), "{err}");
        let mut dev = kernel_config();
        dev.env = Environment::Development;
        assert!(
            Deps::from_figment(&fake, &dev, key())
                .unwrap()
                .notion
                .is_some()
        );
        // On with a broken adapter setting: stopped, naming it.
        let broken = Figment::from(Serialized::defaults(serde_json::json!({
            "notion": {"client_id": "n", "client_secret": "s"},
            "connectors": {"notion": {"enabled": true, "max_depth": 0}}
        })));
        let err = Deps::from_figment(&broken, &kernel_config(), key()).unwrap_err();
        assert!(err.contains("MAX_DEPTH"), "{err}");
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
        assert!(deps.cipher.is_some());
        assert_eq!(
            deps.microsoft.unwrap().token_endpoint().as_str(),
            "https://login.microsoftonline.com/common/oauth2/v2.0/token"
        );
        // The mail tenant, unset, is sign-in's: a single-tenant app never
        // widens. Set, it is its own.
        let token = |settings: serde_json::Value| {
            let figment = Figment::from(Serialized::defaults(settings));
            Deps::from_figment(&figment, &kernel_config(), key())
                .unwrap()
                .microsoft
                .unwrap()
                .token_endpoint()
                .path()
                .to_owned()
        };
        let work = "72f988bf-86f1-41af-91ab-2d7cd011db47";
        assert_eq!(
            token(serde_json::json!({
                "microsoft": {"client_id": "m", "client_secret": "s", "tenant": work},
                "connectors": {"microsoft": {"enabled": true}}
            })),
            format!("/{work}/oauth2/v2.0/token")
        );
        assert_eq!(
            token(serde_json::json!({
                "microsoft": {"client_id": "m", "client_secret": "s", "tenant": work},
                "connectors": {"microsoft": {"enabled": true, "tenant": "common"}}
            })),
            "/common/oauth2/v2.0/token"
        );
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
    fn microsoft_notifications_need_a_public_https_url_and_never_stop_the_server() {
        let key = || Some(SecretKey::from_bytes([1; 32]));
        let with = |connectors: serde_json::Value| {
            Figment::from(Serialized::defaults(serde_json::json!({
                "microsoft": {"client_id": "m", "client_secret": "s"},
                "connectors": connectors
            })))
        };
        let poll = serde_json::json!({"min_seconds": 60, "max_seconds": 120});
        let mut public = kernel_config();
        public.public_url = "https://campbooks.example.com".to_owned();
        // Production's settings: no Microsoft mail, subscription or callback
        // variable. It starts, nothing of Microsoft offered.
        let production = with(serde_json::json!({"poll": poll}));
        for config in [kernel_config(), public.clone()] {
            let deps = Deps::from_figment(&production, &config, key()).unwrap();
            assert!(deps.microsoft.is_none());
        }
        // Off, notifications asked for in any shape: it starts.
        for asked in [serde_json::json!(true), serde_json::json!("maybe")] {
            let off =
                with(serde_json::json!({"poll": poll, "microsoft": {"notifications": asked}}));
            let deps = Deps::from_figment(&off, &public, key()).unwrap();
            assert!(deps.microsoft.is_none());
        }
        // On without notifications: polled.
        let on = with(serde_json::json!({"poll": poll, "microsoft": {"enabled": true}}));
        let deps = Deps::from_figment(&on, &public, key()).unwrap();
        assert!(deps.microsoft.unwrap().notification_url().is_none());
        // On with them and a public https URL: posted to this instance.
        let notify = with(serde_json::json!({
            "poll": poll, "microsoft": {"enabled": true, "notifications": true}
        }));
        let deps = Deps::from_figment(&notify, &public, key()).unwrap();
        assert_eq!(
            deps.microsoft
                .unwrap()
                .notification_url()
                .map(url::Url::as_str),
            Some("https://campbooks.example.com/hooks/microsoft/mail")
        );
        // Without a URL Microsoft can reach: polled, and it starts.
        for unusable in [
            "http://localhost:8080",
            "https://localhost",
            "https://10.0.0.5",
            "http://campbooks.example.com",
        ] {
            let mut config = kernel_config();
            config.public_url = unusable.to_owned();
            let deps = Deps::from_figment(&notify, &config, key()).unwrap();
            assert!(
                deps.microsoft.unwrap().notification_url().is_none(),
                "{unusable}"
            );
        }
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
