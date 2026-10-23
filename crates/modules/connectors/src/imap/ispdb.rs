//! Mail client configuration documents (Thunderbird's autoconfig format,
//! `config-v1.1.xml`): the bundled provider database (ISPDB, MPL-2.0, files
//! in `ispdb/`, refreshed by `cargo xtask ispdb`) and a domain's own
//! autoconfig document are both read here.
//!
//! Only what connecting needs is kept: the provider's id and domains, and
//! its IMAP and SMTP servers that take a password over TLS or STARTTLS.

use std::{collections::HashMap, sync::LazyLock};

use campbooks_kernel::net::guard;
use quick_xml::{Reader, events::Event};

use super::{Security, Server, ispdb_files::FILES};

/// One provider's configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderConfig {
    /// The provider's id (`emailProvider id`, e.g. `yahoo.com`).
    pub id: String,
    /// The domains (and MX domains) it serves, lower-cased.
    pub domains: Vec<String>,
    /// Its IMAP servers, in the document's order of preference.
    pub imap: Vec<Entry>,
    /// Its SMTP servers, in the document's order of preference.
    pub smtp: Vec<Entry>,
}

/// One server of a configuration, as written (with placeholders).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// The host name, maybe with `%EMAILDOMAIN%`.
    pub host: String,
    /// The port.
    pub port: u16,
    /// `SSL`, `STARTTLS` or `plain`, as written.
    pub socket: String,
    /// The username, maybe with `%EMAILADDRESS%`, `%EMAILLOCALPART%` or
    /// `%EMAILDOMAIN%`.
    pub username: String,
    /// Whether it takes a password (`password-cleartext`,
    /// `password-encrypted`).
    pub takes_password: bool,
}

/// The parts of an email address the placeholders name.
#[derive(Debug, Clone, Copy)]
pub struct Parts<'a> {
    /// The whole address, lower-cased.
    pub address: &'a str,
    /// Before the `@`.
    pub local: &'a str,
    /// After the `@`, lower-cased.
    pub domain: &'a str,
}

impl Entry {
    /// The server for `parts`: TLS or STARTTLS and a password only.
    fn server(&self, parts: Parts<'_>) -> Option<Server> {
        let security = match self.socket.to_ascii_uppercase().as_str() {
            "SSL" | "TLS" => Security::Tls,
            "STARTTLS" => Security::Starttls,
            _ => return None,
        };
        if !self.takes_password || self.port == 0 {
            return None;
        }
        let host = self
            .host
            .replace("%EMAILDOMAIN%", parts.domain)
            .to_ascii_lowercase();
        guard::is_host_name(&host).then_some(Server {
            host,
            port: self.port,
            security,
        })
    }

    /// The username for `parts`; the address when the template is unknown.
    fn username(&self, parts: Parts<'_>) -> String {
        let filled = self
            .username
            .replace("%EMAILADDRESS%", parts.address)
            .replace("%EMAILLOCALPART%", parts.local)
            .replace("%EMAILDOMAIN%", parts.domain);
        if filled.trim().is_empty() || filled.contains('%') {
            parts.address.to_owned()
        } else {
            filled.trim().to_owned()
        }
    }
}

impl ProviderConfig {
    /// The first usable IMAP server (with its username) and SMTP server for
    /// `parts`; `None` unless both exist.
    pub fn settings(&self, parts: Parts<'_>) -> Option<(Server, Server, String)> {
        let (imap, username) = self
            .imap
            .iter()
            .find_map(|entry| Some((entry.server(parts)?, entry.username(parts))))?;
        let smtp = self.smtp.iter().find_map(|entry| entry.server(parts))?;
        Some((imap, smtp, username))
    }
}

/// Which server a document entry describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Imap,
    Smtp,
}

/// Read a configuration document; `None` when it is not one (or not
/// well-formed).
pub fn parse(xml: &str) -> Option<ProviderConfig> {
    let mut reader = Reader::from_str(xml);
    let mut config = ProviderConfig::default();
    let mut found = false;
    let mut stack: Vec<String> = Vec::new();
    let mut entry: Option<(Kind, Entry)> = None;
    let mut text = String::new();
    loop {
        match reader.read_event().ok()? {
            Event::Start(start) => {
                let name = String::from_utf8_lossy(start.local_name().as_ref()).into_owned();
                let kind = start
                    .try_get_attribute("type")
                    .ok()
                    .flatten()
                    .map(|a| String::from_utf8_lossy(&a.value).to_ascii_lowercase());
                match (name.as_str(), kind.as_deref()) {
                    ("emailProvider", _) => {
                        found = true;
                        if let Some(id) = start.try_get_attribute("id").ok().flatten() {
                            config.id = String::from_utf8_lossy(&id.value).to_ascii_lowercase();
                        }
                    }
                    ("incomingServer", Some("imap")) => {
                        entry = Some((Kind::Imap, Entry::default()))
                    }
                    ("outgoingServer", Some("smtp")) => {
                        entry = Some((Kind::Smtp, Entry::default()))
                    }
                    ("incomingServer" | "outgoingServer", _) => entry = None,
                    _ => {}
                }
                stack.push(name);
                text.clear();
            }
            Event::Text(chunk) => text.push_str(&chunk.decode().ok()?),
            // An entity in a value: not a value Campbooks uses (host names,
            // ports and placeholders never hold one).
            Event::GeneralRef(_) => text.push('\u{fffd}'),
            Event::End(_) => {
                let name = stack.pop()?;
                let value = std::mem::take(&mut text);
                let value = value.trim();
                match (name.as_str(), stack.last().map(String::as_str)) {
                    ("domain", Some("emailProvider")) => {
                        config.domains.push(value.to_ascii_lowercase());
                    }
                    ("incomingServer" | "outgoingServer", _) => match entry.take() {
                        Some((Kind::Imap, done)) => config.imap.push(done),
                        Some((Kind::Smtp, done)) => config.smtp.push(done),
                        None => {}
                    },
                    (field, Some("incomingServer" | "outgoingServer")) => {
                        if let Some((_, current)) = entry.as_mut() {
                            fill(current, field, value);
                        }
                    }
                    _ => {}
                }
            }
            // An element left open: not a whole document.
            Event::Eof if !stack.is_empty() => return None,
            Event::Eof => break,
            _ => {}
        }
    }
    found.then_some(config)
}

/// Set `field` of `entry` to `value`.
fn fill(entry: &mut Entry, field: &str, value: &str) {
    match field {
        "hostname" => value.clone_into(&mut entry.host),
        "port" => entry.port = value.parse().unwrap_or(0),
        "socketType" => value.clone_into(&mut entry.socket),
        "username" => value.clone_into(&mut entry.username),
        "authentication" => {
            entry.takes_password |= matches!(value, "password-cleartext" | "password-encrypted");
        }
        _ => {}
    }
}

/// The bundled database, read once: its providers and which serves each
/// domain.
struct Database {
    providers: Vec<ProviderConfig>,
    by_domain: HashMap<String, usize>,
}

static DATABASE: LazyLock<Database> = LazyLock::new(|| {
    let mut providers = Vec::with_capacity(FILES.len());
    let mut by_domain = HashMap::new();
    for (name, xml) in FILES {
        let Some(mut config) = parse(xml) else {
            continue;
        };
        if config.id.is_empty() {
            (*name).clone_into(&mut config.id);
        }
        let index = providers.len();
        for domain in &config.domains {
            by_domain.entry(domain.clone()).or_insert(index);
        }
        providers.push(config);
    }
    Database {
        providers,
        by_domain,
    }
});

/// The bundled configuration of the provider serving `domain`.
pub fn by_domain(domain: &str) -> Option<&'static ProviderConfig> {
    let database = &*DATABASE;
    database
        .by_domain
        .get(domain)
        .and_then(|&index| database.providers.get(index))
}

/// The bundled configuration of the provider whose domain `host` (an MX
/// host) is in, or under: `mx01.mail.icloud.com` is looked up as itself,
/// then `mail.icloud.com`, then `icloud.com`.
pub fn by_host(host: &str) -> Option<&'static ProviderConfig> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let mut rest = host.as_str();
    loop {
        if !rest.contains('.') {
            return None;
        }
        if let Some(found) = by_domain(rest) {
            return Some(found);
        }
        rest = rest.split_once('.')?.1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARTS: Parts<'static> = Parts {
        address: "ana@yahoo.com",
        local: "ana",
        domain: "yahoo.com",
    };

    #[test]
    fn the_bundled_database_reads() {
        let with_imap = DATABASE
            .providers
            .iter()
            .filter(|p| !p.imap.is_empty())
            .count();
        assert!(with_imap >= 100, "{with_imap}");
        let yahoo = by_domain("yahoo.com").unwrap();
        assert_eq!(yahoo.id, "yahoo.com");
        let (imap, smtp, username) = yahoo.settings(PARTS).unwrap();
        assert_eq!(
            imap,
            Server {
                host: "imap.mail.yahoo.com".to_owned(),
                port: 993,
                security: Security::Tls,
            }
        );
        assert_eq!(
            (smtp.host.as_str(), smtp.port),
            ("smtp.mail.yahoo.com", 465)
        );
        assert_eq!(username, "ana@yahoo.com");
        // An MX host finds its provider by its domain.
        assert_eq!(by_host("mta5.am0.yahoodns.net").unwrap().id, "yahoo.com");
        assert!(by_host("mx.unknown.example").is_none());
    }

    #[test]
    fn a_document_gives_its_password_servers_over_tls_only() {
        let xml = r#"<?xml version="1.0"?>
            <clientConfig version="1.1">
              <emailProvider id="bakery.example">
                <domain>bakery.example</domain>
                <incomingServer type="pop3">
                  <hostname>pop.bakery.example</hostname><port>995</port>
                  <socketType>SSL</socketType><username>%EMAILADDRESS%</username>
                  <authentication>password-cleartext</authentication>
                </incomingServer>
                <incomingServer type="imap">
                  <hostname>plain.%EMAILDOMAIN%</hostname><port>143</port>
                  <socketType>plain</socketType><username>%EMAILADDRESS%</username>
                  <authentication>password-cleartext</authentication>
                </incomingServer>
                <incomingServer type="imap">
                  <hostname>mail.%EMAILDOMAIN%</hostname><port>143</port>
                  <socketType>STARTTLS</socketType><username>%EMAILLOCALPART%</username>
                  <authentication>password-encrypted</authentication>
                </incomingServer>
                <outgoingServer type="smtp">
                  <hostname>oauth.bakery.example</hostname><port>465</port>
                  <socketType>SSL</socketType><authentication>OAuth2</authentication>
                </outgoingServer>
                <outgoingServer type="smtp">
                  <hostname>smtp.bakery.example</hostname><port>587</port>
                  <socketType>STARTTLS</socketType><username>%EMAILADDRESS%</username>
                  <authentication>password-cleartext</authentication>
                </outgoingServer>
                <documentation url="https://bakery.example/help?a=1&amp;b=2"/>
              </emailProvider>
            </clientConfig>"#;
        let config = parse(xml).unwrap();
        assert_eq!(config.id, "bakery.example");
        assert_eq!(config.domains, ["bakery.example"]);
        let parts = Parts {
            address: "ana@bakery.example",
            local: "ana",
            domain: "bakery.example",
        };
        let (imap, smtp, username) = config.settings(parts).unwrap();
        assert_eq!(
            (imap.host.as_str(), imap.port, imap.security),
            ("mail.bakery.example", 143, Security::Starttls)
        );
        assert_eq!(
            (smtp.host.as_str(), smtp.port, smtp.security),
            ("smtp.bakery.example", 587, Security::Starttls)
        );
        assert_eq!(username, "ana");
        assert!(parse("<html><body>not a configuration</body></html>").is_none());
        assert!(parse("<clientConfig><emailProvider id='x'>").is_none());
    }
}
