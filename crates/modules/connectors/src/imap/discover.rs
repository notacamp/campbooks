//! Finding a mailbox's servers from its address, without sending the
//! address or its domain to anyone but the domain's own servers and DNS:
//!
//! 1. an address at a provider Campbooks connects natively (Gmail, Zoho,
//!    Outlook) is sent to that provider's sign-in;
//! 2. the bundled provider database (Thunderbird's ISPDB);
//! 3. the domain's own autoconfig document, over HTTPS through the guard;
//! 4. its DNS SRV records (RFC 6186, RFC 8314);
//! 5. its MX host: a native provider, a provider of the database by the MX
//!    host's domain, else `imap.<domain>` and `smtp.<domain>` when they
//!    resolve.
//!
//! What is found is only a suggestion: connecting signs in to it first.

use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;

use super::{
    Imap, Security, Server, Servers, Source,
    ispdb::{self, Parts, ProviderConfig},
    net::SrvTarget,
};

/// A provider Campbooks connects natively, with OAuth, rather than over
/// IMAP with a password.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Native {
    /// Gmail and Google Workspace.
    Google,
    /// Zoho Mail.
    Zoho,
    /// Outlook and Microsoft 365 (campbooks-cloud#309).
    Microsoft,
}

impl Native {
    /// The provider's name, as a member knows it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::Zoho => "Zoho",
            Self::Microsoft => "Microsoft",
        }
    }

    /// Whether Campbooks connects it today.
    pub const fn supported(self) -> bool {
        !matches!(self, Self::Microsoft)
    }
}

/// Which password a provider takes over IMAP: what the connect screen's
/// guide says. It never changes what Campbooks does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PasswordKind {
    /// An app password made in the account's security settings.
    AppPassword,
    /// The account's own password.
    AccountPassword,
    /// Not known.
    Unknown,
}

/// Settings found for a mailbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Its servers.
    pub servers: Servers,
    /// The username they most likely want.
    pub username: String,
    /// Where they came from.
    pub source: Source,
    /// The provider database's id of the provider, when known.
    pub provider_hint: Option<String>,
    /// Which password the provider takes.
    pub password: PasswordKind,
}

/// What finding a mailbox's servers answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    /// Its servers.
    Settings(Settings),
    /// Its provider is connected natively: use that instead.
    UseProvider(Native),
    /// Its provider will be connected natively, not yet.
    NotSupportedYet(Native),
    /// Nothing found: the member enters the servers.
    NotFound,
}

/// An email address, split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// The whole address, trimmed and lower-cased.
    pub address: String,
    /// Before the `@`.
    pub local: String,
    /// After the `@`: a host name.
    pub domain: String,
}

impl Address {
    /// `input` as an address: one `@`, something before it, a host name of
    /// at least two labels after it, at most 320 characters.
    pub fn parse(input: &str) -> Option<Self> {
        let address = input.trim().to_lowercase();
        if address.len() > 320 || address.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return None;
        }
        let (local, domain) = address.rsplit_once('@')?;
        let ok = !local.is_empty()
            && !local.contains('@')
            && domain.contains('.')
            && !domain.ends_with('.')
            && campbooks_kernel::net::guard::is_host_name(domain);
        ok.then(|| Self {
            local: local.to_owned(),
            domain: domain.to_owned(),
            address: address.clone(),
        })
    }

    fn parts(&self) -> Parts<'_> {
        Parts {
            address: &self.address,
            local: &self.local,
            domain: &self.domain,
        }
    }
}

/// Find `address`'s servers.
pub async fn discover(imap: &Imap, address: &Address) -> Discovery {
    if let Some(native) = native_by_domain(&address.domain) {
        return route(native);
    }
    if let Some(provider) = ispdb::by_domain(&address.domain) {
        return from_provider(provider, address, Source::Ispdb);
    }
    if let Some(found) = autoconfig(imap, address).await {
        return found;
    }
    if let Some(found) = srv(imap, address).await {
        return found;
    }
    mx(imap, address).await
}

/// The answer for a native provider.
fn route(native: Native) -> Discovery {
    if native.supported() {
        Discovery::UseProvider(native)
    } else {
        Discovery::NotSupportedYet(native)
    }
}

/// The settings of `provider` for `address`.
fn from_provider(provider: &ProviderConfig, address: &Address, source: Source) -> Discovery {
    if let Some(native) = native_by_provider(&provider.id) {
        return route(native);
    }
    let hint = (source != Source::Autoconfig).then(|| provider.id.clone());
    match provider.settings(address.parts()) {
        Some((imap, smtp, username)) => found(
            Servers { imap, smtp },
            username,
            address,
            source,
            hint.filter(|id| !id.is_empty()),
        ),
        None => Discovery::NotFound,
    }
}

/// `servers` found for `address` from `source`.
fn found(
    servers: Servers,
    username: String,
    address: &Address,
    source: Source,
    provider_hint: Option<String>,
) -> Discovery {
    if let Some(native) = native_by_host(&servers.imap.host) {
        return route(native);
    }
    let password = password_kind(provider_hint.as_deref(), &address.domain);
    Discovery::Settings(Settings {
        servers,
        username,
        source,
        provider_hint,
        password,
    })
}

/// The domain's own autoconfig document: at `autoconfig.<domain>`, then at
/// the domain's well-known path.
async fn autoconfig(imap: &Imap, address: &Address) -> Option<Discovery> {
    let domain = &address.domain;
    for url in [
        format!("https://autoconfig.{domain}/mail/config-v1.1.xml"),
        format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"),
    ] {
        let Ok(url) = Url::parse(&url) else {
            continue;
        };
        let Some(body) = imap.web.fetch(&url).await else {
            continue;
        };
        let Some(provider) = std::str::from_utf8(&body).ok().and_then(ispdb::parse) else {
            continue;
        };
        match from_provider(&provider, address, Source::Autoconfig) {
            Discovery::NotFound => {}
            found => return Some(found),
        }
    }
    None
}

/// The domain's SRV records: `_imaps` (TLS) else `_imap` (STARTTLS), and
/// `_submissions` (TLS) else `_submission` (STARTTLS); both needed.
async fn srv(imap: &Imap, address: &Address) -> Option<Discovery> {
    let domain = &address.domain;
    let mut servers = [None, None];
    for (slot, records) in [
        (
            0,
            [("_imaps", Security::Tls), ("_imap", Security::Starttls)],
        ),
        (
            1,
            [
                ("_submissions", Security::Tls),
                ("_submission", Security::Starttls),
            ],
        ),
    ] {
        for (service, security) in records {
            let targets = imap.dns.srv(&format!("{service}._tcp.{domain}")).await;
            if let Some(best) = best(targets) {
                servers[slot] = Some(Server {
                    host: best.host,
                    port: best.port,
                    security,
                });
                break;
            }
        }
    }
    let [Some(imap_server), Some(smtp_server)] = servers else {
        return None;
    };
    Some(found(
        Servers {
            imap: imap_server,
            smtp: smtp_server,
        },
        address.address.clone(),
        address,
        Source::Srv,
        None,
    ))
}

/// The preferred usable target of an SRV answer: lowest priority, then
/// highest weight. A target of `.` says there is no such service.
fn best(mut targets: Vec<SrvTarget>) -> Option<SrvTarget> {
    targets.retain(|t| t.port != 0 && campbooks_kernel::net::guard::is_host_name(&t.host));
    targets.sort_by(|a, b| a.priority.cmp(&b.priority).then(b.weight.cmp(&a.weight)));
    targets.into_iter().next()
}

/// The domain's MX hosts: a native provider, a provider of the database,
/// else the usual names when they resolve.
async fn mx(imap: &Imap, address: &Address) -> Discovery {
    let hosts = imap.dns.mx(&address.domain).await;
    let Some(first) = hosts.first() else {
        return Discovery::NotFound;
    };
    if let Some(native) = native_by_mx(first) {
        return route(native);
    }
    if let Some(provider) = hosts.iter().find_map(|host| ispdb::by_host(host)) {
        return from_provider(provider, address, Source::Mx);
    }
    let guess = Servers {
        imap: Server {
            host: format!("imap.{}", address.domain),
            port: 993,
            security: Security::Tls,
        },
        smtp: Server {
            host: format!("smtp.{}", address.domain),
            port: 465,
            security: Security::Tls,
        },
    };
    if imap.reach(&guess.imap).await.is_err() || imap.reach(&guess.smtp).await.is_err() {
        return Discovery::NotFound;
    }
    found(guess, address.address.clone(), address, Source::Mx, None)
}

/// Whether `host` is `domain` or a name under it.
fn ends_with_domain(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Whether `domain` is `<name>.<public suffix>` for one of `names`
/// (`outlook.com`, `hotmail.co.uk`, `zoho.eu`).
fn is_brand(domain: &str, names: &[&str]) -> bool {
    let Some((first, rest)) = domain.split_once('.') else {
        return false;
    };
    names.contains(&first) && psl::suffix_str(domain) == Some(rest)
}

/// Consumer brands of the native providers.
const GOOGLE_DOMAINS: &[&str] = &["gmail.com", "googlemail.com", "google.com"];
const ZOHO_BRANDS: &[&str] = &["zoho", "zohomail", "zohocloud"];
const MICROSOFT_BRANDS: &[&str] = &["outlook", "hotmail", "live", "msn", "windowslive"];

/// The native provider of an address at `domain`.
pub fn native_by_domain(domain: &str) -> Option<Native> {
    if GOOGLE_DOMAINS.contains(&domain) {
        Some(Native::Google)
    } else if is_brand(domain, ZOHO_BRANDS) {
        Some(Native::Zoho)
    } else if is_brand(domain, MICROSOFT_BRANDS) {
        Some(Native::Microsoft)
    } else {
        None
    }
}

/// The native provider of a provider database id.
fn native_by_provider(id: &str) -> Option<Native> {
    match id {
        "googlemail.com" | "gmail.com" => Some(Native::Google),
        "hotmail.com" | "outlook.com" | "office365.com" => Some(Native::Microsoft),
        _ if is_brand(id, ZOHO_BRANDS) => Some(Native::Zoho),
        _ => None,
    }
}

/// The native provider whose mail exchanger `host` is.
fn native_by_mx(host: &str) -> Option<Native> {
    if ends_with_domain(host, "google.com") || ends_with_domain(host, "googlemail.com") {
        Some(Native::Google)
    } else if ends_with_domain(host, "protection.outlook.com") {
        Some(Native::Microsoft)
    } else if registrable(host).is_some_and(|d| is_brand(d, ZOHO_BRANDS)) {
        Some(Native::Zoho)
    } else {
        None
    }
}

/// The native provider whose mail server `host` is: Gmail over IMAP with
/// an app password is refused, like any native provider.
pub fn native_by_host(host: &str) -> Option<Native> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if ["gmail.com", "googlemail.com", "google.com"]
        .iter()
        .any(|d| ends_with_domain(&host, d))
    {
        Some(Native::Google)
    } else if ["office365.com", "outlook.com", "hotmail.com", "live.com"]
        .iter()
        .any(|d| ends_with_domain(&host, d))
    {
        Some(Native::Microsoft)
    } else if registrable(&host).is_some_and(|d| is_brand(d, ZOHO_BRANDS)) {
        Some(Native::Zoho)
    } else {
        None
    }
}

/// The registrable domain of `host` (`mx.zoho.eu` → `zoho.eu`).
fn registrable(host: &str) -> Option<&str> {
    psl::domain_str(host)
}

/// Providers whose IMAP takes an app password, by database id.
const APP_PASSWORD_PROVIDERS: &[&str] = &[
    "yahoo.com",
    "aol.com",
    "me.com",
    "gmx.com",
    "gmx.net",
    "web.de",
    "yandex.ru",
    "mail.ru",
    "messagingengine.com",
];

/// Their domains, for addresses found another way.
const APP_PASSWORD_DOMAINS: &[&str] = &[
    "yahoo.com",
    "ymail.com",
    "rocketmail.com",
    "aol.com",
    "icloud.com",
    "me.com",
    "mac.com",
    "gmx.com",
    "gmx.net",
    "gmx.de",
    "gmx.at",
    "gmx.ch",
    "web.de",
    "yandex.com",
    "yandex.ru",
    "ya.ru",
    "mail.ru",
    "fastmail.com",
    "fastmail.fm",
];

/// Which password the provider (by database id) or domain takes.
pub fn password_kind(provider_hint: Option<&str>, domain: &str) -> PasswordKind {
    let known = provider_hint.is_some_and(|id| APP_PASSWORD_PROVIDERS.contains(&id))
        || APP_PASSWORD_DOMAINS.contains(&domain);
    if known {
        PasswordKind::AppPassword
    } else {
        PasswordKind::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        let ana = Address::parse("  Ana@Bakery.Example ").unwrap();
        assert_eq!(
            (
                ana.address.as_str(),
                ana.local.as_str(),
                ana.domain.as_str()
            ),
            ("ana@bakery.example", "ana", "bakery.example")
        );
        for bad in [
            "",
            "ana",
            "@bakery.example",
            "ana@",
            "ana@localhost",
            "ana@bakery.example.",
            "a na@x.example",
            "ana@ba kery.example",
        ] {
            assert!(Address::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn native_providers_are_recognised() {
        for (domain, native) in [
            ("gmail.com", Some(Native::Google)),
            ("googlemail.com", Some(Native::Google)),
            ("outlook.com", Some(Native::Microsoft)),
            ("hotmail.co.uk", Some(Native::Microsoft)),
            ("live.com.pt", Some(Native::Microsoft)),
            ("zoho.eu", Some(Native::Zoho)),
            ("zohomail.com", Some(Native::Zoho)),
            ("outlook.bakery.example", None),
            ("yahoo.com", None),
        ] {
            assert_eq!(native_by_domain(domain), native, "{domain}");
        }
        for (mx, native) in [
            ("aspmx.l.google.com", Some(Native::Google)),
            (
                "bakery-example.mail.protection.outlook.com",
                Some(Native::Microsoft),
            ),
            ("mx.zoho.eu", Some(Native::Zoho)),
            ("mx01.mail.icloud.com", None),
        ] {
            assert_eq!(native_by_mx(mx), native, "{mx}");
        }
        assert_eq!(native_by_host("imap.gmail.com"), Some(Native::Google));
        assert_eq!(
            native_by_host("outlook.office365.com"),
            Some(Native::Microsoft)
        );
        assert_eq!(native_by_host("imap.zoho.com"), Some(Native::Zoho));
        assert_eq!(native_by_host("imap.mail.yahoo.com"), None);
    }

    #[test]
    fn app_passwords_by_provider_or_domain() {
        assert_eq!(
            password_kind(Some("yahoo.com"), "yahoo.com"),
            PasswordKind::AppPassword
        );
        assert_eq!(password_kind(None, "icloud.com"), PasswordKind::AppPassword);
        assert_eq!(
            password_kind(Some("messagingengine.com"), "bakery.example"),
            PasswordKind::AppPassword
        );
        assert_eq!(password_kind(None, "bakery.example"), PasswordKind::Unknown);
    }

    #[test]
    fn the_best_srv_target() {
        let target = |host: &str, port, priority, weight| SrvTarget {
            host: host.to_owned(),
            port,
            priority,
            weight,
        };
        assert_eq!(
            best(vec![
                target("b.example.com", 993, 10, 5),
                target("a.example.com", 993, 0, 1),
                target("c.example.com", 993, 0, 9),
            ])
            .unwrap()
            .host,
            "c.example.com"
        );
        // `.`: no such service.
        assert!(best(vec![target("", 0, 0, 0)]).is_none());
    }
}
