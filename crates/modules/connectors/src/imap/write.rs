//! Making an IMAP stream's pending outbound changes (campbooks-cloud#312),
//! over the run's session, before it reads: merge them (the shared plan,
//! over the index's tokens as [`state::view`] shows them), turn each
//! message's change into flag writes on every copy and moves of copies
//! between the folders the reader tracks, make them folder by folder, and
//! say how each ended, with the locations, index tokens and cursor they
//! leave, for the run to record before it reads.
//!
//! - `UNREAD` / `STARRED`: `\Seen` / `\Flagged` off or on, on every copy
//!   (`UID STORE … ±FLAGS.SILENT`, read back); with CONDSTORE,
//!   `UNCHANGEDSINCE` the modseq the folder was last read at, so a later
//!   change at the server wins (`[MODIFIED …]`: `superseded`).
//! - `INBOX` off: the copies in the Inbox or a stream folder to Archive; on:
//!   the copies in Archive to the Inbox (or a member's folder's copied).
//! - `TRASH` / `SPAM` on: the copies in the Inbox, a stream folder, Archive
//!   (and the other bin) to Trash / Junk, where each was kept (bound into the
//!   change, as Zoho's); the member's own folders and Sent are left alone.
//!   Off: those copies back where they were, else to the Inbox for mail the
//!   inbox received, else to Sent (or Archive).
//! - `mark:<stream>`: the inbox copies to that stream's folder
//!   (`Campbooks/<Stream>`, created and subscribed first when a move needs
//!   it and the server has none); every mark off: to the Inbox.
//!
//! A move is `UID MOVE` (RFC 6851), else (UIDPLUS, RFC 4315) `UID COPY`,
//! then, only for the UIDs its `COPYUID` names, `UID STORE +FLAGS.SILENT
//! (\Deleted)` and `UID EXPUNGE` of those UIDs; never when another message
//! of the folder is already `\Deleted` (it would ride along on a server that
//! expunges more than it is asked). Else it is not made
//! (`unsupported_here`). Nothing here ever sends `EXPUNGE` without UIDs,
//! `CLOSE`, `DELETE` or `RENAME`; a written folder is left with `UNSELECT`,
//! or `EXAMINE` of it. A folder whose UIDVALIDITY is not the copies' is not
//! written; a UID the server no longer has is dropped, and looked up by the
//! next complete read. Logs carry ids, codes and counts only.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    time::Duration,
};

use campbooks_kernel::{db::BeginFor as _, tenancy::WorkspaceId};
use sqlx::PgPool;

use super::{
    folders::{self, Role, Tracked},
    read::sequence,
    session::{Done, Limits, Session, WireError, quoted},
    state::{self, Cursor, Folder, Location, LocationOp, Place},
    syntax::{Code, Untagged},
};
use crate::{
    config::ImapConfig,
    contract::ProviderStream,
    domain::{
        marks,
        model::StreamId,
        outbound::{Backlog, LabelChange, State},
        push::{Fate, Plan, Push, Results, retry_or_give_up, without_marks},
    },
    mail::ReadError,
};

/// Most UIDs one command names.
pub const UID_CHUNK: usize = 500;

/// The reason a move the server cannot make safely (neither MOVE nor
/// UIDPLUS, or another message of the folder already `\Deleted`) ends with.
pub const UNSUPPORTED_HERE: &str = "unsupported_here";

/// The prefix of a restore token: the folder a trashed copy was in.
const FROM: &str = "from:";

/// Most UIDs a `UID SEARCH DELETED` answer is read into.
const MAX_DELETED: usize = 100_000;

/// Whether a change may put on or take off `token` at an IMAP server: read,
/// starred, the inbox, the trash, spam and stream marks only.
pub fn writable(token: &str) -> bool {
    matches!(token, "UNREAD" | "STARRED" | "INBOX" | "TRASH" | "SPAM")
        || marks::of_token(token).is_some()
}

/// What stopped a push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// The server said it is busy (`UNAVAILABLE`, `INUSE`, `LIMIT`).
    Busy,
    /// The session was cut short, and must not be used again.
    Wire(WireError),
    /// The server did something it was not asked to (a stable code): the
    /// push stops there.
    Hostile(&'static str),
}

/// What a push did, for the run to record before it reads.
#[derive(Debug, Default)]
pub struct Pushed {
    /// How each change ended, and the index tokens written messages have.
    pub push: Push,
    /// The locations the writes changed, in order.
    pub locations: Vec<LocationOp>,
    /// The cursor after the push: folders it created, `recheck` when a moved
    /// copy's new UID is not known.
    pub cursor: Cursor,
    /// What stopped it, if anything did.
    pub trouble: Option<Trouble>,
}

/// One copy of a message, as the stream recorded it.
#[derive(Debug, Clone)]
struct Located {
    place: Place,
    location: Location,
    role: Role,
}

/// A location row: key, folder, UIDVALIDITY, UID, seen, flagged, EMAILID,
/// fingerprint.
type LocationRow = (
    String,
    String,
    i64,
    i64,
    bool,
    bool,
    Option<String>,
    Option<String>,
);

/// What the server offers that writing needs.
#[derive(Debug, Clone, Copy, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each is one capability the server may offer"
)]
struct Caps {
    moves: bool,
    uidplus: bool,
    condstore: bool,
    unselect: bool,
}

impl Caps {
    fn of(capabilities: &[String]) -> Self {
        let has = |name: &str| capabilities.iter().any(|c| c == name);
        Self {
            moves: has("MOVE"),
            uidplus: has("UIDPLUS"),
            condstore: has("CONDSTORE") || has("QRESYNC"),
            unselect: has("UNSELECT"),
        }
    }
}

/// One flag write: on (`+FLAGS`) or off, and the flags it names.
type FlagWrite = (bool, &'static str);

/// Where a mailbox's places are, by folder name, from the cursor.
#[derive(Debug, Default)]
struct Places {
    inbox: Option<String>,
    sent: Option<String>,
    junk: Option<String>,
    trash: Option<String>,
    archive: Option<String>,
    streams: BTreeMap<ProviderStream, String>,
    /// Every folder read, with its role.
    roles: HashMap<String, Role>,
}

impl Places {
    fn of(cursor: &Cursor) -> Self {
        let name = |role: Role| cursor.folder_of(role).map(|f| f.name.clone());
        let mut streams = BTreeMap::new();
        for folder in cursor.folders.iter().filter(|f| f.role == Role::Stream) {
            if let Some(stream) = folders::stream_of(&folder.display) {
                streams.entry(stream).or_insert_with(|| folder.name.clone());
            }
        }
        Self {
            inbox: name(Role::Inbox),
            sent: name(Role::Sent),
            junk: name(Role::Junk),
            trash: name(Role::Trash),
            archive: name(Role::Archive),
            streams,
            roles: cursor
                .folders
                .iter()
                .map(|f| (f.name.clone(), f.role))
                .collect(),
        }
    }

    /// The folder a restore token names, while it is read and not a bin.
    fn restored(&self, token: &str) -> Option<String> {
        let folder = token.strip_prefix(FROM)?;
        self.roles
            .get(folder)
            .filter(|role| !matches!(role, Role::Trash | Role::Junk))
            .map(|_| folder.to_owned())
    }
}

/// What one message's change asks of the server.
#[derive(Debug, Default, PartialEq, Eq)]
struct Ops {
    /// Flag writes, per copy.
    stores: Vec<(Place, FlagWrite)>,
    /// Copies to move, and where to.
    moves: Vec<(Place, String)>,
    /// Copies to copy, and where to.
    copies: Vec<(Place, String)>,
    /// Where the copies a trash change moves were (`from:<folder>`), for
    /// untrash to put them back.
    origins: Vec<String>,
    /// Part of it has nowhere to go (a place the reader does not track, or
    /// no copy to move): that part is not made.
    unmovable: bool,
    /// A move the server cannot make (neither MOVE nor UIDPLUS).
    unsupported: bool,
}

/// The flag writes that set `seen` and `flagged` (`None`: left as they are),
/// one command when both go the same way.
fn flag_writes(seen: Option<bool>, flagged: Option<bool>) -> Vec<FlagWrite> {
    match (seen, flagged) {
        (Some(a), Some(b)) if a == b => vec![(a, "\\Seen \\Flagged")],
        _ => seen
            .map(|on| (on, "\\Seen"))
            .into_iter()
            .chain(flagged.map(|on| (on, "\\Flagged")))
            .collect(),
    }
}

/// Whether a copy in a folder of `role` is in the inbox.
const fn in_inbox(role: Role) -> bool {
    matches!(role, Role::Inbox | Role::Stream)
}

/// What `change` (tokens to put on and take off) asks of the server for a
/// message whose copies are `held`, in a mailbox whose places are `places`
/// (`can_move`: the server offers MOVE or UIDPLUS; `restore`: where its
/// copies were before Campbooks put it in the trash, as bound).
#[allow(
    clippy::too_many_lines,
    reason = "one table of intents to moves, read top to bottom"
)]
fn ops(
    change: &LabelChange,
    held: &[Located],
    places: &Places,
    can_move: bool,
    restore: Option<&[String]>,
) -> Ops {
    let (add, remove) = change;
    let adds = |token: &str| add.iter().any(|l| l == token);
    let removes = |token: &str| remove.iter().any(|l| l == token);
    let mut ops = Ops::default();
    let seen = if adds("UNREAD") {
        Some(false)
    } else if removes("UNREAD") {
        Some(true)
    } else {
        None
    };
    let flagged = if adds("STARRED") {
        Some(true)
    } else if removes("STARRED") {
        Some(false)
    } else {
        None
    };
    for write in flag_writes(seen, flagged) {
        for copy in held {
            ops.stores.push((copy.place.clone(), write));
        }
    }
    let stream = add.iter().find_map(|t| marks::of_token(t));
    let stream_move = stream.is_some() || remove.iter().any(|t| marks::of_token(t).is_some());
    // Where a copy going to the inbox goes: its stream's folder, or the
    // Inbox.
    let inbox_place = match stream {
        Some(stream) => places.streams.get(&stream).cloned(),
        None => places.inbox.clone(),
    };
    let any = |f: &dyn Fn(Role) -> bool| held.iter().any(|c| f(c.role));
    let mut moving: Vec<(&Located, Option<String>)> = Vec::new();
    let mut copying: Vec<(&Located, Option<String>)> = Vec::new();
    let bin_out = [("TRASH", Role::Trash), ("SPAM", Role::Junk)]
        .into_iter()
        .find(|(token, role)| removes(token) && any(&|r| r == *role))
        .map(|(_, role)| role);
    if adds("TRASH") || adds("SPAM") {
        let trash = adds("TRASH");
        let (role, other, to) = if trash {
            (Role::Trash, Role::Junk, &places.trash)
        } else {
            (Role::Junk, Role::Trash, &places.junk)
        };
        // The inbox, Archive and the other bin; never the member's own
        // folders or Sent.
        let binned: Vec<&Located> = held
            .iter()
            .filter(|c| c.role != role)
            .filter(|c| in_inbox(c.role) || matches!(c.role, Role::Archive) || c.role == other)
            .collect();
        if binned.is_empty() && !held.iter().any(|c| c.role == role) {
            ops.unmovable = true;
        }
        if trash {
            ops.origins = binned
                .iter()
                .map(|c| format!("{FROM}{}", c.place.folder))
                .collect();
        }
        moving.extend(binned.into_iter().map(|c| (c, to.clone())));
    } else if let Some(bin) = bin_out {
        let fallback = if adds("INBOX") {
            inbox_place.clone()
        } else {
            places.sent.clone().or_else(|| places.archive.clone())
        };
        let mut back = restore
            .unwrap_or_default()
            .iter()
            .filter_map(|token| places.restored(token));
        for copy in held.iter().filter(|c| c.role == bin) {
            let to = back.next().or_else(|| fallback.clone());
            moving.push((copy, to));
        }
    } else if removes("INBOX") && any(&in_inbox) {
        moving.extend(
            held.iter()
                .filter(|c| in_inbox(c.role))
                .map(|c| (c, places.archive.clone())),
        );
    } else if adds("INBOX") && !any(&in_inbox) {
        let archived: Vec<&Located> = held.iter().filter(|c| c.role == Role::Archive).collect();
        if archived.is_empty() {
            match held.iter().find(|c| c.role == Role::User) {
                Some(copy) => copying.push((copy, inbox_place.clone())),
                None => ops.unmovable = true,
            }
        } else {
            moving.extend(archived.into_iter().map(|c| (c, inbox_place.clone())));
        }
    } else if stream_move {
        if any(&in_inbox) {
            moving.extend(
                held.iter()
                    .filter(|c| in_inbox(c.role))
                    .map(|c| (c, inbox_place.clone())),
            );
        } else {
            // Archived (or sent): no place in a stream to move it to.
            ops.unmovable = true;
        }
    }
    for (copy, to) in moving {
        match to {
            None => ops.unmovable = true,
            Some(to) if to == copy.place.folder => {}
            Some(_) if !can_move => ops.unsupported = true,
            Some(to) => ops.moves.push((copy.place.clone(), to)),
        }
    }
    for (copy, to) in copying {
        match to {
            None => ops.unmovable = true,
            Some(to) if to == copy.place.folder => {}
            Some(to) => ops.copies.push((copy.place.clone(), to)),
        }
    }
    ops
}

/// The work of one folder: what to write there, and for which message.
#[derive(Debug, Default)]
struct FolderWork {
    /// The copies' UIDVALIDITY as recorded.
    uidvalidity: u32,
    /// Each UID's message.
    messages: BTreeMap<u32, String>,
    stores: BTreeMap<FlagWrite, BTreeSet<u32>>,
    moves: BTreeMap<String, BTreeSet<u32>>,
    copies: BTreeMap<String, BTreeSet<u32>>,
}

/// A copy written elsewhere: from, the folder it went to, its new place
/// when the server said (`COPYUID`), and whether the original is gone.
#[derive(Debug, Clone)]
struct Placed {
    from: Place,
    to: String,
    at: Option<Place>,
    moved: bool,
}

/// What the writes did.
#[derive(Debug, Default)]
struct Wrote {
    /// Flag writes the server took, per copy.
    stored: Vec<(Place, FlagWrite)>,
    /// Copies written to another folder.
    placed: Vec<Placed>,
    /// Copies the server no longer has.
    gone: HashSet<Place>,
    /// Messages a write of the server refused, or did not make as asked.
    refused: HashSet<String>,
    /// Messages changed at the server since their folder was last read.
    modified: HashSet<String>,
    /// Messages not (all) written: a stale UIDVALIDITY, a folder that would
    /// not open, or the push stopped before them. They wait for a later run.
    left: HashSet<String>,
    /// Messages whose move this server cannot make safely.
    unsupported: HashSet<String>,
    /// Where messages a trash change moved were, per message.
    origins: HashMap<String, Vec<String>>,
    /// Somewhere a copy may be that is not recorded: the next complete read
    /// looks for it.
    recheck: bool,
}

/// Everything a push needs.
pub struct Writer<'a> {
    /// The run's signed-in session.
    pub session: &'a mut Session,
    /// The pool (runtime role), for the stream's locations.
    pub pool: &'a PgPool,
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The stream.
    pub stream: StreamId,
    /// Its cursor (its folders).
    pub cursor: Cursor,
    /// The adapter's settings.
    pub config: &'a ImapConfig,
}

/// The hierarchy a folder is created in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hierarchy {
    /// The server's delimiter, or none (a flat server).
    Known(Option<char>),
    /// The server would not say.
    Unknown,
}

/// The merged plan of `backlog`, flags written whatever the index says (it
/// may be behind the server), and anything of a message with no recorded
/// copy left unsure.
fn planned(backlog: &Backlog, copies: &HashMap<String, Vec<Located>>) -> Plan {
    let mut plan = Plan::merge(backlog);
    let unsure: HashSet<(String, String)> = plan
        .wants
        .iter()
        .flat_map(|(message, labels)| {
            let located = copies.contains_key(message);
            labels
                .keys()
                .filter(move |label| !located || matches!(label.as_str(), "UNREAD" | "STARRED"))
                .map(move |label| (message.clone(), label.clone()))
        })
        .collect();
    plan.in_place(backlog, Some(&unsure));
    plan.atomic();
    plan
}

/// A stand-in name for `stream`'s folder while it does not exist: no
/// server name holds a control character.
fn placeholder(stream: ProviderStream) -> String {
    format!("\u{1}{}", stream.as_str())
}

impl Writer<'_> {
    /// Make `backlog`'s changes. Errors are the database's only: what the
    /// server did is in the result, its trouble too.
    #[allow(
        clippy::too_many_lines,
        reason = "one push read top to bottom: copies, folders, plan, writes, ends"
    )]
    pub async fn push(mut self, backlog: &Backlog) -> Result<Pushed, ReadError> {
        let mut pushed = Pushed::default();
        let mut keys: Vec<String> = backlog
            .pending
            .iter()
            .map(|p| p.provider_message_id.clone())
            .collect();
        keys.sort();
        keys.dedup();
        let copies = self.located(&keys).await?;
        let limits = self.small();
        let caps = match self.session.capabilities(limits).await {
            Ok(capabilities) => Caps::of(&capabilities),
            Err(err) => return Ok(self.stopped(backlog, pushed, Trouble::Wire(err))),
        };
        // The plan over what the index shows, the index's tokens seen as
        // flags, the inbox, the bins and stream marks.
        let mut viewed = backlog.clone();
        viewed.labels = backlog
            .labels
            .iter()
            .map(|(key, tokens)| (key.clone(), state::view(tokens, &self.cursor)))
            .collect();
        let can_move = caps.moves || caps.uidplus;
        // The stream folders a move the plan makes needs and the server
        // does not have: created first, and only those.
        let needed = self.needed(&viewed, &copies, can_move);
        let unmade = match self.make_folders(needed).await {
            Ok(unmade) => unmade,
            Err(trouble) => return Ok(self.stopped(backlog, pushed, trouble)),
        };
        let kept = without_marks(&viewed, &unmade, &mut pushed.push.outcome);
        let mut plan = planned(&kept, &copies);

        let places = Places::of(&self.cursor);
        let mut work: BTreeMap<String, FolderWork> = BTreeMap::new();
        let mut asked: HashMap<String, Ops> = HashMap::new();
        for (change, messages) in plan.groups() {
            for message in messages {
                let Some(held) = copies.get(&message) else {
                    // No copy recorded (yet): it waits for the read.
                    continue;
                };
                let restore = kept.restore.get(&message).map(Vec::as_slice);
                let wanted = ops(&change, held, &places, can_move, restore);
                register(&mut work, &wanted, &message);
                asked.insert(message, wanted);
            }
        }
        let mut wrote = Wrote::default();
        let mut trouble = None;
        let mut selected: Option<&String> = None;
        for (folder, todo) in &work {
            if trouble.is_some() {
                wrote.left.extend(todo.messages.values().cloned());
                continue;
            }
            selected = Some(folder);
            if let Err(stop) = self.write_folder(folder, todo, caps, &mut wrote).await {
                wrote.left.extend(todo.messages.values().cloned());
                trouble = Some(stop);
            }
        }
        if let (Some(folder), None) = (selected, trouble) {
            // Leave no folder selected, expunging nothing (never `CLOSE`).
            if let Err(stop) = self.deselect(folder, caps).await {
                trouble = Some(stop);
            }
        }
        self.foreign(&copies, &mut wrote).await?;
        self.settle(
            backlog,
            &kept,
            &mut plan,
            &copies,
            &asked,
            &wrote,
            &mut pushed,
        );
        pushed.trouble = trouble;
        pushed.cursor = self.cursor;
        pushed.cursor.login_refusals = 0;
        tracing::info!(
            workspace = %self.workspace,
            stream = %self.stream,
            changes = backlog.pending.len(),
            folders = work.len(),
            gone = wrote.gone.len(),
            refused = wrote.refused.len(),
            modified = wrote.modified.len(),
            left = wrote.left.len(),
            trouble = trouble.map_or("", Trouble::code),
            "IMAP outbound changes made"
        );
        Ok(pushed)
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

    /// Bounds of a command over `uids` UIDs (a move answers an `EXPUNGE`
    /// per message).
    fn over(&self, uids: usize) -> Limits {
        Limits {
            response: 1024 * 1024,
            responses: uids.saturating_mul(3).saturating_add(100),
            total: 8 * 1024 * 1024,
            time: self.timeout() * 2,
        }
    }

    /// Send `command` within `limits`; a cut session is trouble.
    async fn command(&mut self, command: &str, limits: Limits) -> Result<Done, Trouble> {
        self.session
            .command(command, limits, |_| {})
            .await
            .map_err(Trouble::Wire)
    }

    /// `backlog` given back after `trouble` stopped the push before any
    /// write: every change pending again (or given up).
    fn stopped(self, backlog: &Backlog, mut pushed: Pushed, trouble: Trouble) -> Pushed {
        for change in &backlog.pending {
            let end = retry_or_give_up(
                change.id,
                change.attempts,
                &mut pushed.push.outcome.reemit,
                &change.provider_message_id,
            );
            pushed.push.outcome.ends.push(end);
        }
        pushed.push.outcome.reemit.sort();
        pushed.push.outcome.reemit.dedup();
        pushed.trouble = Some(trouble);
        pushed.cursor = self.cursor;
        pushed
    }

    /// The copies the stream recorded of `keys`, in the folders it reads at
    /// their UIDVALIDITY as read: only those are ever written.
    async fn located(&self, keys: &[String]) -> Result<HashMap<String, Vec<Located>>, ReadError> {
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<LocationRow> = sqlx::query_as(
            "SELECT message_key, folder, uidvalidity, uid, seen, flagged, email_id, fingerprint
               FROM connectors.imap_locations
              WHERE stream_id = $1 AND message_key = ANY($2)
              ORDER BY folder, uid",
        )
        .bind(self.stream)
        .bind(keys)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut copies: HashMap<String, Vec<Located>> = HashMap::new();
        for (key, folder, uidvalidity, uid, seen, flagged, email_id, fingerprint) in rows {
            let (Ok(uidvalidity), Ok(uid)) = (u32::try_from(uidvalidity), u32::try_from(uid))
            else {
                continue;
            };
            let Some(read) = self.cursor.folders.iter().find(|f| f.name == folder) else {
                continue;
            };
            if read.uidvalidity != Some(uidvalidity) {
                continue;
            }
            let role = read.role;
            copies.entry(key.clone()).or_default().push(Located {
                place: Place {
                    folder,
                    uidvalidity,
                    uid,
                },
                location: Location {
                    key,
                    seen,
                    flagged,
                    email_id,
                    fingerprint,
                },
                role,
            });
        }
        Ok(copies)
    }

    /// The streams whose folder a move of `viewed`'s plan would put a copy
    /// in and the server does not have yet.
    fn needed(
        &self,
        viewed: &Backlog,
        copies: &HashMap<String, Vec<Located>>,
        can_move: bool,
    ) -> BTreeSet<ProviderStream> {
        let mut places = Places::of(&self.cursor);
        let missing: Vec<ProviderStream> = ProviderStream::ALL
            .into_iter()
            .filter(|stream| self.cursor.stream_folder(*stream).is_none())
            .collect();
        for stream in &missing {
            places.streams.insert(*stream, placeholder(*stream));
        }
        let mut needed = BTreeSet::new();
        for (change, messages) in planned(viewed, copies).groups() {
            for message in messages {
                let Some(held) = copies.get(&message) else {
                    continue;
                };
                let restore = viewed.restore.get(&message).map(Vec::as_slice);
                let wanted = ops(&change, held, &places, can_move, restore);
                for (_, to) in wanted.moves.iter().chain(&wanted.copies) {
                    if let Some(stream) = missing.iter().find(|s| placeholder(**s) == *to) {
                        needed.insert(*stream);
                    }
                }
            }
        }
        needed
    }

    /// Make the stream folders of `wanted` (each into the cursor). The
    /// streams whose folder could not be made, with why.
    async fn make_folders(
        &mut self,
        wanted: BTreeSet<ProviderStream>,
    ) -> Result<BTreeMap<ProviderStream, &'static str>, Trouble> {
        let mut unmade = BTreeMap::new();
        if wanted.is_empty() {
            return Ok(unmade);
        }
        let hierarchy = self.hierarchy().await?;
        let Hierarchy::Known(delimiter) = hierarchy else {
            for stream in wanted {
                unmade.insert(stream, "refused");
            }
            return Ok(unmade);
        };
        let parent = inbox_parent(&self.cursor, delimiter);
        let max = usize::try_from(self.config.max_created_folders).unwrap_or(usize::MAX);
        for stream in wanted {
            let parts: Vec<&str> = marks::name(stream).split('/').collect();
            let Some(name) = folders::created_name(&parts, delimiter, parent.as_deref()) else {
                tracing::warn!(stream = %self.stream, "a stream folder's name was refused");
                unmade.insert(stream, "folder_name_refused");
                continue;
            };
            let again = self.cursor.created.contains(&name);
            if !again && self.cursor.created.len() >= max {
                tracing::warn!(
                    stream = %self.stream,
                    created = self.cursor.created.len(),
                    "the mailbox has as many folders from Campbooks as it may"
                );
                unmade.insert(stream, "folder_cap");
                continue;
            }
            let Some(created) = self.create(&name).await? else {
                unmade.insert(stream, "refused");
                continue;
            };
            if created && !again {
                self.cursor.created.push(name.clone());
            }
            if !self.cursor.folders.iter().any(|f| f.name == name) {
                self.cursor.folders.push(Folder::new(Tracked {
                    display: folders::display(&name, delimiter),
                    name,
                    role: Role::Stream,
                }));
            }
        }
        Ok(unmade)
    }

    /// The server's hierarchy delimiter (`LIST "" ""`).
    async fn hierarchy(&mut self) -> Result<Hierarchy, Trouble> {
        let mut found = None;
        let limits = self.small();
        let done = self
            .session
            .command(r#"LIST "" """#, limits, |untagged| {
                if let Untagged::List(entry) = untagged {
                    found.get_or_insert(entry.delimiter);
                }
            })
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        Ok(match found.filter(|_| done.is_ok()) {
            Some(delimiter) => Hierarchy::Known(delimiter),
            None => Hierarchy::Unknown,
        })
    }

    /// Create folder `name` and subscribe to it. `Some(true)` when it was
    /// created, `Some(false)` when the server already had it, `None` when it
    /// refused.
    async fn create(&mut self, name: &str) -> Result<Option<bool>, Trouble> {
        let Some(quoted) = quoted(name) else {
            return Ok(None);
        };
        let limits = self.small();
        let done = self.command(&format!("CREATE {quoted}"), limits).await?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        let existed = !done.is_ok() && code_is(done.code.as_ref(), "ALREADYEXISTS");
        if !done.is_ok() && !existed {
            tracing::info!(stream = %self.stream, "the IMAP server refused to create a stream folder");
            return Ok(None);
        }
        // So other mail apps show it; whatever the answer.
        let subscribed = self.command(&format!("SUBSCRIBE {quoted}"), limits).await?;
        if subscribed.is_busy() {
            return Err(Trouble::Busy);
        }
        Ok(Some(!existed))
    }

    /// Leave no folder selected, expunging nothing: `UNSELECT` (RFC 3691)
    /// where offered, else `EXAMINE` of the folder last selected, which
    /// closes it read-write without expunging. Never `CLOSE`.
    async fn deselect(&mut self, folder: &str, caps: Caps) -> Result<(), Trouble> {
        let limits = self.small();
        let command = if caps.unselect {
            "UNSELECT".to_owned()
        } else {
            let Some(name) = quoted(folder) else {
                return Ok(());
            };
            format!("EXAMINE {name}")
        };
        let done = self.command(&command, limits).await?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        Ok(())
    }

    /// Make `todo` in `folder`: open it read-write, check its UIDVALIDITY,
    /// find which UIDs are still there, write flags, then move and copy.
    #[allow(
        clippy::too_many_lines,
        reason = "one folder read top to bottom: open, check, flags, moves, copies"
    )]
    async fn write_folder(
        &mut self,
        folder: &str,
        todo: &FolderWork,
        caps: Caps,
        wrote: &mut Wrote,
    ) -> Result<(), Trouble> {
        let leave = |wrote: &mut Wrote| wrote.left.extend(todo.messages.values().cloned());
        let Some(name) = quoted(folder) else {
            leave(wrote);
            return Ok(());
        };
        let condstore = if caps.condstore { " (CONDSTORE)" } else { "" };
        let mut uidvalidity = None;
        let mut nomodseq = false;
        let limits = self.small();
        let done = self
            .session
            .command(&format!("SELECT {name}{condstore}"), limits, |untagged| {
                if let Untagged::Status {
                    code: Some(code), ..
                } = untagged
                {
                    match code.name.as_str() {
                        "UIDVALIDITY" => {
                            uidvalidity = code.number.and_then(|n| u32::try_from(n).ok());
                        }
                        "NOMODSEQ" => nomodseq = true,
                        _ => {}
                    }
                }
            })
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        if !done.is_ok() {
            tracing::info!(stream = %self.stream, "an IMAP folder would not open for a write");
            leave(wrote);
            return Ok(());
        }
        if code_is(done.code.as_ref(), "READ-ONLY") {
            wrote.refused.extend(todo.messages.values().cloned());
            return Ok(());
        }
        if uidvalidity != Some(todo.uidvalidity) {
            tracing::info!(
                stream = %self.stream,
                "an IMAP folder's UIDVALIDITY changed since it was read: nothing is written there"
            );
            leave(wrote);
            return Ok(());
        }
        let place = |uid: u32| Place {
            folder: folder.to_owned(),
            uidvalidity: todo.uidvalidity,
            uid,
        };
        // What the server still has: a UID it no longer has is no-op.
        let all: Vec<u32> = todo.messages.keys().copied().collect();
        let present = self.present(&all).await?;
        for uid in all.iter().filter(|uid| !present.contains(*uid)) {
            wrote.gone.insert(place(*uid));
        }
        let modseq = self
            .cursor
            .folders
            .iter()
            .find(|f| f.name == folder)
            .and_then(|f| f.highestmodseq)
            .filter(|_| caps.condstore && !nomodseq);
        let mut checked: HashSet<u32> = HashSet::new();
        let open = |uid: &u32, wrote: &Wrote| {
            present.contains(uid)
                && todo
                    .messages
                    .get(uid)
                    .is_some_and(|m| !wrote.refused.contains(m) && !wrote.modified.contains(m))
        };
        for (write, uids) in &todo.stores {
            let uids: Vec<u32> = uids.iter().copied().filter(|u| open(u, &*wrote)).collect();
            for chunk in uids.chunks(UID_CHUNK) {
                // A UID checked once in this push is not checked again: its
                // own first write moved its modseq.
                let (unchecked, again): (Vec<u32>, Vec<u32>) = chunk
                    .iter()
                    .copied()
                    .partition(|uid| !checked.contains(uid));
                for (uids, conditional) in [(unchecked, true), (again, false)] {
                    if uids.is_empty() {
                        continue;
                    }
                    let condition = modseq
                        .filter(|_| conditional)
                        .map(|m| format!(" (UNCHANGEDSINCE {m})"))
                        .unwrap_or_default();
                    let sign = if write.0 { '+' } else { '-' };
                    let command = format!(
                        "UID STORE {}{condition} {sign}FLAGS.SILENT ({})",
                        sequence(&uids),
                        write.1
                    );
                    let limits = self.over(uids.len());
                    let done = self.command(&command, limits).await?;
                    if done.is_busy() {
                        return Err(Trouble::Busy);
                    }
                    if !done.is_ok() {
                        refuse(wrote, todo, &uids);
                        continue;
                    }
                    let modified: HashSet<u32> = done
                        .code
                        .as_ref()
                        .filter(|code| code.name == "MODIFIED")
                        .and_then(|code| code.sets.first())
                        .map(|set| set.iter().copied().collect())
                        .unwrap_or_default();
                    let (overtaken, taken): (Vec<u32>, Vec<u32>) =
                        uids.into_iter().partition(|uid| modified.contains(uid));
                    for uid in overtaken {
                        if let Some(message) = todo.messages.get(&uid) {
                            wrote.modified.insert(message.clone());
                        }
                    }
                    // What the server holds now: a flag it does not show as
                    // written (or will not show at all) was not made, and is
                    // never taken as made.
                    let now = self.flags_now(&taken).await?;
                    for uid in taken {
                        let holds = now.get(&uid).is_some_and(|flags| made(*write, *flags));
                        if holds {
                            checked.insert(uid);
                            wrote.stored.push((place(uid), *write));
                        } else if let Some(message) = todo.messages.get(&uid) {
                            tracing::info!(
                                stream = %self.stream,
                                "an IMAP server did not show a flag change as made"
                            );
                            wrote.refused.insert(message.clone());
                        }
                    }
                }
            }
        }
        for (to, uids) in &todo.moves {
            let uids: Vec<u32> = uids.iter().copied().filter(|u| open(u, &*wrote)).collect();
            for chunk in uids.chunks(UID_CHUNK) {
                self.move_to(chunk, to, caps, todo, &place, wrote).await?;
            }
        }
        for (to, uids) in &todo.copies {
            let uids: Vec<u32> = uids.iter().copied().filter(|u| open(u, &*wrote)).collect();
            for chunk in uids.chunks(UID_CHUNK) {
                let Some(dest) = quoted(to) else {
                    refuse(wrote, todo, chunk);
                    continue;
                };
                let mut copied = None;
                let limits = self.over(chunk.len());
                let done = self
                    .session
                    .command(
                        &format!("UID COPY {} {dest}", sequence(chunk)),
                        limits,
                        |untagged| copy_uid(&untagged, &mut copied),
                    )
                    .await
                    .map_err(Trouble::Wire)?;
                if done.is_busy() {
                    return Err(Trouble::Busy);
                }
                if !done.is_ok() {
                    refuse(wrote, todo, chunk);
                    continue;
                }
                let copied = copied.or_else(|| copy_map(done.code.as_ref()));
                placed(wrote, chunk, to, copied.as_ref(), &place, false);
            }
        }
        Ok(())
    }

    /// The flags (`\Seen`, `\Flagged`) of `uids` in the open folder now;
    /// none known when the server will not say.
    async fn flags_now(&mut self, uids: &[u32]) -> Result<HashMap<u32, (bool, bool)>, Trouble> {
        let mut found = HashMap::new();
        if uids.is_empty() {
            return Ok(found);
        }
        let wanted: HashSet<u32> = uids.iter().copied().collect();
        let limits = self.over(uids.len());
        let done = self
            .session
            .command(
                &format!("UID FETCH {} (UID FLAGS)", sequence(uids)),
                limits,
                |untagged| {
                    if let Untagged::Fetch(fetch) = untagged
                        && let (Some(uid), Some(flags)) = (fetch.uid, fetch.flags)
                        && wanted.contains(&uid)
                    {
                        found.insert(uid, flags);
                    }
                },
            )
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        if !done.is_ok() {
            found.clear();
        }
        Ok(found)
    }

    /// Which of `uids` the open folder still has (`UID SEARCH UID`).
    async fn present(&mut self, uids: &[u32]) -> Result<HashSet<u32>, Trouble> {
        let mut found = HashSet::new();
        for chunk in uids.chunks(UID_CHUNK) {
            let wanted: HashSet<u32> = chunk.iter().copied().collect();
            let limits = self.over(chunk.len());
            let done = self
                .session
                .command(
                    &format!("UID SEARCH UID {}", sequence(chunk)),
                    limits,
                    |untagged| {
                        if let Untagged::Search(uids) = untagged {
                            found.extend(uids.into_iter().filter(|uid| wanted.contains(uid)));
                        }
                    },
                )
                .await
                .map_err(Trouble::Wire)?;
            if done.is_busy() {
                return Err(Trouble::Busy);
            }
            if !done.is_ok() {
                // Not believed either way: as if all were there (a write to
                // a UID the server no longer has does nothing).
                found.extend(chunk.iter().copied());
            }
        }
        Ok(found)
    }

    /// The UIDs of the open folder marked `\Deleted` (`UID SEARCH DELETED`);
    /// `None` when the server will not say, or says more than is read.
    async fn deleted(&mut self) -> Result<Option<HashSet<u32>>, Trouble> {
        let mut found = HashSet::new();
        let mut over = false;
        let limits = Limits {
            response: 16 * 1024 * 1024,
            responses: 1_000,
            total: 32 * 1024 * 1024,
            time: self.timeout() * 2,
        };
        let done = self
            .session
            .command("UID SEARCH DELETED", limits, |untagged| {
                if let Untagged::Search(uids) = untagged {
                    if found.len().saturating_add(uids.len()) > MAX_DELETED {
                        over = true;
                    } else {
                        found.extend(uids);
                    }
                }
            })
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        Ok((done.is_ok() && !over).then_some(found))
    }

    /// Move `uids` of the open folder to `to`: `UID MOVE`, or (UIDPLUS)
    /// `UID COPY`, then for the UIDs its `COPYUID` names only, `\Deleted` and
    /// `UID EXPUNGE` of those UIDs. Never while another message of the
    /// folder is `\Deleted`; never `\Deleted` without a `COPYUID` naming the
    /// copy. A failure after the mark takes it off again, from the UIDs this
    /// push marked only: the copies stay, nothing is lost.
    #[allow(
        clippy::too_many_lines,
        reason = "one move read top to bottom: guard, copy, mark, expunge, undo"
    )]
    async fn move_to(
        &mut self,
        uids: &[u32],
        to: &str,
        caps: Caps,
        todo: &FolderWork,
        place: &impl Fn(u32) -> Place,
        wrote: &mut Wrote,
    ) -> Result<(), Trouble> {
        let Some(dest) = quoted(to) else {
            refuse(wrote, todo, uids);
            return Ok(());
        };
        let limits = self.over(uids.len());
        if caps.moves {
            let mut copied = None;
            let mut expunged = 0_usize;
            let done = self
                .session
                .command(
                    &format!("UID MOVE {} {dest}", sequence(uids)),
                    limits,
                    |untagged| {
                        expunged = expunged.saturating_add(expunges(&untagged));
                        copy_uid(&untagged, &mut copied);
                    },
                )
                .await
                .map_err(Trouble::Wire)?;
            if done.is_busy() {
                return Err(Trouble::Busy);
            }
            if !done.is_ok() {
                refuse(wrote, todo, uids);
                return Ok(());
            }
            let copied = copied.or_else(|| copy_map(done.code.as_ref()));
            placed(wrote, uids, to, copied.as_ref(), place, true);
            return guard_expunged(expunged, uids.len());
        }
        // UIDPLUS. Another message of the folder already `\Deleted` (the
        // member's, in another app) would go with ours on a server that
        // expunges more than it is asked: no move here.
        let Some(deleted) = self.deleted().await? else {
            unsupported(wrote, todo, uids);
            return Ok(());
        };
        let ours: HashSet<u32> = uids.iter().copied().collect();
        if deleted.iter().any(|uid| !ours.contains(uid)) {
            tracing::info!(
                stream = %self.stream,
                "an IMAP folder holds messages marked deleted elsewhere: no copy-and-expunge move there"
            );
            unsupported(wrote, todo, uids);
            return Ok(());
        }
        let mut copied = None;
        let done = self
            .session
            .command(
                &format!("UID COPY {} {dest}", sequence(uids)),
                limits,
                |untagged| copy_uid(&untagged, &mut copied),
            )
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        if !done.is_ok() {
            refuse(wrote, todo, uids);
            return Ok(());
        }
        let copied = copied.or_else(|| copy_map(done.code.as_ref()));
        // Only what the server says it copied is ever marked: no `COPYUID`
        // (which a UIDPLUS server owes), or a UID it does not name, is a
        // copy that may not exist.
        let named: HashSet<u32> = copied
            .as_ref()
            .map(|(_, map)| map.keys().copied().collect())
            .unwrap_or_default();
        let (covered, unnamed): (Vec<u32>, Vec<u32>) =
            uids.iter().copied().partition(|uid| named.contains(uid));
        if !unnamed.is_empty() {
            tracing::warn!(
                stream = %self.stream,
                unnamed = unnamed.len(),
                "an IMAP server copied without naming every copy: the originals are kept"
            );
            placed(wrote, &unnamed, to, None, place, false);
            refuse(wrote, todo, &unnamed);
            wrote.recheck = true;
        }
        if covered.is_empty() {
            return Ok(());
        }
        let set = sequence(&covered);
        let marked = self
            .command(
                &format!("UID STORE {set} +FLAGS.SILENT (\\Deleted)"),
                limits,
            )
            .await?;
        let mut expunged = 0_usize;
        let swept = if marked.is_ok() {
            let done = self
                .session
                .command(&format!("UID EXPUNGE {set}"), limits, |untagged| {
                    expunged = expunged.saturating_add(expunges(&untagged));
                })
                .await
                .map_err(Trouble::Wire)?;
            Some(done)
        } else {
            None
        };
        if swept.as_ref().is_some_and(Done::is_ok) {
            placed(wrote, &covered, to, copied.as_ref(), place, true);
            return guard_expunged(expunged, covered.len());
        }
        // The copies are made; the originals stay as they were: the mark
        // comes off the UIDs this push marked, never one marked before.
        let undo: Vec<u32> = covered
            .iter()
            .copied()
            .filter(|uid| !deleted.contains(uid))
            .collect();
        if !undo.is_empty() {
            let _ = self
                .command(
                    &format!("UID STORE {} -FLAGS.SILENT (\\Deleted)", sequence(&undo)),
                    limits,
                )
                .await?;
        }
        placed(wrote, &covered, to, copied.as_ref(), place, false);
        refuse(wrote, todo, &covered);
        if marked.is_busy() || swept.as_ref().is_some_and(Done::is_busy) {
            return Err(Trouble::Busy);
        }
        Ok(())
    }

    /// Forget a moved copy's new place when it is already recorded for
    /// another message, or named twice (a `COPYUID` that would overwrite a
    /// location): the next complete read looks for it.
    async fn foreign(
        &self,
        copies: &HashMap<String, Vec<Located>>,
        wrote: &mut Wrote,
    ) -> Result<(), ReadError> {
        let key_of: HashMap<&Place, &str> = copies
            .values()
            .flatten()
            .map(|copy| (&copy.place, copy.location.key.as_str()))
            .collect();
        let mut named: HashMap<Place, usize> = HashMap::new();
        for at in wrote.placed.iter().filter_map(|p| p.at.as_ref()) {
            *named.entry(at.clone()).or_default() += 1;
        }
        if named.is_empty() {
            return Ok(());
        }
        let folders: Vec<String> = named.keys().map(|p| p.folder.clone()).collect();
        let uids: Vec<i64> = named.keys().map(|p| i64::from(p.uid)).collect();
        let mut tx = self.pool.begin_for(self.workspace).await?;
        let rows: Vec<(String, i64, i64, String)> = sqlx::query_as(
            "SELECT folder, uidvalidity, uid, message_key FROM connectors.imap_locations
              WHERE stream_id = $1 AND folder = ANY($2) AND uid = ANY($3)",
        )
        .bind(self.stream)
        .bind(&folders)
        .bind(&uids)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let held: HashMap<Place, String> = rows
            .into_iter()
            .filter_map(|(folder, uidvalidity, uid, key)| {
                Some((
                    Place {
                        folder,
                        uidvalidity: u32::try_from(uidvalidity).ok()?,
                        uid: u32::try_from(uid).ok()?,
                    },
                    key,
                ))
            })
            .collect();
        let mut dropped = 0_usize;
        for moved in &mut wrote.placed {
            let Some(at) = moved.at.as_ref() else {
                continue;
            };
            let ours = key_of.get(&moved.from).copied();
            let taken = held.get(at).is_some_and(|key| Some(key.as_str()) != ours);
            if taken || named.get(at).is_some_and(|n| *n > 1) {
                moved.at = None;
                dropped += 1;
            }
        }
        if dropped > 0 {
            tracing::warn!(
                stream = %self.stream,
                dropped,
                "an IMAP server named new UIDs already taken: they are looked for instead"
            );
            wrote.recheck = true;
        }
        Ok(())
    }

    /// End every change (`plan`, over `kept`), with what was written, and
    /// set the locations, index tokens and cursor that follow.
    #[allow(
        clippy::too_many_arguments,
        reason = "one call site; the values are distinct"
    )]
    fn settle(
        &mut self,
        backlog: &Backlog,
        kept: &Backlog,
        plan: &mut Plan,
        copies: &HashMap<String, Vec<Located>>,
        asked: &HashMap<String, Ops>,
        wrote: &Wrote,
        pushed: &mut Pushed,
    ) {
        let mut done = Results::default();
        let mut unsupported = HashSet::new();
        let mut nowhere = HashSet::new();
        for (message, wanted) in asked {
            let held = copies.get(message).map(Vec::as_slice).unwrap_or_default();
            if wrote.modified.contains(message) {
                // Changed at the server since it was read: the server's state
                // stands, the change is overtaken whole.
                if let Some(labels) = plan.wants.get(message) {
                    let pairs: Vec<(String, String)> = labels
                        .keys()
                        .map(|label| (message.clone(), label.clone()))
                        .collect();
                    for pair in pairs {
                        plan.fates.insert(pair, Fate::Overtaken);
                    }
                }
            } else if wrote.refused.contains(message) {
                done.refused.insert(message.clone());
            } else if wrote.left.contains(message) {
                // Waits for a later run.
            } else if !held.is_empty() && held.iter().all(|c| wrote.gone.contains(&c.place)) {
                done.gone.insert(message.clone());
            } else {
                done.written.insert(message.clone());
                if !wanted.origins.is_empty() {
                    done.restore.insert(message.clone(), wanted.origins.clone());
                }
            }
            if wanted.unsupported || wrote.unsupported.contains(message) {
                unsupported.insert(message.clone());
                done.unmovable.insert(message.clone());
            } else if wanted.unmovable {
                nowhere.insert(message.clone());
                done.unmovable.insert(message.clone());
            }
        }
        let outcome = &mut pushed.push.outcome;
        plan.ends(kept, &done, outcome);
        plan.restores(&done, outcome);
        // A change that only moves the message, and could not be: why.
        let place_only = |labels: &[String]| {
            labels.iter().all(|t| {
                matches!(t.as_str(), "INBOX" | "TRASH" | "SPAM") || marks::of_token(t).is_some()
            })
        };
        for change in &kept.pending {
            let message = &change.provider_message_id;
            let labels: Vec<String> = change.add.iter().chain(&change.remove).cloned().collect();
            let reason = if unsupported.contains(message) && place_only(&labels) {
                UNSUPPORTED_HERE
            } else if nowhere.contains(message) && place_only(&labels) {
                "not_movable"
            } else {
                continue;
            };
            if let Some(end) = outcome.ends.iter_mut().find(|e| e.id == change.id)
                && (end.state == State::Applied
                    || (end.state == State::Skipped && end.reason == Some("not_movable")))
            {
                end.state = State::Skipped;
                end.reason = Some(reason);
                if reason == UNSUPPORTED_HERE {
                    outcome.reemit.push(message.clone());
                }
                // Nothing moved: no place to put it back is kept.
                outcome.restore.retain(|(row, _)| *row != change.id);
            }
        }
        if wrote.recheck {
            self.cursor.recheck = true;
        }
        self.locations(backlog, copies, wrote, pushed);
        let outcome = &mut pushed.push.outcome;
        outcome.index.sort();
        outcome.reemit.sort();
        outcome.reemit.dedup();
    }

    /// The locations `wrote` changed, and the index tokens of each message
    /// they belong to as the reader would derive them, into `pushed`; the
    /// cursor's `recheck` when a moved copy's new UID is not known.
    #[allow(
        clippy::too_many_lines,
        reason = "one pass: flags, moves, gone copies, then each message's tokens"
    )]
    fn locations(
        &mut self,
        backlog: &Backlog,
        copies: &HashMap<String, Vec<Located>>,
        wrote: &Wrote,
        pushed: &mut Pushed,
    ) {
        let mut flags: HashMap<Place, (bool, bool)> = HashMap::new();
        let mut changed: BTreeSet<&str> = BTreeSet::new();
        let by_place: HashMap<&Place, &Located> = copies
            .values()
            .flatten()
            .map(|copy| (&copy.place, copy))
            .collect();
        for (place, (on, names)) in &wrote.stored {
            let Some(copy) = by_place.get(place) else {
                continue;
            };
            let entry = flags
                .entry(place.clone())
                .or_insert((copy.location.seen, copy.location.flagged));
            if names.contains("\\Seen") {
                entry.0 = *on;
            }
            if names.contains("\\Flagged") {
                entry.1 = *on;
            }
            changed.insert(copy.location.key.as_str());
        }
        for (place, (seen, flagged)) in &flags {
            if let Some(copy) = by_place.get(place) {
                pushed.locations.push(LocationOp::Put(
                    place.clone(),
                    Location {
                        seen: *seen,
                        flagged: *flagged,
                        ..copy.location.clone()
                    },
                ));
            }
        }
        for moved in &wrote.placed {
            let Some(copy) = by_place.get(&moved.from) else {
                continue;
            };
            changed.insert(copy.location.key.as_str());
            let (seen, flagged) = flags
                .get(&moved.from)
                .copied()
                .unwrap_or((copy.location.seen, copy.location.flagged));
            if moved.moved {
                pushed.locations.push(LocationOp::Drop(moved.from.clone()));
            }
            if let Some(at) = &moved.at {
                pushed.locations.push(LocationOp::Put(
                    at.clone(),
                    Location {
                        seen,
                        flagged,
                        ..copy.location.clone()
                    },
                ));
            } else {
                self.cursor.recheck = true;
            }
        }
        for place in &wrote.gone {
            if let Some(copy) = by_place.get(place) {
                changed.insert(copy.location.key.as_str());
                pushed.locations.push(LocationOp::Drop(place.clone()));
                self.cursor.recheck = true;
            }
        }
        for key in changed {
            let Some(held) = copies.get(key) else {
                continue;
            };
            let mut now: Vec<(Role, String, bool, bool)> = Vec::new();
            for copy in held {
                if wrote.gone.contains(&copy.place) {
                    continue;
                }
                let (seen, flagged) = flags
                    .get(&copy.place)
                    .copied()
                    .unwrap_or((copy.location.seen, copy.location.flagged));
                let mut kept = true;
                for moved in wrote.placed.iter().filter(|m| m.from == copy.place) {
                    kept &= !moved.moved;
                    // Where it went, whether or not its new UID is known.
                    let role = self.cursor.role_of(&moved.to).unwrap_or(Role::User);
                    now.push((role, moved.to.clone(), seen, flagged));
                }
                if kept {
                    now.push((copy.role, copy.place.folder.clone(), seen, flagged));
                }
            }
            let from_self = backlog
                .labels
                .get(key)
                .is_some_and(|tokens| tokens.iter().any(|t| t == state::SELF));
            let tokens = state::tokens(
                now.iter()
                    .map(|(role, folder, seen, flagged)| (*role, folder.as_str(), *seen, *flagged)),
                from_self,
            );
            let mut before = backlog.labels.get(key).cloned().unwrap_or_default();
            before.sort();
            if before != tokens {
                pushed.push.outcome.index.push((key.to_owned(), tokens));
            }
        }
    }
}

/// Add `wanted` (the ops of `message`) to the folders' work.
fn register(work: &mut BTreeMap<String, FolderWork>, wanted: &Ops, message: &str) {
    for (place, write) in &wanted.stores {
        work_of(work, place, message)
            .stores
            .entry(*write)
            .or_default()
            .insert(place.uid);
    }
    for (place, to) in &wanted.moves {
        work_of(work, place, message)
            .moves
            .entry(to.clone())
            .or_default()
            .insert(place.uid);
    }
    for (place, to) in &wanted.copies {
        work_of(work, place, message)
            .copies
            .entry(to.clone())
            .or_default()
            .insert(place.uid);
    }
}

/// The work of `place`'s folder, its UID noted as `message`'s.
fn work_of<'w>(
    work: &'w mut BTreeMap<String, FolderWork>,
    place: &Place,
    message: &str,
) -> &'w mut FolderWork {
    let todo = work.entry(place.folder.clone()).or_default();
    todo.uidvalidity = place.uidvalidity;
    todo.messages.insert(place.uid, message.to_owned());
    todo
}

/// Whether `write` is made on a copy whose flags are now `(seen, flagged)`.
fn made(write: FlagWrite, (seen, flagged): (bool, bool)) -> bool {
    let (on, names) = write;
    (!names.contains("\\Seen") || seen == on) && (!names.contains("\\Flagged") || flagged == on)
}

/// How many messages `untagged` says were expunged (`EXPUNGE`, or the UIDs
/// of a `VANISHED`).
fn expunges(untagged: &Untagged<'_>) -> usize {
    match untagged {
        Untagged::Expunge(_) => 1,
        Untagged::Vanished(ranges) => ranges
            .iter()
            .map(|(low, high)| {
                usize::try_from(high - low)
                    .unwrap_or(usize::MAX)
                    .saturating_add(1)
            })
            .fold(0, usize::saturating_add),
        _ => 0,
    }
}

/// A server that says it expunged more messages than it was asked to is
/// not written to again in this push.
fn guard_expunged(expunged: usize, asked: usize) -> Result<(), Trouble> {
    if expunged > asked {
        tracing::error!(
            code = "imap_expunged_more",
            asked,
            expunged,
            "an IMAP server expunged more messages than it was asked to"
        );
        return Err(Trouble::Hostile("imap_expunged_more"));
    }
    Ok(())
}

/// The messages of `uids` (of `todo`'s folder) refused.
fn refuse(wrote: &mut Wrote, todo: &FolderWork, uids: &[u32]) {
    for uid in uids {
        if let Some(message) = todo.messages.get(uid) {
            wrote.refused.insert(message.clone());
        }
    }
}

/// The messages of `uids` (of `todo`'s folder) this server cannot move
/// safely.
fn unsupported(wrote: &mut Wrote, todo: &FolderWork, uids: &[u32]) {
    for uid in uids {
        if let Some(message) = todo.messages.get(uid) {
            wrote.unsupported.insert(message.clone());
        }
    }
}

/// Record `uids` written to `to` (`moved`: the originals gone), at their new
/// UIDs when `copied` (a `COPYUID` answer) names them.
fn placed(
    wrote: &mut Wrote,
    uids: &[u32],
    to: &str,
    copied: Option<&(u32, HashMap<u32, u32>)>,
    place: &impl Fn(u32) -> Place,
    moved: bool,
) {
    for uid in uids {
        let at = copied.and_then(|(uidvalidity, map)| {
            map.get(uid).map(|new| Place {
                folder: to.to_owned(),
                uidvalidity: *uidvalidity,
                uid: *new,
            })
        });
        wrote.placed.push(Placed {
            from: place(*uid),
            to: to.to_owned(),
            at,
            moved,
        });
    }
}

/// A `COPYUID` an untagged status carries (`MOVE` sends it so), into
/// `copied`.
fn copy_uid(untagged: &Untagged<'_>, copied: &mut Option<(u32, HashMap<u32, u32>)>) {
    if let Untagged::Status { code, .. } = untagged
        && let Some(map) = copy_map(code.as_ref())
    {
        copied.get_or_insert(map);
    }
}

/// The destination UIDVALIDITY (non-zero) and each source UID's new UID a
/// `COPYUID` code says; `None` for any other code, or sets that do not pair
/// up or name a UID twice.
fn copy_map(code: Option<&Code>) -> Option<(u32, HashMap<u32, u32>)> {
    let code = code.filter(|code| code.name == "COPYUID")?;
    let uidvalidity = code
        .number
        .and_then(|n| u32::try_from(n).ok())
        .filter(|v| *v > 0)?;
    let [from, to] = code.sets.as_slice() else {
        return None;
    };
    let distinct = |set: &[u32]| set.iter().collect::<HashSet<_>>().len() == set.len();
    if from.len() != to.len() || !distinct(from) || !distinct(to) {
        return None;
    }
    Some((
        uidvalidity,
        from.iter().copied().zip(to.iter().copied()).collect(),
    ))
}

/// Whether `code` is named `name`.
fn code_is(code: Option<&Code>, name: &str) -> bool {
    code.is_some_and(|code| code.name == name)
}

/// `INBOX`, as the server spells it, when the folders Campbooks creates go
/// under it: a server whose every other folder (one at least) is a child of
/// the Inbox (`INBOX.Sent`), whose personal namespace the Inbox is.
fn inbox_parent(cursor: &Cursor, delimiter: Option<char>) -> Option<String> {
    let delimiter = delimiter?;
    let others: Vec<&Folder> = cursor
        .folders
        .iter()
        .filter(|f| !f.name.eq_ignore_ascii_case("INBOX"))
        .collect();
    let under = |name: &str| {
        name.get(..5)
            .is_some_and(|head| head.eq_ignore_ascii_case("INBOX"))
            && name[5..].starts_with(delimiter)
            && name.len() > 6
    };
    if others.is_empty() || !others.iter().all(|f| under(&f.name)) {
        return None;
    }
    others[0].name.get(..5).map(ToOwned::to_owned)
}

impl Trouble {
    /// The stable code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Busy => "imap_busy",
            Self::Wire(err) => err.code(),
            Self::Hostile(code) => code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn located(folder: &str, uid: u32, role: Role, seen: bool, flagged: bool) -> Located {
        Located {
            place: Place {
                folder: folder.into(),
                uidvalidity: 7,
                uid,
            },
            location: Location {
                key: "mid:a".into(),
                seen,
                flagged,
                email_id: None,
                fingerprint: None,
            },
            role,
        }
    }

    fn places() -> Places {
        Places {
            inbox: Some("INBOX".into()),
            sent: Some("Sent".into()),
            junk: Some("Junk".into()),
            trash: Some("Trash".into()),
            archive: Some("Archive".into()),
            streams: [(ProviderStream::Updates, "Campbooks.Updates".to_owned())].into(),
            roles: [
                ("INBOX", Role::Inbox),
                ("Sent", Role::Sent),
                ("Junk", Role::Junk),
                ("Trash", Role::Trash),
                ("Archive", Role::Archive),
                ("Bakery", Role::User),
                ("Campbooks.Updates", Role::Stream),
            ]
            .map(|(name, role)| (name.to_owned(), role))
            .into(),
        }
    }

    fn change(add: &[&str], remove: &[&str]) -> LabelChange {
        (
            add.iter().map(|&s| s.to_owned()).collect(),
            remove.iter().map(|&s| s.to_owned()).collect(),
        )
    }

    fn moves(ops: &Ops) -> Vec<(&str, u32, &str)> {
        ops.moves
            .iter()
            .map(|(p, to)| (p.folder.as_str(), p.uid, to.as_str()))
            .collect()
    }

    #[test]
    fn flags_go_on_every_copy_in_one_command_when_they_can() {
        let held = [
            located("INBOX", 1, Role::Inbox, false, false),
            located("Bakery", 4, Role::User, false, false),
        ];
        let read_and_star = ops(
            &change(&["STARRED"], &["UNREAD"]),
            &held,
            &places(),
            true,
            None,
        );
        assert_eq!(
            read_and_star
                .stores
                .iter()
                .map(|(p, w)| (p.uid, *w))
                .collect::<Vec<_>>(),
            [
                (1, (true, "\\Seen \\Flagged")),
                (4, (true, "\\Seen \\Flagged"))
            ]
        );
        let unread_and_star = ops(
            &change(&["UNREAD", "STARRED"], &[]),
            &held,
            &places(),
            true,
            None,
        );
        assert_eq!(unread_and_star.stores.len(), 4);
        assert!(unread_and_star.moves.is_empty());
    }

    #[test]
    fn intents_become_moves_between_tracked_folders() {
        let inbox = [located("INBOX", 1, Role::Inbox, true, false)];
        let places = places();
        // Archive.
        let archived = ops(&change(&[], &["INBOX"]), &inbox, &places, true, None);
        assert_eq!(moves(&archived), [("INBOX", 1, "Archive")]);
        // Trash: the inbox copy, never the member's own folder; where it was
        // is kept.
        let both = [
            located("INBOX", 1, Role::Inbox, true, false),
            located("Bakery", 4, Role::User, true, false),
            located("Sent", 6, Role::Sent, true, false),
        ];
        let trashed = ops(&change(&["TRASH"], &["SPAM"]), &both, &places, true, None);
        assert_eq!(moves(&trashed), [("INBOX", 1, "Trash")]);
        assert_eq!(trashed.origins, ["from:INBOX"]);
        // Only in the member's own folder: nothing to put in the trash.
        let filed = [located("Bakery", 4, Role::User, true, false)];
        let kept = ops(&change(&["TRASH"], &["SPAM"]), &filed, &places, true, None);
        assert!(kept.unmovable && kept.moves.is_empty());
        // Untrash: back where it was, else the Inbox (received) or Sent.
        let in_trash = [located("Trash", 9, Role::Trash, true, false)];
        let restore = ["from:Archive".to_owned()];
        let back = ops(
            &change(&["INBOX"], &["TRASH"]),
            &in_trash,
            &places,
            true,
            Some(&restore),
        );
        assert_eq!(moves(&back), [("Trash", 9, "Archive")]);
        let gone_folder = ["from:Projects".to_owned()];
        let fallback = ops(
            &change(&["INBOX"], &["TRASH"]),
            &in_trash,
            &places,
            true,
            Some(&gone_folder),
        );
        assert_eq!(moves(&fallback), [("Trash", 9, "INBOX")]);
        let own = ops(&change(&[], &["TRASH"]), &in_trash, &places, true, None);
        assert_eq!(moves(&own), [("Trash", 9, "Sent")]);
        // Unarchive: from Archive, or a copy from a member's folder.
        let in_archive = [located("Archive", 3, Role::Archive, true, false)];
        let unarchived = ops(&change(&["INBOX"], &[]), &in_archive, &places, true, None);
        assert_eq!(moves(&unarchived), [("Archive", 3, "INBOX")]);
        let in_folder = [located("Bakery", 4, Role::User, true, false)];
        let copied = ops(&change(&["INBOX"], &[]), &in_folder, &places, true, None);
        assert!(copied.moves.is_empty());
        assert_eq!(copied.copies.len(), 1);
        // The member's own reply in Sent stays there.
        let sent = [located("Sent", 2, Role::Sent, true, false)];
        let stays = ops(&change(&["INBOX"], &[]), &sent, &places, true, None);
        assert!(stays.unmovable && stays.moves.is_empty() && stays.copies.is_empty());
        // A stream: the inbox copy to its folder; back to Primary: the Inbox.
        let to_updates = ops(
            &change(
                &["mark:updates"],
                &["mark:primary", "mark:receipts", "mark:newsletters"],
            ),
            &inbox,
            &places,
            true,
            None,
        );
        assert_eq!(moves(&to_updates), [("INBOX", 1, "Campbooks.Updates")]);
        let in_stream = [located("Campbooks.Updates", 5, Role::Stream, true, false)];
        let primary = ops(
            &change(
                &[],
                &[
                    "mark:primary",
                    "mark:updates",
                    "mark:receipts",
                    "mark:newsletters",
                ],
            ),
            &in_stream,
            &places,
            true,
            None,
        );
        assert_eq!(moves(&primary), [("Campbooks.Updates", 5, "INBOX")]);
        // An archived message has no place in a stream.
        let archived_stream = ops(
            &change(&["mark:updates"], &[]),
            &in_archive,
            &places,
            true,
            None,
        );
        assert!(archived_stream.unmovable && archived_stream.moves.is_empty());
        // Nowhere tracked to go: unmade.
        let no_archive = Places {
            archive: None,
            ..places
        };
        assert!(ops(&change(&[], &["INBOX"]), &inbox, &no_archive, true, None).unmovable);
    }

    #[test]
    fn a_server_without_move_or_uidplus_gets_no_move() {
        let inbox = [located("INBOX", 1, Role::Inbox, false, false)];
        let archived = ops(
            &change(&[], &["INBOX", "UNREAD"]),
            &inbox,
            &places(),
            false,
            None,
        );
        assert!(archived.unsupported && archived.moves.is_empty());
        // Its flags are still written.
        assert_eq!(archived.stores.len(), 1);
    }

    #[test]
    fn copyuid_pairs_source_and_destination() {
        let code = Code {
            name: "COPYUID".into(),
            number: Some(38_505),
            sets: vec![vec![304, 319], vec![3956, 3957]],
        };
        let (uidvalidity, map) = copy_map(Some(&code)).unwrap();
        assert_eq!(uidvalidity, 38_505);
        assert_eq!(map[&319], 3957);
        let unpaired = Code {
            sets: vec![vec![304, 319], vec![3956]],
            ..code.clone()
        };
        assert!(copy_map(Some(&unpaired)).is_none());
        // One new UID named for two copies would overwrite a location.
        let twice = Code {
            sets: vec![vec![304, 319], vec![1, 1]],
            ..code.clone()
        };
        assert!(copy_map(Some(&twice)).is_none());
        let no_validity = Code {
            number: Some(0),
            ..code.clone()
        };
        assert!(copy_map(Some(&no_validity)).is_none());
        let other = Code {
            name: "UIDNEXT".into(),
            ..code
        };
        assert!(copy_map(Some(&other)).is_none());
    }

    #[test]
    fn expunges_are_counted() {
        assert_eq!(expunges(&Untagged::Expunge(3)), 1);
        assert_eq!(expunges(&Untagged::Vanished(vec![(4, 6), (9, 9)])), 4);
        assert_eq!(expunges(&Untagged::Exists(3)), 0);
        assert!(guard_expunged(2, 2).is_ok());
        assert_eq!(
            guard_expunged(3, 2),
            Err(Trouble::Hostile("imap_expunged_more"))
        );
    }

    #[test]
    fn folders_go_under_the_inbox_only_where_every_folder_is() {
        let cursor = |names: &[&str]| Cursor {
            folders: names
                .iter()
                .map(|name| {
                    Folder::new(Tracked {
                        name: (*name).into(),
                        display: (*name).into(),
                        role: Role::User,
                    })
                })
                .collect(),
            ..Cursor::default()
        };
        assert_eq!(
            inbox_parent(&cursor(&["INBOX", "INBOX.Sent", "INBOX.Trash"]), Some('.')).as_deref(),
            Some("INBOX")
        );
        assert_eq!(
            inbox_parent(&cursor(&["INBOX", "INBOX.Sent", "Trash"]), Some('.')),
            None
        );
        assert_eq!(inbox_parent(&cursor(&["INBOX"]), Some('.')), None);
        assert_eq!(inbox_parent(&cursor(&["INBOX", "INBOX.Sent"]), None), None);
        assert!(writable("mark:updates") && writable("TRASH") && !writable("folder:Bakery"));
    }
}
