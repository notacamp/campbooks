//! Reading one page of a Gmail stream.

use std::collections::{BTreeMap, HashMap, HashSet};

use campbooks_kernel::{
    blobs::{BlobRef, BlobStore},
    db::BeginFor as _,
    tenancy::WorkspaceId,
};
use chrono::{DateTime, TimeDelta, Utc};
use futures_util::{StreamExt as _, stream::FuturesOrdered};
use sqlx::PgPool;
use uuid::Uuid;

use super::{Cursor, FETCH_CONCURRENCY, LIST_PAGE, MAX_CHANGED, Page, normalize};
use crate::{
    contract::{Direction, MessageChange, NormalizedMessage},
    domain::{
        marks::{self, MarkIds},
        model::{Phase, StreamId},
        outbound,
    },
    google::{Gmail, LabelInfo, MessageMeta, MessageRaw, RawFetch},
    mail::{self, Fetched, IndexOp, ReadError, apply_flags},
    provider::ProviderError,
};

/// Everything a run needs to read one page.
pub struct Reader<'a> {
    /// Gmail, with a valid access token.
    pub gmail: Gmail<'a>,
    /// The pool (runtime role), for reading the index.
    pub pool: &'a PgPool,
    /// The blob store.
    pub blobs: &'a BlobStore,
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The stream.
    pub stream: StreamId,
    /// Its phase.
    pub phase: Phase,
    /// Its cursor.
    pub cursor: Cursor,
    /// The time.
    pub now: DateTime<Utc>,
    /// The window of a backfill or resync, in days.
    pub backfill_days: u32,
    /// Messages larger than this are skipped as oversized.
    pub max_message_bytes: u64,
    /// No download starts once a run has fetched this many bytes.
    pub run_budget_bytes: u64,
}

/// An indexed message.
#[derive(Debug, Clone)]
struct Indexed {
    row: Uuid,
    label_ids: Vec<String>,
    document: BlobRef,
}

/// How a message changed during a run of history.
#[derive(Debug, Default, Clone, Copy)]
struct Seen {
    added: bool,
    deleted: bool,
}

impl Reader<'_> {
    /// Read the next page.
    pub async fn read(&self) -> Result<Page, ReadError> {
        match self.phase {
            Phase::Backfill | Phase::Resync => self.list_window().await,
            Phase::Incremental => self.follow_history().await,
        }
    }

    fn workspace_key(&self) -> String {
        self.workspace.to_string()
    }

    /// One page of the window: backfill, or a resync pass.
    async fn list_window(&self) -> Result<Page, ReadError> {
        let mut cursor = self.cursor.clone();
        if cursor.history_id.is_none() {
            // Start of a pass: remember where history stands now, so the
            // incremental phase picks up whatever arrives meanwhile.
            cursor.history_id = Some(self.gmail.profile().await?.history_id);
        }
        let window_start = *cursor
            .window_start
            .get_or_insert_with(|| self.now - TimeDelta::days(i64::from(self.backfill_days)));
        let query = format!("after:{}", window_start.timestamp());
        let listed = self
            .gmail
            .list(&query, cursor.page_token.as_deref(), LIST_PAGE)
            .await?;
        let ids: Vec<String> = listed.messages.iter().map(|m| m.id.clone()).collect();
        let indexed = self.indexed(&ids).await?;
        let oversized = self.oversized(&ids).await?;
        let mut work = Work::default();
        // Gmail lists newest first; a batch reads oldest first.
        for id in ids.iter().rev() {
            match indexed.get(id) {
                None if oversized.contains(id) => {}
                None => work.fetch_raw.push(id.clone()),
                Some(known) if self.phase == Phase::Resync => {
                    work.recheck.push((id.clone(), known.clone()));
                }
                Some(_) => {}
            }
        }
        let mut page = self.run(work, cursor.generation).await?;
        if !page.complete {
            // Over the byte budget: the same page again next run (what was
            // stored is indexed and skipped then).
            page.phase = self.phase;
            page.more = true;
            page.cursor = cursor;
            return Ok(page);
        }
        cursor.page_token = listed.next_page_token;
        if cursor.page_token.is_none() {
            if self.phase == Phase::Resync {
                self.remove_unlisted(&mut page, window_start, cursor.generation)
                    .await?;
            }
            cursor.window_start = None;
            page.phase = Phase::Incremental;
            page.more = false;
        } else {
            page.phase = self.phase;
            page.more = true;
        }
        page.cursor = cursor;
        Ok(page)
    }

    /// One run of history from the cursor.
    async fn follow_history(&self) -> Result<Page, ReadError> {
        let mut cursor = self.cursor.clone();
        let Some(start) = cursor.history_id.clone() else {
            // No history id yet: list the window first.
            return Ok(Self::empty_page(cursor, Phase::Backfill, true));
        };
        let mut seen: BTreeMap<String, Seen> = BTreeMap::new();
        let mut order: Vec<String> = Vec::new();
        let mut page_token = cursor.page_token.clone();
        let latest = loop {
            let history = match self.gmail.history(&start, page_token.as_deref(), 500).await {
                Ok(history) => history,
                Err(ProviderError::NotFound) => {
                    // Gmail forgot this history id: re-read the window.
                    tracing::info!(stream = %self.stream, "history expired; resyncing the window");
                    let next = Cursor {
                        history_id: None,
                        page_token: None,
                        window_start: None,
                        generation: cursor.generation + 1,
                    };
                    return Ok(Self::empty_page(next, Phase::Resync, true));
                }
                Err(other) => return Err(other.into()),
            };
            for record in &history.history {
                let mut note = |id: &str, f: &dyn Fn(&mut Seen)| {
                    let entry = seen.entry(id.to_owned()).or_insert_with(|| {
                        order.push(id.to_owned());
                        Seen::default()
                    });
                    f(entry);
                };
                for m in &record.messages_added {
                    note(&m.message.id, &|s| s.added = true);
                }
                for m in record.labels_added.iter().chain(&record.labels_removed) {
                    note(&m.message.id, &|_| {});
                }
                for m in &record.messages_deleted {
                    note(&m.message.id, &|s| s.deleted = true);
                }
            }
            page_token = history.next_page_token;
            if page_token.is_none() {
                break Some(history.history_id);
            }
            if seen.len() >= MAX_CHANGED {
                break None;
            }
        };
        let indexed = self.indexed(&order).await?;
        let oversized = self.oversized(&order).await?;
        let mut work = Work::default();
        for id in &order {
            let state = seen[id];
            match (indexed.get(id), state.deleted) {
                (Some(known), true) => work.removed.push((id.clone(), known.row)),
                (Some(known), false) => work.recheck.push((id.clone(), known.clone())),
                (None, false) if state.added && !oversized.contains(id) => {
                    work.fetch_raw.push(id.clone());
                }
                // Deleted before we carried it, or label changes on a
                // message we never carried (older than the window): stays
                // out of Campbooks.
                (None, _) => {}
            }
        }
        let mut page = self.run(work, cursor.generation).await?;
        if !page.complete {
            // Over the byte budget: read the same history again next run;
            // stored messages are known by then and only re-checked.
            page.more = true;
            page.cursor = self.cursor.clone();
            page.phase = Phase::Incremental;
            return Ok(page);
        }
        if let Some(history_id) = latest {
            cursor.history_id = Some(history_id);
            cursor.page_token = None;
            page.more = false;
        } else {
            cursor.page_token = page_token;
            page.more = true;
        }
        page.cursor = cursor;
        page.phase = Phase::Incremental;
        Ok(page)
    }

    fn empty_page(cursor: Cursor, phase: Phase, more: bool) -> Page {
        Page {
            changes: Vec::new(),
            index: Vec::new(),
            labels: None,
            cursor,
            phase,
            more,
            complete: true,
        }
    }

    /// Fetch, normalise and store what `work` lists.
    ///
    /// New messages are downloaded at most [`FETCH_CONCURRENCY`] at a time
    /// and stored as each arrives, so a run holds a handful of messages in
    /// memory, not a page. No new download starts once the run has fetched
    /// its byte budget: the page is then incomplete ([`Page::complete`]) and
    /// the caller keeps its cursor, so the next run picks up the rest (what
    /// was stored is in the index by then and is not fetched again).
    /// Messages over the size limit are skipped and recorded as oversized.
    async fn run(&self, mut work: Work, generation: i32) -> Result<Page, ReadError> {
        let forced = self.reemits(&mut work).await?;
        let rechecked: Vec<String> = work.recheck.iter().map(|(id, _)| id.clone()).collect();
        let overlay = outbound::overlay(self.pool, self.workspace, self.stream, &rechecked).await?;
        let mut names = self.label_names().await?;
        let mut fresh_labels = None;
        let mut changes = Vec::new();
        let mut index = Vec::new();

        let complete = self
            .fetch_new(
                &work.fetch_raw,
                generation,
                &mut names,
                &mut fresh_labels,
                &mut changes,
                &mut index,
            )
            .await?;

        // Known messages: labels only, small answers.
        let checks: Vec<_> = work
            .recheck
            .iter()
            .map(|(id, _)| self.gmail.minimal(id))
            .collect();
        let metas: Vec<Result<MessageMeta, ProviderError>> = futures_util::stream::iter(checks)
            .buffered(FETCH_CONCURRENCY)
            .collect()
            .await;
        let unknown = metas
            .iter()
            .filter_map(|m| m.as_ref().ok())
            .flat_map(|m| &m.label_ids)
            .any(|id| !names.known.contains_key(id));
        if fresh_labels.is_none() && unknown {
            let labels = self.gmail.labels().await?;
            names = LabelNames::from_list(&labels);
            fresh_labels = Some(labels);
        }
        for ((id, known), meta) in work.recheck.into_iter().zip(metas) {
            let meta = match meta {
                Ok(meta) => meta,
                Err(ProviderError::NotFound) => {
                    changes.push(MessageChange::Remove {
                        provider_message_id: id,
                    });
                    index.push(IndexOp::Remove { row: known.row });
                    continue;
                }
                Err(other) => return Err(other.into()),
            };
            let mut labels = meta.label_ids.clone();
            labels.sort_unstable();
            let mut before = known.label_ids.clone();
            before.sort_unstable();
            if labels != before || forced.contains(&id) {
                let mut message = self.document(&known.document).await?;
                // A member's change still waiting to be made shows as
                // they left it (its stream marks as the labels they are).
                let shown = overlay.get(&id).map_or_else(
                    || meta.label_ids.clone(),
                    |changes| {
                        let changes =
                            marks::resolve_changes(changes, &names.marks(), ToOwned::to_owned);
                        outbound::apply_labels(&meta.label_ids, &changes)
                    },
                );
                let mut flags = normalize::flags(&shown, &names.user);
                // Moved to another tab while keeping its stream mark: the
                // tab wins, and the mark is stale.
                if flags.direction == Direction::Inbound
                    && let Some(stream) =
                        normalize::category_won(&known.label_ids, &meta.label_ids, &names.user)
                {
                    flags.provider_stream = Some(stream);
                    flags.stream_mark_stale = true;
                }
                apply_flags(&mut message, &flags);
                changes.push(MessageChange::Upsert {
                    message: Box::new(message),
                });
            }
            index.push(IndexOp::Update {
                row: known.row,
                label_ids: meta.label_ids,
                generation,
            });
        }
        for (id, row) in work.removed {
            changes.push(MessageChange::Remove {
                provider_message_id: id,
            });
            index.push(IndexOp::Remove { row });
        }
        Ok(Page {
            changes,
            index,
            labels: fresh_labels,
            cursor: Cursor::default(),
            phase: self.phase,
            more: false,
            complete,
        })
    }

    /// Download, store and emit the new messages `ids`, at most
    /// [`FETCH_CONCURRENCY`] at a time, starting no download once the byte
    /// budget is spent. Returns whether every one was handled.
    async fn fetch_new(
        &self,
        ids: &[String],
        generation: i32,
        names: &mut LabelNames,
        fresh: &mut Option<Vec<LabelInfo>>,
        changes: &mut Vec<MessageChange>,
        index: &mut Vec<IndexOp>,
    ) -> Result<bool, ReadError> {
        let mut fetched: u64 = 0;
        let mut next = 0;
        let mut in_flight = FuturesOrdered::new();
        loop {
            while in_flight.len() < FETCH_CONCURRENCY
                && fetched < self.run_budget_bytes
                && next < ids.len()
            {
                let id = &ids[next];
                next += 1;
                in_flight.push_back(async move {
                    (id, self.gmail.raw(id, self.max_message_bytes).await)
                });
            }
            let Some((id, result)) = in_flight.next().await else {
                break;
            };
            let raw = match result {
                Ok(RawFetch::Message(raw)) => raw,
                Ok(RawFetch::TooLarge) => {
                    tracing::info!(stream = %self.stream, "message over the size limit skipped");
                    index.push(IndexOp::Oversized {
                        provider_message_id: id.clone(),
                    });
                    continue;
                }
                // Deleted between listing and fetching.
                Err(ProviderError::NotFound) => continue,
                Err(other) => return Err(other.into()),
            };
            fetched = fetched.saturating_add(raw.fetched_bytes);
            if normalize::skipped(&raw.meta.label_ids) {
                continue;
            }
            if fresh.is_none()
                && raw
                    .meta
                    .label_ids
                    .iter()
                    .any(|l| !names.known.contains_key(l))
            {
                let labels = self.gmail.labels().await?;
                *names = LabelNames::from_list(&labels);
                *fresh = Some(labels);
            }
            let (message, op) = self.store_new(id, raw, &names.user, generation).await?;
            changes.push(MessageChange::Upsert {
                message: Box::new(message),
            });
            index.push(op);
        }
        Ok(next >= ids.len())
    }

    /// Store a fetched message's blobs and build its upsert.
    async fn store_new(
        &self,
        id: &str,
        raw: MessageRaw,
        user_labels: &HashMap<String, String>,
        generation: i32,
    ) -> Result<(NormalizedMessage, IndexOp), ReadError> {
        mail::store_message(
            self.blobs,
            &self.workspace_key(),
            fetched(id, raw, user_labels, self.now),
            generation,
        )
        .await
    }

    /// A stored message document.
    async fn document(&self, blob: &BlobRef) -> Result<NormalizedMessage, ReadError> {
        mail::document(self.blobs, &self.workspace_key(), blob).await
    }

    /// Add the messages flagged for re-emitting (a member's change to them
    /// was overtaken or refused) to `work`'s re-checks; returns them.
    async fn reemits(&self, work: &mut Work) -> Result<HashSet<String>, ReadError> {
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows = sqlx::query!(
            r#"SELECT id, provider_message_id, label_ids, document_hash, document_size
                 FROM connectors.messages
                WHERE stream_id = $1 AND reemit
                ORDER BY internal_date LIMIT $2"#,
            self.stream as StreamId,
            i64::try_from(MAX_CHANGED).unwrap_or(100),
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut forced = HashSet::new();
        for row in rows {
            let id = row.provider_message_id;
            let listed = work.recheck.iter().any(|(r, _)| *r == id)
                || work.removed.iter().any(|(r, _)| *r == id);
            if !listed {
                let indexed =
                    indexed_row(row.id, row.label_ids, &row.document_hash, row.document_size)?;
                work.recheck.push((id.clone(), indexed));
            }
            forced.insert(id);
        }
        Ok(forced)
    }

    /// Which of `ids` were skipped as oversized before.
    async fn oversized(&self, ids: &[String]) -> Result<HashSet<String>, ReadError> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows = sqlx::query_scalar!(
            r#"SELECT provider_message_id FROM connectors.oversized
                WHERE stream_id = $1 AND provider_message_id = ANY($2)"#,
            self.stream as StreamId,
            ids,
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows.into_iter().collect())
    }

    /// The index rows of `ids` in this stream.
    async fn indexed(&self, ids: &[String]) -> Result<HashMap<String, Indexed>, ReadError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows = sqlx::query!(
            r#"SELECT id, provider_message_id, label_ids, document_hash, document_size
                 FROM connectors.messages
                WHERE stream_id = $1 AND provider_message_id = ANY($2)"#,
            self.stream as StreamId,
            ids,
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows.into_iter()
            .map(|row| {
                let indexed =
                    indexed_row(row.id, row.label_ids, &row.document_hash, row.document_size)?;
                Ok((row.provider_message_id, indexed))
            })
            .collect()
    }

    /// Indexed messages of the window that this resync pass did not list.
    async fn remove_unlisted(
        &self,
        page: &mut Page,
        window_start: DateTime<Utc>,
        generation: i32,
    ) -> Result<(), ReadError> {
        let listed_now: Vec<Uuid> = page
            .index
            .iter()
            .filter_map(|op| match op {
                IndexOp::Insert { row, .. } | IndexOp::Update { row, .. } => Some(*row),
                IndexOp::Remove { .. } | IndexOp::Oversized { .. } | IndexOp::Skipped { .. } => {
                    None
                }
            })
            .collect();
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let gone = sqlx::query!(
            r#"SELECT id, provider_message_id FROM connectors.messages
                WHERE stream_id = $1 AND internal_date >= $2 AND listed_generation < $3
                  AND NOT (id = ANY($4))
                ORDER BY internal_date"#,
            self.stream as StreamId,
            window_start,
            generation,
            &listed_now,
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        for row in gone {
            page.changes.push(MessageChange::Remove {
                provider_message_id: row.provider_message_id,
            });
            page.index.push(IndexOp::Remove { row: row.id });
        }
        Ok(())
    }

    /// The cached label names of this stream.
    async fn label_names(&self) -> Result<LabelNames, ReadError> {
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows = sqlx::query!(
            "SELECT label_id, name, kind FROM connectors.gmail_labels WHERE stream_id = $1",
            self.stream as StreamId
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut names = LabelNames::default();
        for row in rows {
            if row.kind == "user" {
                names.user.insert(row.label_id.clone(), row.name.clone());
            }
            names.known.insert(row.label_id, row.name);
        }
        Ok(names)
    }
}

/// An index row, decoded.
fn indexed_row(
    row: Uuid,
    label_ids: Vec<String>,
    document_hash: &str,
    document_size: i64,
) -> Result<Indexed, ReadError> {
    Ok(Indexed {
        row,
        label_ids,
        document: mail::document_ref(document_hash, document_size)?,
    })
}

/// What a run has to fetch.
#[derive(Debug, Default)]
struct Work {
    /// Unknown messages: fetch raw.
    fetch_raw: Vec<String>,
    /// Known messages: fetch labels, re-emit if they changed.
    recheck: Vec<(String, Indexed)>,
    /// Known messages deleted at Gmail.
    removed: Vec<(String, Uuid)>,
}

/// Label names: every label Gmail listed, and the user labels among them.
#[derive(Debug, Default)]
struct LabelNames {
    known: HashMap<String, String>,
    user: HashMap<String, String>,
}

impl LabelNames {
    /// The stream marks among the user labels.
    fn marks(&self) -> MarkIds {
        MarkIds::from_labels(
            self.user
                .iter()
                .map(|(id, name)| (id.as_str(), name.as_str())),
        )
    }

    fn from_list(labels: &[LabelInfo]) -> Self {
        let mut names = Self::default();
        for label in labels {
            if label.kind.as_deref() == Some("user") {
                names.user.insert(label.id.clone(), label.name.clone());
            }
            names.known.insert(label.id.clone(), label.name.clone());
        }
        names
    }
}

/// A message downloaded from Gmail (or just sent through it), ready to
/// store.
pub fn fetched(
    id: &str,
    raw: MessageRaw,
    user_labels: &HashMap<String, String>,
    now: DateTime<Utc>,
) -> Fetched {
    let internal_date = raw
        .meta
        .internal_date
        .as_deref()
        .and_then(|ms| ms.parse::<i64>().ok())
        .and_then(DateTime::from_timestamp_millis)
        .unwrap_or(now);
    Fetched {
        id: id.to_owned(),
        thread_key: Some(raw.meta.thread_id),
        flags: normalize::flags(&raw.meta.label_ids, user_labels),
        label_ids: raw.meta.label_ids,
        internal_date,
        raw: raw.raw,
        receiver: mail::Receiver::Gmail,
    }
}
