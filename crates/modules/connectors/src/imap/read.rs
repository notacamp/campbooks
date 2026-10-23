//! Reading one page of an IMAP mailbox stream, over one signed-in session.
//!
//! IMAP has no change log, so a run asks each folder it reads for its
//! `STATUS` and opens (read-only, `EXAMINE`) only a folder that moved or
//! still has its window to read:
//!
//! - **New mail**: the UIDs from the folder's `uidnext` up, oldest first.
//! - **Flags** (CONDSTORE): what changed since its `HIGHESTMODSEQ`, and with
//!   QRESYNC what vanished. Without CONDSTORE, flags wait for the sweep.
//! - **Deletions** without QRESYNC: when the message count does not add up,
//!   the folder's known UIDs are checked.
//! - **The window** (a new mailbox or folder, or a UIDVALIDITY change): the
//!   UIDs since the window's start, newest first, below where the walk got.
//! - **The sweep**, every `sweep_minutes`: every folder's known UIDs and
//!   flags, read again.
//!
//! A UID is identified by its headers first (`Message-ID`, or a digest of
//! its headers when it has none; the server's EMAILID tells apart two
//! messages that share one): a message already known gets a new location
//! and is not downloaded; a new one is downloaded with `BODY.PEEK[]` (never
//! marking it read), within the run's byte and message budget. A message
//! left with no location once every folder is read is looked for by its
//! `Message-ID` in every folder, then removed.
//!
//! Nothing the server says is logged: only counts and stable codes.

use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    time::Duration,
};

use campbooks_kernel::{
    blobs::{BlobRef, BlobStore},
    db::BeginFor as _,
    tenancy::WorkspaceId,
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgPool;
use tokio::time::Instant;
use uuid::Uuid;

use super::{
    folders::{self, Listed, Role},
    session::{Done, Limits, Session, WireError, quoted},
    state::{self, Cursor, Folder, Location, LocationOp, Place},
    syntax::{Fetch, Untagged},
};
use crate::{
    config::ImapConfig,
    contract::{MessageChange, NormalizedMessage},
    domain::model::{Phase, StreamId},
    mail::{self, Fetched, IndexOp, ReadError, Receiver},
};

/// Most messages one run downloads.
pub const MAX_DOWNLOADS: usize = 50;

/// UIDs whose headers one command reads.
pub const HEADER_CHUNK: usize = 100;

/// Most UIDs whose headers one run reads.
pub const MAX_HEADERS: usize = 2_000;

/// Most messages left with no location one run looks for.
pub const MAX_LOOKUPS: usize = 50;

/// `Message-ID`s one search looks for.
const LOOKUP_BATCH: usize = 10;

/// Most UIDs one look-up search may answer with that are read.
const LOOKUP_HITS: usize = 200;

/// How often the folders are listed again.
pub const LIST_EVERY: TimeDelta = TimeDelta::minutes(30);

/// How long a folder that would not open is set aside before it is tried
/// again.
pub const SKIP_FOR: TimeDelta = TimeDelta::hours(1);

/// Downloads of one message that timed out (in as many runs) before it is
/// recorded as oversized and never asked for again.
pub const MAX_TIMEOUTS: i32 = 3;

/// Most UIDs one command's searches are read into.
const MAX_UIDS: usize = 1_000_000;

/// Most vanished ranges one command's answers are read into.
const MAX_VANISHED: usize = 1_000_000;

/// A run starts nothing new after this long.
pub const SOFT_DEADLINE: Duration = Duration::from_mins(2);

/// The header fields a message is identified by.
const HEADER_FIELDS: &str = "BODY.PEEK[HEADER.FIELDS (MESSAGE-ID DATE FROM TO SUBJECT)]";

/// Longest header field value kept, in characters.
const MAX_FIELD: usize = 4_096;

/// Indexed messages with no location one run reads at most.
const MAX_UNLOCATED: i64 = 1_000;

/// What stopped a run.
#[derive(Debug)]
enum Stop {
    /// The session.
    Wire(WireError),
    /// The server said it is busy.
    Busy,
    /// The server refused a command it should have taken.
    Refused,
    /// The database or the blob store.
    Read(ReadError),
}

impl Stop {
    const fn code(&self) -> &'static str {
        match self {
            Self::Wire(err) => err.code(),
            Self::Busy => "imap_busy",
            Self::Refused => "imap_command_refused",
            Self::Read(_) => "imap_store_failed",
        }
    }
}

impl From<WireError> for Stop {
    fn from(err: WireError) -> Self {
        Self::Wire(err)
    }
}

impl From<ReadError> for Stop {
    fn from(err: ReadError) -> Self {
        Self::Read(err)
    }
}

impl From<sqlx::Error> for Stop {
    fn from(err: sqlx::Error) -> Self {
        Self::Read(ReadError::Database(err))
    }
}

/// Why a run read nothing.
#[derive(Debug, thiserror::Error)]
pub enum Failure {
    /// The server failed before anything was read: a stable code.
    #[error("{0}")]
    Server(&'static str),
    /// The server is busy.
    #[error("the IMAP server is busy")]
    Busy,
    /// The database or the blob store failed.
    #[error(transparent)]
    Read(#[from] ReadError),
}

/// What one run read.
#[derive(Debug)]
pub struct Page {
    /// The changes for the batch, in order.
    pub changes: Vec<MessageChange>,
    /// Updates to the index.
    pub index: Vec<IndexOp>,
    /// Updates to the locations, applied with the index.
    pub locations: Vec<LocationOp>,
    /// The cursor after this page.
    pub cursor: Cursor,
    /// The phase after this page.
    pub phase: Phase,
    /// Whether more is waiting (run again at once).
    pub more: bool,
    /// What cut the run short after it had read something (a stable code).
    pub trouble: Option<&'static str>,
}

/// Everything a run needs to read one page.
pub struct Reader<'a> {
    /// The signed-in session.
    pub session: &'a mut Session,
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
    pub config: &'a ImapConfig,
    /// The mailbox's own address, when known.
    pub address: Option<String>,
}

/// What the server offers.
#[derive(Debug, Clone, Copy, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each is one capability the server may offer"
)]
struct Caps {
    condstore: bool,
    qresync: bool,
    objectid: bool,
    special_use: bool,
    list_extended: bool,
}

impl Caps {
    fn of(capabilities: &[String]) -> Self {
        let has = |name: &str| capabilities.iter().any(|c| c == name);
        Self {
            condstore: has("CONDSTORE") || has("QRESYNC"),
            qresync: has("QRESYNC"),
            objectid: has("OBJECTID"),
            special_use: has("SPECIAL-USE"),
            list_extended: has("LIST-EXTENDED"),
        }
    }
}

/// An indexed message.
#[derive(Debug, Clone)]
struct Known {
    row: Uuid,
    labels: Vec<String>,
    document: BlobRef,
    rfc_message_id: Option<String>,
    internal_date: DateTime<Utc>,
}

/// An index row as [`Known`] is read from it: provider message id, row,
/// labels, document hash and size, `Message-ID`, internal date.
type KnownRow = (
    String,
    Uuid,
    Vec<String>,
    String,
    i64,
    Option<String>,
    DateTime<Utc>,
);

/// A location row: key, folder, UIDVALIDITY, UID, EMAILID, fingerprint.
type CopyRow = (String, String, i64, i64, Option<String>, Option<String>);

/// What a key's copies are known by.
#[derive(Debug, Default)]
struct Marks {
    email_ids: HashSet<String>,
    fingerprints: HashSet<String>,
}

impl Marks {
    fn add(&mut self, email_id: Option<String>, fingerprint: Option<String>) {
        self.email_ids.extend(email_id);
        self.fingerprints.extend(fingerprint);
    }
}

/// A message this run downloaded: where its change and index row are.
#[derive(Debug, Clone, Copy)]
struct Inserted {
    change: usize,
    op: usize,
    from_self: bool,
}

/// What a UID's headers say.
#[derive(Debug, Clone)]
struct Header {
    seen: bool,
    flagged: bool,
    size: Option<u64>,
    internal_date: Option<DateTime<Utc>>,
    /// Its normalised `Message-ID`.
    message_id: Option<String>,
    /// What identifies it without one.
    fields: String,
    email_id: Option<String>,
    /// A digest of its `Date`, `From` and size ([`Location::fingerprint`]).
    fingerprint: String,
    from_self: bool,
}

impl Header {
    fn of(fetch: &Fetch<'_>, address: Option<&str>) -> Self {
        let fields = header_fields(fetch.header.as_deref().unwrap_or_default());
        let field = |name: &str| fields.get(name).map_or("", String::as_str);
        let (seen, flagged) = fetch.flags.unwrap_or((false, false));
        let from_self = address.is_some_and(|own| {
            let from = field("from");
            let from = from
                .rfind('<')
                .and_then(|at| from[at + 1..].split('>').next())
                .unwrap_or(from)
                .trim();
            !from.is_empty() && from.eq_ignore_ascii_case(own)
        });
        Self {
            seen,
            flagged,
            size: fetch.size,
            internal_date: fetch.internal_date,
            message_id: message_id(field("message-id")),
            fields: format!(
                "{}\n{}\n{}\n{}\n{}",
                field("date"),
                field("from"),
                field("to"),
                field("subject"),
                fetch.size.unwrap_or_default()
            ),
            email_id: fetch.email_id.clone(),
            fingerprint: hex(&format!(
                "{}\n{}\n{}",
                field("date"),
                field("from"),
                fetch.size.unwrap_or_default()
            ))[..32]
                .to_owned(),
            from_self,
        }
    }

    /// Its key before the EMAILID tie-break.
    fn key(&self) -> String {
        match &self.message_id {
            Some(id) => format!("mid:{}", hex(id)),
            None => format!("digest:{}", hex(&self.fields)),
        }
    }
}

/// What reading a UID does with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// New mail: an unknown message older than the window stays out.
    New,
    /// The window's walk.
    Walk,
    /// A look-up: only known messages are located, nothing is downloaded.
    Lookup,
}

/// A folder's state as `STATUS` or `EXAMINE` gave it.
#[derive(Debug, Clone, Copy, Default)]
struct Opened {
    uidvalidity: Option<u32>,
    uidnext: Option<u32>,
    messages: Option<u32>,
    highestmodseq: Option<u64>,
    nomodseq: bool,
}

/// What a run read so far, and its locations over the stored ones.
#[derive(Debug, Default)]
struct Run {
    cursor: Cursor,
    caps: Caps,
    deadline: Option<Instant>,
    received_at_start: u64,
    changes: Vec<MessageChange>,
    index: Vec<IndexOp>,
    locations: Vec<LocationOp>,
    /// Copies put this run.
    put: HashMap<Place, Location>,
    /// Stored copies dropped this run.
    dropped: HashSet<Place>,
    /// Folders whose stored copies were all dropped this run.
    dropped_folders: HashSet<String>,
    /// Messages whose locations changed.
    touched: HashSet<String>,
    /// Index rows looked up (`None`: not indexed).
    known: HashMap<String, Option<Known>>,
    inserted: HashMap<String, Inserted>,
    oversized: HashSet<String>,
    /// Folders `STATUS` or `EXAMINE` refused: not read this run.
    refused: HashSet<String>,
    downloads: usize,
    headers: usize,
    swept: bool,
}

impl Run {
    fn out_of_time(&self) -> bool {
        self.deadline.is_some_and(|at| Instant::now() >= at)
    }

    fn is_known(&self, key: &str) -> bool {
        self.inserted.contains_key(key) || self.known.get(key).is_some_and(Option::is_some)
    }

    /// Whether a stored copy at `place` still counts.
    fn stored(&self, place: &Place) -> bool {
        !self.dropped_folders.contains(&place.folder)
            && !self.dropped.contains(place)
            && !self.put.contains_key(place)
    }

    fn put(&mut self, place: Place, location: Location) {
        if self.put.get(&place) == Some(&location) {
            return;
        }
        self.dropped.remove(&place);
        self.touched.insert(location.key.clone());
        self.locations
            .push(LocationOp::Put(place.clone(), location.clone()));
        if let Some(before) = self.put.insert(place, location) {
            self.touched.insert(before.key);
        }
    }

    fn drop_copy(&mut self, place: Place, key: &str) {
        self.touched.insert(key.to_owned());
        if let Some(before) = self.put.remove(&place) {
            self.touched.insert(before.key);
        }
        self.locations.push(LocationOp::Drop(place.clone()));
        self.dropped.insert(place);
    }

    fn mark_oversized(&mut self, key: &str) {
        if self.oversized.insert(key.to_owned()) {
            tracing::info!("a message over the size limit is skipped");
            self.index.push(IndexOp::Oversized {
                provider_message_id: key.to_owned(),
            });
        }
    }

    fn progressed(&self, before: &Cursor) -> bool {
        !self.changes.is_empty()
            || !self.index.is_empty()
            || !self.locations.is_empty()
            || self.cursor.folders != before.folders
    }
}

impl Reader<'_> {
    /// Read the next page.
    pub async fn read(mut self) -> Result<Page, Failure> {
        let before = self.cursor.clone();
        let mut run = Run {
            cursor: self.cursor.clone(),
            deadline: Some(Instant::now() + SOFT_DEADLINE),
            received_at_start: self.session.received(),
            ..Run::default()
        };
        run.cursor.login_refusals = 0;
        let (complete, mut trouble) = match self.walk(&mut run).await {
            Ok(complete) => (complete, None),
            Err(Stop::Read(err)) => return Err(Failure::Read(err)),
            Err(Stop::Busy) if !run.progressed(&before) => return Err(Failure::Busy),
            Err(stop) if !run.progressed(&before) => return Err(Failure::Server(stop.code())),
            Err(stop) => (false, Some(stop.code())),
        };
        match self.settle(&mut run, complete && trouble.is_none()).await {
            Ok(()) => {}
            Err(Stop::Read(err)) => return Err(Failure::Read(err)),
            Err(stop) => {
                trouble.get_or_insert(stop.code());
            }
        }
        let backfilled = run.cursor.backfilled();
        let phase = if backfilled || self.phase != Phase::Backfill {
            Phase::Incremental
        } else {
            Phase::Backfill
        };
        if backfilled && run.cursor.swept_at.is_none() {
            // The first backfill ended: the first sweep is a sweep away.
            run.cursor.swept_at = Some(self.now);
        }
        if let Some(code) = trouble {
            tracing::warn!(stream = %self.stream, code, "an IMAP run was cut short; what it read is kept");
        }
        Ok(Page {
            // Only a run that read something runs again at once: one that
            // could not (a folder that will not open, the time) waits.
            more: trouble.is_none() && (!complete || !backfilled) && run.progressed(&before),
            changes: run.changes,
            index: run.index,
            locations: run.locations,
            cursor: run.cursor,
            phase,
            trouble,
        })
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(u64::from(self.config.timeout_seconds))
    }

    /// Bounds of a command with a short answer.
    fn small(&self) -> Limits {
        Limits {
            response: 64 * 1024,
            responses: 1_000,
            total: 1024 * 1024,
            time: self.timeout(),
        }
    }

    /// The start of the window.
    fn window(&self) -> DateTime<Utc> {
        self.now - TimeDelta::days(i64::from(self.config.backfill_days))
    }

    fn max_message(&self) -> u64 {
        self.config.max_message_bytes
    }

    /// Whether the run may start no other download: it has made
    /// [`MAX_DOWNLOADS`], or received its byte budget (after one at least,
    /// so every run moves on).
    fn budget_spent(&self, run: &Run) -> bool {
        run.downloads >= MAX_DOWNLOADS
            || (run.downloads > 0
                && self
                    .session
                    .received()
                    .saturating_sub(run.received_at_start)
                    >= self.config.run_budget_bytes)
    }

    /// Read every folder that moved, then sweep when one is due. Whether
    /// everything was read.
    async fn walk(&mut self, run: &mut Run) -> Result<bool, Stop> {
        let limits = self.small();
        let capabilities = self.session.capabilities(limits).await?;
        run.caps = Caps::of(&capabilities);
        if run.caps.qresync {
            let enabled = self
                .session
                .command("ENABLE QRESYNC", limits, |_| {})
                .await?;
            run.caps.qresync = enabled.is_ok();
        }
        let stale = run.cursor.folders.is_empty()
            || run
                .cursor
                .listed_at
                .is_none_or(|at| self.now - at >= LIST_EVERY);
        if stale {
            self.list(run).await?;
        }
        if run.cursor.folders.is_empty() {
            return Err(Stop::Refused);
        }
        let mut complete = true;
        for i in 0..run.cursor.folders.len() {
            if run.out_of_time() || !self.folder(run, i).await? {
                complete = false;
                break;
            }
        }
        // The first sweep is due a sweep after the backfill ended.
        let due = run.cursor.swept_at.is_some_and(|at| {
            self.now - at >= TimeDelta::minutes(i64::from(self.config.sweep_minutes))
        });
        if complete && run.cursor.backfilled() && due {
            if self.sweep(run).await? {
                run.cursor.swept_at = Some(self.now);
                run.cursor.generation = run.cursor.generation.wrapping_add(1);
                run.swept = true;
            } else {
                complete = false;
            }
        }
        if !run.refused.is_empty() {
            // A folder that would not open: listed again next run, which
            // is what drops it if it is gone.
            run.cursor.listed_at = None;
        }
        Ok(complete)
    }

    /// List the folders and keep those to read, with what was read of
    /// those already known. A folder no longer listed is forgotten.
    async fn list(&mut self, run: &mut Run) -> Result<(), Stop> {
        let command = if run.caps.special_use && run.caps.list_extended {
            r#"LIST "" "*" RETURN (SPECIAL-USE)"#
        } else {
            r#"LIST "" "*""#
        };
        let limits = Limits {
            response: 16 * 1024,
            responses: 20_000,
            total: 8 * 1024 * 1024,
            time: self.timeout() * 2,
        };
        let mut listed = Vec::new();
        let done = self
            .session
            .command(command, limits, |untagged| {
                if let Untagged::List(entry) = untagged
                    && let Ok(name) = String::from_utf8(entry.name.into_owned())
                {
                    listed.push(Listed {
                        attributes: entry.attributes,
                        delimiter: entry.delimiter,
                        name,
                    });
                }
            })
            .await?;
        check(&done)?;
        let max = usize::try_from(self.config.max_folders).unwrap_or(usize::MAX);
        let Some(tracked) = folders::tracked(&listed, max) else {
            tracing::warn!(stream = %self.stream, listed = listed.len(), "an IMAP folder listing without an Inbox is not believed");
            return Ok(());
        };
        let mut before = std::mem::take(&mut run.cursor.folders);
        for folder in tracked {
            let kept = before
                .iter()
                .position(|f| f.name == folder.name)
                .map(|at| before.remove(at));
            match kept {
                Some(mut kept) => {
                    if kept.role != folder.role {
                        self.touch_folder(run, &kept.name).await?;
                    }
                    kept.role = folder.role;
                    kept.display = folder.display;
                    run.cursor.folders.push(kept);
                }
                None => run.cursor.folders.push(Folder::new(folder)),
            }
        }
        for gone in before {
            self.forget_folder(run, &gone.name).await?;
        }
        run.cursor.listed_at = Some(self.now);
        Ok(())
    }

    /// Read folder `i` if it moved. Whether it is read up to date (`false`:
    /// the run's budget or time ran out in it).
    async fn folder(&mut self, run: &mut Run, i: usize) -> Result<bool, Stop> {
        let name = run.cursor.folders[i].name.clone();
        if run.cursor.folders[i]
            .skipped_until
            .is_some_and(|until| until > self.now)
        {
            return Ok(true);
        }
        let Some(status) = self.status(run, &name).await? else {
            self.skip(run, i);
            return Ok(true);
        };
        let (Some(uidvalidity), Some(_)) = (status.uidvalidity, status.uidnext) else {
            tracing::info!(stream = %self.stream, "an IMAP folder without UIDVALIDITY or UIDNEXT is set aside");
            self.skip(run, i);
            return Ok(true);
        };
        let folder = &run.cursor.folders[i];
        let moved = folder.uidvalidity != Some(uidvalidity)
            || folder.uidnext != status.uidnext
            || folder.messages != status.messages
            || (run.caps.condstore
                && status.highestmodseq.is_some()
                && folder.highestmodseq != status.highestmodseq);
        if !moved && folder.backfilled {
            return Ok(true);
        }
        let Some(opened) = self.examine(run, &name).await? else {
            self.skip(run, i);
            return Ok(true);
        };
        run.cursor.folders[i].skipped_until = None;
        let uidvalidity = opened.uidvalidity.unwrap_or(uidvalidity);
        if run.cursor.folders[i]
            .uidvalidity
            .is_some_and(|v| v != uidvalidity)
        {
            tracing::info!(stream = %self.stream, "an IMAP folder's UIDVALIDITY changed: it is read again");
            self.forget_folder(run, &name).await?;
            run.cursor.folders[i].reset();
        }
        run.cursor.folders[i].uidvalidity = Some(uidvalidity);
        let uidnext = opened.uidnext.or(status.uidnext).unwrap_or(1);
        let messages = opened.messages.or(status.messages);
        let highest = opened
            .highestmodseq
            .or(status.highestmodseq)
            .filter(|_| run.caps.condstore && !opened.nomodseq);

        let mut arrived = 0;
        if let Some(from) = run.cursor.folders[i].uidnext.filter(|from| uidnext > *from) {
            let mut uids = self.search(&format!("UID {from}:*")).await?;
            uids.retain(|uid| *uid >= from);
            uids.sort_unstable();
            uids.dedup();
            arrived = uids.len();
            if self.handle(run, i, &uids, Mode::New).await? < uids.len() {
                return Ok(false);
            }
            let folder = &mut run.cursor.folders[i];
            folder.uidnext = Some(folder.uidnext.unwrap_or(1).max(uidnext));
        }
        if let (Some(since), Some(now)) = (run.cursor.folders[i].highestmodseq, highest)
            && now > since
        {
            self.changed_flags(run, i, since).await?;
        }
        if !run.caps.qresync
            && let (Some(before), Some(now)) = (run.cursor.folders[i].messages, messages)
            && u64::from(now)
                != u64::from(before).saturating_add(u64::try_from(arrived).unwrap_or(u64::MAX))
        {
            self.reconcile(run, i, messages).await?;
        }
        if !run.cursor.folders[i].backfilled && !self.walk_window(run, i, uidnext).await? {
            return Ok(false);
        }
        let folder = &mut run.cursor.folders[i];
        folder.messages = messages;
        folder.highestmodseq = highest;
        Ok(true)
    }

    /// Set folder `i` aside for [`SKIP_FOR`]: it would not open.
    fn skip(&self, run: &mut Run, i: usize) {
        run.cursor.folders[i].skipped_until = Some(self.now + SKIP_FOR);
    }

    /// Walk folder `i`'s window, newest first, below where the walk got.
    /// Whether it is all read.
    async fn walk_window(&mut self, run: &mut Run, i: usize, uidnext: u32) -> Result<bool, Stop> {
        let folder = &mut run.cursor.folders[i];
        // New mail from here on is read as new mail.
        let top = *folder.uidnext.get_or_insert(uidnext);
        let below = folder.backfill_low_uid.unwrap_or(top);
        if below > 1 {
            let since = self.window().format("%d-%b-%Y");
            let mut uids = self
                .search(&format!("UID 1:{} SINCE {since}", below - 1))
                .await?;
            uids.retain(|uid| *uid < below);
            uids.sort_unstable_by(|a, b| b.cmp(a));
            uids.dedup();
            if self.handle(run, i, &uids, Mode::Walk).await? < uids.len() {
                return Ok(false);
            }
        }
        let folder = &mut run.cursor.folders[i];
        folder.backfilled = true;
        folder.backfill_low_uid = None;
        Ok(true)
    }

    /// `STATUS` of folder `name`; `None` when the server refuses it (the
    /// folder is listed again next run).
    async fn status(&mut self, run: &mut Run, name: &str) -> Result<Option<Opened>, Stop> {
        let Some(quoted) = quoted(name) else {
            return Ok(None);
        };
        let items = if run.caps.condstore {
            "(UIDNEXT UIDVALIDITY MESSAGES HIGHESTMODSEQ)"
        } else {
            "(UIDNEXT UIDVALIDITY MESSAGES)"
        };
        let mut status = Opened::default();
        let limits = self.small();
        let done = self
            .session
            .command(&format!("STATUS {quoted} {items}"), limits, |untagged| {
                if let Untagged::MailboxStatus { name: of, items } = untagged
                    && of.as_ref() == name.as_bytes()
                {
                    for (item, value) in items {
                        match item.as_str() {
                            "UIDNEXT" => status.uidnext = u32::try_from(value).ok(),
                            "UIDVALIDITY" => {
                                status.uidvalidity = u32::try_from(value).ok().filter(|v| *v > 0);
                            }
                            "MESSAGES" => status.messages = u32::try_from(value).ok(),
                            "HIGHESTMODSEQ" => status.highestmodseq = Some(value),
                            _ => {}
                        }
                    }
                }
            })
            .await?;
        if done.is_busy() {
            return Err(Stop::Busy);
        }
        if !done.is_ok() {
            run.refused.insert(name.to_owned());
            return Ok(None);
        }
        Ok(Some(status))
    }

    /// Open folder `name` read-only; `None` when the server refuses it.
    async fn examine(&mut self, run: &mut Run, name: &str) -> Result<Option<Opened>, Stop> {
        let Some(quoted) = quoted(name) else {
            return Ok(None);
        };
        let condstore = if run.caps.condstore {
            " (CONDSTORE)"
        } else {
            ""
        };
        let mut opened = Opened::default();
        let limits = self.small();
        let done = self
            .session
            .command(
                &format!("EXAMINE {quoted}{condstore}"),
                limits,
                |untagged| match untagged {
                    Untagged::Status {
                        code: Some(code), ..
                    } => {
                        let n = code.number;
                        match code.name.as_str() {
                            "UIDVALIDITY" => {
                                opened.uidvalidity =
                                    n.and_then(|n| u32::try_from(n).ok()).filter(|v| *v > 0);
                            }
                            "UIDNEXT" => opened.uidnext = n.and_then(|n| u32::try_from(n).ok()),
                            "HIGHESTMODSEQ" => opened.highestmodseq = n,
                            "NOMODSEQ" => opened.nomodseq = true,
                            _ => {}
                        }
                    }
                    Untagged::Exists(n) => opened.messages = Some(n),
                    _ => {}
                },
            )
            .await?;
        if done.is_busy() {
            return Err(Stop::Busy);
        }
        if !done.is_ok() {
            run.refused.insert(name.to_owned());
            return Ok(None);
        }
        Ok(Some(opened))
    }

    /// `UID SEARCH criteria` in the open folder: at most [`MAX_UIDS`] UIDs
    /// (more cuts the run short).
    async fn search(&mut self, criteria: &str) -> Result<Vec<u32>, Stop> {
        let limits = Limits {
            response: 16 * 1024 * 1024,
            responses: 1_000,
            total: 32 * 1024 * 1024,
            time: self.timeout() * 2,
        };
        let mut uids = Vec::new();
        let mut over = false;
        let done = self
            .session
            .command(&format!("UID SEARCH {criteria}"), limits, |untagged| {
                if let Untagged::Search(found) = untagged {
                    if uids.len().saturating_add(found.len()) > MAX_UIDS {
                        over = true;
                    } else {
                        uids.extend(found);
                    }
                }
            })
            .await?;
        if over {
            return Err(Stop::Wire(WireError::TooMany));
        }
        check(&done)?;
        Ok(uids)
    }

    /// Read `uids` of folder `i` (open), in order: locate known messages,
    /// download new ones. How many were handled (in order), the cursor
    /// moved past each.
    async fn handle(
        &mut self,
        run: &mut Run,
        i: usize,
        uids: &[u32],
        mode: Mode,
    ) -> Result<usize, Stop> {
        let folder = run.cursor.folders[i].name.clone();
        let uidvalidity = run.cursor.folders[i].uidvalidity.ok_or(Stop::Refused)?;
        let window = self.window();
        let mut handled = 0;
        for chunk in uids.chunks(HEADER_CHUNK) {
            if run.out_of_time() || run.headers >= MAX_HEADERS {
                break;
            }
            run.headers += chunk.len();
            let headers = self.headers(run, chunk).await?;
            let keys = self.identify(run, &headers).await?;
            let wanted: Vec<String> = keys.values().cloned().collect();
            self.load_known(run, &wanted).await?;
            self.load_oversized(run, &wanted).await?;
            for &uid in chunk {
                if let (Some(header), Some(key)) = (headers.get(&uid), keys.get(&uid)) {
                    let place = Place {
                        folder: folder.clone(),
                        uidvalidity,
                        uid,
                    };
                    let location = Location {
                        key: key.clone(),
                        seen: header.seen,
                        flagged: header.flagged,
                        email_id: header.email_id.clone(),
                        fingerprint: Some(header.fingerprint.clone()),
                    };
                    let old =
                        mode == Mode::New && header.internal_date.is_some_and(|at| at < window);
                    // Not this run's to download: a look-up, mail older than
                    // the window, or a message over the size limit.
                    let skip = mode == Mode::Lookup || old || run.oversized.contains(key);
                    if run.is_known(key) {
                        run.put(place, location);
                    } else if skip {
                        // Left as it is.
                    } else if header.size.is_some_and(|size| size > self.max_message()) {
                        run.mark_oversized(key);
                    } else if self.budget_spent(run) {
                        return Ok(handled);
                    } else {
                        let downloaded = self.download(uid).await;
                        match downloaded {
                            Ok(Some(raw)) => {
                                if u64::try_from(raw.len()).unwrap_or(u64::MAX) > self.max_message()
                                {
                                    run.mark_oversized(key);
                                } else {
                                    self.store(run, place, location, header, raw).await?;
                                }
                            }
                            Ok(None) => {}
                            Err(Stop::Wire(WireError::TooLarge)) => {
                                // The server sent more than it said: the
                                // message is skipped, and the session,
                                // left mid-answer, ends.
                                run.mark_oversized(key);
                                advance(run, i, uid, mode);
                                return Err(Stop::Wire(WireError::TooLarge));
                            }
                            Err(Stop::Wire(WireError::TimedOut)) => {
                                // Counted: after [`MAX_TIMEOUTS`] it is
                                // never asked for again.
                                run.index.push(IndexOp::Skipped {
                                    provider_message_id: key.clone(),
                                    at: self.now,
                                });
                                return Err(Stop::Wire(WireError::TimedOut));
                            }
                            Err(err) => return Err(err),
                        }
                    }
                }
                advance(run, i, uid, mode);
                handled += 1;
            }
        }
        Ok(handled)
    }

    /// The headers of `uids` in the open folder (each UID once; any other
    /// is ignored).
    async fn headers(&mut self, run: &Run, uids: &[u32]) -> Result<HashMap<u32, Header>, Stop> {
        let email_id = if run.caps.objectid { " EMAILID" } else { "" };
        let command = format!(
            "UID FETCH {} (UID FLAGS RFC822.SIZE INTERNALDATE{email_id} {HEADER_FIELDS})",
            sequence(uids)
        );
        let limits = Limits {
            response: 1024 * 1024,
            responses: uids.len() * 2 + 100,
            total: 32 * 1024 * 1024,
            time: self.timeout() * 2,
        };
        let wanted: HashSet<u32> = uids.iter().copied().collect();
        let address = self.address.clone();
        let mut found = HashMap::new();
        let done = self
            .session
            .command(&command, limits, |untagged| {
                if let Untagged::Fetch(fetch) = untagged
                    && let Some(uid) = fetch.uid.filter(|uid| wanted.contains(uid))
                {
                    found
                        .entry(uid)
                        .or_insert_with(|| Header::of(&fetch, address.as_deref()));
                }
            })
            .await?;
        check(&done)?;
        Ok(found)
    }

    /// The keys of `headers`: the `Message-ID` (or digest) key, unless
    /// that key's copies say this is another message: another EMAILID (with
    /// OBJECTID), or another fingerprint (`Date`, `From`, size) — a forgery
    /// or an auto-reply reusing the `Message-ID` is its own message, never
    /// the stored body of the one it imitates. A message with no copy left
    /// (moved) keeps its key.
    async fn identify(
        &self,
        run: &Run,
        headers: &HashMap<u32, Header>,
    ) -> Result<HashMap<u32, String>, Stop> {
        let mut keys: HashMap<u32, String> =
            headers.iter().map(|(uid, h)| (*uid, h.key())).collect();
        let mut bases: Vec<String> = keys.values().cloned().collect();
        bases.sort_unstable();
        bases.dedup();
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<CopyRow> = sqlx::query_as(
            "SELECT message_key, folder, uidvalidity, uid, email_id, fingerprint
               FROM connectors.imap_locations
              WHERE stream_id = $1 AND message_key = ANY($2)",
        )
        .bind(self.stream)
        .bind(&bases)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut marks: HashMap<String, Marks> = HashMap::new();
        for (key, folder, uidvalidity, uid, email_id, fingerprint) in rows {
            let place = Place {
                folder,
                uidvalidity: u32::try_from(uidvalidity).unwrap_or_default(),
                uid: u32::try_from(uid).unwrap_or_default(),
            };
            if run.stored(&place) {
                marks.entry(key).or_default().add(email_id, fingerprint);
            }
        }
        for location in run.put.values() {
            marks
                .entry(location.key.clone())
                .or_default()
                .add(location.email_id.clone(), location.fingerprint.clone());
        }
        let mut uids: Vec<&u32> = headers.keys().collect();
        uids.sort_unstable();
        for uid in uids {
            let (Some(header), Some(key)) = (headers.get(uid), keys.get_mut(uid)) else {
                continue;
            };
            if run.caps.objectid
                && let Some(email_id) = &header.email_id
                && marks
                    .get(key.as_str())
                    .is_some_and(|m| !m.email_ids.is_empty() && !m.email_ids.contains(email_id))
            {
                *key = format!("{key}~{}", &hex(email_id)[..16]);
            }
            if marks.get(key.as_str()).is_some_and(|m| {
                !m.fingerprints.is_empty() && !m.fingerprints.contains(&header.fingerprint)
            }) {
                *key = format!("{key}~{}", &header.fingerprint[..16]);
            }
            marks
                .entry(key.clone())
                .or_default()
                .add(header.email_id.clone(), Some(header.fingerprint.clone()));
        }
        Ok(keys)
    }

    /// Download one message whole, without marking it read; `None` when
    /// the server no longer has it.
    async fn download(&mut self, uid: u32) -> Result<Option<Vec<u8>>, Stop> {
        let max = usize::try_from(self.max_message()).unwrap_or(usize::MAX);
        let limits = Limits {
            response: max.saturating_add(64 * 1024),
            responses: 100,
            total: max.saturating_add(2 * 1024 * 1024),
            time: self.timeout() * 4,
        };
        let mut body = None;
        let done = self
            .session
            .command(
                &format!("UID FETCH {uid} (UID BODY.PEEK[])"),
                limits,
                |untagged| {
                    if let Untagged::Fetch(mut fetch) = untagged
                        && fetch.uid == Some(uid)
                        && body.is_none()
                    {
                        body = fetch.body.take().map(std::borrow::Cow::into_owned);
                    }
                },
            )
            .await?;
        if done.is_busy() {
            return Err(Stop::Busy);
        }
        Ok(body.filter(|_| done.is_ok()))
    }

    /// Store a downloaded message, its state as this location says (the
    /// run's end sets it from all its locations).
    async fn store(
        &mut self,
        run: &mut Run,
        place: Place,
        location: Location,
        header: &Header,
        raw: Vec<u8>,
    ) -> Result<(), Stop> {
        run.downloads += 1;
        let role = run.cursor.role_of(&place.folder).unwrap_or(Role::User);
        let tokens = state::tokens(
            [(role, place.folder.as_str(), header.seen, header.flagged)],
            header.from_self,
        );
        let flags = state::flags(&tokens, &run.cursor);
        let (message, op) = mail::store_message(
            self.blobs,
            &self.workspace.to_string(),
            Fetched {
                id: location.key.clone(),
                thread_key: None,
                label_ids: tokens,
                internal_date: header.internal_date.unwrap_or(self.now),
                raw,
                flags,
                receiver: Receiver::Imap,
            },
            run.cursor.generation,
        )
        .await?;
        run.inserted.insert(
            location.key.clone(),
            Inserted {
                change: run.changes.len(),
                op: run.index.len(),
                from_self: header.from_self,
            },
        );
        run.changes.push(MessageChange::Upsert {
            message: Box::new(message),
        });
        run.index.push(op);
        run.put(place, location);
        Ok(())
    }

    /// With CONDSTORE: the flags that changed in folder `i` (open) since
    /// `since`, and with QRESYNC what vanished.
    async fn changed_flags(&mut self, run: &mut Run, i: usize, since: u64) -> Result<(), Stop> {
        let (_, copies) = self.copies(run, i).await?;
        let vanished = if run.caps.qresync { " VANISHED" } else { "" };
        let command = format!("UID FETCH 1:* (UID FLAGS) (CHANGEDSINCE {since}{vanished})");
        let (changed, gone) = self.flags_of(&command, &copies).await?;
        put_flags(run, &copies, &changed);
        for (place, location) in &copies {
            if gone.contains(&place.uid) {
                run.drop_copy(place.clone(), &location.key);
            }
        }
        Ok(())
    }

    /// `FETCH … (UID FLAGS)` over a folder's `copies`: the (seen, flagged)
    /// of each copy's UID, and the copies' UIDs reported vanished. Only
    /// known UIDs are kept, so what is held stays within the copies,
    /// whatever the server sends; past [`MAX_VANISHED`] ranges the answer
    /// is cut short.
    async fn flags_of(
        &mut self,
        command: &str,
        copies: &HashMap<Place, Location>,
    ) -> Result<FlagsRead, Stop> {
        let limits = Limits {
            response: 64 * 1024,
            responses: 2_000_000,
            total: 256 * 1024 * 1024,
            time: self.timeout() * 4,
        };
        let mut known: Vec<u32> = copies.keys().map(|place| place.uid).collect();
        known.sort_unstable();
        let mut changed: HashMap<u32, (bool, bool)> = HashMap::new();
        let mut gone: HashSet<u32> = HashSet::new();
        let mut ranges = 0_usize;
        let done = self
            .session
            .command(command, limits, |untagged| match untagged {
                Untagged::Fetch(fetch) => {
                    if let (Some(uid), Some(flags)) = (fetch.uid, fetch.flags)
                        && known.binary_search(&uid).is_ok()
                    {
                        changed.entry(uid).or_insert(flags);
                    }
                }
                Untagged::Vanished(vanished) => {
                    ranges = ranges.saturating_add(vanished.len());
                    if ranges <= MAX_VANISHED {
                        for (low, high) in vanished {
                            let from = known.partition_point(|uid| *uid < low);
                            let to = known.partition_point(|uid| *uid <= high);
                            gone.extend(known.get(from..to).unwrap_or_default());
                        }
                    }
                }
                _ => {}
            })
            .await?;
        if ranges > MAX_VANISHED {
            return Err(Stop::Wire(WireError::TooMany));
        }
        check(&done)?;
        Ok((changed, gone))
    }

    /// Check folder `i`'s (open) known UIDs are still there; those gone
    /// are dropped. A server that says the folder has messages but finds
    /// none of them is not believed.
    async fn reconcile(
        &mut self,
        run: &mut Run,
        i: usize,
        messages: Option<u32>,
    ) -> Result<(), Stop> {
        let (_, copies) = self.copies(run, i).await?;
        let (Some(low), Some(high)) = (
            copies.keys().map(|p| p.uid).min(),
            copies.keys().map(|p| p.uid).max(),
        ) else {
            return Ok(());
        };
        let mut present = self.search(&format!("UID {low}:{high}")).await?;
        present.sort_unstable();
        if present.is_empty() && messages.is_some_and(|n| n > 0) {
            tracing::warn!(stream = %self.stream, "an IMAP search found none of a folder's messages; not believed");
            return Ok(());
        }
        for (place, location) in copies {
            if present.binary_search(&place.uid).is_err() {
                run.drop_copy(place, &location.key);
            }
        }
        Ok(())
    }

    /// The sweep: every folder's known UIDs and their flags, read again.
    /// Whether it ended.
    async fn sweep(&mut self, run: &mut Run) -> Result<bool, Stop> {
        for i in 0..run.cursor.folders.len() {
            if run.out_of_time() {
                return Ok(false);
            }
            let name = run.cursor.folders[i].name.clone();
            let Some(uidvalidity) = run.cursor.folders[i].uidvalidity else {
                continue;
            };
            if run.cursor.folders[i].skipped_until.is_some() {
                continue;
            }
            let (_, copies) = self.copies(run, i).await?;
            if copies.is_empty() {
                continue;
            }
            let Some(opened) = self.examine(run, &name).await? else {
                self.skip(run, i);
                continue;
            };
            if opened.uidvalidity.is_some_and(|v| v != uidvalidity) {
                // Read again from scratch next run.
                self.forget_folder(run, &name).await?;
                run.cursor.folders[i].reset();
                continue;
            }
            self.reconcile(run, i, opened.messages).await?;
            let (_, copies) = self.copies(run, i).await?;
            let (Some(low), Some(high)) = (
                copies.keys().map(|p| p.uid).min(),
                copies.keys().map(|p| p.uid).max(),
            ) else {
                continue;
            };
            let (changed, _) = self
                .flags_of(&format!("UID FETCH {low}:{high} (UID FLAGS)"), &copies)
                .await?;
            put_flags(run, &copies, &changed);
        }
        Ok(true)
    }

    /// The copies in folder `i` at its UIDVALIDITY, as this run left them.
    async fn copies(
        &self,
        run: &Run,
        i: usize,
    ) -> Result<(String, HashMap<Place, Location>), Stop> {
        let folder = run.cursor.folders[i].name.clone();
        let Some(uidvalidity) = run.cursor.folders[i].uidvalidity else {
            return Ok((folder, HashMap::new()));
        };
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<(i64, String, bool, bool, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT uid, message_key, seen, flagged, email_id, fingerprint
               FROM connectors.imap_locations
              WHERE stream_id = $1 AND folder = $2 AND uidvalidity = $3",
        )
        .bind(self.stream)
        .bind(&folder)
        .bind(i64::from(uidvalidity))
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut copies = HashMap::new();
        for (uid, key, seen, flagged, email_id, fingerprint) in rows {
            let Ok(uid) = u32::try_from(uid) else {
                continue;
            };
            let place = Place {
                folder: folder.clone(),
                uidvalidity,
                uid,
            };
            if run.stored(&place) {
                copies.insert(
                    place,
                    Location {
                        key,
                        seen,
                        flagged,
                        email_id,
                        fingerprint,
                    },
                );
            }
        }
        for (place, location) in &run.put {
            if place.folder == folder && place.uidvalidity == uidvalidity {
                copies.insert(place.clone(), location.clone());
            }
        }
        Ok((folder, copies))
    }

    /// Forget every copy in folder `name` (gone, or its UIDVALIDITY
    /// changed): its messages are settled at the run's end.
    async fn forget_folder(&self, run: &mut Run, name: &str) -> Result<(), Stop> {
        self.touch_folder(run, name).await?;
        run.put.retain(|place, _| place.folder != name);
        run.dropped.retain(|place| place.folder != name);
        run.dropped_folders.insert(name.to_owned());
        run.locations.push(LocationOp::DropFolder(name.to_owned()));
        Ok(())
    }

    /// Mark every message with a copy in folder `name` as changed.
    async fn touch_folder(&self, run: &mut Run, name: &str) -> Result<(), Stop> {
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let keys: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT message_key FROM connectors.imap_locations
              WHERE stream_id = $1 AND folder = $2",
        )
        .bind(self.stream)
        .bind(name)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        run.touched.extend(keys);
        let put: Vec<String> = run
            .put
            .iter()
            .filter(|(place, _)| place.folder == name)
            .map(|(_, location)| location.key.clone())
            .collect();
        run.touched.extend(put);
        Ok(())
    }

    /// The index rows of `keys` not looked up yet.
    async fn load_known(&self, run: &mut Run, keys: &[String]) -> Result<(), Stop> {
        let missing: Vec<String> = keys
            .iter()
            .filter(|key| !run.known.contains_key(*key))
            .cloned()
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<KnownRow> = sqlx::query_as(
            "SELECT provider_message_id, id, label_ids, document_hash, document_size,
                        rfc_message_id, internal_date
                   FROM connectors.messages
                  WHERE stream_id = $1 AND provider_message_id = ANY($2)",
        )
        .bind(self.stream)
        .bind(&missing)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        for key in missing {
            run.known.insert(key.clone(), None);
        }
        for (key, row, labels, hash, size, rfc_message_id, internal_date) in rows {
            run.known.insert(
                key,
                Some(Known {
                    row,
                    labels,
                    document: mail::document_ref(&hash, size)?,
                    rfc_message_id,
                    internal_date,
                }),
            );
        }
        Ok(())
    }

    /// Which of `keys` were recorded as oversized before, or timed out
    /// [`MAX_TIMEOUTS`] times (recorded as oversized now).
    async fn load_oversized(&self, run: &mut Run, keys: &[String]) -> Result<(), Stop> {
        if keys.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT provider_message_id FROM connectors.oversized
              WHERE stream_id = $1 AND provider_message_id = ANY($2)",
        )
        .bind(self.stream)
        .bind(keys)
        .fetch_all(&mut *tx)
        .await?;
        // Downloads that timed out run after run: as good as oversized.
        let stuck: Vec<String> = sqlx::query_scalar(
            "SELECT provider_message_id FROM connectors.skipped_messages
              WHERE stream_id = $1 AND provider_message_id = ANY($2) AND failures >= $3",
        )
        .bind(self.stream)
        .bind(keys)
        .bind(MAX_TIMEOUTS)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        run.oversized.extend(rows);
        for key in stuck {
            run.mark_oversized(&key);
        }
        Ok(())
    }

    /// The end of a run: the state of every message whose locations
    /// changed; then, when every folder is read, messages left with no
    /// location are looked for and removed.
    async fn settle(&mut self, run: &mut Run, complete: bool) -> Result<(), Stop> {
        let touched: Vec<String> = run.touched.iter().cloned().collect();
        let mut orphans = self.settle_states(run, &touched).await?;
        if !complete || !run.cursor.backfilled() {
            // Looked for once every folder is read.
            run.cursor.recheck |= !orphans.is_empty();
            return Ok(());
        }
        if run.swept || run.cursor.recheck {
            run.cursor.recheck = false;
            for key in self.unlocated(run).await? {
                if !orphans.contains(&key) {
                    orphans.push(key);
                }
            }
        }
        if orphans.is_empty() {
            return Ok(());
        }
        self.load_known(run, &orphans).await?;
        let window = self.window();
        let mut searchable = Vec::new();
        let mut removable = Vec::new();
        for key in &orphans {
            let Some(Some(known)) = run.known.get(key) else {
                continue;
            };
            match known.rfc_message_id.as_deref().map(quoted) {
                Some(Some(id)) => searchable.push((key.clone(), id)),
                // A Message-ID a search cannot carry (not ASCII, too long):
                // it cannot be looked for, so it is kept.
                Some(None) => {}
                // Without a Message-ID it cannot be looked for: gone when
                // the window's walk would have found it.
                None if known.internal_date >= window => removable.push(key.clone()),
                None => {}
            }
        }
        if searchable.len() > MAX_LOOKUPS {
            searchable.truncate(MAX_LOOKUPS);
            run.cursor.recheck = true;
        }
        let looked = self.look_up(run, &searchable).await;
        let searched: Vec<String> = searchable.into_iter().map(|(key, _)| key).collect();
        let found = self.settle_states(run, &searched).await;
        let mut still = found?;
        match looked {
            Ok(true) => removable.append(&mut still),
            Ok(false) => run.cursor.recheck = true,
            Err(stop) => {
                run.cursor.recheck = true;
                self.remove(run, &removable);
                return Err(stop);
            }
        }
        self.remove(run, &removable);
        Ok(())
    }

    /// Remove `keys` (indexed, with no location).
    fn remove(&self, run: &mut Run, keys: &[String]) {
        if !keys.is_empty() {
            tracing::info!(stream = %self.stream, removed = keys.len(), "IMAP messages in no folder are removed");
        }
        for key in keys {
            if let Some(Some(known)) = run.known.get(key) {
                run.index.push(IndexOp::Remove { row: known.row });
                run.changes.push(MessageChange::Remove {
                    provider_message_id: key.clone(),
                });
            }
        }
    }

    /// Look for `searchable` (key, quoted Message-ID) in every folder,
    /// locating those found. Whether every folder was searched.
    async fn look_up(
        &mut self,
        run: &mut Run,
        searchable: &[(String, String)],
    ) -> Result<bool, Stop> {
        if searchable.is_empty() {
            return Ok(true);
        }
        let mut whole = true;
        for i in 0..run.cursor.folders.len() {
            if run.out_of_time() {
                return Ok(false);
            }
            let name = run.cursor.folders[i].name.clone();
            if run.cursor.folders[i].skipped_until.is_some() {
                whole = false;
                continue;
            }
            let Some(opened) = self.examine(run, &name).await? else {
                self.skip(run, i);
                whole = false;
                continue;
            };
            if opened
                .uidvalidity
                .is_some_and(|v| Some(v) != run.cursor.folders[i].uidvalidity)
            {
                whole = false;
                continue;
            }
            for batch in searchable.chunks(LOOKUP_BATCH) {
                let mut criteria = "OR ".repeat(batch.len() - 1);
                for (n, (_, id)) in batch.iter().enumerate() {
                    if n > 0 {
                        criteria.push(' ');
                    }
                    let _ = write!(criteria, "HEADER Message-ID {id}");
                }
                let mut uids = self.search(&criteria).await?;
                uids.sort_unstable();
                uids.dedup();
                uids.truncate(LOOKUP_HITS);
                if self.handle(run, i, &uids, Mode::Lookup).await? < uids.len() {
                    whole = false;
                }
            }
        }
        Ok(whole)
    }

    /// Indexed messages of the stream with no stored location (at most
    /// [`MAX_UNLOCATED`]), not already settled by this run.
    async fn unlocated(&self, run: &Run) -> Result<Vec<String>, Stop> {
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let keys: Vec<String> = sqlx::query_scalar(
            "SELECT m.provider_message_id FROM connectors.messages m
              WHERE m.stream_id = $1
                AND NOT EXISTS (SELECT 1 FROM connectors.imap_locations l
                                 WHERE l.stream_id = m.stream_id
                                   AND l.message_key = m.provider_message_id)
              ORDER BY m.internal_date DESC
              LIMIT $2",
        )
        .bind(self.stream)
        .bind(MAX_UNLOCATED)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let located: HashSet<&str> = run.put.values().map(|l| l.key.as_str()).collect();
        Ok(keys
            .into_iter()
            .filter(|key| !run.touched.contains(key) && !located.contains(key.as_str()))
            .collect())
    }

    /// Set the state of `keys` from their locations: a message downloaded
    /// this run is emitted with it; a known one is emitted again (from its
    /// stored document) when it changed. Those with no location left.
    #[allow(
        clippy::too_many_lines,
        reason = "one pass: the locations, then each message's state"
    )]
    async fn settle_states(&self, run: &mut Run, keys: &[String]) -> Result<Vec<String>, Stop> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        self.load_known(run, keys).await?;
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<(String, String, i64, i64, bool, bool)> = sqlx::query_as(
            "SELECT message_key, folder, uidvalidity, uid, seen, flagged
               FROM connectors.imap_locations
              WHERE stream_id = $1 AND message_key = ANY($2)",
        )
        .bind(self.stream)
        .bind(keys)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut places: HashMap<String, Vec<(String, bool, bool)>> = HashMap::new();
        for (key, folder, uidvalidity, uid, seen, flagged) in rows {
            let place = Place {
                folder,
                uidvalidity: u32::try_from(uidvalidity).unwrap_or_default(),
                uid: u32::try_from(uid).unwrap_or_default(),
            };
            if run.stored(&place) {
                places
                    .entry(key)
                    .or_default()
                    .push((place.folder, seen, flagged));
            }
        }
        let wanted: HashSet<&String> = keys.iter().collect();
        for (place, location) in &run.put {
            if wanted.contains(&location.key) {
                places.entry(location.key.clone()).or_default().push((
                    place.folder.clone(),
                    location.seen,
                    location.flagged,
                ));
            }
        }
        let mut unlocated = Vec::new();
        let workspace = self.workspace.to_string();
        for key in keys {
            let located: Vec<(Role, &str, bool, bool)> = places
                .get(key)
                .map(|copies| {
                    copies
                        .iter()
                        .filter_map(|(folder, seen, flagged)| {
                            Some((
                                run.cursor.role_of(folder)?,
                                folder.as_str(),
                                *seen,
                                *flagged,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if located.is_empty() {
                unlocated.push(key.clone());
                continue;
            }
            if let Some(inserted) = run.inserted.get(key).copied() {
                let tokens = state::tokens(located, inserted.from_self);
                let flags = state::flags(&tokens, &run.cursor);
                if let Some(MessageChange::Upsert { message }) =
                    run.changes.get_mut(inserted.change)
                {
                    mail::apply_flags(message, &flags);
                }
                if let Some(IndexOp::Insert { label_ids, .. }) = run.index.get_mut(inserted.op) {
                    *label_ids = tokens;
                }
                continue;
            }
            let Some(Some(known)) = run.known.get(key).cloned() else {
                continue;
            };
            let tokens = state::tokens(located, known.labels.iter().any(|t| t == state::SELF));
            let mut before = known.labels.clone();
            before.sort();
            if before == tokens {
                continue;
            }
            let mut message: NormalizedMessage =
                mail::document(self.blobs, &workspace, &known.document).await?;
            mail::apply_flags(&mut message, &state::flags(&tokens, &run.cursor));
            run.changes.push(MessageChange::Upsert {
                message: Box::new(message),
            });
            run.index.push(IndexOp::Update {
                row: known.row,
                label_ids: tokens.clone(),
                generation: run.cursor.generation,
            });
            if let Some(Some(known)) = run.known.get_mut(key) {
                known.labels = tokens;
            }
        }
        Ok(unlocated)
    }
}

/// What `FETCH … (UID FLAGS)` read: flags by known UID, and the known UIDs
/// reported vanished.
type FlagsRead = (HashMap<u32, (bool, bool)>, HashSet<u32>);

/// Put the flags of `changed` (by UID) on the `copies` of one folder they
/// differ on.
fn put_flags(
    run: &mut Run,
    copies: &HashMap<Place, Location>,
    changed: &HashMap<u32, (bool, bool)>,
) {
    for (place, location) in copies {
        if let Some(&(seen, flagged)) = changed.get(&place.uid)
            && (location.seen, location.flagged) != (seen, flagged)
        {
            run.put(
                place.clone(),
                Location {
                    seen,
                    flagged,
                    ..location.clone()
                },
            );
        }
    }
}

/// Move the cursor of folder `i` past `uid`, read in `mode`.
fn advance(run: &mut Run, i: usize, uid: u32, mode: Mode) {
    let folder = &mut run.cursor.folders[i];
    match mode {
        Mode::New => {
            let next = uid.saturating_add(1);
            folder.uidnext = Some(folder.uidnext.map_or(next, |n| n.max(next)));
        }
        Mode::Walk => folder.backfill_low_uid = Some(uid),
        Mode::Lookup => {}
    }
}

/// A command's end: busy, refused, or fine.
fn check(done: &Done) -> Result<(), Stop> {
    if done.is_busy() {
        Err(Stop::Busy)
    } else if done.is_ok() {
        Ok(())
    } else {
        Err(Stop::Refused)
    }
}

/// `uids` as a sequence set (`1:5,7`).
fn sequence(uids: &[u32]) -> String {
    let mut sorted = uids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut out = String::new();
    let mut i = 0;
    while i < sorted.len() {
        let start = sorted[i];
        let mut end = start;
        while i + 1 < sorted.len() && sorted[i + 1] == end.saturating_add(1) {
            i += 1;
            end = sorted[i];
        }
        if !out.is_empty() {
            out.push(',');
        }
        if start == end {
            let _ = write!(out, "{start}");
        } else {
            let _ = write!(out, "{start}:{end}");
        }
        i += 1;
    }
    out
}

/// SHA-256 of `text`, in hex.
fn hex(text: &str) -> String {
    let mut out = String::with_capacity(64);
    for byte in campbooks_kernel::crypto::hash(text) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The fields of a header block, names lower-cased, folded lines joined,
/// the first of each name kept, each value at most [`MAX_FIELD`]
/// characters.
fn header_fields(bytes: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in text.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            break;
        }
        if line.starts_with([' ', '\t']) {
            if let Some((_, value)) = fields.last_mut()
                && value.len() < MAX_FIELD
            {
                if !value.is_empty() {
                    value.push(' ');
                }
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            fields.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    let mut out = HashMap::new();
    for (name, mut value) in fields {
        if let Some((at, _)) = value.char_indices().nth(MAX_FIELD) {
            value.truncate(at);
        }
        out.entry(name).or_insert(value);
    }
    out
}

/// A `Message-ID` value normalised: what is inside its angle brackets,
/// without white space. `None` when there is nothing usable.
fn message_id(value: &str) -> Option<String> {
    let inner = value
        .find('<')
        .and_then(|open| {
            let rest = &value[open + 1..];
            rest.find('>').map(|close| &rest[..close])
        })
        .unwrap_or(value);
    let id: String = inner.chars().filter(|c| !c.is_whitespace()).collect();
    (!id.is_empty() && id.len() <= 998).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uids_are_sent_as_ranges() {
        assert_eq!(sequence(&[7, 1, 2, 3, 5, 3, 9, 8]), "1:3,5,7:9");
        assert_eq!(sequence(&[42]), "42");
        assert_eq!(sequence(&[u32::MAX, u32::MAX - 1]), "4294967294:4294967295");
    }

    #[test]
    fn messages_are_known_by_their_message_id_or_their_headers() {
        let fields = header_fields(
            b"Message-ID:\r\n <CA+abc@mail.example.com>\r\nFrom: Ana <ANA@example.com>\r\n\
              Subject: Q4\r\n plan\r\nSubject: second\r\n\r\nignored: yes\r\n",
        );
        assert_eq!(fields["message-id"], "<CA+abc@mail.example.com>");
        assert_eq!(fields["subject"], "Q4 plan");
        assert!(!fields.contains_key("ignored"));
        assert_eq!(
            message_id("  <CA+abc@mail.example.com> (comment)").as_deref(),
            Some("CA+abc@mail.example.com")
        );
        assert_eq!(
            message_id("bare@example.com").as_deref(),
            Some("bare@example.com")
        );
        assert_eq!(message_id("  "), None);

        let fetch = |header: &'static [u8]| Fetch {
            uid: Some(1),
            size: Some(10),
            header: Some(std::borrow::Cow::Borrowed(header)),
            ..Fetch::default()
        };
        let a = Header::of(&fetch(b"Message-ID: <a@example.com>\r\n\r\n"), None);
        let moved = Header::of(&fetch(b"Message-Id:   <a@example.com>\r\n\r\n"), None);
        assert_eq!(a.key(), moved.key());
        assert!(a.key().starts_with("mid:") && !a.key().contains("example"));
        let anonymous = Header::of(
            &fetch(b"From: Ana <ana@example.com>\r\n\r\n"),
            Some("ana@example.com"),
        );
        assert!(anonymous.key().starts_with("digest:"));
        assert!(anonymous.from_self);
        let other = Header::of(
            &fetch(b"From: Bea <bea@example.com>\r\n\r\n"),
            Some("ana@example.com"),
        );
        assert_ne!(anonymous.key(), other.key());
        assert!(!other.from_self);
    }
}
