//! Serving one workspace's feed as Server-Sent Events.
//!
//! ```text
//! retry: 3000
//! id: T1  event: ready     data: {"cursor":"T1"}
//! id: T2  event: change    data: {"cursor":"T2","record":{"type":"…","id":"…"},"op":"upsert","version":"…"}
//! id: T3  event: heartbeat data: {"cursor":"T3"}      (every HEARTBEAT_SECONDS from the open)
//! id: T4  event: reset     data: {"cursor":"T4","reason":"cursor_expired"}
//! id: T5  event: reset     data: {"cursor":"T5","reason":"access_changed"}  (a container went out of sight)
//!         event: revoked   data: {}
//!         event: closing   data: {"reason":"max_age"}
//! ```
//!
//! **A stream says nothing about what its caller may not see**
//! (campbooks-cloud#131). What goes on the wire must not change with the
//! changes the caller may not see (another member's inbox, someone's
//! draft):
//!
//! - **Pages hold visible changes only.** The page query filters in SQL by
//!   what the caller may see ([`Sight`]: the record types their scopes read,
//!   the audiences they are in, the containers the visibility port lists for
//!   them). A page is up to [`PAGE`] changes the caller may see, so how
//!   frames group into bursts never depends on hidden changes; the hidden
//!   ones are skipped by moving the read position to the head the read saw.
//! - **The containers are listed once and kept** ([`Containers`]) until a
//!   change that may change who sees inside one, or who belongs to the
//!   workspace, is numbered in a range the stream read (seen or not: a new
//!   inbox is announced inside itself), and at every heartbeat. The page is
//!   then read again under the new list before anything of it is shown.
//! - **Catching up is paced** ([`Pacer`]): from the open, and after a full
//!   page, each read's frames leave on the next catch-up tick counted from
//!   the open, so when a page leaves does not depend on how many hidden
//!   changes its read went past. A stream that has caught up sends what a
//!   wake-up brings at once.
//! - **Cursors are sealed resume tokens** (`T1`… above), never the
//!   workspace's sequence. A token holds a [`Position`] sealed with the
//!   instance's key ([`Signer::seal`](crate::crypto::Signer::seal),
//!   AES-256-GCM-SIV, a fresh nonce each time, fixed length), bound to the
//!   workspace and the caller. Clients cannot read, order or alter it, nor
//!   use another caller's; every frame's token is new, so neither its value
//!   nor whether it changed says anything.
//! - **`ready` carries no head**, and heartbeats are frames of one shape on
//!   a fixed timer from the open, busy or idle: whether hidden changes
//!   happened never decides whether or when one is sent.
//! - **Every visible change is one frame** (none are collapsed).
//! - **A token expires by its age alone.** Its [`Position`] carries a time
//!   no later change was numbered before; changes are pruned by the time
//!   they were numbered, so a token younger than the retention (less
//!   [`CLOCK_SLACK`]) always resumes, and an older one always resets
//!   (`cursor_expired`), however busy the workspace was. There is no
//!   backlog limit counted in changes; opening streams is rate limited per
//!   caller instead.
//!
//! What is left: a read happens when a change is numbered (any change, so a
//! hidden one too) or on the poll, and its frames leave together; two
//! visible changes numbered apart may go out in one burst or two depending
//! on when the stream read, which hidden changes can shift by milliseconds.
//! While catching up, a read that takes longer than a tick (a scan past a
//! great many hidden changes) leaves on a later tick: the delay shows, in
//! whole ticks.

use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    convert::Infallible,
    time::Duration,
};

use axum::response::sse::Event as Frame;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use futures_util::Stream;
use serde_json::json;
use sqlx::PgConnection;
use tokio::{sync::watch, time::Instant};

use super::{Audience, ChangeOp, hub::StreamSlot};
use crate::{
    AppState, Error,
    auth::{Principal, PrincipalKind},
    crypto::Signer,
    db::BeginFor as _,
    operations::ReadRules,
    rate_limit::{Limit, RateKey},
    records::RecordRef,
    tenancy::{Access, WorkspaceId},
};

/// Visible changes read per query.
pub(super) const PAGE: i64 = 200;

/// How long `EventSource` waits before reconnecting (the SSE `retry` field).
const CLIENT_RETRY: Duration = Duration::from_secs(3);

/// The part of the maximum connection time taken off at random, so streams
/// opened together do not all reconnect together.
const JITTER_DIVISOR: u64 = 10;

/// Seconds a token's time may run ahead of when a change after it was
/// numbered: a numbering's timestamp is taken a moment before it commits,
/// so a read that just missed it can carry a slightly later time. Tokens
/// expire this much before the retention, so every change after a token
/// that has not expired is still kept.
const CLOCK_SLACK: i64 = 300;

/// The purpose a resume token is sealed for, followed by the workspace and
/// the caller. Never change it (every token in flight would reset).
const TOKEN_CONTEXT: &[u8] = b"changes.cursor v1:";

/// A request to follow a workspace's changes.
#[derive(Debug, Clone)]
pub struct Subscription {
    /// The workspace.
    pub workspace: WorkspaceId,
    /// What the caller may do there, as the membership port resolved it.
    pub access: Access,
    /// The caller.
    pub principal: Principal,
    /// The resume token (`Last-Event-ID` or `?after=`); `None` starts at
    /// the current head.
    pub after: Option<String>,
}

/// A place in a workspace's feed: what a resume token holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Position {
    /// Every change up to this sequence number was delivered, or is not the
    /// caller's to see.
    seq: i64,
    /// Unix seconds (the database's clock) before which no change after
    /// `seq` was numbered (give or take [`CLOCK_SLACK`]). Chosen from what
    /// the caller sees only: when the stream opened, a heartbeat fired, or
    /// the delivered change was published.
    at: i64,
}

/// Who the caller is, for binding tokens and counting opens: a person by
/// their subject, a client by its id.
fn caller(principal: &Principal) -> String {
    match principal.kind {
        PrincipalKind::Client => format!(
            "client:{}",
            principal.client_id.as_deref().unwrap_or(&principal.subject)
        ),
        PrincipalKind::User => format!("user:{}", principal.subject),
    }
}

/// What a token is bound to: the workspace and the caller, so it opens for
/// nobody else.
fn binding(workspace: &str, principal: &Principal) -> Vec<u8> {
    [
        TOKEN_CONTEXT,
        workspace.as_bytes(),
        b"\0",
        caller(principal).as_bytes(),
    ]
    .concat()
}

impl Position {
    /// A token for this position under `binding`: a fresh value every time.
    fn seal(self, signer: &Signer, binding: &[u8]) -> String {
        let mut plain = [0_u8; 16];
        plain[..8].copy_from_slice(&self.seq.to_be_bytes());
        plain[8..].copy_from_slice(&self.at.to_be_bytes());
        URL_SAFE_NO_PAD.encode(signer.seal(binding, &plain))
    }

    /// The position `token` holds, if it was sealed under `binding` by this
    /// install and not altered.
    fn open(signer: &Signer, binding: &[u8], token: &str) -> Option<Self> {
        let sealed = URL_SAFE_NO_PAD.decode(token.trim()).ok()?;
        let plain = signer.open(binding, &sealed)?;
        let (seq, at) = plain.split_at_checked(8)?;
        Some(Self {
            seq: i64::from_be_bytes(seq.try_into().ok()?),
            at: i64::from_be_bytes(at.try_into().ok()?),
        })
    }
}

/// Where a stream starts.
#[derive(Debug, PartialEq, Eq)]
enum Start {
    /// From this position.
    From(Position),
    /// From the head, after telling the client to drop its cache.
    Reset(&'static str),
}

/// Where a stream with resume token `after` (opened, or `Err` when it does
/// not open) starts, at `now`, given the feed's `head`, its `oldest` kept
/// change and the retention. Nothing here depends on how many changes
/// there were, only on the token and the time.
fn start(
    after: Option<Result<Position, ()>>,
    now: i64,
    head: i64,
    oldest: Option<i64>,
    retention: i64,
) -> Start {
    let position = match after {
        None => return Start::From(Position { seq: head, at: now }),
        Some(Err(())) => return Start::Reset("unknown_cursor"),
        Some(Ok(position)) => position,
    };
    // Sealed by this install for this workspace, yet beyond its head: the
    // database was restored from an older backup.
    if position.seq < 0 || position.seq > head {
        return Start::Reset("unknown_cursor");
    }
    if now.saturating_sub(position.at) >= retention.saturating_sub(CLOCK_SLACK) {
        return Start::Reset("cursor_expired");
    }
    // Every change after the token must still be here: the oldest kept one
    // is the next after it, or nothing was kept and nothing is missing. By
    // the age rule above this holds unless a numbering stalled for longer
    // than the slack; the check keeps a client from missing changes then.
    let complete = position.seq == head || oldest.is_some_and(|oldest| oldest <= position.seq + 1);
    if complete {
        Start::From(position)
    } else {
        Start::Reset("cursor_expired")
    }
}

/// A stream's catch-up pacing. A read that starts while the stream is
/// behind (the first read after the open, or any read after a full page)
/// releases its frames at the first tick after it ends, ticks counted from
/// the open; one that starts caught up releases them at once. A read that
/// fits in a tick therefore leaves exactly on the tick, however long it
/// scanned.
#[derive(Debug, Clone, Copy)]
struct Pacer {
    /// Where the ticks are counted from (the open).
    origin: Instant,
    tick: Duration,
    /// Whether the next read starts behind the head.
    catching_up: bool,
}

impl Pacer {
    /// Ticks of `tick` from `origin`; the stream starts behind.
    fn new(origin: Instant, tick: Duration) -> Self {
        Self {
            origin,
            tick: tick.max(Duration::from_millis(1)),
            catching_up: true,
        }
    }

    /// A read that ended at `ended` stopped at a full page (`full`) or not:
    /// when its frames may leave (`None`: at once).
    fn after_read(&mut self, ended: Instant, full: bool) -> Option<Instant> {
        let paced = self.catching_up;
        self.catching_up = full;
        paced.then(|| self.tick_after(ended))
    }

    /// The first tick strictly after `at`.
    fn tick_after(&self, at: Instant) -> Instant {
        let elapsed = at.saturating_duration_since(self.origin).as_nanos();
        let ticks = elapsed / self.tick.as_nanos() + 1;
        self.origin
            + self
                .tick
                .saturating_mul(u32::try_from(ticks).unwrap_or(u32::MAX))
    }
}

/// The containers a stream may show changes from, listed once and kept
/// until something may have changed them.
struct Containers {
    /// What the page query filters by (the containers among the rest).
    sight: Sight,
    /// The head read just before they were listed: what any change up to it
    /// did to who sees inside a container is in the list.
    covers: i64,
}

/// What was numbered in a range a read covered, seen or not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AccessChanges {
    /// A change that may change who belongs to the workspace.
    members: bool,
    /// A change that may change who sees inside a container.
    containers: bool,
}

/// The record types [`access_changes`] looks for, each after its own
/// sequence number (what was already accounted for).
struct Watched<'a> {
    /// Record types that may change who belongs to the workspace.
    members: &'a [String],
    /// Membership changes up to this one were resolved.
    members_from: i64,
    /// Record types that may change who sees inside a container.
    containers: &'a [String],
    /// The containers were listed after this one.
    containers_from: i64,
}

/// Whether a change of a watched record type, after its own sequence
/// number, was numbered in `(after, upto]` of `workspace`, whoever may see
/// it. Ids are not read: only whether one exists.
async fn access_changes(
    conn: &mut PgConnection,
    workspace: &str,
    after: i64,
    upto: i64,
    watched: &Watched<'_>,
) -> Result<AccessChanges, sqlx::Error> {
    if upto <= after || (watched.members.is_empty() && watched.containers.is_empty()) {
        return Ok(AccessChanges::default());
    }
    // The same range scan the page query made, on the access record types
    // only (a handful of rows at most).
    let (members, containers): (bool, bool) = sqlx::query_as(
        "SELECT coalesce(bool_or(record_type = ANY($4::text[]) AND seq > $5), false),
                coalesce(bool_or(record_type = ANY($6::text[]) AND seq > $7), false)
           FROM kernel.changes
          WHERE workspace_id = $1 AND seq > $2 AND seq <= $3
            AND (record_type = ANY($4::text[]) OR record_type = ANY($6::text[]))",
    )
    .bind(workspace)
    .bind(after)
    .bind(upto)
    .bind(watched.members)
    .bind(watched.members_from)
    .bind(watched.containers)
    .bind(watched.containers_from)
    .fetch_one(&mut *conn)
    .await?;
    Ok(AccessChanges {
        members,
        containers,
    })
}

/// The workspace's head (0 before its first change).
async fn head(conn: &mut PgConnection, workspace: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT coalesce((SELECT head FROM kernel.change_heads WHERE workspace_id = $1), 0)",
    )
    .bind(workspace)
    .fetch_one(&mut *conn)
    .await
}

/// What a caller may see, as the page query filters it.
#[derive(Debug, Clone, Default)]
pub(super) struct Sight {
    /// Record types a read operation the caller holds a scope for returns.
    pub(super) types: Vec<String>,
    /// The caller's user id; `None` for a client acting for nobody.
    pub(super) user: Option<String>,
    /// Whether the caller manages the workspace (owner or admin).
    pub(super) manager: bool,
    /// The containers the caller may see inside, as parallel type and id
    /// lists; a change in any other container is not theirs to see.
    pub(super) container_types: Vec<String>,
    pub(super) container_ids: Vec<String>,
}

impl Sight {
    fn new(types: Vec<String>, access: &Access, containers: &HashSet<RecordRef>) -> Self {
        let (container_types, container_ids) = containers
            .iter()
            .map(|c| (c.record_type.clone(), c.id.clone()))
            .unzip();
        Self {
            types,
            user: access.user.map(|user| user.to_string()),
            manager: access.role.is_some_and(crate::tenancy::Role::can_manage),
            container_types,
            container_ids,
        }
    }
}

/// A change's audience as stored.
pub(super) enum RowAudience {
    /// Everyone the other rules admit.
    Everyone,
    /// This audience.
    Only(Audience),
    /// One this version cannot read: nobody.
    Unknown,
}

/// A change as read from `kernel.changes`.
pub(super) struct Row {
    pub(super) seq: i64,
    pub(super) event_id: uuid::Uuid,
    pub(super) record: RecordRef,
    pub(super) op: ChangeOp,
    pub(super) container: Option<RecordRef>,
    /// Who it is private to.
    pub(super) audience: RowAudience,
    /// When it was published (unix seconds, the database's clock): no change
    /// after it was numbered earlier.
    pub(super) published_at: i64,
}

/// One read of the visible changes after a position.
pub(super) struct Page {
    /// The head the read saw: every change up to it after the position that
    /// is not in `rows` is not the caller's to see, unless `full`.
    pub(super) head: i64,
    /// When the read started (unix seconds, the database's clock): any
    /// change after `head` was numbered after it (give or take
    /// [`CLOCK_SLACK`]).
    pub(super) at: i64,
    /// Up to `limit` changes the caller may see, after the position and up
    /// to `head`, in order.
    pub(super) rows: Vec<Row>,
    /// Whether the read stopped at `limit` (more visible changes may be
    /// waiting before `head`).
    pub(super) full: bool,
}

/// Read up to `limit` changes of `workspace` after `after` that `sight`
/// admits, in a transaction scoped to `workspace`. Hidden changes are
/// skipped in the database, never counted toward the page.
pub(super) async fn read_visible(
    conn: &mut PgConnection,
    workspace: &str,
    after: i64,
    sight: &Sight,
    limit: i64,
) -> Result<Page, sqlx::Error> {
    let bounds = sqlx::query!(
        r#"SELECT coalesce((SELECT head FROM kernel.change_heads WHERE workspace_id = $1), 0) AS "head!",
                  floor(extract(epoch FROM now()))::bigint AS "now!""#,
        workspace
    )
    .fetch_one(&mut *conn)
    .await?;
    // Walks `changes_workspace_seq_idx` from `after`, testing each change
    // against the caller's sight until `limit` pass or `head` is reached;
    // the next read starts at that head, so each change is tested once.
    let rows = sqlx::query!(
        r#"SELECT seq AS "seq!", event_id, record_type, record_id, op,
                  container_type, container_id, audience, audience_user,
                  floor(extract(epoch FROM occurred_at))::bigint AS "published_at!"
             FROM kernel.changes
            WHERE workspace_id = $1 AND seq > $2 AND seq <= $3
              AND record_type = ANY($4::text[])
              AND (audience IS NULL
                   OR (audience = 'user' AND audience_user = $5::text)
                   OR (audience = 'user_and_managers'
                       AND ($5::text IS NULL OR audience_user = $5::text OR $6::bool))
                   OR (audience = 'clients' AND $5::text IS NULL))
              AND (container_id IS NULL
                   OR (container_type, container_id) IN
                      (SELECT t, i FROM unnest($7::text[], $8::text[]) AS c(t, i)))
            ORDER BY seq
            LIMIT $9"#,
        workspace,
        after,
        bounds.head,
        &sight.types,
        sight.user.as_deref(),
        sight.manager,
        &sight.container_types,
        &sight.container_ids,
        limit
    )
    .fetch_all(&mut *conn)
    .await?;
    let full = i64::try_from(rows.len()).unwrap_or(i64::MAX) >= limit;
    let rows = rows
        .into_iter()
        .map(|row| Row {
            seq: row.seq,
            event_id: row.event_id,
            record: RecordRef {
                record_type: row.record_type,
                id: row.record_id,
            },
            op: ChangeOp::parse(&row.op),
            container: row
                .container_type
                .zip(row.container_id)
                .map(|(t, id)| RecordRef { record_type: t, id }),
            audience: match row.audience.as_deref() {
                None => RowAudience::Everyone,
                Some(kind) => Audience::from_columns(kind, row.audience_user.as_deref())
                    .map_or(RowAudience::Unknown, RowAudience::Only),
            },
            published_at: row.published_at,
        })
        .collect();
    Ok(Page {
        head: bounds.head,
        at: bounds.now,
        rows,
        full,
    })
}

/// Opening streams, per caller: a client reconnecting in a loop with an old
/// token would read up to the retention's worth of changes each time.
fn opens_limit(config: &crate::config::ChangesConfig) -> Option<Limit> {
    (config.opens_per_window > 0).then(|| {
        Limit::new(
            "changes.open",
            config.opens_per_window,
            Duration::from_secs(config.opens_window_seconds),
        )
    })
}

/// Open `subscription`'s feed. The caller has been admitted to the
/// workspace already (`Workspace<S>`); this checks the per-caller limits
/// and decides where to start.
///
/// # Errors
/// [`Error::RateLimited`] when the caller opened too many streams lately;
/// [`Error::TooManyRequests`] (`too_many_streams`) beyond the per-caller
/// limit of open streams; a database error reading the head.
pub async fn open(
    state: &AppState,
    subscription: Subscription,
) -> Result<impl Stream<Item = Result<Frame, Infallible>> + Send + 'static, Error> {
    let config = &state.config.changes;
    let workspace = subscription.workspace.to_string();
    let binding = binding(&workspace, &subscription.principal);
    if let Some(limit) = opens_limit(config) {
        // Counted per caller, whatever happens in the workspace.
        let key = RateKey::text(&caller(&subscription.principal));
        state.rate_limits.check(&state.db, &limit, &key).await?;
    }
    let slot = state
        .changes
        .admit(&subscription.principal.subject)
        .ok_or_else(|| Error::TooManyRequests(crate::i18n::MESSAGES.reason("too_many_streams")))?;
    // Subscribe before reading the head, so nothing sequenced in between
    // goes unnoticed.
    let wake = state.changes.watch(&workspace);
    // The feed of one workspace, read in that workspace's transaction.
    let mut tx = state.db.begin_for(subscription.workspace).await?;
    let bounds = sqlx::query!(
        r#"SELECT coalesce((SELECT head FROM kernel.change_heads WHERE workspace_id = $1), 0) AS "head!",
                  (SELECT min(seq) FROM kernel.changes WHERE workspace_id = $1 AND seq IS NOT NULL) AS oldest,
                  floor(extract(epoch FROM now()))::bigint AS "now!""#,
        workspace
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    let after = subscription
        .after
        .as_deref()
        .map(|token| Position::open(&state.signer, &binding, token).ok_or(()));
    let retention = i64::try_from(config.retention_hours.saturating_mul(3600)).unwrap_or(i64::MAX);
    let (position, reason) = match start(after, bounds.now, bounds.head, bounds.oldest, retention) {
        Start::From(position) => (position, None),
        Start::Reset(reason) => (
            Position {
                seq: bounds.head,
                at: bounds.now,
            },
            Some(reason),
        ),
    };
    let token = position.seal(&state.signer, &binding);
    let (event, data) = match reason {
        None => ("ready", json!({ "cursor": token })),
        Some(reason) => ("reset", json!({ "cursor": token, "reason": reason })),
    };
    let first = Frame::default()
        .event(event)
        .id(token)
        .retry(CLIENT_RETRY)
        .data(data.to_string());

    let now = Instant::now();
    let max_age = Duration::from_secs(config.max_connection_seconds);
    let jitter = max_age / u32::try_from(JITTER_DIVISOR).unwrap_or(1);
    let jitter = jitter.mul_f64(f64::from(uuid::Uuid::now_v7().as_bytes()[15]) / 255.0);
    let mut deadline = now + max_age.saturating_sub(jitter);
    let mut deadline_reason = "max_age";
    if let Some(expires_at) = subscription.principal.expires_at {
        let left = (expires_at - Utc::now()).to_std().unwrap_or_default();
        if now + left < deadline {
            deadline = now + left;
            deadline_reason = "token_expired";
        }
    }
    let heartbeat = Duration::from_secs(config.heartbeat_seconds);
    let rules = ReadRules::from_state(state);
    let readable = rules.readable(&subscription.principal);
    let records = |types: &[&str]| types.iter().map(ToString::to_string).collect::<Vec<_>>();
    let feed = Feed {
        rules,
        readable,
        member_records: records(state.membership.access_records()),
        container_records: records(state.visibility.access_records()),
        containers: None,
        members_checked: position.seq,
        pacer: Pacer::new(now, Duration::from_millis(config.catch_up_tick_ms)),
        release_at: None,
        state: state.clone(),
        workspace,
        binding,
        subscription,
        cursor: position.seq,
        position,
        wake,
        deadline,
        deadline_reason,
        heartbeat,
        next_heartbeat: now + heartbeat,
        poll: Duration::from_millis(config.poll_interval_ms),
        out: VecDeque::from([first]),
        shown: HashSet::new(),
        done: false,
        _slot: slot,
    };
    Ok(futures_util::stream::unfold(feed, |mut feed| async move {
        feed.next().await.map(|frame| (Ok(frame), feed))
    }))
}

/// One open stream.
struct Feed {
    state: AppState,
    workspace: String,
    /// What this stream's tokens are bound to.
    binding: Vec<u8>,
    subscription: Subscription,
    rules: ReadRules,
    /// The record types the caller's scopes read (its scopes are fixed for
    /// the stream's life).
    readable: Vec<String>,
    /// Record types whose changes may change who belongs to the workspace:
    /// one numbered in a page's range, seen or not, makes the stream resolve
    /// membership before showing the page.
    member_records: Vec<String>,
    /// Record types whose changes may change who sees inside a container:
    /// one numbered in a page's range, seen or not, makes the stream list
    /// the containers again and read the page anew.
    container_records: Vec<String>,
    /// The containers the caller may see, as last listed; `None` until the
    /// next read lists them.
    containers: Option<Containers>,
    /// Membership changes up to this sequence number were resolved.
    members_checked: i64,
    pacer: Pacer,
    /// When the queued frames may leave (a catch-up read's tick).
    release_at: Option<Instant>,
    /// Where the next read starts: every change up to it was delivered or
    /// is not the caller's to see.
    cursor: i64,
    /// Where the client may resume from: what the next token holds. It
    /// moves only at what the client sees (a change delivered, a
    /// heartbeat), never at a hidden change.
    position: Position,
    wake: watch::Receiver<u64>,
    deadline: Instant,
    deadline_reason: &'static str,
    /// The heartbeat interval.
    heartbeat: Duration,
    /// The next heartbeat: a fixed timer from the open, kept whatever the
    /// stream delivers or reads.
    next_heartbeat: Instant,
    poll: Duration,
    out: VecDeque<Frame>,
    /// Containers this stream has shown changes from. When one of these
    /// goes out of sight (the containers listed again, or one asked about
    /// before its page is shown) the client is told to drop what it cached
    /// (`reset`, `access_changed`).
    shown: HashSet<RecordRef>,
    done: bool,
    _slot: StreamSlot,
}

impl Feed {
    /// The next frame, or `None` once the stream is over.
    async fn next(&mut self) -> Option<Frame> {
        loop {
            // A catch-up read's frames leave on its tick.
            if let Some(at) = self.release_at.take() {
                tokio::time::sleep_until(at).await;
            }
            if let Some(frame) = self.out.pop_front() {
                return Some(frame);
            }
            if self.done {
                return None;
            }
            let now = Instant::now();
            if now >= self.deadline {
                self.end("closing", &json!({ "reason": self.deadline_reason }));
                continue;
            }
            // Due even while busy: the cadence never depends on what was read.
            if now >= self.next_heartbeat {
                if let Err(e) = self.heartbeat().await {
                    tracing::warn!(error = %e, "change feed: heartbeat failed; closing the stream");
                    self.end("closing", &json!({ "reason": "unavailable" }));
                }
                continue;
            }
            match self.read_page().await {
                // More visible changes are waiting (after the frames queued).
                Ok(true) => continue,
                Ok(false) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "change feed: reading changes failed; closing the stream");
                    self.end("closing", &json!({ "reason": "unavailable" }));
                    continue;
                }
            }
            if !self.out.is_empty() {
                continue;
            }
            // Nothing new: wait for a wake-up, the poll, a heartbeat or the end.
            let poll = tokio::time::sleep(self.poll);
            let heartbeat = tokio::time::sleep_until(self.next_heartbeat);
            let deadline = tokio::time::sleep_until(self.deadline);
            tokio::select! {
                changed = self.wake.changed() => {
                    if changed.is_err() {
                        // The hub dropped this workspace's channel; poll instead.
                        self.wake = self.state.changes.watch(&self.workspace);
                    }
                }
                () = poll => {}
                () = heartbeat => {}
                () = deadline => {}
            }
        }
    }

    fn end(&mut self, event: &'static str, data: &serde_json::Value) {
        self.out
            .push_back(Frame::default().event(event).data(data.to_string()));
        self.done = true;
    }

    /// A fresh token for the current position.
    fn token(&self) -> String {
        self.position.seal(&self.state.signer, &self.binding)
    }

    /// Every container the caller may see inside now.
    async fn visible_containers(&self) -> Result<HashSet<RecordRef>, Error> {
        let mut visible = HashSet::new();
        for container_type in self.state.visibility.container_types() {
            visible.extend(
                self.state
                    .visibility
                    .visible_containers(&self.state.db, &self.subscription.access, container_type)
                    .await?,
            );
        }
        Ok(visible)
    }

    /// Read the next page of visible changes and queue them. Returns
    /// whether the page was full (more may be waiting); `false` means the
    /// stream caught up with the head the read saw.
    async fn read_page(&mut self) -> Result<bool, Error> {
        Ok(self.read().await?.is_some_and(|page| page.full))
    }

    /// List the containers the caller may see now. The head is read first:
    /// every change up to it was committed before the list is, so what it
    /// did is in the list. A container shown before that is not in it any
    /// more makes the client drop its cache.
    async fn list_containers(&mut self) -> Result<Containers, Error> {
        let mut tx = self.state.db.begin_for(self.subscription.workspace).await?;
        let covers = head(&mut tx, &self.workspace).await?;
        tx.commit().await?;
        let set = self.visible_containers().await?;
        if self.shown.iter().any(|c| !set.contains(c)) {
            self.lost_sight();
        }
        let sight = Sight::new(self.readable.clone(), &self.subscription.access, &set);
        Ok(Containers { sight, covers })
    }

    /// Read the next page of visible changes and queue them, returning what
    /// the read saw (`None` when the stream ended on the way).
    async fn read(&mut self) -> Result<Option<PageSeen>, Error> {
        loop {
            // One permit for the listing (when due) and the page: the
            // listing is one more query of the same read, never nested in
            // another permit.
            let (page, end, changed) = {
                let _permit = self.state.changes.query_permit().await;
                let containers = match self.containers.take() {
                    Some(containers) => containers,
                    None => self.list_containers().await?,
                };
                let mut tx = self.state.db.begin_for(self.subscription.workspace).await?;
                let page = read_visible(
                    &mut tx,
                    &self.workspace,
                    self.cursor,
                    &containers.sight,
                    PAGE,
                )
                .await?;
                // Where the next read starts: every change up to it was
                // read (shown or not the caller's).
                let end = match page.rows.last() {
                    Some(last) if page.full => last.seq,
                    _ => page.head.max(self.cursor),
                };
                let watched = Watched {
                    members: &self.member_records,
                    members_from: self.members_checked.max(self.cursor),
                    containers: &self.container_records,
                    containers_from: containers.covers.max(self.cursor),
                };
                let changed =
                    access_changes(&mut tx, &self.workspace, self.cursor, end, &watched).await?;
                tx.commit().await?;
                if !changed.containers {
                    self.containers = Some(containers);
                }
                (page, end, changed)
            };
            // Someone's access to a container may have changed in this
            // range (a membership ended, an inbox was made for someone):
            // list them again and read the page anew, before any of it is
            // shown.
            if changed.containers {
                continue;
            }
            // Who belongs to the workspace may have changed: resolve again
            // before showing the page (a former member gets nothing more),
            // and read it again under a changed access.
            if changed.members {
                let before = self.subscription.access;
                self.check_access().await?;
                if self.done {
                    return Ok(None);
                }
                self.members_checked = end;
                if self.subscription.access != before {
                    self.containers = None;
                    continue;
                }
            }
            self.show(&page).await?;
            self.cursor = end;
            self.release_at = self.pacer.after_read(Instant::now(), page.full);
            return Ok(Some(PageSeen {
                at: page.at,
                full: page.full,
            }));
        }
    }

    /// Queue the frames of `page`'s changes, asking once more about the
    /// containers in it (one may have gone out of sight while it was read)
    /// and checking each change against the caller's sight again.
    async fn show(&mut self, page: &Page) -> Result<(), Error> {
        let mut still: HashMap<&RecordRef, bool> = HashMap::new();
        let in_page: BTreeSet<&RecordRef> = page
            .rows
            .iter()
            .filter_map(|row| row.container.as_ref())
            .collect();
        for container in in_page {
            let visible = self
                .state
                .visibility
                .container_visible(&self.state.db, &self.subscription.access, container)
                .await?;
            still.insert(container, visible);
        }
        if still
            .iter()
            .any(|(container, visible)| !visible && self.shown.contains(*container))
        {
            self.lost_sight();
        }
        for row in &page.rows {
            if !self.admits(row) {
                continue;
            }
            if let Some(container) = &row.container {
                if !still.get(container).copied().unwrap_or(false) {
                    continue;
                }
                self.shown.insert(container.clone());
            }
            self.position = Position {
                seq: row.seq,
                at: row.published_at,
            };
            let frame = change_frame(row, &self.token());
            self.out.push_back(frame);
        }
        Ok(())
    }

    /// The page query's rules again, in Rust: a change it let through that
    /// these refuse is not shown (fail closed).
    fn admits(&self, row: &Row) -> bool {
        if !self
            .rules
            .allows(&self.subscription.principal, &row.record.record_type)
        {
            return false;
        }
        match row.audience {
            RowAudience::Everyone => true,
            RowAudience::Unknown => false,
            RowAudience::Only(audience) => audience.admits(&self.subscription.access),
        }
    }

    /// Ask again about every container shown so far; tell the client to
    /// drop its cache if one went out of sight.
    async fn recheck_shown(&mut self) -> Result<(), Error> {
        let mut lost = false;
        for container in self.shown.clone() {
            let visible = self
                .state
                .visibility
                .container_visible(&self.state.db, &self.subscription.access, &container)
                .await?;
            if !visible {
                lost = true;
            }
        }
        if lost {
            self.lost_sight();
        }
        Ok(())
    }

    /// Something the client cached is no longer the caller's to see: it
    /// drops every record of the workspace and refetches (`reset`,
    /// `access_changed`). The token is the position it already had: what
    /// the stream read since is not the client's to skip by.
    fn lost_sight(&mut self) {
        self.shown.clear();
        let token = self.token();
        self.out.push_back(
            Frame::default()
                .event("reset")
                .id(token.clone())
                .data(json!({ "cursor": token, "reason": "access_changed" }).to_string()),
        );
    }

    /// Resolve membership again; when it is gone, say so and end. A token
    /// that no longer stands (its account suspended or signed out
    /// everywhere since the stream opened) ends it the same way.
    async fn check_access(&mut self) -> Result<(), Error> {
        if !self
            .state
            .auth
            .still_stands(&self.subscription.principal)
            .await?
        {
            self.end("revoked", &json!({}));
            return Ok(());
        }
        let access = match crate::tenancy::resolve_access(
            self.state.membership.as_ref(),
            self.state.clients.as_ref(),
            &self.state.db,
            &self.subscription.principal,
            self.subscription.workspace,
        )
        .await
        {
            Ok(access) => access,
            // A revoked client: as gone as a former member.
            Err(Error::Unauthorized(_)) => None,
            Err(error) => return Err(error),
        };
        match access {
            Some(access) => {
                if access != self.subscription.access {
                    // A changed role may change what the caller sees.
                    self.subscription.access = access;
                    self.recheck_shown().await?;
                }
            }
            None => {
                self.end("revoked", &json!({}));
            }
        }
        Ok(())
    }

    /// The heartbeat, on its fixed timer: check membership, list the
    /// containers again and read, then send a fresh token. When that
    /// read reached the head it saw, the token moves to that head and the
    /// read's time, so a resume skips the hidden changes before it; whether
    /// it moved is sealed inside it.
    async fn heartbeat(&mut self) -> Result<(), Error> {
        let now = Instant::now();
        while self.next_heartbeat <= now {
            self.next_heartbeat += self.heartbeat;
        }
        // The containers are listed again at least once a heartbeat, so a
        // change of access nothing announced is not kept any longer.
        self.containers = None;
        self.check_access().await?;
        if self.done {
            return Ok(());
        }
        let Some(page) = self.read().await? else {
            return Ok(());
        };
        if !page.full {
            self.position = Position {
                seq: self.cursor,
                at: page.at,
            };
        }
        let token = self.token();
        self.out.push_back(
            Frame::default()
                .event("heartbeat")
                .id(token.clone())
                .data(json!({ "cursor": token }).to_string()),
        );
        Ok(())
    }
}

/// What a read saw, for the heartbeat.
struct PageSeen {
    /// When the read started.
    at: i64,
    /// Whether it stopped at a full page before the head.
    full: bool,
}

/// The frame of one change: ids only, and its resume token.
fn change_frame(row: &Row, token: &str) -> Frame {
    Frame::default().event("change").id(token).data(
        json!({
            "cursor": token,
            "record": row.record,
            "op": row.op,
            "version": row.event_id.to_string(),
        })
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 24 * 3600;
    const NOW: i64 = 1_800_000_000;

    fn at(seq: i64, at: i64) -> Position {
        Position { seq, at }
    }

    #[test]
    fn starts_at_the_head_without_a_token() {
        assert_eq!(
            start(None, NOW, 7, Some(1), DAY),
            Start::From(Position { seq: 7, at: NOW })
        );
        assert_eq!(
            start(None, NOW, 0, None, DAY),
            Start::From(Position { seq: 0, at: NOW })
        );
    }

    #[test]
    fn resumes_from_a_young_token() {
        let young = NOW - 60;
        assert_eq!(
            start(Some(Ok(at(3, young))), NOW, 7, Some(1), DAY),
            Start::From(Position { seq: 3, at: young })
        );
        assert_eq!(
            start(Some(Ok(at(3, young))), NOW, 7, Some(4), DAY),
            Start::From(Position { seq: 3, at: young })
        );
        assert_eq!(
            start(Some(Ok(at(7, young))), NOW, 7, None, DAY),
            Start::From(Position { seq: 7, at: young })
        );
    }

    #[test]
    fn expiry_depends_on_the_tokens_age_alone() {
        // However many changes came after it: a million, or none.
        for head in [7, 1_000_000] {
            let fresh = NOW - (DAY - CLOCK_SLACK) + 1;
            assert!(matches!(
                start(Some(Ok(at(7, fresh))), NOW, head, Some(1), DAY),
                Start::From(_)
            ));
            let old = NOW - (DAY - CLOCK_SLACK);
            assert_eq!(
                start(Some(Ok(at(7, old))), NOW, head, Some(1), DAY),
                Start::Reset("cursor_expired")
            );
        }
    }

    #[test]
    fn resets_unknown_tokens_and_missing_changes() {
        assert_eq!(
            start(Some(Err(())), NOW, 7, Some(1), DAY),
            Start::Reset("unknown_cursor")
        );
        assert_eq!(
            start(Some(Ok(at(8, NOW))), NOW, 7, Some(1), DAY),
            Start::Reset("unknown_cursor")
        );
        assert_eq!(
            start(Some(Ok(at(-1, NOW))), NOW, 7, Some(1), DAY),
            Start::Reset("unknown_cursor")
        );
        // A numbering that stalled past the slack: never a missed change.
        assert_eq!(
            start(Some(Ok(at(2, NOW))), NOW, 7, Some(5), DAY),
            Start::Reset("cursor_expired")
        );
        assert_eq!(
            start(Some(Ok(at(2, NOW))), NOW, 7, None, DAY),
            Start::Reset("cursor_expired")
        );
    }

    const TICK: Duration = Duration::from_millis(50);

    /// A catch-up as a stream makes it, on a paused clock: each read scans
    /// for its time, then its frames leave when the pacer says, and the
    /// next read starts. Returns when each read's frames left, from the
    /// open.
    async fn catch_up(reads: &[(Duration, bool)]) -> Vec<Duration> {
        let origin = Instant::now();
        let mut pacer = Pacer::new(origin, TICK);
        let mut left = Vec::new();
        for &(scan, full) in reads {
            tokio::time::sleep(scan).await;
            if let Some(at) = pacer.after_read(Instant::now(), full) {
                tokio::time::sleep_until(at).await;
            }
            left.push(origin.elapsed());
        }
        left
    }

    /// Three pages of a catch-up (two full, then the rest), then one live
    /// read, with `hidden` changes scanned past in each, at a few
    /// microseconds a change.
    fn reads(hidden: [u32; 4]) -> Vec<(Duration, bool)> {
        let query = Duration::from_millis(2);
        let per_hidden = Duration::from_micros(2);
        hidden
            .iter()
            .enumerate()
            .map(|(n, &h)| (query + per_hidden * h, n < 2))
            .collect()
    }

    #[tokio::test(start_paused = true)]
    async fn catch_up_pages_leave_on_the_tick_whatever_was_scanned() {
        let ms = Duration::from_millis;
        // A few hidden changes, or up to 23,000 (46 ms of scanning) per page.
        let tiny = catch_up(&reads([0, 3, 10, 0])).await;
        let large = catch_up(&reads([20_000, 23_000, 15_000, 0])).await;
        let on_the_tick = [ms(50), ms(100), ms(150)];
        assert_eq!(tiny[..3], on_the_tick, "{tiny:?}");
        assert_eq!(large[..3], on_the_tick, "{large:?}");
        // Caught up, a wake-up's read leaves as soon as it is read.
        assert_eq!(tiny[3], ms(152));
        assert_eq!(large[3], ms(152));
    }

    #[tokio::test(start_paused = true)]
    async fn a_scan_longer_than_the_tick_shows_in_whole_ticks() {
        let ms = Duration::from_millis;
        // 35,000 hidden changes: 72 ms of scanning, more than a tick.
        let slow = catch_up(&reads([0, 35_000, 0, 0])).await;
        assert_eq!(slow[..3], [ms(50), ms(150), ms(200)], "{slow:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_stream_starts_behind_and_paces_again_after_a_full_page() {
        let origin = Instant::now();
        let mut pacer = Pacer::new(origin, TICK);
        // The first read after the open is paced even when it is not full.
        assert_eq!(pacer.after_read(origin, false), Some(origin + TICK));
        // Caught up: at once.
        assert_eq!(pacer.after_read(origin + TICK, false), None);
        // A live burst of more than a page: the read after it is paced.
        assert_eq!(pacer.after_read(origin + TICK * 3, true), None);
        let later = origin + TICK * 3 + Duration::from_millis(1);
        assert_eq!(pacer.after_read(later, false), Some(origin + TICK * 4));
        assert_eq!(pacer.after_read(later, false), None);
    }

    fn principal(subject: &str, kind: PrincipalKind, client: Option<&str>) -> Principal {
        Principal {
            subject: subject.to_owned(),
            kind,
            client_id: client.map(ToOwned::to_owned),
            scopes: std::collections::BTreeSet::new(),
            workspace: None,
            email: None,
            email_verified: false,
            name: None,
            expires_at: None,
            issued_at: None,
            auth_time: None,
        }
    }

    #[test]
    fn tokens_are_sealed_fresh_fixed_length_and_bound_to_workspace_and_caller() {
        let signer = Signer::new(&crate::crypto::SecretKey::from_bytes([7; 32]));
        let ana = principal("ana", PrincipalKind::User, Some("campbooks-web"));
        let bea = principal("bea", PrincipalKind::User, Some("campbooks-web"));
        let agent = principal("ana", PrincipalKind::Client, Some("ana-agent"));
        let here = binding("ws-a", &ana);
        let small = Position { seq: 1, at: NOW };
        let large = Position {
            seq: 9_000_000_000,
            at: NOW,
        };
        let a = small.seal(&signer, &here);
        let b = small.seal(&signer, &here);
        assert_ne!(a, b, "a fresh value every time");
        assert_eq!(a.len(), 60);
        assert_eq!(a.len(), large.seal(&signer, &here).len(), "one length");
        assert_eq!(Position::open(&signer, &here, &a), Some(small));
        assert_eq!(
            Position::open(
                &signer,
                &here,
                &format!(
                    " {a}
"
                )
            ),
            Some(small)
        );
        assert_eq!(
            Position::open(&signer, &binding("ws-a", &ana), &a),
            Some(small),
            "the same caller on another connection"
        );
        for (other, what) in [
            (binding("ws-b", &ana), "another workspace"),
            (binding("ws-a", &bea), "another person"),
            (binding("ws-a", &agent), "a client, even one acting for Ana"),
        ] {
            assert_eq!(Position::open(&signer, &other, &a), None, "{what}");
        }
        assert_eq!(Position::open(&Signer::default(), &here, &a), None);
        for legacy in ["41", "", "nonsense", "-3"] {
            assert_eq!(Position::open(&signer, &here, legacy), None, "{legacy}");
        }
    }
}
