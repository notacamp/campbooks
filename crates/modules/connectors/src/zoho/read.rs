//! Reading one page of a Zoho mailbox stream.
//!
//! Zoho has no change log, so the adapter reads folders, newest first:
//!
//! - **Backfill / sweep** (`backfill`, `resync`): every tracked folder's
//!   window, [`WALK_PAGE`] messages per listing, one listing per run. At
//!   the end of a sweep, indexed messages of the window no listing showed
//!   are resolved (looked up, archived or removed).
//! - **Incremental**: the newest [`HEAD_PAGE`] of every tracked folder.
//!   New messages are downloaded as raw MIME; known ones whose state changed
//!   are re-emitted from the stored document. A known message that left the
//!   range its folder's page covered, and shows in no other listing, is
//!   looked up: archived in place, moved, or gone.
//!
//! The cursor keeps the tracked folders (refreshed at the start of each
//! pass), the walk's position, the window and the pass generation.
//!
//! With the labels scope, a message's Zoho labels (and stream marks) are
//! kept as `label:<id>` tokens, compared like the rest. Zoho's listing names
//! no labels, so they are read through its `labelid` filter: each run the
//! account's labels, then the newest messages of each stream mark (more
//! pages during a walk), which can only add a mark; at the end of each pass
//! every label's messages, a message missing from a label's whole listing
//! having lost it. Indexed messages whose labels this changes are emitted
//! again from the index, without a download.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Mutex, MutexGuard, PoisonError},
};

use campbooks_kernel::{
    blobs::{BlobRef, BlobStore},
    db::BeginFor as _,
    tenancy::WorkspaceId,
};
use chrono::{DateTime, TimeDelta, Utc};
use futures_util::{StreamExt as _, stream::FuturesOrdered};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    Listed, OriginalFetch, ZohoLabel, ZohoMail,
    state::{self, FolderKind},
};
use crate::{
    config::ZohoMailConfig,
    contract::{MessageChange, NormalizedMessage},
    domain::{
        marks::{self, MarkIds},
        model::{Phase, StreamId},
        outbound,
    },
    mail::{self, Fetched, Flags, IndexOp, ReadError, apply_flags},
    provider::ProviderError,
};

/// Messages listed per walk (backfill, sweep) run; Zoho's maximum.
pub const WALK_PAGE: u32 = 200;

/// Messages listed per `labelid` page.
pub const LABEL_PAGE: u32 = 200;

/// Pages of one label read at most: a label on more messages is read in
/// part, which only ever adds it.
pub const LABEL_PAGES: u32 = 10;

/// Messages listed per folder in an incremental run.
pub const HEAD_PAGE: u32 = 50;

/// Most new messages one run downloads.
pub const MAX_NEW: usize = 50;

/// Most vanished messages one run looks up.
pub const MAX_LOOKUPS: usize = 20;

/// Downloads in flight at once within a run.
pub const FETCH_CONCURRENCY: usize = 5;

/// Downloads and look-ups a run is charged for up front, besides its
/// listings: a quiet mailbox's run makes a few; a backfill's makes more,
/// and Zoho's own 429 slows those down.
pub const EXPECTED_FETCHES: usize = 10;

/// What one run of a mailbox with `folders` tracked folders is charged
/// before it starts: a listing per folder, the folder list, and
/// [`EXPECTED_FETCHES`]. Charging the worst case ([`MAX_NEW`] downloads and
/// [`MAX_LOOKUPS`] look-ups) would hold every member's change back for a
/// minute after each run.
pub fn requests_per_run(folders: usize) -> u32 {
    u32::try_from(folders + 1 + EXPECTED_FETCHES).unwrap_or(u32::MAX)
}

/// The label requests a run of a stream with `labels` (id, name) is charged
/// before it starts, when the grant has the labels scope: the account's
/// labels, a page per stream mark, and during a walk a page per label (the
/// pass's last listing reads them all).
pub fn label_requests(granted: bool, phase: Phase, labels: &[(String, String)]) -> u32 {
    if !granted {
        return 0;
    }
    let marks = labels
        .iter()
        .filter(|(_, name)| marks::of_name(name).is_some())
        .count();
    let walk = if phase == Phase::Incremental {
        0
    } else {
        labels.len()
    };
    u32::try_from(1 + marks + walk).unwrap_or(u32::MAX)
}

/// Trouble that stops the whole run (the stream's backoff, throttling and
/// health handle it), rather than one message.
fn is_run_trouble(err: &ProviderError) -> bool {
    matches!(
        err,
        ProviderError::RateLimited { .. }
            | ProviderError::Unauthorized
            | ProviderError::InvalidGrant
    )
}

/// A tracked folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackedFolder {
    /// Zoho's id.
    pub id: String,
    /// What it is.
    pub kind: FolderKind,
}

/// Where a walk is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Walk {
    /// The folder (an index into the cursor's folders).
    pub folder: usize,
    /// The 1-based position of the next listing.
    pub start: u32,
}

/// A Zoho stream's cursor (the `streams.cursor` column).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    /// The folders read, as of the last pass's start.
    #[serde(default)]
    pub folders: Vec<TrackedFolder>,
    /// The walk in progress (backfill, sweep).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk: Option<Walk>,
    /// The start of the window being walked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_start: Option<DateTime<Utc>>,
    /// The current pass; index rows listed in it carry it.
    #[serde(default)]
    pub generation: i32,
    /// When the last pass ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swept_at: Option<DateTime<Utc>>,
    /// The newest message seen, epoch milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest: Option<i64>,
    /// When the first sync (the backfill) completed: mail received before
    /// is never mirrored (set by the run, `sync_stream`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<DateTime<Utc>>,
    /// Removals the end of the last pass held back (label id → the messages
    /// its listing left out): the next pass makes them when its listing
    /// leaves out the same messages, or fewer.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub held_back: BTreeMap<String, Vec<String>>,
    /// Labels the account's listing left out at the end of the last pass,
    /// held back likewise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vanished: Vec<String>,
}

impl Cursor {
    /// The cursor as the opaque string events carry (no personal data).
    pub fn token(&self) -> String {
        format!("{}:{}", self.generation, self.newest.unwrap_or_default())
    }

    /// The Inbox folder.
    pub fn inbox(&self) -> Option<&str> {
        self.folder_of_kind(FolderKind::Inbox)
    }

    /// The first folder of `kind`.
    pub fn folder_of_kind(&self, kind: FolderKind) -> Option<&str> {
        self.folders
            .iter()
            .find(|f| f.kind == kind)
            .map(|f| f.id.as_str())
    }

    fn kind_of(&self, folder: &str) -> FolderKind {
        self.folders
            .iter()
            .find(|f| f.id == folder)
            .map_or(FolderKind::Untracked, |f| f.kind)
    }
}

/// What one run read.
#[derive(Debug, Clone)]
pub struct Page {
    /// The changes for the batch, in order.
    pub changes: Vec<MessageChange>,
    /// Updates to the index, applied with the batch.
    pub index: Vec<IndexOp>,
    /// The cursor after this page.
    pub cursor: Cursor,
    /// The phase after this page.
    pub phase: Phase,
    /// Whether more is waiting (run again at once).
    pub more: bool,
}

/// What a pass held back: removals per label (label → messages), and
/// deletions.
type Held = (BTreeMap<String, Vec<String>>, Vec<String>);

/// The account's Zoho labels as a run knows them, for a grant with the labels
/// scope: their names (id → name), the listing read fresh when it changed
/// them (for the run to store), and the messages each label was read on.
#[derive(Debug, Default)]
pub struct LabelCache(Mutex<Labels>);

#[derive(Debug, Default)]
struct Labels {
    names: HashMap<String, String>,
    /// Whether Zoho listed the labels this run.
    listed: bool,
    fresh: Option<Vec<ZohoLabel>>,
    read: HashMap<String, Membership>,
    /// (message, label) pairs a push of ours changed since the last pass
    /// ended: never taken off by a listing.
    written: HashSet<(String, String)>,
    /// Label requests made this run.
    calls: u32,
    /// Whether this run read every label whole (the end of a pass).
    passed: bool,
    /// Removals held back this pass (label → messages).
    held: BTreeMap<String, Vec<String>>,
    /// Deletions held back this pass.
    vanished: Vec<String>,
}

/// The messages a label was read on this run; `whole` when its whole
/// listing was read, so a message missing from it does not carry it.
#[derive(Debug, Default)]
struct Membership {
    messages: HashSet<String>,
    whole: bool,
}

impl LabelCache {
    /// The cache, holding `names`.
    pub fn new(names: HashMap<String, String>) -> Self {
        Self(Mutex::new(Labels {
            names,
            ..Labels::default()
        }))
    }

    /// Label requests made this run.
    pub fn calls(&self) -> u32 {
        self.lock().calls
    }

    fn called(&self) {
        let mut labels = self.lock();
        labels.calls = labels.calls.saturating_add(1);
    }

    /// Of the labels `held` (on indexed messages), those Zoho's listing this
    /// run does not have (deleted): on no message. None when Zoho did not
    /// list the labels this run.
    fn deleted(&self, held: Vec<String>) -> Vec<String> {
        let labels = self.lock();
        if !labels.listed {
            return Vec::new();
        }
        held.into_iter()
            .filter(|id| !labels.names.contains_key(id))
            .collect()
    }

    /// The run read every label whole; what it held back.
    fn pass(&self) -> Option<Held> {
        let labels = self.lock();
        labels
            .passed
            .then(|| (labels.held.clone(), labels.vanished.clone()))
    }

    /// Never take labels off the (message, label) pairs of `written`.
    fn protect(&self, written: HashSet<(String, String)>) {
        self.lock().written = written;
    }

    fn lock(&self) -> MutexGuard<'_, Labels> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The names now.
    pub fn names(&self) -> HashMap<String, String> {
        self.lock().names.clone()
    }

    /// The labels' ids, stream marks only when `only_marks`.
    fn ids(&self, only_marks: bool) -> Vec<String> {
        let mut ids: Vec<String> = self
            .lock()
            .names
            .iter()
            .filter(|(_, name)| !only_marks || marks::of_name(name).is_some())
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();
        ids
    }

    /// Zoho's listing of the labels, kept as fresh when it changed them.
    fn replace(&self, listed: Vec<ZohoLabel>) {
        let mut labels = self.lock();
        // An empty answer while labels are known is not believed: read as
        // no answer at all.
        if listed.is_empty() && !labels.names.is_empty() {
            return;
        }
        labels.listed = true;
        let names: HashMap<String, String> = listed
            .iter()
            .map(|l| (l.label_id.clone(), l.name()))
            .collect();
        if names != labels.names {
            labels.names = names;
            labels.fresh = Some(listed);
        }
    }

    /// The labels this run read from Zoho, when they changed.
    pub fn fresh(&self) -> Option<Vec<ZohoLabel>> {
        self.lock().fresh.take()
    }

    /// Label `label` was read on `messages` (its whole listing when
    /// `whole`).
    fn record(&self, label: &str, messages: HashSet<String>, whole: bool) {
        let mut labels = self.lock();
        let held = labels.read.entry(label.to_owned()).or_default();
        held.messages.extend(messages);
        held.whole |= whole;
    }

    /// The messages labels were read on this run, and the tokens of the
    /// labels read whole.
    fn read(&self) -> (Vec<String>, Vec<String>) {
        let labels = self.lock();
        let mut messages: Vec<String> = labels
            .read
            .values()
            .flat_map(|m| m.messages.iter().cloned())
            .collect();
        messages.sort();
        messages.dedup();
        let whole = labels
            .read
            .iter()
            .filter(|(_, m)| m.whole)
            .map(|(label, _)| state::label_token(label))
            .collect();
        (messages, whole)
    }

    /// `tokens` of message `message` with what this run read of its
    /// labels: a label read on it is put on, one read whole without it is
    /// taken off. Mail in the trash or spam keeps the labels it had (a
    /// listing may leave those folders out), and so does a label a push of
    /// ours changed on it since the last pass ended. Sorted.
    fn relabel(&self, message: &str, tokens: &[String]) -> Vec<String> {
        let binned = tokens.iter().any(|t| t == "TRASH" || t == "SPAM");
        let mut out = tokens.to_vec();
        let labels = self.lock();
        for (label, read) in &labels.read {
            let token = state::label_token(label);
            let written = labels
                .written
                .contains(&(message.to_owned(), label.clone()));
            if read.messages.contains(message) {
                out.push(token);
            } else if read.whole && !binned && !written {
                out.retain(|t| *t != token);
            }
        }
        drop(labels);
        out.sort();
        out.dedup();
        out
    }
}

/// Everything a run needs to read one page.
pub struct Reader<'a> {
    /// The mail account, with a valid access token.
    pub mail: ZohoMail<'a>,
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
    /// The adapter's settings.
    pub config: &'a ZohoMailConfig,
    /// The account's label names, when the grant has the labels scope;
    /// `None` reads no labels and no stream.
    pub labels: Option<&'a LabelCache>,
}

/// An indexed message.
#[derive(Debug, Clone)]
struct Indexed {
    row: Uuid,
    labels: Vec<String>,
    document: BlobRef,
}

/// What a run emits, as it is built.
#[derive(Debug, Default)]
struct Out {
    changes: Vec<MessageChange>,
    index: Vec<IndexOp>,
}

impl Reader<'_> {
    /// Read the next page.
    pub async fn read(&self) -> Result<Page, ReadError> {
        if self.labels.is_some() {
            // The labels, then the newest messages of each stream mark
            // (during a walk, as many pages as there are).
            self.refresh_labels().await?;
            let pages = if self.phase == Phase::Incremental {
                1
            } else {
                LABEL_PAGES
            };
            for mark in self.labels.map_or_else(Vec::new, |cache| cache.ids(true)) {
                self.read_label(&mark, pages, false).await?;
            }
        }
        let mut page = self.page().await?;
        self.relabel_known(&mut page).await?;
        let ended = self.phase != Phase::Incremental && page.phase == Phase::Incremental;
        if let Some((held, vanished)) = self.labels.and_then(LabelCache::pass).filter(|_| ended) {
            // What this pass held back, for the next to confirm; nothing
            // held back clears it. Only a pass that ended: a listing read
            // again (over the run's limits) is the same pass.
            page.cursor.held_back = held;
            page.cursor.vanished = vanished;
        }
        Ok(page)
    }

    /// The page of this run's phase.
    async fn page(&self) -> Result<Page, ReadError> {
        match self.phase {
            Phase::Backfill | Phase::Resync => self.walk().await,
            Phase::Incremental => {
                let due = self.cursor.swept_at.is_none_or(|at| {
                    self.now - at >= TimeDelta::minutes(i64::from(self.config.sweep_minutes))
                });
                if due || self.cursor.folders.is_empty() {
                    let next = Cursor {
                        walk: None,
                        window_start: None,
                        generation: self.cursor.generation + 1,
                        ..self.cursor.clone()
                    };
                    return Ok(Page {
                        changes: Vec::new(),
                        index: Vec::new(),
                        cursor: next,
                        phase: Phase::Resync,
                        more: true,
                    });
                }
                self.head().await
            }
        }
    }

    fn workspace_key(&self) -> String {
        self.workspace.to_string()
    }

    /// The account's labels again, into the cache: a failure other than
    /// one that stops the run keeps the names known (logged).
    async fn refresh_labels(&self) -> Result<(), ReadError> {
        let Some(cache) = self.labels else {
            return Ok(());
        };
        cache.called();
        match self.mail.labels().await {
            Ok(listed) => {
                cache.replace(listed);
                Ok(())
            }
            // Refused the grant (it may lack the labels scope): not trouble
            // with the grant itself.
            Err(err) if is_run_trouble(&err) && err != ProviderError::Unauthorized => {
                Err(err.into())
            }
            Err(err) => {
                tracing::info!(stream = %self.stream, error = %err, "Zoho's labels could not be read; known names kept");
                Ok(())
            }
        }
    }

    /// Read which messages carry label `label` (`labelid`), up to `pages`
    /// pages, into the cache: its whole listing when `whole` and it ends
    /// within them. A label Zoho answers 404 for, while it still lists the
    /// label, is left unread (so nothing is taken off). A failure other
    /// than one that stops the run leaves the label unread too (logged).
    async fn read_label(&self, label: &str, pages: u32, whole: bool) -> Result<(), ReadError> {
        let Some(cache) = self.labels else {
            return Ok(());
        };
        let mut messages = HashSet::new();
        let mut ended = false;
        let mut start = 1;
        for _ in 0..pages {
            cache.called();
            match self.mail.labelled(label, start, LABEL_PAGE).await {
                Ok(page) => {
                    let full = u32::try_from(page.len()).unwrap_or(u32::MAX) >= LABEL_PAGE;
                    messages.extend(page.into_iter().map(|m| m.message_id));
                    if !full {
                        ended = true;
                        break;
                    }
                    start += LABEL_PAGE;
                }
                Err(err) if is_run_trouble(&err) && err != ProviderError::Unauthorized => {
                    return Err(err.into());
                }
                Err(err) => {
                    tracing::info!(stream = %self.stream, error = %err, "a Zoho label's messages could not be read");
                    return Ok(());
                }
            }
        }
        let whole = whole && ended && self.plausible(label, &messages).await?;
        cache.record(label, messages, whole);
        Ok(())
    }

    /// Whether a whole listing of `label` (on `messages`) may take it off
    /// the indexed messages it leaves out: not when that would be more than
    /// a fifth of those carrying it, and five or more (a listing that may
    /// miss folders or lag behind), unless the last pass held back the same
    /// messages, or more (a member's bulk change, confirmed a pass late).
    ///
    /// TODO(campbooks-cloud#292): check against a recorded Zoho answer
    /// which folders a `labelid` listing covers (archived, spam, trash,
    /// the member's own), and how soon it shows a change.
    async fn plausible(&self, label: &str, messages: &HashSet<String>) -> Result<bool, ReadError> {
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let carrying: Vec<String> = sqlx::query_scalar(
            "SELECT provider_message_id FROM connectors.messages
              WHERE stream_id = $1 AND $2 = ANY(label_ids)",
        )
        .bind(self.stream)
        .bind(state::label_token(label))
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut losing: Vec<String> = carrying
            .iter()
            .filter(|id| !messages.contains(*id))
            .cloned()
            .collect();
        losing.sort();
        if losing.len() < 5 || losing.len() * 5 <= carrying.len() {
            return Ok(true);
        }
        // The last pass held back the same, or more: confirmed.
        let before = self.cursor.held_back.get(label);
        if before.is_some_and(|held| losing.iter().all(|id| held.contains(id))) {
            return Ok(true);
        }
        tracing::warn!(
            stream = %self.stream,
            label,
            carrying = carrying.len(),
            losing = losing.len(),
            "a Zoho label's listing would take it off too many messages; held back until the next pass sees the same"
        );
        if let Some(cache) = self.labels {
            cache.lock().held.insert(label.to_owned(), losing);
        }
        Ok(false)
    }

    /// Every label's messages, whole (the end of a pass): a label Zoho no
    /// longer lists is on none. Labels a push of ours changed since the
    /// last pass ended stay as written.
    async fn read_all_labels(&self) -> Result<(), ReadError> {
        let Some(cache) = self.labels else {
            return Ok(());
        };
        cache.protect(self.written().await?);
        for label in cache.ids(false) {
            self.read_label(&label, LABEL_PAGES, true).await?;
        }
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let held: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT substr(token, 7) FROM connectors.messages, unnest(label_ids) AS token
              WHERE stream_id = $1 AND token LIKE 'label:%'",
        )
        .bind(self.stream)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let known = held
            .iter()
            .cloned()
            .chain(cache.ids(false))
            .collect::<HashSet<String>>()
            .len();
        let mut gone = cache.deleted(held);
        gone.sort();
        let confirmed = gone.iter().all(|id| self.cursor.vanished.contains(id));
        if gone.len() >= 2 && gone.len() * 5 > known && !confirmed {
            // A listing that lost many labels at once: held back until the
            // next pass sees them gone too.
            tracing::warn!(
                stream = %self.stream,
                labels = ?gone,
                known,
                "Zoho's label listing left out many labels; held back until the next pass sees the same"
            );
            cache.lock().vanished = gone;
        } else {
            for label in gone {
                cache.record(&label, HashSet::new(), true);
            }
        }
        cache.lock().passed = true;
        Ok(())
    }

    /// The (message, label id) pairs our applied changes put on or took off
    /// since the last pass ended (all those still kept when none did).
    async fn written(&self) -> Result<HashSet<(String, String)>, ReadError> {
        let since = self.cursor.swept_at.unwrap_or(DateTime::UNIX_EPOCH);
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<(String, Vec<String>, Vec<String>)> = sqlx::query_as(
            "SELECT provider_message_id, add_labels, remove_labels
               FROM connectors.outbound_changes
              WHERE stream_id = $1 AND state = 'applied' AND done_at >= $2",
        )
        .bind(self.stream)
        .bind(since)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let names = self.labels.map_or_else(HashMap::new, LabelCache::names);
        let ids = MarkIds::from_labels(names.iter().map(|(id, n)| (id.as_str(), n.as_str())));
        let mut written = HashSet::new();
        for (message, add, remove) in rows {
            for token in add.iter().chain(&remove) {
                let label = state::label_of(token).map(ToOwned::to_owned).or_else(|| {
                    marks::of_token(token).and_then(|stream| ids.get(stream).map(ToOwned::to_owned))
                });
                if let Some(label) = label {
                    written.insert((message.clone(), label));
                }
            }
        }
        Ok(written)
    }

    /// Indexed messages `page` does not carry whose labels this run's reads
    /// changed: emitted again from the index (no download), and indexed.
    async fn relabel_known(&self, page: &mut Page) -> Result<(), ReadError> {
        let Some(cache) = self.labels else {
            return Ok(());
        };
        let (messages, whole) = cache.read();
        if messages.is_empty() && whole.is_empty() {
            return Ok(());
        }
        let mut carried: HashSet<String> = HashSet::new();
        let mut rows: HashSet<Uuid> = HashSet::new();
        for change in &page.changes {
            match change {
                MessageChange::Upsert { message } => {
                    carried.insert(message.provider_message_id.clone());
                }
                MessageChange::Remove {
                    provider_message_id,
                } => {
                    carried.insert(provider_message_id.clone());
                }
            }
        }
        for op in &page.index {
            match op {
                IndexOp::Insert { row, .. }
                | IndexOp::Update { row, .. }
                | IndexOp::Remove { row } => {
                    rows.insert(*row);
                }
                IndexOp::Oversized {
                    provider_message_id,
                }
                | IndexOp::Skipped {
                    provider_message_id,
                    ..
                } => {
                    carried.insert(provider_message_id.clone());
                }
            }
        }
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let found: Vec<(Uuid, String, Vec<String>, String, i64)> = sqlx::query_as(
            "SELECT id, provider_message_id, label_ids, document_hash, document_size
               FROM connectors.messages
              WHERE stream_id = $1 AND (provider_message_id = ANY($2) OR label_ids && $3)
              ORDER BY internal_date",
        )
        .bind(self.stream)
        .bind(&messages)
        .bind(&whole)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut changed: Vec<(String, Indexed, Vec<String>)> = Vec::new();
        for (row, id, labels, hash, size) in found {
            if rows.contains(&row) || carried.contains(&id) {
                continue;
            }
            let mut before = labels.clone();
            before.sort();
            let now = cache.relabel(&id, &labels);
            if now != before {
                let known = Indexed {
                    row,
                    labels,
                    document: mail::document_ref(&hash, size)?,
                };
                changed.push((id, known, now));
            }
        }
        let ids: Vec<String> = changed.iter().map(|(id, _, _)| id.clone()).collect();
        let overlay = outbound::overlay(self.pool, self.workspace, self.stream, &ids).await?;
        let mut out = Out::default();
        for (id, known, now) in changed {
            let shown = overlay.get(&id).map_or_else(
                || now.clone(),
                |changes| outbound::apply_labels(&now, &self.resolved(changes)),
            );
            self.reemit(&known, &shown, &mut out).await?;
            // Not listed by this pass: its pass generation stays.
            out.index.push(IndexOp::Update {
                row: known.row,
                label_ids: now,
                generation: 0,
            });
        }
        page.changes.extend(out.changes);
        page.index.extend(out.index);
        Ok(())
    }

    /// The tokens of a message listed as `listed` in a folder of `kind`
    /// (archived there when `archived`); its labels from the listing, or
    /// the index's (`known`) when the listing names none.
    fn tokens_of(
        &self,
        listed: &Listed,
        kind: FolderKind,
        archived: bool,
        known: Option<&[String]>,
    ) -> Vec<String> {
        let tokens = state::tokens(
            &listed.folder_id,
            kind,
            listed.read(),
            listed.flagged(),
            archived,
        );
        let tokens = if archived {
            state::archived(&tokens)
        } else {
            tokens
        };
        match self.labels {
            Some(cache) => cache.relabel(
                &listed.message_id,
                &state::with_labels(tokens, known.unwrap_or_default()),
            ),
            None => tokens,
        }
    }

    /// The contract's state of a message with `tokens`.
    fn flags(&self, tokens: &[String]) -> Flags {
        match self.labels {
            Some(cache) => state::flags_with(tokens, &cache.names()),
            None => state::flags(tokens),
        }
    }

    /// `changes` (pending, the overlay) with their stream marks' tokens as
    /// Zoho's labels.
    fn resolved(&self, changes: &[outbound::LabelChange]) -> Vec<outbound::LabelChange> {
        let names = self.labels.map_or_else(HashMap::new, LabelCache::names);
        let ids = MarkIds::from_labels(names.iter().map(|(id, n)| (id.as_str(), n.as_str())));
        marks::resolve_changes(changes, &ids, state::label_token)
    }

    /// One listing of a walk (backfill or sweep).
    async fn walk(&self) -> Result<Page, ReadError> {
        let mut cursor = self.cursor.clone();
        if cursor.walk.is_none() {
            // A pass starts: the folders as they are now, the window.
            cursor.folders = self.tracked_folders().await?;
            cursor.window_start =
                Some(self.now - TimeDelta::days(i64::from(self.config.backfill_days)));
            cursor.walk = Some(Walk {
                folder: 0,
                start: 1,
            });
        }
        let window = cursor.window_start.unwrap_or(self.now).timestamp_millis();
        let Some(walk) = cursor.walk else {
            return Err(ReadError::Corrupt("a walk without a position".into()));
        };
        let Some(folder) = cursor.folders.get(walk.folder).cloned() else {
            self.read_all_labels().await?;
            return self.finish_pass(cursor, Out::default()).await;
        };
        let listed = match self.mail.list(&folder.id, walk.start, WALK_PAGE).await {
            Ok(listed) => listed,
            // The folder went away since the pass started.
            Err(ProviderError::NotFound) => Vec::new(),
            Err(other) => return Err(other.into()),
        };
        let reached_end = u32::try_from(listed.len()).unwrap_or(u32::MAX) < WALK_PAGE
            || listed
                .iter()
                .any(|m| m.received_time.is_some_and(|t| t < window));
        let in_window: Vec<(Listed, FolderKind)> = listed
            .into_iter()
            .filter(|m| m.received_time.is_none_or(|t| t >= window))
            .map(|m| (m, folder.kind))
            .collect();
        if reached_end && walk.folder + 1 >= cursor.folders.len() {
            // The last listing of the pass: every label's messages first,
            // so it is read with them.
            self.read_all_labels().await?;
        }
        let mut out = Out::default();
        let complete = self
            .process(&in_window, cursor.generation, true, &mut cursor, &mut out)
            .await?;
        if !complete {
            // Over the run's limits: the same listing again next run; what
            // was stored is known by then.
            return Ok(Page {
                changes: out.changes,
                index: out.index,
                cursor: Cursor {
                    newest: cursor.newest,
                    ..self.cursor.clone()
                },
                phase: self.phase,
                more: true,
            });
        }
        cursor.walk = Some(if reached_end {
            Walk {
                folder: walk.folder + 1,
                start: 1,
            }
        } else {
            Walk {
                folder: walk.folder,
                start: walk.start + WALK_PAGE,
            }
        });
        if reached_end && walk.folder + 1 >= cursor.folders.len() {
            return self.finish_pass(cursor, out).await;
        }
        Ok(Page {
            changes: out.changes,
            index: out.index,
            cursor,
            phase: self.phase,
            more: true,
        })
    }

    /// The end of a walk: a sweep resolves what no listing showed; then
    /// incremental reading starts.
    async fn finish_pass(&self, mut cursor: Cursor, mut out: Out) -> Result<Page, ReadError> {
        if self.phase == Phase::Resync {
            let window = cursor.window_start.unwrap_or(self.now);
            self.resolve_unlisted(&cursor, window, &mut out).await?;
        }
        cursor.walk = None;
        cursor.window_start = None;
        cursor.swept_at = Some(self.now);
        Ok(Page {
            changes: out.changes,
            index: out.index,
            cursor,
            phase: Phase::Incremental,
            more: false,
        })
    }

    /// An incremental run: the newest page of every tracked folder.
    async fn head(&self) -> Result<Page, ReadError> {
        let mut cursor = self.cursor.clone();
        let mut listed: Vec<(Listed, FolderKind)> = Vec::new();
        // Per folder: the oldest time its page covers (None: all of it).
        let mut covered: Vec<(TrackedFolder, Option<i64>)> = Vec::new();
        for folder in &self.cursor.folders {
            let page = match self.mail.list(&folder.id, 1, HEAD_PAGE).await {
                Ok(page) => page,
                Err(ProviderError::NotFound) => {
                    // A folder went away: re-read the folders soon.
                    cursor.swept_at = None;
                    continue;
                }
                Err(other) => return Err(other.into()),
            };
            let full = u32::try_from(page.len()).unwrap_or(u32::MAX) >= HEAD_PAGE;
            let from = if full {
                page.iter().filter_map(|m| m.received_time).min()
            } else {
                None
            };
            covered.push((folder.clone(), from));
            listed.extend(page.into_iter().map(|m| (m, folder.kind)));
        }
        // Mail older than the window that was never read stays out, as in
        // the backfill.
        let window =
            (self.now - TimeDelta::days(i64::from(self.config.backfill_days))).timestamp_millis();
        let indexed = self.indexed_ids(&listed).await?;
        let listed: Vec<(Listed, FolderKind)> = listed
            .into_iter()
            .filter(|(m, _)| {
                indexed.contains(&m.message_id) || m.received_time.is_none_or(|t| t >= window)
            })
            .collect();
        let mut out = Out::default();
        let complete = self
            .process(&listed, cursor.generation, false, &mut cursor, &mut out)
            .await?;
        if !complete {
            return Ok(Page {
                changes: out.changes,
                index: out.index,
                cursor: Cursor {
                    newest: cursor.newest,
                    ..self.cursor.clone()
                },
                phase: Phase::Incremental,
                more: true,
            });
        }
        let seen: HashSet<&str> = listed.iter().map(|(m, _)| m.message_id.as_str()).collect();
        let mut vanished: Vec<(String, String, Indexed)> = Vec::new();
        for (folder, from) in &covered {
            for (id, known) in self.unlisted_in(folder, *from).await? {
                if !seen.contains(id.as_str()) {
                    vanished.push((folder.id.clone(), id, known));
                }
            }
        }
        let more = vanished.len() > MAX_LOOKUPS;
        vanished.truncate(MAX_LOOKUPS);
        self.look_up(&cursor, vanished, cursor.generation, &mut out)
            .await?;
        Ok(Page {
            changes: out.changes,
            index: out.index,
            cursor,
            phase: Phase::Incremental,
            more,
        })
    }

    /// The account's folders the adapter reads.
    async fn tracked_folders(&self) -> Result<Vec<TrackedFolder>, ReadError> {
        let mut folders: Vec<TrackedFolder> = self
            .mail
            .folders()
            .await?
            .into_iter()
            .filter_map(|f| {
                FolderKind::of(&f.kind, &f.path).map(|kind| TrackedFolder { id: f.id, kind })
            })
            .collect();
        // The Inbox first: new mail shows soonest.
        folders.sort_by_key(|f| f.kind != FolderKind::Inbox);
        Ok(folders)
    }

    /// Emit what `listed` says: new messages downloaded (within the run's
    /// limits), known ones re-emitted when their state changed, and those a
    /// member's refused or overtaken change flagged. `walking` marks every
    /// listed row with `generation`. Returns whether every new message was
    /// handled.
    async fn process(
        &self,
        listed: &[(Listed, FolderKind)],
        generation: i32,
        walking: bool,
        cursor: &mut Cursor,
        out: &mut Out,
    ) -> Result<bool, ReadError> {
        let ids: Vec<String> = listed.iter().map(|(m, _)| m.message_id.clone()).collect();
        let indexed = self.indexed(&ids).await?;
        let oversized = self.oversized(&ids).await?;
        let reemit = self.reemits().await?;
        let skipped = self.skipped(&ids).await?;
        let overlay = outbound::overlay(self.pool, self.workspace, self.stream, &ids).await?;
        let mut new: Vec<(&Listed, Vec<String>)> = Vec::new();
        for (message, kind) in listed {
            if let Some(time) = message.received_time {
                cursor.newest = Some(cursor.newest.map_or(time, |n| n.max(time)));
            }
            let id = &message.message_id;
            let known = indexed.get(id);
            let now = self.tokens_of(message, *kind, false, known.map(|k| k.labels.as_slice()));
            match known {
                Some(known) => {
                    let mut before = known.labels.clone();
                    before.sort();
                    if before != now || reemit.contains_key(id) {
                        let shown = overlay.get(id).map_or_else(
                            || now.clone(),
                            |changes| outbound::apply_labels(&now, &self.resolved(changes)),
                        );
                        self.reemit(known, &shown, out).await?;
                    }
                    if walking || before != now || reemit.contains_key(id) {
                        out.index.push(IndexOp::Update {
                            row: known.row,
                            label_ids: now,
                            generation,
                        });
                    }
                }
                None if oversized.contains(id) || skipped.contains(id) => {}
                None if message.size.is_some_and(|s| {
                    u64::try_from(s).unwrap_or(0) > self.config.max_message_bytes
                }) =>
                {
                    out.index.push(IndexOp::Oversized {
                        provider_message_id: id.clone(),
                    });
                }
                None => new.push((message, now)),
            }
        }
        // Flagged messages the listing did not show: their stored state.
        for (id, known) in &reemit {
            if !ids.contains(id) {
                self.reemit(known, &known.labels, out).await?;
                out.index.push(IndexOp::Update {
                    row: known.row,
                    label_ids: known.labels.clone(),
                    generation,
                });
            }
        }
        self.fetch_new(&new, generation, out).await
    }

    /// Emit a known message with state `tokens`, from its stored document.
    async fn reemit(
        &self,
        known: &Indexed,
        tokens: &[String],
        out: &mut Out,
    ) -> Result<(), ReadError> {
        let mut message: NormalizedMessage =
            mail::document(self.blobs, &self.workspace_key(), &known.document).await?;
        apply_flags(&mut message, &self.flags(tokens));
        out.changes.push(MessageChange::Upsert {
            message: Box::new(message),
        });
        Ok(())
    }

    /// Download, store and emit `new`, at most [`FETCH_CONCURRENCY`] at a
    /// time, starting none once [`MAX_NEW`] or the byte budget is reached.
    /// Returns whether every one was handled.
    async fn fetch_new(
        &self,
        new: &[(&Listed, Vec<String>)],
        generation: i32,
        out: &mut Out,
    ) -> Result<bool, ReadError> {
        let mut fetched: u64 = 0;
        let mut next = 0;
        let mut in_flight = FuturesOrdered::new();
        let max = self.config.max_message_bytes;
        loop {
            while in_flight.len() < FETCH_CONCURRENCY
                && fetched < self.config.run_budget_bytes
                && next < new.len()
                && next < MAX_NEW
            {
                let (message, tokens) = &new[next];
                next += 1;
                in_flight.push_back(async move {
                    (
                        *message,
                        tokens,
                        self.mail.original(&message.message_id, max).await,
                    )
                });
            }
            let Some((message, tokens, result)) = in_flight.next().await else {
                break;
            };
            let raw = match result {
                Ok(OriginalFetch::Message { raw, fetched_bytes }) => {
                    fetched = fetched.saturating_add(fetched_bytes);
                    raw
                }
                Ok(OriginalFetch::TooLarge) => {
                    tracing::info!(stream = %self.stream, "message over the size limit skipped");
                    out.index.push(IndexOp::Oversized {
                        provider_message_id: message.message_id.clone(),
                    });
                    continue;
                }
                // Deleted between listing and fetching.
                Err(ProviderError::NotFound) => continue,
                Err(err) if is_run_trouble(&err) => return Err(err.into()),
                Err(err) => {
                    self.skip(&message.message_id, &err, out);
                    continue;
                }
            };
            let internal_date = message
                .received_time
                .and_then(DateTime::from_timestamp_millis)
                .unwrap_or(self.now);
            let (stored, op) = mail::store_message(
                self.blobs,
                &self.workspace_key(),
                Fetched {
                    id: message.message_id.clone(),
                    thread_key: None,
                    label_ids: tokens.clone(),
                    internal_date,
                    raw,
                    flags: self.flags(tokens),
                    receiver: mail::Receiver::Zoho,
                },
                generation,
            )
            .await?;
            out.changes.push(MessageChange::Upsert {
                message: Box::new(stored),
            });
            out.index.push(op);
        }
        Ok(next >= new.len())
    }

    /// Look up where `vanished` messages went (folder, id, row): archived in
    /// place, moved, or gone. A message its folder no longer has is looked
    /// for in the account's other folders before it counts as gone. Zoho
    /// failing on one message skips that message (and backs off), not the
    /// run.
    async fn look_up(
        &self,
        cursor: &Cursor,
        vanished: Vec<(String, String, Indexed)>,
        generation: i32,
        out: &mut Out,
    ) -> Result<(), ReadError> {
        let ids: Vec<String> = vanished.iter().map(|(_, id, _)| id.clone()).collect();
        let skipped = self.skipped(&ids).await?;
        let mut elsewhere: Option<Vec<String>> = None;
        for (folder, id, known) in vanished {
            if skipped.contains(&id) {
                continue;
            }
            let found = match self.mail.details(&folder, &id).await {
                Ok(found) => Some(found),
                Err(ProviderError::NotFound) => {
                    if elsewhere.is_none() {
                        elsewhere = Some(self.other_folders().await?);
                    }
                    let folders = elsewhere.as_deref().unwrap_or_default();
                    match self.relocate(folders, &folder, &id).await {
                        Ok(found) => found,
                        Err(err) if is_run_trouble(&err) => return Err(err.into()),
                        Err(err) => {
                            self.skip(&id, &err, out);
                            continue;
                        }
                    }
                }
                Err(err) if is_run_trouble(&err) => return Err(err.into()),
                Err(err) => {
                    self.skip(&id, &err, out);
                    continue;
                }
            };
            let was = Some(known.labels.as_slice());
            let now = match found {
                Some(found) if found.folder_id == folder => {
                    self.tokens_of(&found, cursor.kind_of(&folder), true, was)
                }
                Some(found) => self.tokens_of(&found, cursor.kind_of(&found.folder_id), false, was),
                None => {
                    out.changes.push(MessageChange::Remove {
                        provider_message_id: id,
                    });
                    out.index.push(IndexOp::Remove { row: known.row });
                    continue;
                }
            };
            let mut before = known.labels.clone();
            before.sort();
            if before != now {
                self.reemit(&known, &now, out).await?;
            }
            out.index.push(IndexOp::Update {
                row: known.row,
                label_ids: now,
                generation,
            });
        }
        Ok(())
    }

    /// Every folder of the account a message can have moved to (all but
    /// drafts and templates), the untracked ones too.
    async fn other_folders(&self) -> Result<Vec<String>, ReadError> {
        Ok(self
            .mail
            .folders()
            .await?
            .into_iter()
            .filter(|f| !matches!(f.kind.to_ascii_lowercase().as_str(), "drafts" | "templates"))
            .map(|f| f.id)
            .collect())
    }

    /// Message `id`, which folder `gone_from` no longer has, in another of
    /// `folders`; `None` when none has it (it is gone).
    async fn relocate(
        &self,
        folders: &[String],
        gone_from: &str,
        id: &str,
    ) -> Result<Option<Listed>, ProviderError> {
        for folder in folders.iter().filter(|f| *f != gone_from) {
            match self.mail.details(folder, id).await {
                Ok(found) => return Ok(Some(found)),
                Err(ProviderError::NotFound) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(None)
    }

    /// Zoho failed on message `id` alone: skip it this run and back off.
    fn skip(&self, id: &str, err: &ProviderError, out: &mut Out) {
        tracing::info!(stream = %self.stream, error = %err, "a message Zoho failed on is skipped for now");
        out.index.push(IndexOp::Skipped {
            provider_message_id: id.to_owned(),
            at: self.now,
        });
    }

    /// Which of `ids` are skipped for now (Zoho failed on them).
    async fn skipped(&self, ids: &[String]) -> Result<HashSet<String>, ReadError> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows = sqlx::query_scalar!(
            r#"SELECT provider_message_id FROM connectors.skipped_messages
                WHERE stream_id = $1 AND provider_message_id = ANY($2) AND retry_at > $3"#,
            self.stream as StreamId,
            ids,
            self.now,
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(rows.into_iter().collect())
    }

    /// The end of a sweep: indexed messages of the window no listing of
    /// this pass showed and not known to be archived. The first
    /// [`MAX_LOOKUPS`] are looked up; the rest are taken as archived, or as
    /// gone from the trash and spam (the next sweep corrects a wrong guess).
    async fn resolve_unlisted(
        &self,
        cursor: &Cursor,
        window: DateTime<Utc>,
        out: &mut Out,
    ) -> Result<(), ReadError> {
        let listed_now: HashSet<Uuid> = out
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
        let rows = sqlx::query!(
            r#"SELECT id, provider_message_id, label_ids, document_hash, document_size
                 FROM connectors.messages
                WHERE stream_id = $1 AND internal_date >= $2 AND listed_generation < $3
                  AND NOT ('ARCHIVED' = ANY(label_ids))
                ORDER BY internal_date DESC"#,
            self.stream as StreamId,
            window,
            cursor.generation,
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut look_ups = Vec::new();
        for row in rows.into_iter().filter(|r| !listed_now.contains(&r.id)) {
            let known = Indexed {
                row: row.id,
                document: mail::document_ref(&row.document_hash, row.document_size)?,
                labels: row.label_ids,
            };
            let Some(folder) = state::folder_of(&known.labels).map(ToOwned::to_owned) else {
                continue;
            };
            if look_ups.len() < MAX_LOOKUPS {
                look_ups.push((folder, row.provider_message_id, known));
                continue;
            }
            match cursor.kind_of(&folder) {
                FolderKind::Trash | FolderKind::Spam => {
                    out.changes.push(MessageChange::Remove {
                        provider_message_id: row.provider_message_id,
                    });
                    out.index.push(IndexOp::Remove { row: known.row });
                }
                _ => {
                    let now = state::archived(&known.labels);
                    self.reemit(&known, &now, out).await?;
                    out.index.push(IndexOp::Update {
                        row: known.row,
                        label_ids: now,
                        generation: cursor.generation,
                    });
                }
            }
        }
        self.look_up(cursor, look_ups, cursor.generation, out).await
    }

    /// Indexed messages of `folder` not archived, received at or after
    /// `from` (all of them for `None`).
    async fn unlisted_in(
        &self,
        folder: &TrackedFolder,
        from: Option<i64>,
    ) -> Result<Vec<(String, Indexed)>, ReadError> {
        let from = from
            .and_then(DateTime::from_timestamp_millis)
            .unwrap_or(DateTime::UNIX_EPOCH);
        let token = state::folder_token(&folder.id);
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows = sqlx::query!(
            r#"SELECT id, provider_message_id, label_ids, document_hash, document_size
                 FROM connectors.messages
                WHERE stream_id = $1 AND $2 = ANY(label_ids)
                  AND NOT ('ARCHIVED' = ANY(label_ids)) AND internal_date >= $3"#,
            self.stream as StreamId,
            token,
            from,
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows.into_iter()
            .map(|row| {
                Ok((
                    row.provider_message_id,
                    Indexed {
                        row: row.id,
                        document: mail::document_ref(&row.document_hash, row.document_size)?,
                        labels: row.label_ids,
                    },
                ))
            })
            .collect()
    }

    /// Which of `listed` the index holds.
    async fn indexed_ids(
        &self,
        listed: &[(Listed, FolderKind)],
    ) -> Result<HashSet<String>, ReadError> {
        let ids: Vec<String> = listed.iter().map(|(m, _)| m.message_id.clone()).collect();
        Ok(self.indexed(&ids).await?.into_keys().collect())
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
                Ok((
                    row.provider_message_id,
                    Indexed {
                        row: row.id,
                        document: mail::document_ref(&row.document_hash, row.document_size)?,
                        labels: row.label_ids,
                    },
                ))
            })
            .collect()
    }

    /// Messages flagged to be emitted again (a member's change was refused
    /// or given up), at most [`MAX_NEW`].
    async fn reemits(&self) -> Result<HashMap<String, Indexed>, ReadError> {
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows = sqlx::query!(
            r#"SELECT id, provider_message_id, label_ids, document_hash, document_size
                 FROM connectors.messages
                WHERE stream_id = $1 AND reemit
                ORDER BY internal_date LIMIT $2"#,
            self.stream as StreamId,
            i64::try_from(MAX_NEW).unwrap_or(50),
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows.into_iter()
            .map(|row| {
                Ok((
                    row.provider_message_id,
                    Indexed {
                        row: row.id,
                        document: mail::document_ref(&row.document_hash, row.document_size)?,
                        labels: row.label_ids,
                    },
                ))
            })
            .collect()
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
}
