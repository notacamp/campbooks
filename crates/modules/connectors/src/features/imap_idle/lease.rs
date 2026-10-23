//! IDLE leases (`connectors.imap_idle_leases`): which IMAP connections may
//! be held in IDLE, and which process holds each.
//!
//! A sync run **offers** its connection's row, or **withdraws** it, in its
//! own workspace ([`offer`], [`withdraw`]). The pools **claim**, **renew**,
//! **release** and **give up** rows across workspaces in the maintenance
//! scope (`begin_maintenance`, allowed for this file in `architecture.toml`),
//! through the table's own maintenance policies: they read ids, two keyed
//! digests, a flag and times, and change only a row's holder and times (a
//! trigger keeps the rest). Times are the database's, the one clock every
//! process shares.

use std::collections::{HashMap, HashSet};

use campbooks_kernel::{Result, crypto::Signer, db::begin_maintenance, tenancy::WorkspaceId};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    config::ImapConfig,
    domain::model::{ConnectionId, StreamId},
};

/// How long a claimed or renewed lease lasts.
pub const LEASE_SECONDS: i32 = 180;

/// Most free rows one claim reads.
const CANDIDATES: i64 = 500;

/// The transaction advisory lock every claim takes, so two processes never
/// count the same free room twice (above the 32-bit keys `hashtext` gives).
const CLAIM_LOCK: i64 = 0x0063_6f6e_6e69_646c;

/// What a sync run offers for IDLE: its keys, and whether it may IDLE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The keyed digest of the mailbox's login ([`login_key`]).
    pub login_key: String,
    /// The keyed digest of its IMAP host ([`host_key`]).
    pub host_key: String,
    /// The host is a known provider's.
    pub provider_host: bool,
    /// IDLE is on and the server offered it at this run.
    pub capable: bool,
}

/// A digest of `value` under the instance's signer, for `context`: 32 hex
/// characters that only the instance can link back to `value`.
fn keyed(signer: &Signer, context: &[u8], value: &str) -> String {
    let tag = signer.sign(context, value.as_bytes());
    let mut out = String::with_capacity(32);
    for byte in tag.iter().take(16) {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

const HEX: &[u8; 16] = b"0123456789abcdef";

/// The key of the mailbox login `username` at `host` (one IDLE session per
/// login, whichever connection holds it).
pub fn login_key(signer: &Signer, username: &str, host: &str) -> String {
    let login = format!(
        "{}\u{0}{}",
        username.trim().to_lowercase(),
        host.to_ascii_lowercase()
    );
    keyed(signer, b"connectors.imap_idle.login", &login)
}

/// The key of IMAP host `host` (the sessions a host sees are counted by it).
pub fn host_key(signer: &Signer, host: &str) -> String {
    keyed(
        signer,
        b"connectors.imap_idle.host",
        &host.to_ascii_lowercase(),
    )
}

/// In `connection`'s workspace: offer its row when `offer` is capable and
/// the stream changed since `quiet_since`, withdraw it otherwise. Whether a
/// lease on it is live (held by a process, not expired).
pub async fn offer(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    connection: ConnectionId,
    stream: StreamId,
    offer: &Offer,
    quiet_since: DateTime<Utc>,
) -> Result<bool> {
    if !offer.capable {
        withdraw(conn, connection).await?;
        return Ok(false);
    }
    let changed: Option<Option<DateTime<Utc>>> =
        sqlx::query_scalar("SELECT last_change_at FROM connectors.streams WHERE id = $1")
            .bind(stream)
            .fetch_optional(&mut *conn)
            .await?;
    if changed.flatten().is_none_or(|at| at < quiet_since) {
        withdraw(conn, connection).await?;
        return Ok(false);
    }
    let held: bool = sqlx::query_scalar(
        "INSERT INTO connectors.imap_idle_leases
             (connection_id, workspace_id, stream_id, login_key, host_key, provider_host)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (connection_id) DO UPDATE
            SET stream_id = EXCLUDED.stream_id, login_key = EXCLUDED.login_key,
                host_key = EXCLUDED.host_key, provider_host = EXCLUDED.provider_host
         RETURNING coalesce(holder IS NOT NULL AND expires_at > now(), false)",
    )
    .bind(connection)
    .bind(workspace)
    .bind(stream)
    .bind(&offer.login_key)
    .bind(&offer.host_key)
    .bind(offer.provider_host)
    .fetch_one(&mut *conn)
    .await?;
    Ok(held)
}

/// In `connection`'s workspace: withdraw its row (its holder loses the
/// lease at its next renewal).
pub async fn withdraw(conn: &mut PgConnection, connection: ConnectionId) -> Result<()> {
    sqlx::query("DELETE FROM connectors.imap_idle_leases WHERE connection_id = $1")
        .bind(connection)
        .execute(conn)
        .await?;
    Ok(())
}

/// The live IDLE sessions on the host keyed `host_key`: all of them, and
/// `workspace`'s. What a sync run's slots must leave room for.
pub async fn live_on_host(
    pool: &PgPool,
    host_key: &str,
    workspace: WorkspaceId,
) -> Result<(u32, u32)> {
    let mut tx = begin_maintenance(pool).await?;
    let (all, own): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE workspace_id = $2)
           FROM connectors.imap_idle_leases
          WHERE host_key = $1 AND holder IS NOT NULL AND expires_at > now()",
    )
    .bind(host_key)
    .bind(workspace)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((
        u32::try_from(all).unwrap_or(u32::MAX),
        u32::try_from(own).unwrap_or(u32::MAX),
    ))
}

/// The caps on IDLE sessions, per IMAP host across every process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// A small host's IDLE sessions: half its sessions.
    pub small_host: usize,
    /// A known provider's host's IDLE sessions: half its sessions.
    pub provider_host: usize,
    /// One workspace's on a small host: half its run share, at most the
    /// workspace IDLE share.
    pub small_workspace: usize,
    /// One workspace's on a known provider's host.
    pub provider_workspace: usize,
}

impl Caps {
    /// The caps `config` sets.
    pub fn of(config: &ImapConfig) -> Self {
        let size = |n: u32| usize::try_from(n).unwrap_or(usize::MAX);
        Self {
            small_host: size(config.sessions_per_host / 2),
            provider_host: size(config.provider_sessions_per_host / 2),
            small_workspace: size(
                (config.workspace_sessions_per_host / 2)
                    .min(config.workspace_idle_sessions_per_host),
            ),
            provider_workspace: size(config.workspace_idle_sessions_per_host),
        }
    }

    const fn host(&self, provider: bool) -> usize {
        if provider {
            self.provider_host
        } else {
            self.small_host
        }
    }

    const fn workspace(&self, provider: bool) -> usize {
        if provider {
            self.provider_workspace
        } else {
            self.small_workspace
        }
    }
}

/// A lease row, as a claim reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub connection: ConnectionId,
    pub workspace: WorkspaceId,
    pub stream: StreamId,
    pub login_key: String,
    pub host_key: String,
    pub provider_host: bool,
}

type RowTuple = (ConnectionId, WorkspaceId, StreamId, String, String, bool);

impl From<RowTuple> for Row {
    fn from((connection, workspace, stream, login_key, host_key, provider_host): RowTuple) -> Self {
        Self {
            connection,
            workspace,
            stream,
            login_key,
            host_key,
            provider_host,
        }
    }
}

/// Of the `free` rows, in order, those a pool with `room` sessions left
/// may claim beside the `live` ones: one session per login, at most
/// [`Caps::host`] per host and [`Caps::workspace`] per workspace on a host.
pub fn choose(live: &[Row], free: Vec<Row>, room: usize, caps: Caps) -> Vec<Row> {
    let mut logins: HashSet<String> = live.iter().map(|r| r.login_key.clone()).collect();
    let mut hosts: HashMap<String, usize> = HashMap::new();
    let mut shares: HashMap<(WorkspaceId, String), usize> = HashMap::new();
    for row in live {
        *hosts.entry(row.host_key.clone()).or_default() += 1;
        *shares
            .entry((row.workspace, row.host_key.clone()))
            .or_default() += 1;
    }
    let mut chosen = Vec::new();
    for row in free {
        if chosen.len() >= room {
            break;
        }
        if logins.contains(&row.login_key) {
            continue;
        }
        let on_host = hosts.get(&row.host_key).copied().unwrap_or(0);
        let share_key = (row.workspace, row.host_key.clone());
        let in_share = shares.get(&share_key).copied().unwrap_or(0);
        if on_host >= caps.host(row.provider_host) || in_share >= caps.workspace(row.provider_host)
        {
            continue;
        }
        logins.insert(row.login_key.clone());
        *hosts.entry(row.host_key.clone()).or_default() += 1;
        *shares.entry(share_key).or_default() += 1;
        chosen.push(row);
    }
    chosen
}

/// Claim up to `room` free rows for `holder`, within `caps`. The rows
/// claimed.
pub async fn claim(pool: &PgPool, holder: Uuid, room: usize, caps: Caps) -> Result<Vec<Row>> {
    if room == 0 {
        return Ok(Vec::new());
    }
    let mut tx = begin_maintenance(pool).await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(CLAIM_LOCK)
        .execute(&mut *tx)
        .await?;
    let live: Vec<RowTuple> = sqlx::query_as(
        "SELECT connection_id, workspace_id, stream_id, login_key, host_key, provider_host
           FROM connectors.imap_idle_leases
          WHERE holder IS NOT NULL AND expires_at > now()",
    )
    .fetch_all(&mut *tx)
    .await?;
    let free: Vec<RowTuple> = sqlx::query_as(
        "SELECT connection_id, workspace_id, stream_id, login_key, host_key, provider_host
           FROM connectors.imap_idle_leases
          WHERE expires_at IS NULL OR expires_at <= now()
          ORDER BY claimed_at NULLS FIRST, connection_id
          LIMIT $1",
    )
    .bind(CANDIDATES)
    .fetch_all(&mut *tx)
    .await?;
    let live: Vec<Row> = live.into_iter().map(Row::from).collect();
    let chosen = choose(&live, free.into_iter().map(Row::from).collect(), room, caps);
    if chosen.is_empty() {
        tx.commit().await?;
        return Ok(Vec::new());
    }
    let ids: Vec<ConnectionId> = chosen.iter().map(|row| row.connection).collect();
    let claimed: Vec<ConnectionId> = sqlx::query_scalar(
        "UPDATE connectors.imap_idle_leases
            SET holder = $1, expires_at = now() + $2 * interval '1 second', claimed_at = now()
          WHERE connection_id = ANY($3) AND (expires_at IS NULL OR expires_at <= now())
        RETURNING connection_id",
    )
    .bind(holder)
    .bind(LEASE_SECONDS)
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(chosen
        .into_iter()
        .filter(|row| claimed.contains(&row.connection))
        .collect())
}

/// Extend `holder`'s live leases among `held`. The ones it still holds:
/// any other was lost (taken, withdrawn, expired or deleted).
pub async fn renew(
    pool: &PgPool,
    holder: Uuid,
    held: &[ConnectionId],
) -> Result<Vec<ConnectionId>> {
    if held.is_empty() {
        return Ok(Vec::new());
    }
    let mut tx = begin_maintenance(pool).await?;
    let kept: Vec<ConnectionId> = sqlx::query_scalar(
        "UPDATE connectors.imap_idle_leases
            SET expires_at = now() + $3 * interval '1 second'
          WHERE holder = $1 AND connection_id = ANY($2) AND expires_at > now()
        RETURNING connection_id",
    )
    .bind(holder)
    .bind(held)
    .bind(LEASE_SECONDS)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(kept)
}

/// Release every lease `holder` holds (a clean shutdown): free at once.
pub async fn release(pool: &PgPool, holder: Uuid) -> Result<u64> {
    let mut tx = begin_maintenance(pool).await?;
    let released = sqlx::query(
        "UPDATE connectors.imap_idle_leases SET holder = NULL, expires_at = NULL
          WHERE holder = $1",
    )
    .bind(holder)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(released)
}

/// Give up `holder`'s lease on `connection` for `cooldown_seconds`: no
/// process claims it before then (the stream is polled meanwhile).
pub async fn give_up(
    pool: &PgPool,
    holder: Uuid,
    connection: ConnectionId,
    cooldown_seconds: i32,
) -> Result<()> {
    let mut tx = begin_maintenance(pool).await?;
    sqlx::query(
        "UPDATE connectors.imap_idle_leases
            SET holder = NULL, expires_at = now() + $3 * interval '1 second'
          WHERE holder = $1 AND connection_id = $2",
    )
    .bind(holder)
    .bind(connection)
    .bind(cooldown_seconds)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(workspace: WorkspaceId, login: &str, host: &str, provider: bool) -> Row {
        Row {
            connection: ConnectionId::new(),
            workspace,
            stream: StreamId::new(),
            login_key: login.to_owned(),
            host_key: host.to_owned(),
            provider_host: provider,
        }
    }

    fn caps() -> Caps {
        Caps::of(&ImapConfig::default())
    }

    #[test]
    fn the_caps_follow_the_settings() {
        assert_eq!(
            caps(),
            Caps {
                small_host: 2,
                provider_host: 100,
                small_workspace: 1,
                provider_workspace: 10,
            }
        );
    }

    #[test]
    fn one_session_per_login() {
        let (a, b) = (WorkspaceId::new(), WorkspaceId::new());
        // The same login connected in two workspaces.
        let free = vec![row(a, "login", "host", true), row(b, "login", "host", true)];
        let chosen = choose(&[], free.clone(), 10, caps());
        assert_eq!(chosen, vec![free[0].clone()]);
        // Held elsewhere already: none.
        let live = [row(a, "login", "host", true)];
        assert!(choose(&live, vec![free[1].clone()], 10, caps()).is_empty());
    }

    #[test]
    fn a_small_host_keeps_half_its_sessions_for_runs() {
        let (a, b, c) = (WorkspaceId::new(), WorkspaceId::new(), WorkspaceId::new());
        let free = vec![
            row(a, "1", "small", false),
            row(a, "2", "small", false),
            row(b, "3", "small", false),
            row(c, "4", "small", false),
        ];
        let chosen = choose(&[], free.clone(), 10, caps());
        // One per workspace (half of its share of two), two for the host.
        assert_eq!(chosen, vec![free[0].clone(), free[2].clone()]);
    }

    #[test]
    fn a_known_provider_is_not_held_to_a_small_hosts_cap() {
        let a = WorkspaceId::new();
        let free: Vec<Row> = (0..12)
            .map(|n| row(a, &n.to_string(), "provider", true))
            .collect();
        let chosen = choose(&[], free.clone(), 100, caps());
        // The workspace's share of the provider's host: ten.
        assert_eq!(chosen.len(), 10);
        // Another workspace still gets its own.
        let b = WorkspaceId::new();
        let more = choose(&chosen, vec![row(b, "x", "provider", true)], 100, caps());
        assert_eq!(more.len(), 1);
    }

    #[test]
    fn a_pool_claims_no_more_than_its_room() {
        let free: Vec<Row> = (0..5)
            .map(|n| row(WorkspaceId::new(), &n.to_string(), &n.to_string(), false))
            .collect();
        assert_eq!(choose(&[], free, 3, caps()).len(), 3);
    }

    #[test]
    fn keys_are_keyed_digests() {
        let signer = Signer::default();
        let key = login_key(&signer, "Ana@Bakery.example", "imap.example.com");
        assert_eq!(key.len(), 32);
        assert!(key.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(!key.contains("bakery"));
        assert_eq!(
            key,
            login_key(&signer, "ana@bakery.example", "IMAP.example.com")
        );
        assert_ne!(
            key,
            login_key(&signer, "bea@bakery.example", "imap.example.com")
        );
        // Another instance's key: another digest.
        assert_ne!(
            key,
            login_key(&Signer::default(), "ana@bakery.example", "imap.example.com")
        );
        assert_ne!(
            host_key(&signer, "imap.example.com"),
            host_key(&signer, "imap.example.org")
        );
    }
}
