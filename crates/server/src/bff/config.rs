//! BFF settings: `CAMPBOOKS_BFF__*` plus the instance secret
//! (`CAMPBOOKS_SECRET_KEY` or `CAMPBOOKS_SECRET_KEY_FILE`).

use std::{path::PathBuf, sync::Arc};

use campbooks_kernel::Config;
use chrono::TimeDelta;
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;

use campbooks_kernel::crypto::{Cipher, SecretKey};

/// The HKDF label of the BFF's token-encryption key. Never change it: the
/// tokens of every live session were sealed under it.
pub const KEY_PURPOSE: &[u8] = b"campbooks bff token encryption v1";

/// Scopes every web sign-in asks for, before the API's own.
pub const OIDC_SCOPES: [&str; 4] = ["openid", "profile", "email", "offline_access"];

/// How long a rotated-away session id keeps working, so requests already in
/// flight when the id changed still succeed.
pub const ROTATION_GRACE: TimeDelta = TimeDelta::minutes(2);

/// `CAMPBOOKS_BFF__*`.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BffConfig {
    /// The web app's OIDC client (`CAMPBOOKS_BFF__CLIENT_ID`).
    pub client_id: String,
    /// Its secret, inline (`CAMPBOOKS_BFF__CLIENT_SECRET`); for development.
    pub client_secret: Option<SecretString>,
    /// A file holding its secret (`CAMPBOOKS_BFF__CLIENT_SECRET_FILE`); what
    /// docker compose uses. Read when first needed, and again after the IdP
    /// rejects it, so a rotated secret is picked up without a restart.
    pub client_secret_file: Option<PathBuf>,
    /// The issuer as the server reaches it (`CAMPBOOKS_BFF__IDP_INTERNAL_URL`,
    /// e.g. `http://idp:8080/auth/v1/` in compose). Discovery, token and
    /// revocation requests go there; the browser still uses the public
    /// issuer. Unset: the issuer itself.
    pub idp_internal_url: Option<String>,
    /// Space-separated scopes to request (`CAMPBOOKS_BFF__SCOPES`). Unset:
    /// `openid profile email offline_access` plus every API scope.
    pub scopes: Option<String>,
    /// A session unused this long ends (`CAMPBOOKS_BFF__IDLE_TIMEOUT_MINUTES`).
    pub idle_timeout_minutes: u64,
    /// A session ends this long after sign-in, used or not
    /// (`CAMPBOOKS_BFF__ABSOLUTE_TIMEOUT_HOURS`).
    pub absolute_timeout_hours: u64,
    /// A session's cookie value is replaced this often
    /// (`CAMPBOOKS_BFF__ROTATE_MINUTES`).
    pub rotate_minutes: u64,
    /// How long a started sign-in may take at the IdP
    /// (`CAMPBOOKS_BFF__LOGIN_TIMEOUT_SECONDS`).
    pub login_timeout_seconds: u64,
}

impl Default for BffConfig {
    fn default() -> Self {
        Self {
            client_id: "campbooks-web".to_owned(),
            client_secret: None,
            client_secret_file: None,
            idp_internal_url: None,
            scopes: None,
            // Rauthy's refresh tokens live 48 h: an idle session cannot
            // outlive its refresh token anyway.
            idle_timeout_minutes: 48 * 60,
            absolute_timeout_hours: 30 * 24,
            rotate_minutes: 60,
            login_timeout_seconds: 600,
        }
    }
}

/// Everything read from the environment.
#[derive(Debug, Deserialize)]
struct Env {
    #[serde(default)]
    bff: BffConfig,
}

/// Where the client secret comes from.
#[derive(Debug, Clone)]
pub enum ClientSecret {
    /// `CAMPBOOKS_BFF__CLIENT_SECRET`.
    Inline(SecretString),
    /// `CAMPBOOKS_BFF__CLIENT_SECRET_FILE`.
    File(PathBuf),
}

/// Session lifetimes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// Ends a session nobody used for this long.
    pub idle: TimeDelta,
    /// Ends a session this long after sign-in.
    pub absolute: TimeDelta,
    /// Replaces a session's cookie value this often.
    pub rotate: TimeDelta,
    /// Lifetime of a started sign-in.
    pub login: TimeDelta,
}

/// A BFF ready to run: the web app can sign in.
#[derive(Debug, Clone)]
pub struct Settings {
    /// `scheme://host[:port]` of `CAMPBOOKS_PUBLIC_URL`, what the browser
    /// sees: the base of the redirect URIs and the only accepted `Origin`.
    pub public_origin: String,
    /// Cookies are `Secure` and `__Host-` prefixed (an `https` public URL).
    pub secure_cookies: bool,
    /// The trusted issuer (`CAMPBOOKS_AUTH__ISSUER`).
    pub issuer: String,
    /// Where the server reaches the issuer.
    pub internal_issuer: String,
    /// The OIDC client id.
    pub client_id: String,
    /// The OIDC client secret.
    pub client_secret: ClientSecret,
    /// Space-separated scopes requested at sign-in.
    pub scopes: String,
    /// Session lifetimes.
    pub timeouts: Timeouts,
    /// Encrypts tokens at rest.
    pub cipher: Arc<Cipher>,
    /// Talks to the IdP.
    pub http: reqwest::Client,
}

/// What [`load`] found.
#[derive(Debug)]
pub enum Loaded {
    /// Web sign-in works.
    Enabled(Box<Settings>),
    /// Web sign-in is off; every BFF endpoint but `/bff/session` answers 503.
    Disabled(&'static str),
}

/// Read the BFF settings from the environment. `api_scopes` is the scope
/// catalogue, requested at sign-in unless `CAMPBOOKS_BFF__SCOPES` says
/// otherwise. A broken setting is an error naming it.
pub fn load(config: &Config, api_scopes: &[&str]) -> Result<Loaded, String> {
    let env: Env = campbooks_kernel::config::env_source()
        .extract()
        .map_err(|e| format!("invalid configuration: {e}"))?;
    let secret = SecretKey::from_env()?;
    settings(config, env.bff, secret, api_scopes)
}

/// [`load`] from values already read (tests use it directly).
pub fn settings(
    config: &Config,
    bff: BffConfig,
    secret: Option<SecretKey>,
    api_scopes: &[&str],
) -> Result<Loaded, String> {
    let Some(issuer) = config.auth.issuer() else {
        return Ok(Loaded::Disabled(
            "CAMPBOOKS_AUTH__ISSUER is unset: there is no identity provider to sign in with",
        ));
    };
    let timeouts = timeouts(&bff)?;
    let client_secret = match (bff.client_secret_file, bff.client_secret) {
        (Some(path), _) if !path.as_os_str().is_empty() => ClientSecret::File(path),
        (_, Some(secret)) if !secret.expose_secret().trim().is_empty() => {
            ClientSecret::Inline(secret)
        }
        _ => {
            return Ok(Loaded::Disabled(
                "neither CAMPBOOKS_BFF__CLIENT_SECRET_FILE nor CAMPBOOKS_BFF__CLIENT_SECRET is set",
            ));
        }
    };
    let Some(secret) = secret else {
        return Err(
            "web sign-in needs CAMPBOOKS_SECRET_KEY_FILE or CAMPBOOKS_SECRET_KEY (32 random \
             bytes, base64; e.g. `openssl rand -base64 32`) to encrypt the tokens it keeps"
                .to_owned(),
        );
    };
    if bff.client_id.trim().is_empty() {
        return Err("CAMPBOOKS_BFF__CLIENT_ID must not be blank".to_owned());
    }
    let (public_origin, secure_cookies) = public_origin(&config.public_url)?;
    let internal_issuer = match bff
        .idp_internal_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        Some(url) => {
            check_http_url("CAMPBOOKS_BFF__IDP_INTERNAL_URL", url)?;
            format!("{}/", url.trim_end_matches('/'))
        }
        None => issuer.to_owned(),
    };
    let scopes = match bff.scopes.as_deref().map(str::trim) {
        Some(scopes) if !scopes.is_empty() => scopes.to_owned(),
        _ => OIDC_SCOPES
            .iter()
            .chain(api_scopes)
            .copied()
            .collect::<Vec<_>>()
            .join(" "),
    };
    if !scopes.split_whitespace().any(|scope| scope == "openid") {
        return Err("CAMPBOOKS_BFF__SCOPES must include `openid`".to_owned());
    }
    let http = reqwest::Client::builder()
        .timeout(super::oidc::HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("campbooks-server/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("could not build the identity provider client: {e}"))?;
    Ok(Loaded::Enabled(Box::new(Settings {
        public_origin,
        secure_cookies,
        issuer: issuer.to_owned(),
        internal_issuer,
        client_id: bff.client_id.trim().to_owned(),
        client_secret,
        scopes,
        timeouts,
        cipher: Arc::new(Cipher::new(&secret, KEY_PURPOSE)),
        http,
    })))
}

fn timeouts(bff: &BffConfig) -> Result<Timeouts, String> {
    let checks = [
        (
            "IDLE_TIMEOUT_MINUTES",
            bff.idle_timeout_minutes >= 5,
            "at least 5",
        ),
        (
            "ABSOLUTE_TIMEOUT_HOURS",
            bff.absolute_timeout_hours >= 1,
            "at least 1",
        ),
        ("ROTATE_MINUTES", bff.rotate_minutes >= 5, "at least 5"),
        (
            "LOGIN_TIMEOUT_SECONDS",
            (60..=3_600).contains(&bff.login_timeout_seconds),
            "between 60 and 3600",
        ),
    ];
    if let Some((name, _, rule)) = checks.iter().find(|(_, ok, _)| !ok) {
        return Err(format!("CAMPBOOKS_BFF__{name} must be {rule}"));
    }
    let minutes = |m: u64| TimeDelta::minutes(i64::try_from(m).unwrap_or(i64::MAX / 60_000));
    Ok(Timeouts {
        idle: minutes(bff.idle_timeout_minutes),
        absolute: minutes(bff.absolute_timeout_hours.saturating_mul(60)),
        rotate: minutes(bff.rotate_minutes),
        login: TimeDelta::seconds(i64::try_from(bff.login_timeout_seconds).unwrap_or(3_600)),
    })
}

/// `scheme://host[:port]` of the public URL and whether it is `https`. The
/// app must sit at the root of its origin: `__Host-` cookies need `Path=/`.
fn public_origin(public_url: &str) -> Result<(String, bool), String> {
    check_http_url("CAMPBOOKS_PUBLIC_URL", public_url)?;
    let url = url::Url::parse(public_url.trim())
        .map_err(|e| format!("CAMPBOOKS_PUBLIC_URL={public_url} is not a URL: {e}"))?;
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(format!(
            "CAMPBOOKS_PUBLIC_URL={public_url} must be an origin with no path: web sign-in \
             serves the app at the root of its host"
        ));
    }
    Ok((url.origin().ascii_serialization(), url.scheme() == "https"))
}

fn check_http_url(setting: &str, value: &str) -> Result<(), String> {
    match url::Url::parse(value.trim()) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.has_host() => Ok(()),
        Ok(_) => Err(format!("{setting}={value} must be an http(s) URL")),
        Err(e) => Err(format!("{setting}={value} is not a URL: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::testing;

    use super::*;

    fn config(issuer: Option<&str>, public_url: &str) -> Config {
        let mut config = testing::config("postgres://localhost/x");
        config.auth.issuer = issuer.map(ToOwned::to_owned);
        public_url.clone_into(&mut config.public_url);
        config
    }

    fn with_secret() -> BffConfig {
        BffConfig {
            client_secret: Some(SecretString::from("s3cret")),
            ..BffConfig::default()
        }
    }

    const ISSUER: &str = "https://auth.example.com/auth/v1/";

    fn enabled(result: Result<Loaded, String>) -> Settings {
        match result {
            Ok(Loaded::Enabled(settings)) => *settings,
            other => panic!("expected enabled, got {other:?}"),
        }
    }

    #[test]
    fn defaults_derive_from_the_public_url_and_catalogue() {
        let public = enabled(settings(
            &config(Some(ISSUER), "https://app.example.com/"),
            with_secret(),
            Some(SecretKey::from_bytes([1; 32])),
            &["system:read"],
        ));
        assert_eq!(public.public_origin, "https://app.example.com");
        assert!(public.secure_cookies);
        assert_eq!(public.internal_issuer, ISSUER);
        assert_eq!(public.client_id, "campbooks-web");
        assert_eq!(
            public.scopes,
            "openid profile email offline_access system:read"
        );
        assert_eq!(public.timeouts.idle, TimeDelta::hours(48));
        assert_eq!(public.timeouts.absolute, TimeDelta::days(30));
        assert_eq!(public.timeouts.login, TimeDelta::minutes(10));

        let local = enabled(settings(
            &config(Some(ISSUER), "http://localhost:8080"),
            BffConfig {
                idp_internal_url: Some("http://idp:8080/auth/v1".into()),
                scopes: Some("openid system:read".into()),
                ..with_secret()
            },
            Some(SecretKey::from_bytes([1; 32])),
            &[],
        ));
        assert_eq!(local.public_origin, "http://localhost:8080");
        assert!(!local.secure_cookies);
        assert_eq!(local.internal_issuer, "http://idp:8080/auth/v1/");
        assert_eq!(local.scopes, "openid system:read");
    }

    #[test]
    fn disabled_without_an_issuer_or_a_client_secret() {
        let no_issuer = settings(
            &config(None, "http://localhost:8080"),
            with_secret(),
            None,
            &[],
        );
        assert!(matches!(no_issuer, Ok(Loaded::Disabled(reason)) if reason.contains("ISSUER")));
        let no_secret = settings(
            &config(Some(ISSUER), "http://localhost:8080"),
            BffConfig::default(),
            Some(SecretKey::from_bytes([1; 32])),
            &[],
        );
        assert!(
            matches!(no_secret, Ok(Loaded::Disabled(reason)) if reason.contains("CLIENT_SECRET"))
        );
    }

    #[test]
    fn bad_settings_are_named() {
        let base = config(Some(ISSUER), "https://app.example.com");
        let err = |bff: BffConfig, secret: Option<SecretKey>, config: &Config| {
            settings(config, bff, secret, &[]).unwrap_err()
        };
        assert!(err(with_secret(), None, &base).contains("CAMPBOOKS_SECRET_KEY"));
        assert!(
            err(
                with_secret(),
                Some(SecretKey::from_bytes([1; 32])),
                &config(Some(ISSUER), "https://app.example.com/app")
            )
            .contains("CAMPBOOKS_PUBLIC_URL")
        );
        assert!(
            err(
                BffConfig {
                    idp_internal_url: Some("idp:8080".into()),
                    ..with_secret()
                },
                Some(SecretKey::from_bytes([1; 32])),
                &base
            )
            .contains("CAMPBOOKS_BFF__IDP_INTERNAL_URL")
        );
        assert!(
            err(
                BffConfig {
                    scopes: Some("profile".into()),
                    ..with_secret()
                },
                Some(SecretKey::from_bytes([1; 32])),
                &base
            )
            .contains("openid")
        );
        assert!(
            err(
                BffConfig {
                    rotate_minutes: 1,
                    ..with_secret()
                },
                Some(SecretKey::from_bytes([1; 32])),
                &base
            )
            .contains("CAMPBOOKS_BFF__ROTATE_MINUTES")
        );
    }

    #[test]
    fn a_secret_file_wins_over_an_inline_secret() {
        let loaded = enabled(settings(
            &config(Some(ISSUER), "https://app.example.com"),
            BffConfig {
                client_secret_file: Some(PathBuf::from("/run/secrets/web")),
                ..with_secret()
            },
            Some(SecretKey::from_bytes([1; 32])),
            &[],
        ));
        assert!(
            matches!(loaded.client_secret, ClientSecret::File(path) if path == std::path::Path::new("/run/secrets/web"))
        );
    }
}
