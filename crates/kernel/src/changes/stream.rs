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
//! (campbooks-cloud#131). Every change of the workspace is read, and most
//! are filtered out (another member's inbox, someone's draft); what goes on
//! the wire must not change with them:
//!
//! - **Cursors are sealed resume tokens** (`T1`… above), never the
//!   workspace's sequence. A token holds a [`Position`] sealed with the
//!   instance's key ([`Signer::seal`](crate::crypto::Signer::seal),
//!   AES-256-GCM, a fresh nonce each time, fixed length), bound to the
//!   workspace. Clients cannot read, order or alter it; every frame's token
//!   is new, so neither its value nor whether it changed says anything.
//! - **`ready` carries no head**, and heartbeats are frames of one shape on
//!   a fixed timer from the open, busy or idle: whether hidden changes
//!   happened never decides whether or when one is sent.
//! - **Every visible change is one frame.** Changes are not collapsed per
//!   page, since where a page ends depends on the hidden changes in it.
//! - **A token expires by its age alone.** Its [`Position`] carries a time
//!   no later change was numbered before; changes are pruned by the time
//!   they were numbered, so a token younger than the retention (less
//!   [`CLOCK_SLACK`]) always resumes, and an older one always resets
//!   (`cursor_expired`), however busy the workspace was. There is no
//!   backlog limit counted in changes.
//!
//! What is left: a visible change reaches the client when the stream reads
//! it, a few milliseconds later if it was reading hidden changes just then;
//! and resuming costs a read of the hidden changes since the token (at most
//! one heartbeat's worth for a token from a heartbeat).

use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    time::Duration,
};

use axum::response::sse::Event as Frame;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use futures_util::Stream;
use serde_json::json;
use tokio::{sync::watch, time::Instant};

use super::{Audience, ChangeOp, hub::StreamSlot};
use crate::{
    AppState, Error,
    auth::Principal,
    crypto::Signer,
    db::BeginFor as _,
    operations::ReadRules,
    records::RecordRef,
    tenancy::{Access, WorkspaceId},
};

/// Changes read per query.
const PAGE: i64 = 200;

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

/// The purpose a resume token is sealed for, followed by the workspace.
/// Never change it (every token in flight would reset).
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

impl Position {
    fn context(workspace: &str) -> Vec<u8> {
        [TOKEN_CONTEXT, workspace.as_bytes()].concat()
    }

    /// A token for this position in `workspace`: a fresh value every time.
    fn seal(self, signer: &Signer, workspace: &str) -> String {
        let mut plain = [0_u8; 16];
        plain[..8].copy_from_slice(&self.seq.to_be_bytes());
        plain[8..].copy_from_slice(&self.at.to_be_bytes());
        URL_SAFE_NO_PAD.encode(signer.seal(&Self::context(workspace), &plain))
    }

    /// The position `token` holds, if it was sealed for `workspace` by
    /// this install and not altered.
    fn open(signer: &Signer, workspace: &str, token: &str) -> Option<Self> {
        let sealed = URL_SAFE_NO_PAD.decode(token.trim()).ok()?;
        let plain = signer.open(&Self::context(workspace), &sealed)?;
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

/// Open `subscription`'s feed. The caller has been admitted to the
/// workspace already (`Workspace<S>`); this checks the per-caller stream
/// limit and decides where to start.
///
/// # Errors
/// [`Error::TooManyRequests`] (`too_many_streams`) beyond the per-caller
/// limit; a database error reading the head.
pub async fn open(
    state: &AppState,
    subscription: Subscription,
) -> Result<impl Stream<Item = Result<Frame, Infallible>> + Send + 'static, Error> {
    let config = &state.config.changes;
    let slot = state
        .changes
        .admit(&subscription.principal.subject)
        .ok_or_else(|| Error::TooManyRequests(crate::i18n::MESSAGES.reason("too_many_streams")))?;
    let workspace = subscription.workspace.to_string();
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
        .map(|token| Position::open(&state.signer, &workspace, token).ok_or(()));
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
    let token = position.seal(&state.signer, &workspace);
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
    let feed = Feed {
        rules: ReadRules::from_state(state),
        access_records: state.membership.access_records(),
        container_records: state.visibility.access_records(),
        state: state.clone(),
        workspace,
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
    subscription: Subscription,
    rules: ReadRules,
    /// Record types whose changes may change who belongs to the workspace.
    access_records: &'static [&'static str],
    /// Record types whose changes may change who sees inside a container.
    container_records: &'static [&'static str],
    /// The last change read (sent or hidden): where the next read starts.
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
    /// Containers this stream has shown changes from. Visibility is asked
    /// afresh for every page (never cached across pages, so a member taken
    /// out of an inbox stops getting its ids at once); when one of these
    /// goes out of sight the client is told to drop what it cached
    /// (`reset`, `access_changed`).
    shown: HashSet<RecordRef>,
    done: bool,
    _slot: StreamSlot,
}

/// A change's audience as stored.
enum RowAudience {
    /// Everyone the other rules admit.
    Everyone,
    /// This audience.
    Only(Audience),
    /// One this version cannot read: nobody.
    Unknown,
}

/// A change as read from `kernel.changes`.
struct Row {
    seq: i64,
    event_id: uuid::Uuid,
    record: RecordRef,
    op: ChangeOp,
    container: Option<RecordRef>,
    /// Who it is private to.
    audience: RowAudience,
    /// When it was published (unix seconds, the database's clock): no change
    /// after it was numbered earlier.
    published_at: i64,
}

/// What one read found.
#[derive(Debug, Clone, Copy)]
struct Page {
    /// Changes read.
    read: usize,
    /// Whether the read stopped at [`PAGE`] (more may be waiting).
    full: bool,
}

impl Feed {
    /// The next frame, or `None` once the stream is over.
    async fn next(&mut self) -> Option<Frame> {
        loop {
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
                Ok(page) if page.read > 0 => continue,
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "change feed: reading changes failed; closing the stream");
                    self.end("closing", &json!({ "reason": "unavailable" }));
                    continue;
                }
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
        self.position.seal(&self.state.signer, &self.workspace)
    }

    /// Read the next page after the cursor and queue its visible changes.
    async fn read_page(&mut self) -> Result<Page, Error> {
        let rows = {
            let _permit = self.state.changes.query_permit().await;
            let mut tx = self.state.db.begin_for(self.subscription.workspace).await?;
            let rows = sqlx::query!(
                r#"SELECT seq AS "seq!", event_id, record_type, record_id, op,
                          container_type, container_id, audience, audience_user,
                          floor(extract(epoch FROM occurred_at))::bigint AS "published_at!"
                     FROM kernel.changes
                    WHERE workspace_id = $1 AND seq > $2
                    ORDER BY seq
                    LIMIT $3"#,
                self.workspace,
                self.cursor,
                PAGE
            )
            .fetch_all(&mut *tx)
            .await?;
            tx.commit().await?;
            rows
        };
        let page = Page {
            read: rows.len(),
            full: i64::try_from(rows.len()).unwrap_or(i64::MAX) >= PAGE,
        };
        let Some(last) = rows.last().map(|row| row.seq) else {
            return Ok(page);
        };
        let rows: Vec<Row> = rows
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
        self.cursor = last;
        let touches = |records: &[&str]| {
            rows.iter()
                .any(|row| records.contains(&row.record.record_type.as_str()))
        };
        let access_changed = touches(self.access_records);
        let containers_changed = touches(self.container_records);

        // Who may see what changed before this page is read: the membership
        // first (a former member gets nothing more), then the containers
        // already shown.
        if access_changed {
            self.check_access().await?;
            if self.done {
                return Ok(page);
            }
        }
        if access_changed || containers_changed {
            self.recheck_shown().await?;
        }

        // Container visibility, asked once per container for this page.
        let mut page_containers: HashMap<RecordRef, bool> = HashMap::new();
        for row in &rows {
            if self.visible(row, &mut page_containers).await? {
                self.position = Position {
                    seq: row.seq,
                    at: row.published_at,
                };
                let frame = change_frame(row, &self.token());
                self.out.push_back(frame);
            }
        }
        // A container shown before and hidden now: drop the client's cache.
        let out_of_sight = page_containers
            .iter()
            .any(|(container, visible)| !visible && self.shown.contains(container));
        if out_of_sight {
            self.lost_sight();
        }
        Ok(page)
    }

    /// Whether the caller may see `row`; container answers are kept in
    /// `page` for the rest of the page.
    async fn visible(
        &mut self,
        row: &Row,
        page: &mut HashMap<RecordRef, bool>,
    ) -> Result<bool, Error> {
        if !self
            .rules
            .allows(&self.subscription.principal, &row.record.record_type)
        {
            return Ok(false);
        }
        let admitted = match row.audience {
            RowAudience::Everyone => true,
            RowAudience::Unknown => false,
            RowAudience::Only(audience) => audience.admits(&self.subscription.access),
        };
        if !admitted {
            return Ok(false);
        }
        let Some(container) = &row.container else {
            return Ok(true);
        };
        let visible = if let Some(visible) = page.get(container) {
            *visible
        } else {
            let visible = self
                .state
                .visibility
                .container_visible(&self.state.db, &self.subscription.access, container)
                .await?;
            page.insert(container.clone(), visible);
            visible
        };
        if visible {
            self.shown.insert(container.clone());
        }
        Ok(visible)
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

    /// The heartbeat, on its fixed timer: check membership and the
    /// containers shown, then send a fresh token. The token moves past
    /// what was read when a read that started after the heartbeat's time
    /// reached the end of the feed, so a resume skips the hidden changes
    /// before it; whether it moved is sealed inside it.
    async fn heartbeat(&mut self) -> Result<(), Error> {
        let now = Instant::now();
        while self.next_heartbeat <= now {
            self.next_heartbeat += self.heartbeat;
        }
        self.check_access().await?;
        if self.done {
            return Ok(());
        }
        self.recheck_shown().await?;
        // The time first, then a read: a change that read did not see was
        // numbered after it.
        let at = {
            let _permit = self.state.changes.query_permit().await;
            let mut tx = self.state.db.begin_for(self.subscription.workspace).await?;
            let at =
                sqlx::query_scalar!(r#"SELECT floor(extract(epoch FROM now()))::bigint AS "now!""#)
                    .fetch_one(&mut *tx)
                    .await?;
            tx.commit().await?;
            at
        };
        let page = self.read_page().await?;
        if self.done {
            return Ok(());
        }
        if !page.full {
            self.position = Position {
                seq: self.cursor,
                at,
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

    #[test]
    fn tokens_are_sealed_fresh_fixed_length_and_bound_to_their_workspace() {
        let signer = Signer::new(&crate::crypto::SecretKey::from_bytes([7; 32]));
        let small = Position { seq: 1, at: NOW };
        let large = Position {
            seq: 9_000_000_000,
            at: NOW,
        };
        let a = small.seal(&signer, "ws-a");
        let b = small.seal(&signer, "ws-a");
        assert_ne!(a, b, "a fresh value every time");
        assert_eq!(a.len(), large.seal(&signer, "ws-a").len(), "one length");
        assert_eq!(Position::open(&signer, "ws-a", &a), Some(small));
        assert_eq!(
            Position::open(&signer, "ws-a", &format!(" {a}\n")),
            Some(small)
        );
        assert_eq!(
            Position::open(&signer, "ws-b", &a),
            None,
            "another workspace"
        );
        assert_eq!(Position::open(&Signer::default(), "ws-a", &a), None);
        for legacy in ["41", "", "nonsense", "-3"] {
            assert_eq!(Position::open(&signer, "ws-a", legacy), None, "{legacy}");
        }
    }
}
