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
//! read with it from the listing, as `label:<id>` tokens compared like the
//! rest, and the account's label names are read again at the start of each
//! pass and whenever a listing names a label the cache does not know.

use std::{
    collections::{HashMap, HashSet},
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

/// The names of the account's Zoho labels (id → name) as a run knows them,
/// for a grant with the labels scope, and the listing it read fresh, if it
/// did (for the run to store).
#[derive(Debug, Default)]
pub struct LabelCache(Mutex<Labels>);

#[derive(Debug, Default)]
struct Labels {
    names: HashMap<String, String>,
    fresh: Option<Vec<ZohoLabel>>,
}

impl LabelCache {
    /// The cache, holding `names`.
    pub fn new(names: HashMap<String, String>) -> Self {
        Self(Mutex::new(Labels { names, fresh: None }))
    }

    fn lock(&self) -> MutexGuard<'_, Labels> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The names now.
    fn names(&self) -> HashMap<String, String> {
        self.lock().names.clone()
    }

    /// Whether every one of `ids` is known, or the labels were read this
    /// run already.
    fn knows<'i>(&self, mut ids: impl Iterator<Item = &'i String>) -> bool {
        let labels = self.lock();
        labels.fresh.is_some() || ids.all(|id| labels.names.contains_key(id))
    }

    fn replace(&self, listed: Vec<ZohoLabel>) {
        let mut labels = self.lock();
        labels.names = listed
            .iter()
            .map(|l| (l.label_id.clone(), l.name()))
            .collect();
        labels.fresh = Some(listed);
    }

    /// The labels this run read from Zoho, if it did.
    pub fn fresh(&self) -> Option<Vec<ZohoLabel>> {
        self.lock().fresh.take()
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
            Some(_) => state::with_labels(tokens, listed.labels(), known),
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
        let names = self.labels.map(LabelCache::names).unwrap_or_default();
        let ids = MarkIds::from_labels(names.iter().map(|(id, n)| (id.as_str(), n.as_str())));
        marks::resolve_changes(changes, &ids, state::label_token)
    }

    /// One listing of a walk (backfill or sweep).
    async fn walk(&self) -> Result<Page, ReadError> {
        let mut cursor = self.cursor.clone();
        if cursor.walk.is_none() {
            // A pass starts: the labels as they are now too.
            self.refresh_labels().await?;
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
        // A label the names do not know yet: read them again.
        let named = listed
            .iter()
            .flat_map(|(m, _)| m.labels().unwrap_or_default());
        if self.labels.is_some_and(|cache| !cache.knows(named)) {
            self.refresh_labels().await?;
        }
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
