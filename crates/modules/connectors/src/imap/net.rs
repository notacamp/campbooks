//! What finding a mailbox's servers asks the network: DNS records (SRV and
//! MX, through the system's resolver) and a domain's autoconfig document
//! (over HTTPS, through the guard). Both are faked in tests.

use std::{future::Future, pin::Pin};

use campbooks_kernel::net::guard;
use hickory_resolver::{ResolverBuilder, TokioResolver, proto::rr::RData};
use url::Url;

/// A future of what the network answered.
pub type NetFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// An SRV record's target (RFC 2782).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrvTarget {
    /// The host, lower-cased, without the final dot (empty: "no service").
    pub host: String,
    /// The port.
    pub port: u16,
    /// Lower is tried first.
    pub priority: u16,
    /// Among equal priorities, higher is preferred.
    pub weight: u16,
}

/// Looks up DNS records.
pub trait Dns: Send + Sync + std::fmt::Debug {
    /// The SRV records of `name` (e.g. `_imaps._tcp.example.com`); empty
    /// when there are none or the lookup failed.
    fn srv<'a>(&'a self, name: &'a str) -> NetFuture<'a, Vec<SrvTarget>>;

    /// The MX hosts of `domain`, lower-cased, most preferred first; empty
    /// when there are none or the lookup failed.
    fn mx<'a>(&'a self, domain: &'a str) -> NetFuture<'a, Vec<String>>;
}

/// Fetches a domain's autoconfig document.
pub trait Web: Send + Sync + std::fmt::Debug {
    /// The XML document at `url`, when there is one.
    fn fetch<'a>(&'a self, url: &'a Url) -> NetFuture<'a, Option<Vec<u8>>>;
}

/// The system's resolver (hickory), with the system's configuration read
/// on first use.
#[derive(Default)]
pub struct SystemDns {
    resolver: tokio::sync::OnceCell<Option<TokioResolver>>,
}

impl std::fmt::Debug for SystemDns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemDns").finish_non_exhaustive()
    }
}

impl SystemDns {
    async fn resolver(&self) -> Option<&TokioResolver> {
        self.resolver
            .get_or_init(|| async {
                let built = TokioResolver::builder_tokio().and_then(ResolverBuilder::build);
                if built.is_err() {
                    tracing::warn!("the system's DNS configuration could not be read");
                }
                built.ok()
            })
            .await
            .as_ref()
    }
}

/// `name` as an absolute DNS name (one query, no search domains).
fn absolute(name: &str) -> String {
    format!("{}.", name.trim_end_matches('.'))
}

/// A DNS name (in ASCII) as a host: lower-cased, without the final dot.
fn host_of(name: &str) -> String {
    name.trim_end_matches('.').to_ascii_lowercase()
}

impl Dns for SystemDns {
    fn srv<'a>(&'a self, name: &'a str) -> NetFuture<'a, Vec<SrvTarget>> {
        Box::pin(async move {
            let Some(resolver) = self.resolver().await else {
                return Vec::new();
            };
            let Ok(lookup) = resolver.srv_lookup(absolute(name)).await else {
                return Vec::new();
            };
            lookup
                .answers()
                .iter()
                .filter_map(|record| match &record.data {
                    RData::SRV(srv) => Some(SrvTarget {
                        host: host_of(&srv.target.to_ascii()),
                        port: srv.port,
                        priority: srv.priority,
                        weight: srv.weight,
                    }),
                    _ => None,
                })
                .collect()
        })
    }

    fn mx<'a>(&'a self, domain: &'a str) -> NetFuture<'a, Vec<String>> {
        Box::pin(async move {
            let Some(resolver) = self.resolver().await else {
                return Vec::new();
            };
            let Ok(lookup) = resolver.mx_lookup(absolute(domain)).await else {
                return Vec::new();
            };
            let mut hosts: Vec<(u16, String)> = lookup
                .answers()
                .iter()
                .filter_map(|record| match &record.data {
                    RData::MX(mx) => Some((mx.preference, host_of(&mx.exchange.to_ascii()))),
                    _ => None,
                })
                .filter(|(_, host)| !host.is_empty())
                .collect();
            hosts.sort();
            hosts.into_iter().map(|(_, host)| host).collect()
        })
    }
}

/// Fetches through the kernel's guard: HTTPS to a public name on the
/// default port, no redirects, 64 KiB at most, 5 seconds.
#[derive(Debug, Clone, Copy, Default)]
pub struct GuardedWeb;

impl Web for GuardedWeb {
    fn fetch<'a>(&'a self, url: &'a Url) -> NetFuture<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            guard::fetch_as(url, &["application/xml", "text/xml"])
                .await
                .ok()
                .map(|fetched| fetched.body)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_absolute_and_hosts_plain() {
        assert_eq!(
            absolute("_imaps._tcp.example.com"),
            "_imaps._tcp.example.com."
        );
        assert_eq!(absolute("example.com."), "example.com.");
        assert_eq!(host_of("MX1.Example.COM."), "mx1.example.com");
    }
}
