//! The SSRF guard for documents the issuer fetches from URLs a stranger
//! chose (a Client ID Metadata Document's `client_id`).
//!
//! - `https` only, a domain name (no IP literal, no userinfo), the default
//!   port, at most [`MAX_URL_LEN`] characters;
//! - the name is resolved once and **every** address must be public (no
//!   loopback, private, link-local and cloud metadata, CGNAT, multicast,
//!   documentation, benchmarking or reserved ranges, IPv6 unique-local and
//!   site-local, nor IPv4-mapped or NAT64 forms of those);
//! - the request goes to the address that was checked, so a second DNS
//!   answer cannot swap it; the certificate is still checked against the
//!   name;
//! - no redirects, [`TIMEOUT`] overall, at most [`MAX_BODY`] bytes read,
//!   `application/json` only.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use url::Url;

/// The longest URL fetched.
pub const MAX_URL_LEN: usize = 2048;
/// The largest document read.
pub const MAX_BODY: usize = 64 * 1024;
/// How long a fetch may take, connection included.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// Why a document was not fetched. Never carries the URL or the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// Not an `https` URL of a domain on the default port.
    #[error("not an https URL of a public name")]
    BadUrl,
    /// The name does not resolve.
    #[error("the name does not resolve")]
    Unresolved,
    /// The name resolves to an address that is not public.
    #[error("the name resolves to a non-public address")]
    PrivateAddress,
    /// The server could not be reached, timed out or failed.
    #[error("the server did not answer")]
    Unavailable,
    /// Not a 200 `application/json` answer, or a redirect.
    #[error("the answer is not a JSON document")]
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

/// The one public address to connect to for `host`: every address it
/// resolves to must be public.
pub async fn resolve(host: &str) -> Result<SocketAddr, Refused> {
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, 443))
        .await
        .map_err(|_| Refused::Unresolved)?
        .collect();
    if addresses.is_empty() {
        return Err(Refused::Unresolved);
    }
    if addresses.iter().any(|addr| !is_public(addr.ip())) {
        return Err(Refused::PrivateAddress);
    }
    Ok(addresses[0])
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

/// Fetch `url` through the guard.
pub async fn fetch(url: &Url) -> Result<Fetched, Refused> {
    let host = check_url(url)?;
    let address = resolve(host).await?;
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(TIMEOUT)
        .connect_timeout(TIMEOUT)
        .resolve(host, address)
        .build()
        .map_err(|_| Refused::Unavailable)?;
    let mut response = http
        .get(url.clone())
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|_| Refused::Unavailable)?;
    let json = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if response.status() != reqwest::StatusCode::OK || !json {
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
