//! Rate limits for endpoints anyone can call: sign-in, token, client
//! registration. The authenticated API's per-caller limits, applied to every
//! operation at once, are in [`api`].
//!
//! A [`Limit`] allows `max` hits per fixed `window` for one [`RateKey`],
//! usually the caller's address ([`ClientIp`]), sometimes an account or a
//! client. Counters live in Postgres (`kernel.rate_limits`), so every `api`
//! process of an install shares them. Keys are stored as an HMAC under a key
//! derived from the instance secret, never as addresses. The runtime role
//! holds no privilege on the table: it counts through
//! `kernel.take_rate_limit` and prunes through `kernel.prune_rate_limits`,
//! functions running as `campbooks_definer` (a role that cannot log in and
//! holds only what they need, campbooks-cloud#214), so no transaction can
//! read, lower or reset a counter (campbooks-cloud#210). Over the limit,
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

pub mod api;
mod bucket;

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
        match self.hit(db, limit, key).await? {
            None => Ok(()),
            Some(refusal) => {
                if refusal.first {
                    tracing::info!(limit = limit.name, "rate limit reached");
                }
                Err(Error::RateLimited {
                    retry_after: refusal.retry_after,
                })
            }
        }
    }

    /// Count one hit of `key` against `limit`: `None` when allowed, else
    /// when the window ends and whether this is the window's first refusal.
    pub(crate) async fn hit(
        &self,
        db: &PgPool,
        limit: &Limit,
        key: &RateKey,
    ) -> Result<Option<Refusal>, Error> {
        let Some(key) = &key.0 else {
            return Ok(None);
        };
        let (hits, retry_after) = take(db, &self.bucket(limit, key), limit.window, 1).await?;
        let hits = i64::from(hits);
        let max = i64::from(limit.max);
        Ok((hits > max).then(|| Refusal {
            retry_after: retry_after.max(1).unsigned_abs(),
            first: hits == max + 1,
        }))
    }
}

/// Add `cost` hits to `bucket` in a window of `window` (one starts when the
/// last ended): the bucket's hits now and the seconds left in its window.
/// Through `kernel.take_rate_limit`, the runtime role's only door to the
/// counters.
async fn take(
    db: &PgPool,
    bucket: &[u8],
    window: Duration,
    cost: u32,
) -> Result<(i32, i64), sqlx::Error> {
    sqlx::query_as("SELECT hits, retry_after FROM kernel.take_rate_limit($1, $2, $3)")
        .bind(bucket)
        .bind(window.as_secs_f64())
        .bind(i32::try_from(cost).unwrap_or(i32::MAX))
        .fetch_one(db)
        .await
}

/// A hit over its limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Refusal {
    /// Seconds until the next hit is allowed (at least 1).
    pub(crate) retry_after: u64,
    /// Whether it is the first refused hit since the key was last allowed
    /// one, so a caller hammering on is reported once.
    pub(crate) first: bool,
}

impl RateLimits {
    /// Use up `key`'s whole budget under `limit` for a window, as if it had
    /// been hit `max` times more (tests of what happens past a large
    /// limit). Through the same door as every hit: the runtime role cannot
    /// write a counter directly.
    ///
    /// # Panics
    /// If the database refuses the write.
    #[cfg(any(test, feature = "test-util"))]
    pub async fn exhaust(&self, db: &PgPool, limit: &Limit, key: &RateKey) {
        let Some(key) = &key.0 else {
            return;
        };
        take(db, &self.bucket(limit, key), limit.window, limit.max.max(1))
            .await
            .unwrap_or_else(|e| panic!("exhausting a rate limit: {e}"));
    }
}

/// The address of the caller: the socket peer, or, when the peer is a
/// trusted proxy, the right-most readable `X-Forwarded-For` address that is
/// not a trusted proxy (the peer when there is none).
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
        // Walk from the right: each trusted proxy appends the address it saw,
        // so the right-most hop that is not one of them is the client; what
        // lies further left is the client's to forge, and an unreadable hop
        // is skipped rather than ending the walk. With no such hop (no
        // header, or only proxies), the peer is the best we know.
        let hops: Vec<&str> = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .collect();
        let client = hops
            .into_iter()
            .rev()
            .filter_map(|hop| hop.trim().parse::<IpAddr>().ok().map(canonical))
            .find(|hop| !trusted.contains(*hop))
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

/// Delete every counter whose window ended; how many. Through
/// `kernel.prune_rate_limits` (the kernel scope, as `campbooks_definer`), a
/// batch per call.
async fn prune(db: &PgPool) -> Result<u64, sqlx::Error> {
    let mut pruned = 0_u64;
    loop {
        let deleted: i64 = sqlx::query_scalar("SELECT kernel.prune_rate_limits($1)")
            .bind(i32::try_from(PRUNE_BATCH).unwrap_or(i32::MAX))
            .fetch_one(db)
            .await?;
        let deleted = deleted.unsigned_abs();
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
        // A forged, unreadable hop neither ends the walk nor sends everyone
        // to the proxy's bucket.
        let forged = forwarded("1.1.1.1, not-an-address, 203.0.113.9, 192.0.2.1");
        assert_eq!(
            ClientIp::resolve(Some(ip("10.1.2.3")), &forged, &proxies).0,
            Some(ip("203.0.113.9"))
        );
        let garbage_first = forwarded("garbage, 203.0.113.9");
        assert_eq!(
            ClientIp::resolve(Some(ip("10.1.2.3")), &garbage_first, &proxies).0,
            Some(ip("203.0.113.9"))
        );
        // Several header lines read as one list, in order.
        let mut lines = forwarded("1.1.1.1");
        lines.append("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));
        assert_eq!(
            ClientIp::resolve(Some(ip("10.1.2.3")), &lines, &proxies).0,
            Some(ip("203.0.113.7"))
        );
        // Only proxies, or nothing readable: the peer.
        for only in ["192.0.2.1, 10.9.9.9", "nonsense"] {
            assert_eq!(
                ClientIp::resolve(Some(ip("10.1.2.3")), &forwarded(only), &proxies).0,
                Some(ip("10.1.2.3")),
                "{only}"
            );
        }
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

    /// The runtime role counts, and prunes ended windows, only through the
    /// kernel's two functions: no transaction of it, in a workspace or not,
    /// reads, resets, raises or forges a counter (campbooks-cloud#210).
    #[sqlx::test(migrations = "../../migrations")]
    async fn the_runtime_role_reaches_counters_only_through_the_kernels_functions(db: PgPool) {
        use crate::db::BeginFor as _;

        let pool = crate::testing::app_role_pool(&db).await;
        let limits = RateLimits::default();
        let limit = Limit::new("test.limit", 2, Duration::from_mins(1));
        let key = RateKey::ip(ClientIp(Some(ip("198.51.100.4"))));
        limits.check(&pool, &limit, &key).await.unwrap();
        limits.check(&pool, &limit, &key).await.unwrap();
        let refused = limits.check(&pool, &limit, &key).await.unwrap_err();
        assert!(matches!(refused, Error::RateLimited { .. }), "{refused:?}");

        for sql in [
            "SELECT count(*) FROM kernel.rate_limits",
            "UPDATE kernel.rate_limits SET hits = 0",
            "DELETE FROM kernel.rate_limits",
            "INSERT INTO kernel.rate_limits (bucket, hits, resets_at) VALUES ('\\x00', 0, now())",
        ] {
            let mut tx = pool
                .begin_for(crate::tenancy::WorkspaceId::new())
                .await
                .unwrap();
            let denied = sqlx::query(sql).execute(&mut *tx).await.unwrap_err();
            assert!(
                denied.to_string().contains("permission denied"),
                "{sql}: {denied}"
            );
            drop(tx);
            let denied = sqlx::query(sql).execute(&pool).await.unwrap_err();
            assert!(
                denied.to_string().contains("permission denied"),
                "{sql}: {denied}"
            );
        }
        // Still limited: nothing was reset.
        assert!(limits.check(&pool, &limit, &key).await.is_err());
        // An exhausted budget, through the same door.
        let other = RateKey::text("account:a-1");
        limits.exhaust(&pool, &limit, &other).await;
        assert!(limits.check(&pool, &limit, &other).await.is_err());

        // The function takes buckets, windows and costs the limiter makes,
        // nothing else: no window longer than a day (a counter that never
        // resets), no free or negative hit.
        for (bucket, window, cost) in [
            (vec![0_u8; 8], 60.0, 1),
            (vec![0_u8; 32], 0.0, 1),
            (vec![0_u8; 32], 86_401.0, 1),
            (vec![0_u8; 32], 60.0, 0),
            (vec![0_u8; 32], 60.0, -5),
        ] {
            let refused = sqlx::query("SELECT * FROM kernel.take_rate_limit($1, $2, $3)")
                .bind(bucket)
                .bind(window)
                .bind(cost)
                .execute(&pool)
                .await;
            assert!(refused.is_err(), "{window} {cost}");
        }

        // Ended windows are pruned by the runtime role, through the kernel.
        sqlx::query("UPDATE kernel.rate_limits SET resets_at = now() - interval '1 second'")
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(prune(&pool).await.unwrap(), 2);
        limits.check(&pool, &limit, &key).await.unwrap();
    }
}
