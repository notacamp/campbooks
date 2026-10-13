//! Serving one workspace's feed as Server-Sent Events.
//!
//! ```text
//! retry: 3000
//! id: 41  event: ready    data: {"cursor":"41","head":"44"}
//! id: 42  event: change   data: {"cursor":"42","record":{"type":"…","id":"…"},"op":"upsert","version":"…"}
//! : heartbeat
//! id: 44  event: heartbeat data: {"cursor":"44"}       (the cursor moved past hidden changes)
//! id: 44  event: reset    data: {"cursor":"44","reason":"cursor_expired"}
//!         event: revoked  data: {}
//!         event: closing  data: {"reason":"max_age"}
//! ```

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    convert::Infallible,
    time::Duration,
};

use axum::response::sse::Event as Frame;
use chrono::Utc;
use futures_util::Stream;
use serde_json::json;
use tokio::{sync::watch, time::Instant};

use super::{ChangeOp, hub::StreamSlot};
use crate::{
    AppState, Error,
    auth::Principal,
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

/// A request to follow a workspace's changes.
#[derive(Debug, Clone)]
pub struct Subscription {
    /// The workspace.
    pub workspace: WorkspaceId,
    /// What the caller may do there, as the membership port resolved it.
    pub access: Access,
    /// The caller.
    pub principal: Principal,
    /// The cursor to resume after (`Last-Event-ID` or `?after=`); `None`
    /// starts at the current head.
    pub after: Option<String>,
}

/// Which callers may see changes of each record type: the scopes of the
/// read operations that return it (ADR 0013 `x-campbooks.records`).
#[derive(Debug, Default)]
struct ReadRules(BTreeMap<String, Option<BTreeSet<String>>>);

impl ReadRules {
    fn from_state(state: &AppState) -> Self {
        let mut rules: BTreeMap<String, Option<BTreeSet<String>>> = BTreeMap::new();
        let Some(catalogue) = state.operations.catalogue() else {
            return Self(rules);
        };
        for operation in catalogue.iter().filter(|op| !op.mutates()) {
            let Some(meta) = &operation.meta else {
                continue;
            };
            for record in &meta.records {
                let rule = rules
                    .entry(record.clone())
                    .or_insert_with(|| Some(BTreeSet::new()));
                match (operation.scopes.is_empty(), rule) {
                    // A public read: anyone in the workspace may see it.
                    (true, rule) => *rule = None,
                    (false, Some(scopes)) => scopes.extend(operation.scopes.iter().cloned()),
                    (false, None) => {}
                }
            }
        }
        Self(rules)
    }

    /// Whether `principal` holds a scope of some read of `record_type`.
    /// A record type no read operation returns is shown to nobody.
    fn allows(&self, principal: &Principal, record_type: &str) -> bool {
        match self.0.get(record_type) {
            None => false,
            Some(None) => true,
            Some(Some(scopes)) => scopes.iter().any(|scope| principal.has_scope(scope)),
        }
    }
}

/// Where a stream starts.
#[derive(Debug, PartialEq, Eq)]
enum Start {
    /// From this cursor.
    From(i64),
    /// From the head, after telling the client to drop its cache.
    Reset(&'static str),
}

fn start(after: Option<&str>, head: i64, oldest: Option<i64>, max_backlog: u64) -> Start {
    let Some(after) = after else {
        return Start::From(head);
    };
    let Ok(cursor) = after.trim().parse::<i64>() else {
        return Start::Reset("unknown_cursor");
    };
    if cursor < 0 || cursor > head {
        return Start::Reset("unknown_cursor");
    }
    if u64::try_from(head - cursor).unwrap_or(u64::MAX) > max_backlog {
        return Start::Reset("too_far_behind");
    }
    // Every change after `cursor` must still be here: the oldest kept one
    // is the next after it, or nothing was kept and nothing is missing.
    let complete = cursor == head || oldest.is_some_and(|oldest| oldest <= cursor + 1);
    if complete {
        Start::From(cursor)
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
    let bounds = sqlx::query!(
        r#"SELECT coalesce((SELECT head FROM kernel.change_heads WHERE workspace_id = $1), 0) AS "head!",
                  (SELECT min(seq) FROM kernel.changes WHERE workspace_id = $1 AND seq IS NOT NULL) AS oldest"#,
        workspace
    )
    .fetch_one(&state.db)
    .await?;
    let (cursor, first) = match start(
        subscription.after.as_deref(),
        bounds.head,
        bounds.oldest,
        config.max_backlog,
    ) {
        Start::From(cursor) => (
            cursor,
            Frame::default()
                .event("ready")
                .id(cursor.to_string())
                .retry(CLIENT_RETRY)
                .data(
                    json!({ "cursor": cursor.to_string(), "head": bounds.head.to_string() })
                        .to_string(),
                ),
        ),
        Start::Reset(reason) => (
            bounds.head,
            Frame::default()
                .event("reset")
                .id(bounds.head.to_string())
                .retry(CLIENT_RETRY)
                .data(json!({ "cursor": bounds.head.to_string(), "reason": reason }).to_string()),
        ),
    };

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
        state: state.clone(),
        workspace,
        subscription,
        cursor,
        delivered: cursor,
        wake,
        deadline,
        deadline_reason,
        heartbeat,
        next_heartbeat: now + heartbeat,
        poll: Duration::from_millis(config.poll_interval_ms),
        out: VecDeque::from([first]),
        containers: HashMap::new(),
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
    /// The last change read (sent or hidden).
    cursor: i64,
    /// The last cursor the client was told.
    delivered: i64,
    wake: watch::Receiver<u64>,
    deadline: Instant,
    deadline_reason: &'static str,
    heartbeat: Duration,
    next_heartbeat: Instant,
    poll: Duration,
    out: VecDeque<Frame>,
    /// Visibility of containers already asked about.
    containers: HashMap<RecordRef, bool>,
    done: bool,
    _slot: StreamSlot,
}

/// A change as read from `kernel.changes`.
struct Row {
    seq: i64,
    event_id: uuid::Uuid,
    record: RecordRef,
    op: ChangeOp,
    container: Option<RecordRef>,
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
            if Instant::now() >= self.deadline {
                self.end("closing", &json!({ "reason": self.deadline_reason }));
                continue;
            }
            match self.read_page().await {
                Ok(true) => continue,
                Ok(false) => {}
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
                () = heartbeat => {
                    if let Err(e) = self.heartbeat().await {
                        tracing::warn!(error = %e, "change feed: membership check failed; closing the stream");
                        self.end("closing", &json!({ "reason": "unavailable" }));
                    }
                }
                () = deadline => {}
            }
        }
    }

    fn end(&mut self, event: &'static str, data: &serde_json::Value) {
        self.out
            .push_back(Frame::default().event(event).data(data.to_string()));
        self.done = true;
    }

    /// Read the next page after the cursor and queue its visible changes.
    /// Returns whether anything was read.
    async fn read_page(&mut self) -> Result<bool, Error> {
        let rows = {
            let _permit = self.state.changes.query_permit().await;
            sqlx::query!(
                r#"SELECT seq AS "seq!", event_id, record_type, record_id, op,
                          container_type, container_id
                     FROM kernel.changes
                    WHERE workspace_id = $1 AND seq > $2
                    ORDER BY seq
                    LIMIT $3"#,
                self.workspace,
                self.cursor,
                PAGE
            )
            .fetch_all(&self.state.db)
            .await?
        };
        let Some(last) = rows.last().map(|row| row.seq) else {
            return Ok(false);
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
            })
            .collect();
        self.cursor = last;
        let access_changed = rows.iter().any(|row| {
            self.access_records
                .contains(&row.record.record_type.as_str())
        });

        // Several changes of one record in a page: the last one is enough
        // (the client refetches the current state either way).
        let mut latest: HashMap<&RecordRef, i64> = HashMap::new();
        for row in &rows {
            latest.insert(&row.record, row.seq);
        }
        let keep: BTreeSet<i64> = latest.into_values().collect();
        for row in rows.iter().filter(|row| keep.contains(&row.seq)) {
            if self.visible(row).await? {
                self.out.push_back(change_frame(row));
                self.delivered = row.seq;
            }
        }
        if !self.out.is_empty() {
            self.next_heartbeat = Instant::now() + self.heartbeat;
        }
        if access_changed {
            self.check_access().await?;
        }
        Ok(true)
    }

    /// Whether the caller may see `row`.
    async fn visible(&mut self, row: &Row) -> Result<bool, Error> {
        if !self
            .rules
            .allows(&self.subscription.principal, &row.record.record_type)
        {
            return Ok(false);
        }
        let Some(container) = &row.container else {
            return Ok(true);
        };
        if let Some(visible) = self.containers.get(container) {
            return Ok(*visible);
        }
        let visible = self
            .state
            .visibility
            .container_visible(&self.state.db, &self.subscription.access, container)
            .await?;
        self.containers.insert(container.clone(), visible);
        Ok(visible)
    }

    /// Resolve membership again; when it is gone, say so and end.
    async fn check_access(&mut self) -> Result<(), Error> {
        let access = self
            .state
            .membership
            .resolve(
                &self.state.db,
                &self.subscription.principal,
                self.subscription.workspace,
            )
            .await?
            .filter(|access| access.workspace == self.subscription.workspace);
        match access {
            Some(access) => {
                if access != self.subscription.access {
                    // A changed role may change what containers show.
                    self.containers.clear();
                    self.subscription.access = access;
                }
            }
            None => {
                self.end("revoked", &json!({}));
            }
        }
        Ok(())
    }

    /// An idle stream's heartbeat: check membership, then a comment, or the
    /// cursor when it moved past changes the caller may not see.
    async fn heartbeat(&mut self) -> Result<(), Error> {
        self.next_heartbeat = Instant::now() + self.heartbeat;
        self.check_access().await?;
        if self.done {
            return Ok(());
        }
        let frame = if self.delivered == self.cursor {
            Frame::default().comment("heartbeat")
        } else {
            self.delivered = self.cursor;
            Frame::default()
                .event("heartbeat")
                .id(self.cursor.to_string())
                .data(json!({ "cursor": self.cursor.to_string() }).to_string())
        };
        self.out.push_back(frame);
        Ok(())
    }
}

/// The frame of one change: ids only.
fn change_frame(row: &Row) -> Frame {
    let cursor = row.seq.to_string();
    Frame::default().event("change").id(cursor.clone()).data(
        json!({
            "cursor": cursor,
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

    #[test]
    fn starts_at_the_head_without_a_cursor() {
        assert_eq!(start(None, 7, Some(1), 100), Start::From(7));
        assert_eq!(start(None, 0, None, 100), Start::From(0));
    }

    #[test]
    fn resumes_from_a_kept_cursor() {
        assert_eq!(start(Some("3"), 7, Some(1), 100), Start::From(3));
        assert_eq!(start(Some("3"), 7, Some(4), 100), Start::From(3));
        assert_eq!(start(Some("7"), 7, None, 100), Start::From(7));
        assert_eq!(start(Some("0"), 0, None, 100), Start::From(0));
    }

    #[test]
    fn resets_unknown_expired_and_distant_cursors() {
        assert_eq!(
            start(Some("x"), 7, Some(1), 100),
            Start::Reset("unknown_cursor")
        );
        assert_eq!(
            start(Some("-1"), 7, Some(1), 100),
            Start::Reset("unknown_cursor")
        );
        assert_eq!(
            start(Some("8"), 7, Some(1), 100),
            Start::Reset("unknown_cursor")
        );
        assert_eq!(
            start(Some("2"), 7, Some(5), 100),
            Start::Reset("cursor_expired")
        );
        assert_eq!(
            start(Some("2"), 7, None, 100),
            Start::Reset("cursor_expired")
        );
        assert_eq!(
            start(Some("2"), 700, Some(1), 100),
            Start::Reset("too_far_behind")
        );
    }
}
