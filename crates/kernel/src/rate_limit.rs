//! Rate limits for endpoints anyone can call: sign-in, token, client
//! registration.
//!
//! A [`Limit`] allows `max` hits per fixed `window` for one [`RateKey`],
//! usually the caller's address ([`ClientIp`]), sometimes an account or a
//! client. Counters live in Postgres (`kernel.rate_limits`), so every `api`
//! process of an install shares them. Keys are stored as an HMAC under a key
//! derived from the instance secret, never as addresses. Over the limit,
//! [`RateLimits::check`] answers [`Error::RateLimited`]: 429 problem+json
//! `rate_limited` with `Retry-After`.
//!
//! ```text
//! const TOKEN: Limit = Limit::new("identity.token", 60, Duration::from_mins(1));
//!
//! async fn handler(State(state): State<AppState>, ip: ClientIp) -> Result<…> {
//!     state.rate_limits.check(&state.db, &TOKEN, &RateKey::ip(ip)).await?;
//!     …
//! }
//! ```
//!
//! Behind a reverse proxy the socket peer is the proxy, so [`ClientIp`]
//! reads `X-Forwarded-For` only when the peer is one of
//! `CAMPBOOKS_TRUSTED_PROXIES`, taking the right-most address that is not
//! itself a trusted proxy (the left of the header is the client's to forge).

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use aws_lc_rs::hmac;
use axum::{
    extract::{ConnectInfo, FromRef, FromRequestParts},
    http::{HeaderMap, request::Parts},
};
use ipnet::IpNet;
use secrecy::ExposeSecret as _;
use serde::{Deserialize, Deserializer};
use sqlx::PgPool;

use crate::{
    AppState, Error,
    crypto::{SecretKey, derive_secret, random_token},
};

/// The HKDF label of the key rate-limit buckets are hashed with. Never
/// change it (every counter would start again).
const KEY_PURPOSE: &[u8] = b"campbooks kernel rate limits v1";

/// Rows deleted per statement when pruning.
const PRUNE_BATCH: i64 = 5_000;

/// How many hits one key may make per window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    /// `<owner>.<what>`, e.g. `identity.token`: part of every bucket, so two
    /// limits never share a counter.
    pub name: &'static str,
    /// Hits allowed per window.
    pub max: u32,
    /// The window.
    pub window: Duration,
}

impl Limit {
    /// `max` hits per `window` under `name`.
    pub const fn new(name: &'static str, max: u32, window: Duration) -> Self {
        Self { name, max, window }
    }
}

/// What a limit counts per: an address, or any other text (an account, a
/// client id). A request from inside the process has no address and is
/// never limited by one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateKey(Option<Vec<u8>>);

impl RateKey {
    /// Per client address. An IPv6 address counts per /64, the block one
    /// subscriber usually holds, so rotating inside it does not reset the
    /// count.
    pub fn ip(ip: ClientIp) -> Self {
        let Some(ip) = ip.0 else {
            return Self(None);
        };
        let mut key = b"ip:".to_vec();
        match ip {
            IpAddr::V4(v4) => key.extend_from_slice(&v4.octets()),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => key.extend_from_slice(&v4.octets()),
                None => key.extend_from_slice(&v6.octets()[..8]),
            },
        }
        Self(Some(key))
    }

    /// Per `text`, e.g. `client:<id>` or `account:<id>`.
    pub fn text(text: &str) -> Self {
        let mut key = b"text:".to_vec();
        key.extend_from_slice(text.as_bytes());
        Self(Some(key))
    }
}

/// The rate limiter. Cheap to clone.
#[derive(Clone)]
pub struct RateLimits {
    key: Arc<hmac::Key>,
}

impl std::fmt::Debug for RateLimits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimits").finish_non_exhaustive()
    }
}

impl Default for RateLimits {
    /// A limiter with a random key: counters work in this process but start
    /// again on restart and are not shared with other processes. The server
    /// installs [`RateLimits::new`] instead.
    fn default() -> Self {
        Self::with_key(random_token().as_bytes())
    }
}

impl RateLimits {
    /// A limiter whose buckets are keyed by `secret` (the instance secret),
    /// so every process of the install counts together.
    pub fn new(secret: &SecretKey) -> Self {
        Self::with_key(
            derive_secret(secret, KEY_PURPOSE)
                .expose_secret()
                .as_bytes(),
        )
    }

    fn with_key(key: &[u8]) -> Self {
        Self {
            key: Arc::new(hmac::Key::new(hmac::HMAC_SHA256, key)),
        }
    }

    fn bucket(&self, limit: &Limit, key: &[u8]) -> Vec<u8> {
        let mut context = hmac::Context::with_key(&self.key);
        context.update(limit.name.as_bytes());
        context.update(&[0]);
        context.update(key);
        context.sign().as_ref().to_vec()
    }

    /// Count one hit of `key` against `limit`. [`Error::RateLimited`] when it
    /// is over (the hit still counts, so hammering does not help). A key
    /// with no address (a request from inside the process) is not counted.
    pub async fn check(&self, db: &PgPool, limit: &Limit, key: &RateKey) -> Result<(), Error> {
        let Some(key) = &key.0 else {
            return Ok(());
        };
        let bucket = self.bucket(limit, key);
        let window = limit.window.as_secs_f64();
        let row = sqlx::query!(
            r#"
            INSERT INTO kernel.rate_limits (bucket, hits, resets_at)
            VALUES ($1, 1, now() + make_interval(secs => $2))
            ON CONFLICT (bucket) DO UPDATE SET
                hits = CASE WHEN kernel.rate_limits.resets_at <= now() THEN 1
                            ELSE kernel.rate_limits.hits + 1 END,
                resets_at = CASE WHEN kernel.rate_limits.resets_at <= now()
                                 THEN now() + make_interval(secs => $2)
                                 ELSE kernel.rate_limits.resets_at END
            RETURNING hits,
                      ceil(extract(epoch FROM resets_at - now()))::bigint AS "retry_after!"
            "#,
            bucket,
            window,
        )
        .fetch_one(db)
        .await?;
        if i64::from(row.hits) > i64::from(limit.max) {
            tracing::info!(limit = limit.name, "rate limit reached");
            return Err(Error::RateLimited {
                retry_after: row.retry_after.max(1).unsigned_abs(),
            });
        }
        Ok(())
    }
}

/// The address of the caller: the socket peer, or, when the peer is a
/// trusted proxy, the right-most `X-Forwarded-For` address that is not.
/// Never rejects. `None` for a request with no socket address: one the
/// server sends itself (the web BFF's in-process calls, operations invoked
/// by id). Every request from the network has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientIp(pub Option<IpAddr>);

impl ClientIp {
    /// The caller's address from the socket `peer` and the request
    /// `headers`, trusting `X-Forwarded-For` from `trusted` proxies only.
    pub fn resolve(peer: Option<IpAddr>, headers: &HeaderMap, trusted: &TrustedProxies) -> Self {
        let Some(peer) = peer.map(canonical) else {
            return Self(None);
        };
        if !trusted.contains(peer) {
            return Self(Some(peer));
        }
        let hops: Vec<IpAddr> = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .map_while(|hop| hop.trim().parse::<IpAddr>().ok())
            .map(canonical)
            .collect();
        let client = hops
            .iter()
            .rev()
            .copied()
            .find(|hop| !trusted.contains(*hop))
            // Every hop is a proxy we trust (or none was sent): the
            // left-most is the closest thing to a client.
            .or_else(|| hops.first().copied())
            .unwrap_or(peer);
        Self(Some(client))
    }
}

/// An IPv4-mapped IPv6 address as IPv4.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(addr)| addr.ip());
        Ok(Self::resolve(
            peer,
            &parts.headers,
            &state.config.trusted_proxies,
        ))
    }
}

/// The reverse proxies whose `X-Forwarded-For` is believed
/// (`CAMPBOOKS_TRUSTED_PROXIES`: comma-separated addresses or CIDR blocks,
/// e.g. `10.0.0.0/8,172.16.0.0/12`). Empty by default: the socket peer is
/// the client.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedProxies(Vec<IpNet>);

impl TrustedProxies {
    /// Parse a comma-separated list; the error names the bad entry.
    pub fn parse(list: &str) -> Result<Self, String> {
        list.split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                entry
                    .parse::<IpNet>()
                    .or_else(|_| entry.parse::<IpAddr>().map(IpNet::from))
                    .map_err(|_| {
                        format!(
                            "CAMPBOOKS_TRUSTED_PROXIES: `{entry}` is not an address or a CIDR block"
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    /// Whether `ip` is a trusted proxy.
    pub fn contains(&self, ip: IpAddr) -> bool {
        self.0.iter().any(|net| net.contains(&ip))
    }
}

impl<'de> Deserialize<'de> for TrustedProxies {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let list = String::deserialize(deserializer)?;
        Self::parse(&list).map_err(serde::de::Error::custom)
    }
}

/// Delete counters whose window ended. Scheduled hourly by the kernel.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct PruneRateLimits;

impl crate::Job for PruneRateLimits {
    const NAME: &'static str = "kernel.prune_rate_limits";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 3;

    async fn run(self, ctx: &mut crate::JobContext) -> Result<(), crate::JobError> {
        let pruned = prune(ctx.pool()).await?;
        tracing::info!(pruned, "pruned ended rate-limit windows");
        Ok(())
    }
}

/// Delete every counter whose window ended; how many.
async fn prune(db: &PgPool) -> Result<u64, sqlx::Error> {
    let mut pruned = 0_u64;
    loop {
        let deleted = sqlx::query!(
            r#"
            DELETE FROM kernel.rate_limits
             WHERE bucket IN (
                 SELECT bucket FROM kernel.rate_limits WHERE resets_at <= now() LIMIT $1)
            "#,
            PRUNE_BATCH,
        )
        .execute(db)
        .await?
        .rows_affected();
        pruned += deleted;
        if deleted < PRUNE_BATCH.unsigned_abs() {
            return Ok(pruned);
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
    }

    fn forwarded(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static(value));
        headers
    }

    #[test]
    fn the_peer_is_the_client_unless_it_is_a_trusted_proxy() {
        let none = TrustedProxies::default();
        let spoofed = forwarded("203.0.113.9");
        assert_eq!(
            ClientIp::resolve(Some(ip("198.51.100.4")), &spoofed, &none).0,
            Some(ip("198.51.100.4"))
        );
        let proxies = TrustedProxies::parse("10.0.0.0/8, 192.0.2.1").unwrap();
        assert_eq!(
            ClientIp::resolve(Some(ip("10.1.2.3")), &spoofed, &proxies).0,
            Some(ip("203.0.113.9"))
        );
        // The client prepends a forged hop; the right-most untrusted wins.
        let chain = forwarded("1.1.1.1, 203.0.113.9, 192.0.2.1");
        assert_eq!(
            ClientIp::resolve(Some(ip("10.1.2.3")), &chain, &proxies).0,
            Some(ip("203.0.113.9"))
        );
        // A proxy that sent no header is the client as far as we know.
        assert_eq!(
            ClientIp::resolve(Some(ip("10.1.2.3")), &HeaderMap::new(), &proxies).0,
            Some(ip("10.1.2.3"))
        );
        assert_eq!(
            ClientIp::resolve(None, &spoofed, &proxies).0,
            None,
            "no socket: sent by the server itself"
        );
        assert_eq!(
            ClientIp::resolve(Some(ip("::ffff:198.51.100.4")), &spoofed, &none).0,
            Some(ip("198.51.100.4"))
        );
    }

    #[test]
    fn trusted_proxies_parse_addresses_and_blocks() {
        assert!(TrustedProxies::parse("").unwrap().0.is_empty());
        let parsed = TrustedProxies::parse("10.0.0.0/8,::1, 192.0.2.7").unwrap();
        assert!(parsed.contains(ip("10.200.0.1")));
        assert!(parsed.contains(ip("::1")));
        assert!(parsed.contains(ip("192.0.2.7")));
        assert!(!parsed.contains(ip("192.0.2.8")));
        let err = TrustedProxies::parse("10.0.0.0/8,proxy.example.com").unwrap_err();
        assert!(err.contains("proxy.example.com"), "{err}");
    }

    #[test]
    fn ipv6_counts_per_64_and_buckets_never_hold_the_address() {
        let a = RateKey::ip(ClientIp(Some(ip("2001:db8:1:2:aaaa::1"))));
        let b = RateKey::ip(ClientIp(Some(ip("2001:db8:1:2:bbbb::9"))));
        let c = RateKey::ip(ClientIp(Some(ip("2001:db8:1:3::1"))));
        assert_eq!(a, b);
        assert_ne!(a, c);
        let limits = RateLimits::new(&SecretKey::from_bytes([3; 32]));
        let limit = Limit::new("test.limit", 1, Duration::from_mins(1));
        let other = Limit::new("test.other", 1, Duration::from_mins(1));
        let bytes = |key: &RateKey| key.0.clone().unwrap_or_default();
        let bucket = limits.bucket(&limit, &bytes(&a));
        assert_eq!(bucket.len(), 32);
        assert_ne!(
            bucket,
            limits.bucket(&other, &bytes(&a)),
            "limits never share"
        );
        assert_eq!(
            bucket,
            RateLimits::new(&SecretKey::from_bytes([3; 32])).bucket(&limit, &bytes(&b)),
            "every process agrees"
        );
        assert_ne!(
            bucket,
            RateLimits::new(&SecretKey::from_bytes([4; 32])).bucket(&limit, &bytes(&a))
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn hits_over_the_limit_are_refused_until_the_window_ends(db: PgPool) {
        let limits = RateLimits::default();
        let limit = Limit::new("test.limit", 2, Duration::from_mins(1));
        let key = RateKey::ip(ClientIp(Some(ip("198.51.100.4"))));
        limits.check(&db, &limit, &key).await.unwrap();
        limits.check(&db, &limit, &key).await.unwrap();
        let refused = limits.check(&db, &limit, &key).await.unwrap_err();
        let Error::RateLimited { retry_after } = refused else {
            panic!("{refused:?}");
        };
        assert!((1..=60).contains(&retry_after), "{retry_after}");
        // Someone else is not affected.
        let other = RateKey::ip(ClientIp(Some(ip("198.51.100.5"))));
        limits.check(&db, &limit, &other).await.unwrap();
        // Requests the server sends itself are never limited by address.
        for _ in 0..5 {
            limits
                .check(&db, &limit, &RateKey::ip(ClientIp(None)))
                .await
                .unwrap();
        }

        // The window ends: counting starts again, and pruning removes it.
        sqlx::query("UPDATE kernel.rate_limits SET resets_at = now() - interval '1 second'")
            .execute(&db)
            .await
            .unwrap();
        limits.check(&db, &limit, &key).await.unwrap();
        sqlx::query("UPDATE kernel.rate_limits SET resets_at = now() - interval '1 second'")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(prune(&db).await.unwrap(), 2);
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM kernel.rate_limits")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(left, 0);
    }
}
