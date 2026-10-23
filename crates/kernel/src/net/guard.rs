//! The SSRF guard for hosts and documents a stranger chose: a Client ID
//! Metadata Document's `client_id` (identity), a mailbox's IMAP and SMTP
//! servers and its domain's autoconfig document (connectors).
//!
//! - documents: `https` only, a domain name (no IP literal, no userinfo),
//!   the default port, at most [`MAX_URL_LEN`] characters;
//! - the name is resolved once and **every** address must be public (no
//!   loopback, private, link-local and cloud metadata, CGNAT, multicast,
//!   documentation, benchmarking or reserved ranges, IPv6 unique-local and
//!   site-local, nor IPv4-mapped or NAT64 forms of those), unless a
//!   [`Policy`] lets a self-hosted instance reach its own network;
//! - the connection goes to the address that was checked, so a second DNS
//!   answer cannot swap it; the certificate is still checked against the
//!   name;
//! - documents: no redirects and no proxy, [`TIMEOUT`] overall, at most
//!   [`MAX_BODY`] bytes read, of the media types asked for only.

use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    pin::Pin,
    time::Duration,
};

use url::Url;

/// The longest URL fetched.
pub const MAX_URL_LEN: usize = 2048;
/// The largest document read.
pub const MAX_BODY: usize = 64 * 1024;
/// How long a fetch may take, connection included.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// Why a host was not reached or a document not fetched. Never carries the
/// URL, the host or the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// Not an `https` URL of a domain on the default port.
    #[error("not an https URL of a public name")]
    BadUrl,
    /// The name does not resolve.
    #[error("the name does not resolve")]
    Unresolved,
    /// The name resolves to an address that is not public (or, under a
    /// [`Policy`] that allows private hosts, to one that is never reached).
    #[error("the name resolves to a non-public address")]
    PrivateAddress,
    /// The port is not one the [`Policy`] allows.
    #[error("the port is not allowed")]
    Port,
    /// The server could not be reached, timed out or failed.
    #[error("the server did not answer")]
    Unavailable,
    /// Not a 200 answer of a media type asked for (`application/json` for
    /// [`fetch`]), or a redirect.
    #[error("the answer is not a document of the expected type")]
    NotJson,
    /// Larger than [`MAX_BODY`].
    #[error("the document is too large")]
    TooLarge,
}

/// The host of `url` when it is a URL the guard may fetch.
pub fn check_url(url: &Url) -> Result<&str, Refused> {
    let ok = url.scheme() == "https"
        && url.as_str().len() <= MAX_URL_LEN
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.fragment().is_none();
    match url.host() {
        Some(url::Host::Domain(host)) if ok && host.contains('.') && host != "localhost" => {
            Ok(host)
        }
        _ => Err(Refused::BadUrl),
    }
}

/// Whether `ip` is an address on the public internet.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let segments = v6.segments();
            // ::ffff:0:0:0/96, IPv4-translated (RFC 2765): never a public host.
            if segments[..4] == [0; 4] && segments[4] == 0xffff && segments[5] == 0 {
                return false;
            }
            // NAT64 (64:ff9b::/96 and the local-use 64:ff9b:1::/48) embeds
            // an IPv4 address: judge that one.
            if segments[0] == 0x64 && segments[1] == 0xff9b {
                let [.., a, b, c, d] = v6.octets();
                return segments[2] == 0 && is_public_v4(Ipv4Addr::new(a, b, c, d));
            }
            !(v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                // fc00::/7 unique local, fe80::/10 link-local, fec0::/10 site-local
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] & 0xffc0) == 0xfec0
                // 2001:db8::/32 documentation, 100::/64 discard, 2001::/23 IETF
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
                || (segments[0] == 0x0100 && segments[1..4] == [0, 0, 0])
                || (segments[0] == 0x2001 && segments[1] < 0x0200)
                // ::/96 IPv4-compatible (deprecated), 2002::/16 6to4
                || segments[..6] == [0; 6]
                || segments[0] == 0x2002)
        }
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || a == 0
        // 100.64.0.0/10 shared address space (CGNAT)
        || (a == 100 && (b & 0xc0) == 64)
        // 192.0.0.0/24 IETF protocol assignments
        || (a == 192 && b == 0 && c == 0)
        // 198.18.0.0/15 benchmarking
        || (a == 198 && (b & 0xfe) == 18)
        // 240.0.0.0/4 reserved
        || a >= 240)
}

/// The one public address to connect to for `host` on port 443: every
/// address it resolves to must be public.
pub async fn resolve(host: &str) -> Result<SocketAddr, Refused> {
    resolve_socket(host, 443, Policy::public(&[443])).await
}

/// What a guarded connection may reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// Also reach loopback, private (RFC 1918), CGNAT and IPv6 unique-local
    /// addresses: a self-hosted instance's own network. Never link-local
    /// (cloud metadata), unspecified, multicast, broadcast or reserved
    /// addresses. Off unless an instance's administrator turns it on.
    pub allow_private: bool,
    /// Reach any port, not only [`ports`](Self::ports).
    pub allow_any_port: bool,
    /// The ports reached unless [`allow_any_port`](Self::allow_any_port).
    pub ports: &'static [u16],
}

impl Policy {
    /// Public addresses on `ports` only: the default.
    pub const fn public(ports: &'static [u16]) -> Self {
        Self {
            allow_private: false,
            allow_any_port: false,
            ports,
        }
    }

    /// Whether `port` may be reached.
    pub fn allows_port(&self, port: u16) -> bool {
        port != 0 && (self.allow_any_port || self.ports.contains(&port))
    }

    /// Whether `ip` may be reached.
    pub fn allows(&self, ip: IpAddr) -> bool {
        is_public(ip) || (self.allow_private && is_own_network(ip))
    }
}

/// Whether `ip` is an address of an instance's own network: loopback,
/// private (RFC 1918), CGNAT, IPv6 unique-local, or the IPv4-mapped form of
/// one of those.
pub fn is_own_network(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_own_network_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_own_network_v4(v4);
            }
            v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

fn is_own_network_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback() || ip.is_private() || (a == 100 && (b & 0xc0) == 64)
}

/// A future of a name's addresses.
pub type ResolveFuture<'a> =
    Pin<Box<dyn Future<Output = std::io::Result<Vec<SocketAddr>>> + Send + 'a>>;

/// Resolves a name to its addresses: the system's resolver
/// ([`SystemResolver`]) in production, a fake in tests.
pub trait Resolve: Send + Sync + std::fmt::Debug {
    /// The addresses of `host`, each with `port`.
    fn resolve<'a>(&'a self, host: &'a str, port: u16) -> ResolveFuture<'a>;
}

/// The system's resolver.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemResolver;

impl Resolve for SystemResolver {
    fn resolve<'a>(&'a self, host: &'a str, port: u16) -> ResolveFuture<'a> {
        Box::pin(async move { Ok(tokio::net::lookup_host((host, port)).await?.collect()) })
    }
}

/// The one address to connect to for `host` (a name or an IP literal) and
/// `port`, through the system's resolver (see [`resolve_socket_with`]).
pub async fn resolve_socket(host: &str, port: u16, policy: Policy) -> Result<SocketAddr, Refused> {
    resolve_socket_with(&SystemResolver, host, port, policy).await
}

/// The one address to connect to for `host` and `port`: the port must be
/// one `policy` allows, the name is resolved once with `resolver`, and
/// **every** address it resolves to must be one `policy` allows. Connect to
/// the address returned (never resolve the name again: a second answer
/// could point elsewhere), and check the certificate against `host`.
pub async fn resolve_socket_with(
    resolver: &dyn Resolve,
    host: &str,
    port: u16,
    policy: Policy,
) -> Result<SocketAddr, Refused> {
    if !policy.allows_port(port) {
        return Err(Refused::Port);
    }
    let host = host.trim();
    let literal = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    let addresses: Vec<SocketAddr> = match literal.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) if is_host_name(host) => resolver
            .resolve(host, port)
            .await
            .map_err(|_| Refused::Unresolved)?,
        Err(_) => return Err(Refused::BadUrl),
    };
    let Some(first) = addresses.first().copied() else {
        return Err(Refused::Unresolved);
    };
    if addresses.iter().any(|addr| !policy.allows(addr.ip())) {
        return Err(Refused::PrivateAddress);
    }
    Ok(SocketAddr::new(first.ip(), port))
}

/// Whether `host` reads as a DNS name: letters, digits, hyphens and dots,
/// at most 253 characters, no empty label.
pub fn is_host_name(host: &str) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host);
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

/// A fetched document and how long it may be cached
/// (`Cache-Control: max-age`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The body, at most [`MAX_BODY`] bytes.
    pub body: Vec<u8>,
    /// `max-age`, when the server said.
    pub max_age: Option<Duration>,
}

/// Fetch the JSON document at `url` through the guard.
pub async fn fetch(url: &Url) -> Result<Fetched, Refused> {
    fetch_as(url, &["application/json"]).await
}

/// Fetch the document at `url` through the guard: a 200 answer whose media
/// type is one of `media_types` (also sent as `Accept`).
pub async fn fetch_as(url: &Url, media_types: &[&str]) -> Result<Fetched, Refused> {
    let host = check_url(url)?;
    let address = resolve(host).await?;
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        // Straight to the checked address: a proxy from the environment
        // would resolve the name again, and reach what it can reach.
        .no_proxy()
        .timeout(TIMEOUT)
        .connect_timeout(TIMEOUT)
        .resolve(host, address)
        .build()
        .map_err(|_| Refused::Unavailable)?;
    let mut response = http
        .get(url.clone())
        .header(reqwest::header::ACCEPT, media_types.join(", "))
        .send()
        .await
        .map_err(|_| Refused::Unavailable)?;
    let expected = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| {
            media_types
                .iter()
                .any(|wanted| v.trim().eq_ignore_ascii_case(wanted))
        });
    if response.status() != reqwest::StatusCode::OK || !expected {
        return Err(Refused::NotJson);
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BODY as u64)
    {
        return Err(Refused::TooLarge);
    }
    let max_age = response
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .and_then(max_age);
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Refused::Unavailable)? {
        if body.len() + chunk.len() > MAX_BODY {
            return Err(Refused::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Fetched { body, max_age })
}

/// `max-age` of a `Cache-Control` value; `no-store` and `no-cache` are 0.
pub fn max_age(cache_control: &str) -> Option<Duration> {
    let mut found = None;
    for directive in cache_control.split(',').map(str::trim) {
        let lower = directive.to_ascii_lowercase();
        if lower == "no-store" || lower == "no-cache" {
            return Some(Duration::ZERO);
        }
        if let Some(seconds) = lower.strip_prefix("max-age=") {
            found = seconds
                .trim_matches('"')
                .parse()
                .ok()
                .map(Duration::from_secs);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
    }

    #[test]
    fn only_public_addresses_pass() {
        for public in [
            "93.184.216.34",
            "1.1.1.1",
            "2606:4700:4700::1111",
            "64:ff9b::5db8:d822",
        ] {
            assert!(is_public(ip(public)), "{public}");
        }
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "0.1.2.3",
            "255.255.255.255",
            "224.0.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "192.0.0.8",
            "240.0.0.1",
            "::",
            "::1",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "fec0::1",
            "ff02::1",
            "2001:db8::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::ffff:0:7f00:1",
            "::ffff:0:5db8:d822",
            "::127.0.0.1",
            "64:ff9b::7f00:1",
            "64:ff9b::a9fe:a9fe",
            "2002:7f00:1::",
            "100::1",
        ] {
            assert!(!is_public(ip(private)), "{private}");
        }
    }

    #[test]
    fn only_https_urls_of_names_on_the_default_port() {
        let ok = Url::parse("https://agent.example.com/client.json").unwrap();
        assert_eq!(check_url(&ok), Ok("agent.example.com"));
        for bad in [
            "http://agent.example.com/client.json",
            "https://127.0.0.1/client.json",
            "https://[::1]/client.json",
            "https://localhost/client.json",
            "https://intranet/client.json",
            "https://agent.example.com:8443/client.json",
            "https://user:pw@agent.example.com/client.json",
            "https://agent.example.com/client.json#x",
        ] {
            assert_eq!(
                check_url(&Url::parse(bad).unwrap()),
                Err(Refused::BadUrl),
                "{bad}"
            );
        }
        let long = Url::parse(&format!(
            "https://agent.example.com/{}",
            "a".repeat(MAX_URL_LEN)
        ))
        .unwrap();
        assert_eq!(check_url(&long), Err(Refused::BadUrl));
    }

    #[tokio::test]
    async fn a_name_resolving_to_loopback_is_refused_before_any_request() {
        // Whatever passes the name check, the address decides.
        assert_eq!(resolve("localhost").await, Err(Refused::PrivateAddress));
    }

    /// A resolver that answers `answers` in turn, counting its calls.
    #[derive(Debug)]
    struct Scripted {
        answers: std::sync::Mutex<Vec<Vec<IpAddr>>>,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl Scripted {
        fn new(answers: &[&[&str]]) -> Self {
            Self {
                answers: std::sync::Mutex::new(
                    answers
                        .iter()
                        .rev()
                        .map(|ips| ips.iter().map(|s| ip(s)).collect())
                        .collect(),
                ),
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl Resolve for Scripted {
        fn resolve<'a>(&'a self, _host: &'a str, port: u16) -> ResolveFuture<'a> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let next = self.answers.lock().unwrap().pop().unwrap_or_default();
            Box::pin(async move {
                Ok(next
                    .into_iter()
                    .map(|ip| SocketAddr::new(ip, port))
                    .collect())
            })
        }
    }

    const MAIL: &[u16] = &[993, 143, 465, 587];

    #[tokio::test]
    async fn a_mail_host_is_resolved_once_and_connected_to_as_checked() {
        // Public at the check, private a moment later: the checked address
        // is the one returned, and the name is never looked up again.
        let resolver = Scripted::new(&[&["93.184.216.34"], &["10.0.0.5"]]);
        let address = resolve_socket_with(&resolver, "imap.example.com", 993, Policy::public(MAIL))
            .await
            .unwrap();
        assert_eq!(address, "93.184.216.34:993".parse().unwrap());
        assert_eq!(resolver.calls(), 1);
    }

    #[tokio::test]
    async fn every_answer_must_be_public() {
        for private in [
            "127.0.0.1",
            "10.0.0.5",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "192.0.2.1",
            "::1",
            "fd12:3456::1",
            "fe80::1",
            "::ffff:169.254.169.254",
        ] {
            let resolver = Scripted::new(&[&["93.184.216.34", private]]);
            assert_eq!(
                resolve_socket_with(&resolver, "imap.example.com", 993, Policy::public(MAIL)).await,
                Err(Refused::PrivateAddress),
                "{private}"
            );
            // An IP literal is judged the same way, without a lookup.
            let literal = Scripted::new(&[]);
            assert_eq!(
                resolve_socket_with(&literal, private, 993, Policy::public(MAIL)).await,
                Err(Refused::PrivateAddress),
                "{private}"
            );
            assert_eq!(literal.calls(), 0);
        }
        let nothing = Scripted::new(&[&[]]);
        assert_eq!(
            resolve_socket_with(&nothing, "imap.example.com", 993, Policy::public(MAIL)).await,
            Err(Refused::Unresolved)
        );
        let bad = Scripted::new(&[]);
        assert_eq!(
            resolve_socket_with(&bad, "imap example.com", 993, Policy::public(MAIL)).await,
            Err(Refused::BadUrl)
        );
    }

    #[tokio::test]
    async fn only_the_allowed_ports_unless_any_port_is_allowed() {
        for port in [993, 143, 465, 587] {
            let resolver = Scripted::new(&[&["93.184.216.34"]]);
            assert!(
                resolve_socket_with(&resolver, "mail.example.com", port, Policy::public(MAIL))
                    .await
                    .is_ok(),
                "{port}"
            );
        }
        for port in [0, 22, 25, 80, 443, 5432, 6379] {
            let resolver = Scripted::new(&[&["93.184.216.34"]]);
            assert_eq!(
                resolve_socket_with(&resolver, "mail.example.com", port, Policy::public(MAIL))
                    .await,
                Err(Refused::Port),
                "{port}"
            );
            // Refused before any lookup.
            assert_eq!(resolver.calls(), 0);
        }
        let any = Policy {
            allow_any_port: true,
            ..Policy::public(MAIL)
        };
        let resolver = Scripted::new(&[&["93.184.216.34"]]);
        assert!(
            resolve_socket_with(&resolver, "mail.example.com", 2993, any)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn a_self_hosted_instance_may_reach_its_own_network_never_metadata() {
        let own = Policy {
            allow_private: true,
            ..Policy::public(MAIL)
        };
        for reachable in ["10.0.0.5", "192.168.1.20", "127.0.0.1", "::1", "fd00::5"] {
            let resolver = Scripted::new(&[&[reachable]]);
            assert!(
                resolve_socket_with(&resolver, "mail.example.com", 993, own)
                    .await
                    .is_ok(),
                "{reachable}"
            );
        }
        for never in [
            "169.254.169.254",
            "fe80::1",
            "0.0.0.0",
            "224.0.0.1",
            "255.255.255.255",
        ] {
            let resolver = Scripted::new(&[&[never]]);
            assert_eq!(
                resolve_socket_with(&resolver, "mail.example.com", 993, own).await,
                Err(Refused::PrivateAddress),
                "{never}"
            );
        }
    }

    #[test]
    fn host_names() {
        for good in [
            "imap.example.com",
            "imap.example.com.",
            "mx-1.example.net",
            "localhost",
        ] {
            assert!(is_host_name(good), "{good}");
        }
        for bad in [
            "",
            ".",
            "a..b",
            "-a.example.com",
            "a b.example.com",
            "a/b",
            "user@example.com",
        ] {
            assert!(!is_host_name(bad), "{bad}");
        }
    }

    #[test]
    fn cache_control_max_age() {
        assert_eq!(
            max_age("public, max-age=600"),
            Some(Duration::from_mins(10))
        );
        assert_eq!(max_age("no-store"), Some(Duration::ZERO));
        assert_eq!(max_age("private"), None);
    }
}
