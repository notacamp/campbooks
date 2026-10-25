//! Reading one page of a Microsoft mailbox stream (campbooks-cloud#316).
//!
//! Graph keeps a change log per folder (`messages/delta`), so the adapter
//! reads folders the way Gmail reads history:
//!
//! - **Folders.** At a pass's start and every [`FOLDERS_EVERY`]: the
//!   well-known ones by Graph's names, then the folder tree, at most
//!   `…__MAX_FOLDERS`. A folder that was read and is gone starts a pass.
//! - **Rounds.** Each run reads one page of every folder's delta: a folder
//!   without a position starts a round over the window (`$filter` on
//!   `receivedDateTime`), a `$skiptoken` continues it (the run says more is
//!   waiting), a `$deltatoken` is where the next round starts.
//! - **Messages.** A new one is downloaded as raw MIME (`$value`) and
//!   stored; a known one whose tokens changed is emitted again from its
//!   stored document. One `@removed` from a folder and seen nowhere else
//!   this run is looked up: gone is a removal, elsewhere a move.
//! - **Passes.** The backfill, and a resync when Graph forgets a folder's
//!   delta (410, `syncStateNotFound`) or a folder vanishes: every folder's
//!   round again, then the indexed messages of the window the pass did not
//!   list are removed.
//! - **A member's changes** (campbooks-cloud#317). An emission shows a
//!   change still waiting to be made as the member left it; a message whose
//!   change was refused, overtaken or given up (`reemit`) is emitted again
//!   with Graph's state, at most [`MAX_LOOKUPS`] a run.
//!
//! Nothing here changes anything at Microsoft: the push (`write.rs`) does,
//! before the read.

use std::collections::{HashMap, HashSet};

use campbooks_kernel::{
    blobs::{BlobRef, BlobStore},
    db::BeginFor as _,
    tenancy::WorkspaceId,
};
use chrono::{DateTime, TimeDelta, Utc};
use futures_util::{StreamExt as _, stream::FuturesOrdered};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    Microsoft,
    graph::{self, Delta, DeltaItem, MessageState, Next, Position, RawFetch},
    state::{self, Cursor, Folder},
};
use crate::{
    config::MicrosoftAdapter,
    contract::{MessageChange, NormalizedMessage},
    domain::{
        marks,
        model::Phase,
        model::StreamId,
        outbound::{self, LabelChange},
    },
    imap::folders::Role,
    mail::{self, Fetched, IndexOp, ReadError, Receiver, apply_flags},
    provider::ProviderError,
};

/// A member's changes still waiting, per message (`outbound::overlay`).
type Waiting = HashMap<String, Vec<LabelChange>>;

/// How often the folders are listed again outside a pass.
pub const FOLDERS_EVERY: TimeDelta = TimeDelta::hours(6);

/// Most messages one run looks up.
pub const MAX_LOOKUPS: usize = 20;

/// Most look-ups a cursor keeps waiting: more starts a pass instead.
pub const MAX_PENDING_LOOKUPS: usize = 1_000;

/// Downloads in flight at once (Graph allows a mailbox four concurrent
/// requests).
pub const FETCH_CONCURRENCY: usize = 4;

/// How deep the folder tree is read.
pub const MAX_DEPTH: usize = 10;

/// The most folders one listing walks (the largest `…__MAX_FOLDERS`): a
/// mailbox with more is listed in part ([`Cursor::partial`]).
pub const MAX_LISTED: usize = 1_000;

/// The most listing requests one walk makes (pages and folders' children).
pub const MAX_LIST_REQUESTS: usize = 200;

/// The well-known folders read, by Graph's name, and their role.
const READ: [(&str, Role); 5] = [
    ("inbox", Role::Inbox),
    ("sentitems", Role::Sent),
    ("junkemail", Role::Junk),
    ("deleteditems", Role::Trash),
    ("archive", Role::Archive),
];

/// The well-known folders never read (nor anything under them).
const NEVER_READ: [&str; 4] = ["drafts", "outbox", "conversationhistory", "syncissues"];

/// The requests a run of a stream with `folders` folders is charged before
/// it starts: a delta page per folder, a listing of the folders when due,
/// and a few downloads and look-ups (Graph's own 429 slows a backfill's
/// more).
pub fn requests_per_run(folders: usize, listing: bool) -> u32 {
    let listing = if listing {
        READ.len() + NEVER_READ.len() + 2
    } else {
        0
    };
    u32::try_from(folders.max(1) + listing + 10).unwrap_or(u32::MAX)
}

/// Whether a run with `cursor` lists the folders first.
pub fn lists_folders(cursor: &Cursor, now: DateTime<Utc>) -> bool {
    cursor.folders.is_empty() || cursor.listed_at.is_none_or(|at| now - at >= FOLDERS_EVERY)
}

/// Everything a run needs to read one page.
pub struct Reader<'a> {
    /// Microsoft's client.
    pub microsoft: &'a Microsoft,
    /// A valid access token.
    pub token: &'a str,
    /// The pool (runtime role), for reading the index.
    pub pool: &'a PgPool,
    /// The blob store.
    pub blobs: &'a BlobStore,
    pub workspace: WorkspaceId,
    pub stream: StreamId,
    pub phase: Phase,
    pub cursor: Cursor,
    pub now: DateTime<Utc>,
    /// The adapter's settings.
    pub settings: MicrosoftAdapter,
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

/// An indexed message.
#[derive(Debug, Clone)]
struct Indexed {
    row: Uuid,
    tokens: Vec<String>,
    document: BlobRef,
}

/// What a run has read so far.
#[derive(Debug, Default)]
struct Out {
    changes: Vec<MessageChange>,
    index: Vec<IndexOp>,
    /// Bytes downloaded.
    fetched: u64,
    /// Messages stored this run (never stored twice).
    stored: HashSet<String>,
    /// Messages a page showed in a folder this run.
    present: HashSet<String>,
    /// Messages a page showed leaving a folder this run.
    removed: Vec<String>,
    /// Messages to emit again whatever they show (`reemit`).
    forced: HashSet<String>,
    /// Messages emitted again or removed this run.
    handled: HashSet<String>,
}

impl Reader<'_> {
    fn workspace_key(&self) -> String {
        self.workspace.to_string()
    }

    /// The oldest mail a new folder's round, or the window of a pass,
    /// reads.
    fn floor(&self) -> DateTime<Utc> {
        self.now - TimeDelta::days(i64::from(self.settings.backfill_days))
    }

    /// Read the next page.
    #[allow(
        clippy::too_many_lines,
        reason = "one run read top to bottom: folders, rounds, look-ups, the pass's end"
    )]
    pub async fn read(&self) -> Result<Page, ReadError> {
        let mut cursor = self.cursor.clone();
        let mut phase = self.phase;
        if lists_folders(&cursor, self.now) {
            let Some(listed) = self.list_folders(&cursor).await? else {
                return Err(ProviderError::Unexpected(
                    "Graph named no Inbox for the mailbox".to_owned(),
                )
                .into());
            };
            if merge_folders(&mut cursor, listed) {
                tracing::info!(stream = %self.stream, "a folder is gone; reading the window again");
                Self::restart(&mut cursor, &mut phase);
            }
            cursor.listed_at = Some(self.now);
        }
        if phase != Phase::Incremental {
            cursor.window_start.get_or_insert_with(|| self.floor());
        }
        let window = cursor.window_start.unwrap_or_else(|| self.floor());
        let reemits = self.reemits().await?;
        let mut out = Out {
            forced: reemits.keys().cloned().collect(),
            ..Out::default()
        };
        let mut more = false;
        let mut dropped: Vec<String> = Vec::new();
        for n in 0..cursor.folders.len() {
            if out.fetched >= self.settings.run_budget_bytes {
                more = true;
                break;
            }
            let folder = cursor.folders[n].clone();
            let position = match (&folder.skip, &folder.delta) {
                (Some(skip), _) => Position::Skip(skip),
                (None, Some(delta)) => Position::Delta(delta),
                (None, None) => Position::Start(window),
            };
            let starting = matches!(position, Position::Start(_));
            let page = match self.microsoft.delta(self.token, &folder.id, position).await {
                Ok(Delta::Page(page)) => page,
                // A folder gone since the listing: no longer read, and the
                // folders listed again next run.
                Err(ProviderError::NotFound) if starting => {
                    cursor.listed_at = None;
                    dropped.push(folder.id.clone());
                    continue;
                }
                Ok(Delta::Expired) if starting => {
                    return Err(ProviderError::Unexpected(
                        "Graph refused to start a folder's delta".to_owned(),
                    )
                    .into());
                }
                Ok(Delta::Expired) | Err(ProviderError::NotFound) => {
                    tracing::info!(stream = %self.stream, "a folder's delta expired; reading the window again");
                    Self::restart(&mut cursor, &mut phase);
                    // The folders again too: one of them may be gone.
                    cursor.listed_at = None;
                    return Ok(Page {
                        changes: out.changes,
                        index: out.index,
                        cursor,
                        phase,
                        more: true,
                    });
                }
                Err(other) => return Err(other.into()),
            };
            let listed = page
                .items
                .iter()
                .filter(|item| item.state.is_some())
                .count();
            let complete = self
                .read_items(&cursor, &folder, page.items, &mut out)
                .await?;
            if !complete {
                // Over the byte budget: the same page again next run (what
                // was stored is indexed and only compared then).
                more = true;
                break;
            }
            let at = &mut cursor.folders[n];
            if starting {
                at.listed = Some(0);
            }
            if let Some(count) = at.listed.as_mut() {
                *count = count.saturating_add(u32::try_from(listed).unwrap_or(u32::MAX));
            }
            match page.next {
                Next::More(skip) => {
                    at.skip = Some(skip);
                    more = true;
                }
                Next::Done(delta) => {
                    at.skip = None;
                    at.delta = Some(delta);
                    // A first round that listed as many as Graph lists with
                    // a `$filter` may have left the oldest out.
                    if let Some(count) = at.listed.take() {
                        at.truncated = count >= self.microsoft.filter_limit();
                    }
                }
            }
        }
        if !dropped.is_empty() {
            cursor.folders.retain(|f| !dropped.contains(&f.id));
            cursor.partial = true;
        }
        // What left a folder and showed nowhere else this run.
        let left = std::mem::take(&mut out.removed);
        for id in left {
            if !out.present.contains(&id) && !cursor.lookups.contains(&id) {
                cursor.lookups.push(id);
            }
        }
        if cursor.lookups.len() > MAX_PENDING_LOOKUPS {
            tracing::info!(stream = %self.stream, "too many moves to look up; reading the window again");
            Self::restart(&mut cursor, &mut phase);
            return Ok(Page {
                changes: out.changes,
                index: out.index,
                cursor,
                phase,
                more: true,
            });
        }
        self.look_up(&mut cursor, &mut out).await?;
        if !cursor.lookups.is_empty() {
            more = true;
        }
        if phase == Phase::Incremental {
            self.reemit_rest(&cursor, reemits, &mut out).await?;
        }
        if phase != Phase::Incremental && !more && cursor.folders.iter().all(Folder::round_done) {
            if cursor.generation > 0 {
                self.remove_unlisted(&mut out, &cursor, window).await?;
            }
            cursor.window_start = None;
            phase = Phase::Incremental;
        }
        Ok(Page {
            changes: out.changes,
            index: out.index,
            cursor,
            phase,
            more,
        })
    }

    /// Start a new pass: a resync (the backfill stays the backfill).
    fn restart(cursor: &mut Cursor, phase: &mut Phase) {
        cursor.restart();
        if *phase == Phase::Incremental {
            *phase = Phase::Resync;
        }
    }

    /// The folders to read, Inbox first; `None` when Graph names no Inbox
    /// and none was read before (an answer not believed).
    ///
    /// The well-known folders come by name and are always read, whatever the
    /// folder cap; a well-known folder Graph does not name this time (a
    /// passing 404) keeps the one read before, its role unchanged, and the
    /// listing counts as partial. The member's folders come from the tree,
    /// by name: those read before first, then the others, up to the cap.
    #[allow(
        clippy::too_many_lines,
        reason = "one listing read top to bottom: well-known folders, the tree, the cap"
    )]
    async fn list_folders(&self, before: &Cursor) -> Result<Option<Listing>, ProviderError> {
        let mut complete = true;
        let mut known: Vec<(Folder, u32)> = Vec::new();
        for (name, role) in READ {
            match self.microsoft.well_known(self.token, name).await? {
                Some(found) if !known.iter().any(|(f, _)| f.id == found.id) => {
                    known.push((Folder::new(found.id, found.name, role), found.children));
                }
                Some(_) => {}
                None => {
                    // Gone for now, or the mailbox has none: the one read
                    // before stays what it was.
                    if let Some(old) = before.folders.iter().find(|f| f.role == role) {
                        complete = false;
                        known.push((old.clone(), 0));
                    }
                }
            }
        }
        let Some(inbox) = known
            .iter()
            .find(|(f, _)| f.role == Role::Inbox)
            .map(|(f, _)| f.id.clone())
        else {
            return Ok(None);
        };
        // Never read: those Graph names now, and those it named before.
        let mut skipped: Vec<String> = before.skipped.clone();
        for name in NEVER_READ {
            if let Some(found) = self.microsoft.well_known(self.token, name).await?
                && !skipped.contains(&found.id)
            {
                skipped.push(found.id);
            }
        }
        known.retain(|(f, _)| !skipped.contains(&f.id));
        let mut present: HashSet<String> = known.iter().map(|(f, _)| f.id.clone()).collect();
        let mut others: Vec<Folder> = Vec::new();
        let mut requests = 0_usize;
        // (parent, its shown path, depth), breadth first: the top level,
        // and the well-known folders' own folders (the Inbox's shown
        // without it).
        let mut queue: std::collections::VecDeque<(Option<String>, String, usize)> =
            std::collections::VecDeque::from([(None, String::new(), 0)]);
        for (folder, children) in &known {
            if *children > 0 {
                let under = if folder.id == inbox {
                    String::new()
                } else {
                    folder.display.clone()
                };
                queue.push_back((Some(folder.id.clone()), under, 1));
            }
        }
        'tree: while let Some((parent, path, depth)) = queue.pop_front() {
            let mut skip = 0_u32;
            loop {
                if requests >= MAX_LIST_REQUESTS {
                    complete = false;
                    break 'tree;
                }
                requests += 1;
                let page = self
                    .microsoft
                    .folders(self.token, parent.as_deref(), skip)
                    .await?;
                for listed in page.folders {
                    if skipped.contains(&listed.id) || present.contains(&listed.id) {
                        continue;
                    }
                    if present.len() >= MAX_LISTED {
                        complete = false;
                        break 'tree;
                    }
                    present.insert(listed.id.clone());
                    let display = if path.is_empty() {
                        listed.name.clone()
                    } else {
                        format!("{path}/{}", listed.name)
                    };
                    let role = if marks::of_name(&display).is_some() {
                        Role::Stream
                    } else {
                        Role::User
                    };
                    if listed.children > 0 && depth < MAX_DEPTH {
                        queue.push_back((Some(listed.id.clone()), display.clone(), depth + 1));
                    }
                    others.push(Folder::new(listed.id, display, role));
                }
                if !page.full {
                    break;
                }
                skip = skip.saturating_add(graph::FOLDER_PAGE);
            }
        }
        // The cap: the folders read before keep their place ahead of new
        // ones.
        let max = usize::try_from(self.settings.max_folders).unwrap_or(usize::MAX);
        let room = max.saturating_sub(known.len());
        others.sort_by(|a, b| {
            let is_new = |f: &Folder| before.folder(&f.id).is_none();
            is_new(a)
                .cmp(&is_new(b))
                .then_with(|| a.role.cmp(&b.role))
                .then_with(|| a.display.cmp(&b.display))
        });
        let capped = others.len() > room;
        others.truncate(room);
        let mut folders: Vec<Folder> = known.into_iter().map(|(f, _)| f).collect();
        folders.extend(others);
        folders.sort_by(|a, b| a.role.cmp(&b.role).then_with(|| a.display.cmp(&b.display)));
        Ok(Some(Listing {
            folders,
            skipped,
            present,
            complete,
            capped,
        }))
    }

    /// Read one delta page's `items` of `folder`: emit what changed, store
    /// what is new. Whether every one was handled (no download was left out
    /// for the byte budget).
    async fn read_items(
        &self,
        cursor: &Cursor,
        folder: &Folder,
        items: Vec<DeltaItem>,
        out: &mut Out,
    ) -> Result<bool, ReadError> {
        let ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
        let indexed = self.indexed(&ids).await?;
        let oversized = self.oversized(&ids).await?;
        let known_ids: Vec<String> = indexed.keys().cloned().collect();
        let waiting = outbound::overlay(self.pool, self.workspace, self.stream, &known_ids).await?;
        let mut new = Vec::new();
        for DeltaItem { id, state } in items {
            let Some(state) = state else {
                if indexed.contains_key(&id) {
                    out.removed.push(id);
                }
                continue;
            };
            if state.draft {
                continue;
            }
            out.present.insert(id.clone());
            let place = state.folder.clone().unwrap_or_else(|| folder.id.clone());
            let tokens = state::tokens(cursor, &place, &state);
            match indexed.get(&id) {
                Some(known) => {
                    self.compare(cursor, &id, known, tokens, &waiting, out)
                        .await?;
                }
                None if oversized.contains(&id) || out.stored.contains(&id) => {}
                // Older than the window: never carried, stays out.
                None if state.received.is_some_and(|at| at < self.floor()) => {}
                None => new.push((id, state, tokens)),
            }
        }
        self.fetch_new(cursor, new, out).await
    }

    /// A known message as Graph shows it now (`tokens`): emitted again from
    /// its stored document when that changed (or it is to be emitted again
    /// whatever it shows), a member's changes still `waiting` over it, and
    /// listed by this pass.
    async fn compare(
        &self,
        cursor: &Cursor,
        id: &str,
        known: &Indexed,
        tokens: Vec<String>,
        waiting: &Waiting,
        out: &mut Out,
    ) -> Result<(), ReadError> {
        if tokens != known.tokens || out.forced.contains(id) {
            self.emit(
                cursor,
                known,
                &tokens,
                waiting.get(id).map(Vec::as_slice),
                out,
            )
            .await?;
            out.handled.insert(id.to_owned());
        }
        out.index.push(IndexOp::Update {
            row: known.row,
            label_ids: tokens,
            generation: cursor.generation,
        });
        out.present.insert(id.to_owned());
        Ok(())
    }

    /// Download, store and emit the new messages of `new`, at most
    /// [`FETCH_CONCURRENCY`] at a time, starting no download once the byte
    /// budget is spent. Whether every one was handled.
    async fn fetch_new(
        &self,
        cursor: &Cursor,
        new: Vec<(String, MessageState, Vec<String>)>,
        out: &mut Out,
    ) -> Result<bool, ReadError> {
        let mut next = 0;
        let mut in_flight = FuturesOrdered::new();
        let max = self.settings.max_message_bytes;
        loop {
            while in_flight.len() < FETCH_CONCURRENCY
                && out.fetched < self.settings.run_budget_bytes
                && next < new.len()
            {
                let (id, _, _) = &new[next];
                next += 1;
                let n = next - 1;
                in_flight
                    .push_back(async move { (n, self.microsoft.raw(self.token, id, max).await) });
            }
            let Some((n, result)) = in_flight.next().await else {
                break;
            };
            let (id, state, tokens) = &new[n];
            let raw = match result {
                Ok(Some(RawFetch::Message(raw))) => raw,
                Ok(Some(RawFetch::TooLarge)) => {
                    tracing::info!(stream = %self.stream, "message over the size limit skipped");
                    out.index.push(IndexOp::Oversized {
                        provider_message_id: id.clone(),
                    });
                    continue;
                }
                // Deleted between listing and fetching.
                Ok(None) => continue,
                Err(other) => return Err(other.into()),
            };
            out.fetched = out
                .fetched
                .saturating_add(u64::try_from(raw.len()).unwrap_or(u64::MAX));
            let fetched = Fetched {
                id: id.clone(),
                thread_key: state.conversation.clone(),
                label_ids: tokens.clone(),
                internal_date: state.received.unwrap_or(self.now),
                raw,
                flags: state::flags(tokens, cursor),
                receiver: Receiver::Microsoft,
            };
            let (message, op) = mail::store_message(
                self.blobs,
                &self.workspace_key(),
                fetched,
                cursor.generation,
            )
            .await?;
            out.changes.push(MessageChange::Upsert {
                message: Box::new(message),
            });
            out.index.push(op);
            out.stored.insert(id.clone());
        }
        Ok(next >= new.len())
    }

    /// Emit the known message `known` with state `tokens` from its stored
    /// document, `waiting` (a member's changes still to be made) over it.
    async fn emit(
        &self,
        cursor: &Cursor,
        known: &Indexed,
        tokens: &[String],
        waiting: Option<&[LabelChange]>,
        out: &mut Out,
    ) -> Result<(), ReadError> {
        let mut message = self.document(&known.document).await?;
        let mut flags = state::flags(tokens, cursor);
        if let Some(changes) = waiting {
            crate::imap::state::overlay(&mut flags, changes);
        }
        apply_flags(&mut message, &flags);
        out.changes.push(MessageChange::Upsert {
            message: Box::new(message),
        });
        Ok(())
    }

    /// Emit again, from the index, the messages flagged for it (`reemits`)
    /// that this run neither emitted nor removed.
    async fn reemit_rest(
        &self,
        cursor: &Cursor,
        reemits: HashMap<String, Indexed>,
        out: &mut Out,
    ) -> Result<(), ReadError> {
        let rest: Vec<(String, Indexed)> = reemits
            .into_iter()
            .filter(|(id, _)| !out.handled.contains(id))
            .collect();
        if rest.is_empty() {
            return Ok(());
        }
        let ids: Vec<String> = rest.iter().map(|(id, _)| id.clone()).collect();
        let waiting = outbound::overlay(self.pool, self.workspace, self.stream, &ids).await?;
        for (id, known) in rest {
            self.emit(
                cursor,
                &known,
                &known.tokens,
                waiting.get(&id).map(Vec::as_slice),
                out,
            )
            .await?;
            out.index.push(IndexOp::Update {
                row: known.row,
                label_ids: known.tokens.clone(),
                generation: cursor.generation,
            });
            out.handled.insert(id);
        }
        Ok(())
    }

    /// Messages flagged to be emitted again (a member's change was refused,
    /// overtaken, not movable or given up), at most [`MAX_LOOKUPS`].
    async fn reemits(&self) -> Result<HashMap<String, Indexed>, ReadError> {
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<(Uuid, String, Vec<String>, String, i64)> = sqlx::query_as(
            "SELECT id, provider_message_id, label_ids, document_hash, document_size
               FROM connectors.messages
              WHERE stream_id = $1 AND reemit
              ORDER BY internal_date LIMIT $2",
        )
        .bind(self.stream)
        .bind(i64::try_from(MAX_LOOKUPS).unwrap_or(20))
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows.into_iter()
            .map(|(row, id, mut tokens, hash, size)| {
                tokens.sort();
                Ok((
                    id,
                    Indexed {
                        row,
                        tokens,
                        document: mail::document_ref(&hash, size)?,
                    },
                ))
            })
            .collect()
    }

    /// Look up at most [`MAX_LOOKUPS`] of the messages that left a folder:
    /// gone is a removal; elsewhere is where it now is.
    async fn look_up(&self, cursor: &mut Cursor, out: &mut Out) -> Result<(), ReadError> {
        let take = cursor.lookups.len().min(MAX_LOOKUPS);
        let ids: Vec<String> = cursor.lookups.drain(..take).collect();
        let indexed = self.indexed(&ids).await?;
        let known_ids: Vec<String> = indexed.keys().cloned().collect();
        let waiting = outbound::overlay(self.pool, self.workspace, self.stream, &known_ids).await?;
        for id in ids {
            let Some(known) = indexed.get(&id) else {
                continue;
            };
            match self.microsoft.message(self.token, &id).await {
                Ok(Some(state)) if !state.draft => {
                    let place = state.folder.clone().unwrap_or_default();
                    if !place.is_empty() && !cursor.knows(&place) {
                        // A folder made since the last listing: list again.
                        cursor.listed_at = None;
                    }
                    let tokens = state::tokens(cursor, &place, &state);
                    self.compare(cursor, &id, known, tokens, &waiting, out)
                        .await?;
                }
                // Gone, or a draft again: out of Campbooks.
                Ok(_) => {
                    out.changes.push(MessageChange::Remove {
                        provider_message_id: id.clone(),
                    });
                    out.index.push(IndexOp::Remove { row: known.row });
                    out.handled.insert(id.clone());
                }
                Err(other) => return Err(other.into()),
            }
        }
        Ok(())
    }

    /// A stored message document.
    async fn document(&self, blob: &BlobRef) -> Result<NormalizedMessage, ReadError> {
        mail::document(self.blobs, &self.workspace_key(), blob).await
    }

    /// Which of `ids` were recorded as oversized before.
    async fn oversized(&self, ids: &[String]) -> Result<HashSet<String>, ReadError> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT provider_message_id FROM connectors.oversized
              WHERE stream_id = $1 AND provider_message_id = ANY($2)",
        )
        .bind(self.stream)
        .bind(ids)
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
        let rows: Vec<(Uuid, String, Vec<String>, String, i64)> = sqlx::query_as(
            "SELECT id, provider_message_id, label_ids, document_hash, document_size
               FROM connectors.messages
              WHERE stream_id = $1 AND provider_message_id = ANY($2)",
        )
        .bind(self.stream)
        .bind(ids)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows.into_iter()
            .map(|(row, id, mut tokens, hash, size)| {
                tokens.sort();
                Ok((
                    id,
                    Indexed {
                        row,
                        tokens,
                        document: mail::document_ref(&hash, size)?,
                    },
                ))
            })
            .collect()
    }

    /// The indexed messages of the window that this pass did not list:
    /// removed, except where the pass cannot tell: a message in a folder
    /// whose first round may have been cut short at Graph's `$filter` limit
    /// ([`Folder::truncated`]), or in a folder not read while the listing
    /// left some out ([`Cursor::partial`]). Those stay until a change or a
    /// complete pass says otherwise.
    async fn remove_unlisted(
        &self,
        out: &mut Out,
        cursor: &Cursor,
        window_start: DateTime<Utc>,
    ) -> Result<(), ReadError> {
        let listed_now: Vec<Uuid> = out
            .index
            .iter()
            .filter_map(|op| match op {
                IndexOp::Insert { row, .. } | IndexOp::Update { row, .. } => Some(*row),
                IndexOp::Remove { .. } | IndexOp::Oversized { .. } | IndexOp::Skipped { .. } => {
                    None
                }
            })
            .collect();
        let removed: HashSet<Uuid> = out
            .index
            .iter()
            .filter_map(|op| match op {
                IndexOp::Remove { row } => Some(*row),
                _ => None,
            })
            .collect();
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let gone: Vec<(Uuid, String, Vec<String>)> = sqlx::query_as(
            "SELECT id, provider_message_id, label_ids FROM connectors.messages
              WHERE stream_id = $1 AND internal_date >= $2 AND listed_generation < $3
                AND NOT (id = ANY($4))
              ORDER BY internal_date",
        )
        .bind(self.stream)
        .bind(window_start)
        .bind(cursor.generation)
        .bind(&listed_now)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        for (row, id, tokens) in gone {
            let certain = match state::folder_of(&tokens).and_then(|f| cursor.folder(f)) {
                Some(folder) => !folder.truncated,
                None => !cursor.partial,
            };
            if removed.contains(&row) || !certain {
                continue;
            }
            out.changes.push(MessageChange::Remove {
                provider_message_id: id,
            });
            out.index.push(IndexOp::Remove { row });
        }
        Ok(())
    }
}

/// The folders a listing found, and those never read.
#[derive(Debug)]
struct Listing {
    /// The folders to read.
    folders: Vec<Folder>,
    /// The folders never read.
    skipped: Vec<String>,
    /// Every folder the listing saw, read or not.
    present: HashSet<String>,
    /// The listing saw every folder: one it did not see is gone.
    complete: bool,
    /// The folder cap left some out.
    capped: bool,
}

/// Put `listed` in `cursor`, keeping each folder's position where its id
/// and role are the same. Whether a folder that was read is gone, or its
/// role changed: then a new pass is needed. A folder the listing did not
/// see is gone only when the listing was complete; otherwise it is kept,
/// as it was. One the cap left out is not gone either: it is no longer
/// read.
fn merge_folders(cursor: &mut Cursor, listed: Listing) -> bool {
    let before = std::mem::take(&mut cursor.folders);
    let mut changed = false;
    let mut folders = listed.folders;
    for old in &before {
        let read = old.skip.is_some() || old.delta.is_some();
        if let Some(now) = folders.iter_mut().find(|f| f.id == old.id) {
            if now.role == old.role {
                now.skip.clone_from(&old.skip);
                now.delta.clone_from(&old.delta);
                now.listed = old.listed;
                now.truncated = old.truncated;
            } else if read {
                changed = true;
            }
        } else if !listed.present.contains(&old.id) {
            if listed.complete {
                changed |= read;
            } else {
                folders.push(old.clone());
            }
        }
    }
    folders.sort_by(|a, b| a.role.cmp(&b.role).then_with(|| a.display.cmp(&b.display)));
    cursor.folders = folders;
    cursor.skipped = listed.skipped;
    cursor.partial = !listed.complete || listed.capped;
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(id: &str, role: Role, delta: Option<&str>) -> Folder {
        let mut folder = Folder::new(id.into(), id.into(), role);
        folder.delta = delta.map(ToOwned::to_owned);
        folder
    }

    fn listing(folders: Vec<Folder>, complete: bool, capped: bool) -> Listing {
        Listing {
            present: folders.iter().map(|f| f.id.clone()).collect(),
            folders,
            skipped: vec!["dr".into()],
            complete,
            capped,
        }
    }

    #[test]
    fn a_listing_keeps_positions_and_notices_a_vanished_folder() {
        let mut cursor = Cursor {
            folders: vec![
                folder("in", Role::Inbox, Some("d1")),
                folder("cl", Role::User, Some("d2")),
            ],
            ..Cursor::default()
        };
        let all = listing(
            vec![
                folder("in", Role::Inbox, None),
                folder("cl", Role::User, None),
                folder("new", Role::User, None),
            ],
            true,
            false,
        );
        assert!(!merge_folders(&mut cursor, all));
        let delta = |cursor: &Cursor, id: &str| cursor.folder(id).unwrap().delta.clone();
        assert_eq!(delta(&cursor, "in").as_deref(), Some("d1"));
        assert_eq!(delta(&cursor, "cl").as_deref(), Some("d2"));
        assert_eq!(delta(&cursor, "new"), None);
        assert_eq!(cursor.skipped, ["dr"]);
        assert!(!cursor.partial);
        // "Clients" is gone from a complete listing: a new pass.
        let gone = listing(vec![folder("in", Role::Inbox, None)], true, false);
        assert!(merge_folders(&mut cursor, gone));
        assert!(cursor.folder("cl").is_none());
    }

    #[test]
    fn a_listing_left_short_never_takes_a_folder_for_gone() {
        let read = || Cursor {
            folders: vec![
                folder("in", Role::Inbox, Some("d1")),
                folder("cl", Role::User, Some("d2")),
            ],
            ..Cursor::default()
        };
        // Cut short (a walk past its bounds, a well-known folder not named):
        // "Clients" is kept as it was.
        let mut cursor = read();
        let short = listing(vec![folder("in", Role::Inbox, None)], false, false);
        assert!(!merge_folders(&mut cursor, short));
        assert_eq!(cursor.folder("cl").unwrap().delta.as_deref(), Some("d2"));
        assert!(cursor.partial);
        // Seen but left out by the cap: no longer read, not gone.
        let mut cursor = read();
        let mut capped = listing(vec![folder("in", Role::Inbox, None)], true, true);
        capped.present.insert("cl".into());
        assert!(!merge_folders(&mut cursor, capped));
        assert!(cursor.folder("cl").is_none());
        assert!(cursor.partial);
        // Its role changed (renamed into a stream folder): a new pass.
        let mut cursor = read();
        let renamed = listing(
            vec![
                folder("in", Role::Inbox, None),
                folder("cl", Role::Stream, None),
            ],
            true,
            false,
        );
        assert!(merge_folders(&mut cursor, renamed));
    }

    #[test]
    fn a_run_is_charged_for_its_folders() {
        assert_eq!(requests_per_run(0, true), 22);
        assert_eq!(requests_per_run(5, false), 15);
        let now = Utc::now();
        assert!(lists_folders(&Cursor::default(), now));
        let listed = Cursor {
            folders: vec![folder("in", Role::Inbox, None)],
            listed_at: Some(now - TimeDelta::hours(1)),
            ..Cursor::default()
        };
        assert!(!lists_folders(&listed, now));
        assert!(lists_folders(&listed, now + FOLDERS_EVERY));
    }
}
