//! Any mailbox over IMAP and SMTP (ADR 0010, campbooks-cloud#310): finding
//! its servers from the address ([`discover`]) and checking that its
//! password opens both ([`verify`]) before it becomes a connection; then
//! reading it (campbooks-cloud#311, [`read`]): its folders as places
//! ([`folders`]), each message once wherever it is ([`state`]).
//! Write-back (#312) and sending (#313) come later, over the same
//! [`session::Session`].
//!
//! Every host is reached through the kernel's SSRF guard
//! ([`campbooks_kernel::net::guard`]), at the address it checked, on a mail
//! port; TLS (implicit or STARTTLS) is required, and certificates are
//! checked against the host name. A server's own words are never repeated
//! or logged: what went wrong is a stable [`Refusal`] code.

pub mod discover;
pub mod folders;
mod ispdb;
mod ispdb_files;
pub mod net;
pub mod read;
pub mod session;
pub mod smtp;
pub mod state;
pub mod syntax;
pub mod tls;

use std::{net::SocketAddr, sync::Arc, time::Duration};

use campbooks_kernel::{
    Error,
    net::guard::{self, Policy, Refused, Resolve, SystemResolver},
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tokio_rustls::rustls::pki_types::CertificateDer;
use utoipa::ToSchema;

use crate::config::ImapConfig;

/// The ports mail is reached on: IMAP over TLS (993) or STARTTLS (143),
/// SMTP submission over TLS (465) or STARTTLS (587).
pub const MAIL_PORTS: &[u16] = &[993, 143, 465, 587];

/// How a connection to a mail server is encrypted. Never in the clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Security {
    /// TLS from the first byte (IMAP 993, SMTP 465).
    Tls,
    /// A plain connection upgraded with STARTTLS before anything else is
    /// said (IMAP 143, SMTP 587); refused when the server does not offer it.
    Starttls,
}

impl Security {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tls => "tls",
            Self::Starttls => "starttls",
        }
    }
}

/// A mail server.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Server {
    /// Its host name (or IP address).
    pub host: String,
    /// Its port.
    pub port: u16,
    /// How the connection is encrypted.
    pub security: Security,
}

impl Server {
    /// The same server with its host lower-cased and trimmed of a final
    /// dot; `None` when the host is neither a host name nor an IP address.
    pub fn normalized(&self) -> Option<Self> {
        let host = self.host.trim().trim_end_matches('.').to_ascii_lowercase();
        let literal = host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(&host);
        let ok = literal.parse::<std::net::IpAddr>().is_ok() || guard::is_host_name(&host);
        (ok && self.port != 0).then(|| Self {
            host: literal.to_owned(),
            port: self.port,
            security: self.security,
        })
    }
}

/// A mailbox's two servers: where its mail is read, and where it sends.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Servers {
    /// The IMAP server.
    pub imap: Server,
    /// The SMTP submission server.
    pub smtp: Server,
}

/// Where a mailbox's settings came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The provider database bundled with Campbooks (Thunderbird's ISPDB).
    Ispdb,
    /// The domain's own autoconfig document.
    Autoconfig,
    /// The domain's DNS SRV records (RFC 6186).
    Srv,
    /// The domain's MX host: the provider it belongs to, or a guess.
    Mx,
    /// Typed in by the member.
    Manual,
}

impl Source {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ispdb => "ispdb",
            Self::Autoconfig => "autoconfig",
            Self::Srv => "srv",
            Self::Mx => "mx",
            Self::Manual => "manual",
        }
    }
}

/// Why a mailbox's servers were not reached or did not let it in: a stable
/// code, explained in the module's catalogue. Never the server's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The IMAP server refused the username and password.
    ImapLoginRefused,
    /// The IMAP server is busy (`UNAVAILABLE`, `INUSE`, `LIMIT`): it did
    /// not refuse the password.
    ImapBusy,
    /// The SMTP server refused them (or asks for no sign-in at all).
    SmtpLoginRefused,
    /// The IMAP server could not be reached, or did not answer in time.
    ImapUnreachable,
    /// The SMTP server could not be reached, or did not answer in time.
    SmtpUnreachable,
    /// The IMAP server's certificate is not valid for its name.
    ImapTlsFailed,
    /// The SMTP server's certificate is not valid for its name.
    SmtpTlsFailed,
    /// What answered is not an IMAP server.
    ImapNotAnImapServer,
    /// The host is not one Campbooks may reach (a private or metadata
    /// address, or a port other than the mail ports).
    HostRefused,
    /// A server on a STARTTLS port does not offer STARTTLS: nothing was
    /// sent in the clear.
    StarttlsMissing,
}

impl Refusal {
    /// The stable code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::ImapLoginRefused => "imap_login_refused",
            Self::ImapBusy => "imap_busy",
            Self::SmtpLoginRefused => "smtp_login_refused",
            Self::ImapUnreachable => "imap_unreachable",
            Self::SmtpUnreachable => "smtp_unreachable",
            Self::ImapTlsFailed => "imap_tls_failed",
            Self::SmtpTlsFailed => "smtp_tls_failed",
            Self::ImapNotAnImapServer => "imap_not_an_imap_server",
            Self::HostRefused => "host_refused",
            Self::StarttlsMissing => "starttls_missing",
        }
    }

    /// Whether a server refused the password (what counts towards the
    /// failed-login limit).
    pub const fn is_login(self) -> bool {
        matches!(self, Self::ImapLoginRefused | Self::SmtpLoginRefused)
    }

    /// Why `refused` stopped the guard: an unknown name is a server that
    /// cannot be reached (`unreachable`); anything else is a host Campbooks
    /// may not reach.
    fn of(refused: Refused, unreachable: Self) -> Self {
        match refused {
            Refused::Unresolved | Refused::Unavailable => unreachable,
            _ => Self::HostRefused,
        }
    }
}

/// How a mail server's certificate is checked: against the platform's
/// roots in production; against the test servers' own root in tests.
#[derive(Clone, Default)]
pub enum Trust {
    /// The platform's verifier and roots.
    #[default]
    Platform,
    /// Only these roots (tests).
    Roots(Vec<CertificateDer<'static>>),
}

impl std::fmt::Debug for Trust {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Platform => f.write_str("Platform"),
            Self::Roots(roots) => write!(f, "Roots({})", roots.len()),
        }
    }
}

/// What reaching mail servers needs: the settings, the resolvers (the
/// system's in production, fakes in tests) and the trust.
#[derive(Clone)]
pub struct Imap {
    /// `CAMPBOOKS_CONNECTORS__IMAP__*`.
    pub config: ImapConfig,
    /// Resolves the servers' names (through the guard).
    pub resolver: Arc<dyn Resolve>,
    /// Looks up SRV and MX records.
    pub dns: Arc<dyn net::Dns>,
    /// Fetches autoconfig documents (through the guard).
    pub web: Arc<dyn net::Web>,
    /// How certificates are checked.
    pub trust: Trust,
}

impl std::fmt::Debug for Imap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Imap")
            .field("config", &self.config)
            .field("trust", &self.trust)
            .finish_non_exhaustive()
    }
}

impl Default for Imap {
    fn default() -> Self {
        Self::new(ImapConfig::default())
    }
}

impl Imap {
    /// The production adapter with `config`: the system's resolvers and the
    /// platform's roots.
    pub fn new(config: ImapConfig) -> Self {
        Self {
            config,
            resolver: Arc::new(SystemResolver),
            dns: Arc::new(net::SystemDns::default()),
            web: Arc::new(net::GuardedWeb),
            trust: Trust::Platform,
        }
    }

    /// What the guard lets this instance reach.
    pub fn policy(&self) -> Policy {
        Policy {
            allow_private: self.config.allow_private_hosts,
            allow_any_port: self.config.allow_any_port,
            ports: MAIL_PORTS,
        }
    }

    /// How long opening a connection may take.
    pub fn connect_timeout(&self) -> Duration {
        Duration::from_secs(u64::from(self.config.connect_timeout_seconds))
    }

    /// How long talking to one server may take in all.
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(u64::from(self.config.timeout_seconds))
    }

    /// The one address to connect to for `server`, checked by the guard:
    /// connect to it, never to the name again.
    pub async fn reach(&self, server: &Server) -> Result<SocketAddr, Refused> {
        guard::resolve_socket_with(&*self.resolver, &server.host, server.port, self.policy()).await
    }
}

/// The IMAP hosts of the big mail providers, which serve millions of
/// mailboxes each: their sessions are capped as a provider's rather than as
/// a small server's. A short list kept by hand, matched exactly: every other
/// host (a small ISP, a hosting company, a self-hosted server) keeps the
/// small hosts' caps, which protect it.
pub const PROVIDER_HOSTS: [&str; 20] = [
    "export.imap.mail.yahoo.com",
    "imap-mail.outlook.com",
    "imap.aol.com",
    "imap.fastmail.com",
    "imap.gmail.com",
    "imap.gmx.com",
    "imap.gmx.net",
    "imap.googlemail.com",
    "imap.mail.com",
    "imap.mail.me.com",
    "imap.mail.ru",
    "imap.mail.yahoo.com",
    "imap.web.de",
    "imap.yandex.com",
    "imap.yandex.ru",
    "imap.zoho.com",
    "imap.zoho.eu",
    "imap.zoho.in",
    "outlook.office365.com",
    "imap.mail.yahoo.co.jp",
];

/// Whether `host` is one of [`PROVIDER_HOSTS`].
pub fn is_provider_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    PROVIDER_HOSTS.contains(&host.as_str())
}

/// Sign in to `servers` as `username` with `password`: the IMAP server (and
/// its inbox listed), then the SMTP server (authenticated, nothing sent).
/// The first refusal stops it.
pub async fn verify(
    imap: &Imap,
    servers: &Servers,
    username: &str,
    password: &SecretString,
) -> Result<(), Refusal> {
    Box::pin(session::verify(imap, &servers.imap, username, password)).await?;
    Box::pin(smtp::verify(imap, &servers.smtp, username, password)).await
}

/// The kernel error a refusal answers with. `app_password` says the
/// provider wants an app password: a login refusal then says so.
pub fn refusal_error(refusal: Refusal, app_password: bool) -> Error {
    let reason = crate::MESSAGES.reason(refusal.code());
    let reason = if refusal.is_login() {
        // `count` picks the message: one for a provider that wants an app
        // password, the general one otherwise.
        reason.arg(
            campbooks_kernel::i18n::COUNT,
            if app_password { 1 } else { 2 },
        )
    } else {
        reason
    };
    Error::BadRequest(reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_are_normalized() {
        let server = |host: &str| Server {
            host: host.to_owned(),
            port: 993,
            security: Security::Tls,
        };
        assert_eq!(
            server(" IMAP.Example.COM. ").normalized().unwrap().host,
            "imap.example.com"
        );
        assert_eq!(
            server("[2001:db8::1]").normalized().unwrap().host,
            "2001:db8::1"
        );
        assert_eq!(server("192.0.2.1").normalized().unwrap().host, "192.0.2.1");
        for bad in ["", "imap example.com", "user@imap.example.com", "imap/x"] {
            assert!(server(bad).normalized().is_none(), "{bad}");
        }
    }

    #[test]
    fn known_providers_are_a_short_list_matched_exactly() {
        assert!(is_provider_host("imap.mail.yahoo.com"));
        assert!(is_provider_host("IMAP.mail.me.com."));
        assert!(!is_provider_host("smtp.mail.yahoo.com"));
        assert!(!is_provider_host("imap.mail.yahoo.com.example.com"));
        assert!(!is_provider_host("evil.imap.gmail.com"));
        assert!(!is_provider_host("imap.example.com"));
        // A small ISP of the bundled database keeps the small hosts' caps.
        assert!(!is_provider_host("imap.bakery-isp.example"));
        assert!(
            PROVIDER_HOSTS
                .iter()
                .all(|host| guard::is_host_name(host) && *host == host.to_ascii_lowercase())
        );
    }

    #[test]
    fn a_login_refusal_says_when_an_app_password_is_needed() {
        use campbooks_kernel::i18n::Locale;
        let render = |error: Error| match error {
            Error::BadRequest(reason) => (reason.code(), reason.render(Locale::En)),
            other => panic!("{other:?}"),
        };
        let (code, app) = render(refusal_error(Refusal::ImapLoginRefused, true));
        let (same, plain) = render(refusal_error(Refusal::ImapLoginRefused, false));
        assert_eq!((code, same), ("imap_login_refused", "imap_login_refused"));
        assert!(app.contains("app password"), "{app}");
        assert_ne!(app, plain);
        let (code, _) = render(refusal_error(Refusal::HostRefused, true));
        assert_eq!(code, "host_refused");
    }
}
