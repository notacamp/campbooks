//! An IMAP stream's cursor, where its messages are, and the state that
//! follows from it.
//!
//! A message is one message wherever it is: its key (the index's provider
//! message id) is its normalised `Message-ID`, and each copy of it in a
//! folder is a **location** (folder, UIDVALIDITY, UID) with its own `\Seen`
//! and `\Flagged` (`connectors.imap_locations`). Its state is the union of
//! its locations, kept in the index as tokens like Zoho's: the places it is
//! in (`INBOX`, `STREAM`, `SENT`, `JUNK`, `TRASH`, `ARCHIVE`, `USER`),
//! `folder:<name>` per stream folder and folder of the member's own,
//! `UNREAD` when no copy is seen, `STARRED` when one is flagged, and `SELF`
//! when the mailbox sent it.

use std::collections::{BTreeSet, HashMap};

use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

use super::folders::{self, Role, Tracked};
use crate::{
    contract::{Direction, Label, Mailbox, ProviderStream},
    domain::{
        marks::{self, Marked},
        model::StreamId,
    },
    mail::Flags,
};

/// The prefix of a member's folder's token.
const FOLDER: &str = "folder:";

/// No copy is `\Seen`.
pub const UNREAD: &str = "UNREAD";

/// A copy is `\Flagged`.
pub const STARRED: &str = "STARRED";

/// The mailbox's own address sent it.
pub const SELF: &str = "SELF";

/// Locations written per statement.
const BATCH: usize = 1_000;

/// An IMAP stream's cursor (the `streams.cursor` column).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cursor {
    /// The folders read, Inbox first.
    pub folders: Vec<Folder>,
    /// When the folders were last listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listed_at: Option<DateTime<Utc>>,
    /// The sweep pass; index rows a sweep listed carry it.
    pub generation: i32,
    /// When the last sweep ended.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swept_at: Option<DateTime<Utc>>,
    /// When the first sync (the backfill) completed (set by the run,
    /// `sync_stream`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<DateTime<Utc>>,
    /// Sign-ins the server refused in a row.
    #[serde(skip_serializing_if = "is_zero")]
    pub login_refusals: u32,
    /// Messages left with no location still wait to be looked for.
    #[serde(skip_serializing_if = "is_false")]
    pub recheck: bool,
    /// Whether the server offered IDLE at the last run (none before the
    /// first): what makes the stream a candidate for push.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle: Option<bool>,
    /// The folders Campbooks created in the mailbox (stream folders), as
    /// the server spells them: at most
    /// `CAMPBOOKS_CONNECTORS__IMAP__MAX_CREATED_FOLDERS`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
const fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
const fn is_false(b: &bool) -> bool {
    !*b
}

impl Cursor {
    /// The cursor as the opaque string events carry (numbers only).
    pub fn token(&self) -> String {
        let next: u64 = self
            .folders
            .iter()
            .filter_map(|f| f.uidnext)
            .map(u64::from)
            .sum();
        format!("{}:{}:{next}", self.generation, self.folders.len())
    }

    /// The role of folder `name`, when it is read.
    pub fn role_of(&self, name: &str) -> Option<Role> {
        self.folders.iter().find(|f| f.name == name).map(|f| f.role)
    }

    /// The stream folder `name` files, when it is a stream folder read.
    pub fn stream_of(&self, name: &str) -> Option<ProviderStream> {
        self.folders
            .iter()
            .find(|f| f.name == name && f.role == Role::Stream)
            .and_then(|f| folders::stream_of(&f.display))
    }

    /// The stream folder read for `stream`, if there is one.
    pub fn stream_folder(&self, stream: ProviderStream) -> Option<&Folder> {
        self.folders
            .iter()
            .find(|f| f.role == Role::Stream && folders::stream_of(&f.display) == Some(stream))
    }

    /// The first folder read with `role`.
    pub fn folder_of(&self, role: Role) -> Option<&Folder> {
        self.folders.iter().find(|f| f.role == role)
    }

    /// Whether every folder has read its window, or is set aside for now
    /// (it would not open: [`Folder::skipped_until`]).
    pub fn backfilled(&self) -> bool {
        !self.folders.is_empty()
            && self
                .folders
                .iter()
                .all(|f| f.backfilled || f.skipped_until.is_some())
    }
}

/// A folder the stream reads, and how far.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Its name as the server spells it.
    pub name: String,
    /// Its name as shown.
    pub display: String,
    /// What it is.
    pub role: Role,
    /// Its UIDVALIDITY when last read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uidvalidity: Option<u32>,
    /// The UID its next new message gets: below it, all is read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uidnext: Option<u32>,
    /// Its HIGHESTMODSEQ when last read (CONDSTORE).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highestmodseq: Option<u64>,
    /// How many messages it had when last read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messages: Option<u32>,
    /// How far down its window the walk has read: the lowest UID read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backfill_low_uid: Option<u32>,
    /// Its window is read.
    #[serde(default)]
    pub backfilled: bool,
    /// It would not open (`STATUS` or `EXAMINE` refused, or no UIDVALIDITY):
    /// set aside until then, so a run neither retries it nor runs again at
    /// once for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_until: Option<DateTime<Utc>>,
}

impl Folder {
    /// A folder not read yet.
    pub fn new(tracked: Tracked) -> Self {
        Self {
            name: tracked.name,
            display: tracked.display,
            role: tracked.role,
            uidvalidity: None,
            uidnext: None,
            highestmodseq: None,
            messages: None,
            backfill_low_uid: None,
            backfilled: false,
            skipped_until: None,
        }
    }

    /// Read it again from scratch (its UIDVALIDITY changed).
    pub fn reset(&mut self) {
        self.uidvalidity = None;
        self.uidnext = None;
        self.highestmodseq = None;
        self.messages = None;
        self.backfill_low_uid = None;
        self.backfilled = false;
    }
}

/// Where a copy is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Place {
    pub folder: String,
    pub uidvalidity: u32,
    pub uid: u32,
}

/// The copy there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// The message's key.
    pub key: String,
    pub seen: bool,
    pub flagged: bool,
    /// The server's own id of the message (RFC 8474), when given.
    pub email_id: Option<String>,
    /// A digest of this copy's `Date`, `From` and size: another message
    /// reusing the `Message-ID` does not share it.
    pub fingerprint: Option<String>,
}

/// One change to the locations, applied in order with the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocationOp {
    /// A copy is there (or its flags changed).
    Put(Place, Location),
    /// The copy is gone.
    Drop(Place),
    /// Every copy in the folder is gone (it went away, or its UIDVALIDITY
    /// changed).
    DropFolder(String),
}

/// The tokens of a message with copies in `places` (role, folder, seen,
/// flagged), sent by the mailbox when `from_self`. Sorted.
pub fn tokens<'a>(
    places: impl IntoIterator<Item = (Role, &'a str, bool, bool)>,
    from_self: bool,
) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut seen = false;
    let mut flagged = false;
    for (role, folder, is_seen, is_flagged) in places {
        tokens.push(role.token().to_owned());
        if matches!(role, Role::User | Role::Stream) {
            tokens.push(format!("{FOLDER}{folder}"));
        }
        seen |= is_seen;
        flagged |= is_flagged;
    }
    if !seen {
        tokens.push(UNREAD.to_owned());
    }
    if flagged {
        tokens.push(STARRED.to_owned());
    }
    if from_self {
        tokens.push(SELF.to_owned());
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// Whether a message with `tokens` is in the bin `role` (Junk or Trash): a
/// copy is there and none is in the Inbox, a stream folder, Archive or the
/// other bin. Copies in Sent or the member's own folders do not hold it out:
/// putting a message in the trash leaves those alone.
fn bin_only(tokens: &[String], role: Role) -> bool {
    let has = |role: Role| tokens.iter().any(|t| t == role.token());
    let other = if role == Role::Trash {
        Role::Junk
    } else {
        Role::Trash
    };
    has(role)
        && ![Role::Inbox, Role::Stream, Role::Archive, other]
            .into_iter()
            .any(has)
}

/// The contract's state of a message with `tokens`, its folders named as
/// `cursor` shows them: in the inbox when a copy is in the Inbox or a
/// stream folder; in spam (or the trash) when a copy is and none is in the
/// inbox, Archive or the other bin ([`bin_only`]); read when
/// a copy is seen; starred when one is flagged; sent when a copy is in Sent
/// (or the mailbox sent it and no copy is in the inbox); labelled with the
/// member's folders it is in (never a stream folder). IMAP has no
/// categories; where a message's stream shows is [`provider_stream`]'s.
pub fn flags(tokens: &[String], cursor: &Cursor) -> Flags {
    let has = |token: &str| tokens.iter().any(|t| t == token);
    let binned = |role: Role| bin_only(tokens, role);
    let in_inbox = has(Role::Inbox.token()) || has(Role::Stream.token());
    let names: HashMap<&str, &str> = cursor
        .folders
        .iter()
        .map(|f| (f.name.as_str(), f.display.as_str()))
        .collect();
    let mut labels: Vec<Label> = tokens
        .iter()
        .filter_map(|t| t.strip_prefix(FOLDER))
        .filter(|name| cursor.role_of(name) != Some(Role::Stream))
        .map(|name| Label {
            key: name.to_owned(),
            name: names
                .get(name)
                .map_or_else(|| folders::display(name, None), |d| (*d).to_owned()),
        })
        .collect();
    labels.sort_by(|a, b| a.key.cmp(&b.key));
    Flags {
        mailbox: Mailbox {
            in_inbox,
            in_spam: binned(Role::Junk),
            in_trash: binned(Role::Trash),
        },
        read: !has(UNREAD),
        starred: has(STARRED),
        direction: if has(Role::Sent.token()) || (has(SELF) && !in_inbox) {
            Direction::Outbound
        } else {
            Direction::Inbound
        },
        categories: Vec::new(),
        labels,
        provider_stream: None,
        stream_mark_stale: false,
    }
}

/// The streams of the stream folders among `tokens`, as `cursor` shows
/// them.
fn streams_in(tokens: &[String], cursor: &Cursor) -> BTreeSet<ProviderStream> {
    tokens
        .iter()
        .filter_map(|t| t.strip_prefix(FOLDER))
        .filter_map(|name| cursor.stream_of(name))
        .collect()
}

/// Where a received message with `after` shows its stream at the server, as
/// an upsert's `provider_stream`, given its tokens `before` (`None` for a
/// new message): the stream of its stream folder (none for several); the
/// Inbox's, `primary`, for a new message there or one the server moved
/// there from a stream folder; otherwise no word (archived, sent, or not
/// moved between the Inbox and a stream folder), which leaves the stream
/// conversations keeps as it is.
#[allow(
    clippy::option_option,
    reason = "absent, null or a stream: the contract's three cases"
)]
pub fn provider_stream(
    before: Option<&[String]>,
    after: &[String],
    cursor: &Cursor,
) -> Option<Option<ProviderStream>> {
    if flags(after, cursor).direction == Direction::Outbound {
        return None;
    }
    let now = streams_in(after, cursor);
    if !now.is_empty() {
        return Some(match marks::marked(&now) {
            Marked::One(stream) => Some(stream),
            Marked::Several | Marked::Unmarked => None,
        });
    }
    if !after.iter().any(|t| t == Role::Inbox.token()) {
        return None;
    }
    match before {
        None => Some(Some(ProviderStream::Primary)),
        Some(before) if !streams_in(before, cursor).is_empty() => {
            Some(Some(ProviderStream::Primary))
        }
        Some(_) => None,
    }
}

/// `tokens` as an outbound change's plan sees them (`imap/write.rs`): read
/// (`UNREAD`) and starred as they are; `INBOX` when a copy is in the Inbox
/// or a stream folder; `TRASH` or `SPAM` when the message is in the trash or
/// spam ([`bin_only`]); and the token of each stream folder's mark (`mark:updates`).
pub fn view(tokens: &[String], cursor: &Cursor) -> Vec<String> {
    let has = |token: &str| tokens.iter().any(|t| t == token);
    let mut view = Vec::new();
    for token in [UNREAD, STARRED] {
        if has(token) {
            view.push(token.to_owned());
        }
    }
    if has(Role::Inbox.token()) || has(Role::Stream.token()) {
        view.push("INBOX".to_owned());
    }
    if bin_only(tokens, Role::Trash) {
        view.push("TRASH".to_owned());
    }
    if bin_only(tokens, Role::Junk) {
        view.push("SPAM".to_owned());
    }
    view.extend(streams_in(tokens, cursor).into_iter().map(marks::token));
    view
}

/// `flags` with a member's `changes` still waiting to be made (their
/// tokens, oldest first: `outbound::map_imap`) applied, as an emission
/// shows them until the push makes them. A pending stream move says no
/// stream.
pub fn overlay(flags: &mut Flags, changes: &[(Vec<String>, Vec<String>)]) {
    let mailbox = &mut flags.mailbox;
    for (add, remove) in changes {
        for token in add {
            match token.as_str() {
                UNREAD => flags.read = false,
                STARRED => flags.starred = true,
                "INBOX" => mailbox.in_inbox = true,
                "TRASH" => {
                    mailbox.in_trash = true;
                    mailbox.in_spam = false;
                    mailbox.in_inbox = false;
                }
                "SPAM" => {
                    mailbox.in_spam = true;
                    mailbox.in_trash = false;
                    mailbox.in_inbox = false;
                }
                _ => {}
            }
        }
        for token in remove {
            match token.as_str() {
                UNREAD => flags.read = true,
                STARRED => flags.starred = false,
                "INBOX" => mailbox.in_inbox = false,
                "TRASH" => mailbox.in_trash = false,
                "SPAM" => mailbox.in_spam = false,
                _ => {}
            }
        }
        if add
            .iter()
            .chain(remove)
            .any(|t| marks::of_token(t).is_some())
        {
            flags.provider_stream = None;
        }
    }
}

/// Apply `ops` to `stream`'s locations, in order, in `tx`.
pub async fn apply(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    stream: StreamId,
    ops: Vec<LocationOp>,
) -> Result<(), sqlx::Error> {
    let mut puts: Vec<(Place, Location)> = Vec::new();
    for op in ops {
        match op {
            LocationOp::Put(place, location) => {
                puts.retain(|(p, _)| *p != place);
                puts.push((place, location));
                if puts.len() >= BATCH {
                    put(tx, workspace, stream, &mut puts).await?;
                }
            }
            LocationOp::Drop(place) => {
                put(tx, workspace, stream, &mut puts).await?;
                sqlx::query(
                    "DELETE FROM connectors.imap_locations
                      WHERE stream_id = $1 AND folder = $2 AND uidvalidity = $3 AND uid = $4",
                )
                .bind(stream)
                .bind(&place.folder)
                .bind(i64::from(place.uidvalidity))
                .bind(i64::from(place.uid))
                .execute(&mut **tx)
                .await?;
            }
            LocationOp::DropFolder(folder) => {
                put(tx, workspace, stream, &mut puts).await?;
                sqlx::query(
                    "DELETE FROM connectors.imap_locations WHERE stream_id = $1 AND folder = $2",
                )
                .bind(stream)
                .bind(&folder)
                .execute(&mut **tx)
                .await?;
            }
        }
    }
    put(tx, workspace, stream, &mut puts).await
}

/// Write `puts` (each place once) and empty it.
async fn put(
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    stream: StreamId,
    puts: &mut Vec<(Place, Location)>,
) -> Result<(), sqlx::Error> {
    if puts.is_empty() {
        return Ok(());
    }
    let mut folder = Vec::with_capacity(puts.len());
    let mut uidvalidity = Vec::with_capacity(puts.len());
    let mut uid = Vec::with_capacity(puts.len());
    let mut key = Vec::with_capacity(puts.len());
    let mut seen = Vec::with_capacity(puts.len());
    let mut flagged = Vec::with_capacity(puts.len());
    let mut email_id = Vec::with_capacity(puts.len());
    let mut fingerprint = Vec::with_capacity(puts.len());
    for (place, location) in puts.drain(..) {
        folder.push(place.folder);
        uidvalidity.push(i64::from(place.uidvalidity));
        uid.push(i64::from(place.uid));
        key.push(location.key);
        seen.push(location.seen);
        flagged.push(location.flagged);
        email_id.push(location.email_id);
        fingerprint.push(location.fingerprint);
    }
    sqlx::query(
        "INSERT INTO connectors.imap_locations
             (workspace_id, stream_id, folder, uidvalidity, uid, message_key, seen, flagged, email_id,
              fingerprint)
         SELECT $1::uuid, $2::uuid, l.*
           FROM unnest($3::text[], $4::bigint[], $5::bigint[], $6::text[], $7::bool[], $8::bool[],
                       $9::text[], $10::text[]) AS l
         ON CONFLICT (stream_id, folder, uidvalidity, uid) DO UPDATE
            SET message_key = EXCLUDED.message_key, seen = EXCLUDED.seen,
                flagged = EXCLUDED.flagged, email_id = EXCLUDED.email_id,
                fingerprint = EXCLUDED.fingerprint",
    )
    .bind(workspace)
    .bind(stream)
    .bind(folder)
    .bind(uidvalidity)
    .bind(uid)
    .bind(key)
    .bind(seen)
    .bind(flagged)
    .bind(email_id)
    .bind(fingerprint)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor() -> Cursor {
        let folder = |name: &str, display: &str, role| {
            Folder::new(Tracked {
                name: name.into(),
                display: display.into(),
                role,
            })
        };
        Cursor {
            folders: vec![
                folder("INBOX", "INBOX", Role::Inbox),
                folder("INBOX.Sent", "Sent", Role::Sent),
                folder("Spam", "Spam", Role::Junk),
                folder("INBOX.Bakery", "Bakery", Role::User),
            ],
            ..Cursor::default()
        }
    }

    #[test]
    fn a_message_is_the_union_of_its_copies() {
        let cursor = cursor();
        // In the Inbox, unread; a copy in Bakery is flagged.
        let both = tokens(
            [
                (Role::Inbox, "INBOX", false, false),
                (Role::User, "INBOX.Bakery", false, true),
            ],
            false,
        );
        assert_eq!(
            both,
            ["INBOX", "STARRED", "UNREAD", "USER", "folder:INBOX.Bakery"]
        );
        let flags = flags(&both, &cursor);
        assert!(flags.mailbox.in_inbox && !flags.read && flags.starred);
        assert_eq!(flags.labels.len(), 1);
        assert_eq!(
            (flags.labels[0].key.as_str(), flags.labels[0].name.as_str()),
            ("INBOX.Bakery", "Bakery")
        );
        assert_eq!(flags.direction, Direction::Inbound);
        assert_eq!(flags.provider_stream, None);

        // In spam only when nowhere else; seen on any copy is read.
        let spam = super::flags(&tokens([(Role::Junk, "Spam", true, false)], false), &cursor);
        assert!(spam.mailbox.in_spam && spam.read && !spam.mailbox.in_inbox);
        let spam_and_inbox = super::flags(
            &tokens(
                [
                    (Role::Junk, "Spam", false, false),
                    (Role::Inbox, "INBOX", true, false),
                ],
                false,
            ),
            &cursor,
        );
        assert!(!spam_and_inbox.mailbox.in_spam && spam_and_inbox.read);

        // Sent: a copy in Sent, or the mailbox's own mail out of the Inbox.
        let sent = super::flags(
            &tokens([(Role::Sent, "INBOX.Sent", true, false)], false),
            &cursor,
        );
        assert_eq!(sent.direction, Direction::Outbound);
        let own_archived = super::flags(
            &tokens([(Role::Archive, "Archive", true, false)], true),
            &cursor,
        );
        assert_eq!(own_archived.direction, Direction::Outbound);
        let own_to_self = super::flags(
            &tokens([(Role::Inbox, "INBOX", true, false)], true),
            &cursor,
        );
        assert_eq!(own_to_self.direction, Direction::Inbound);
    }

    #[test]
    fn a_stream_folder_is_the_inbox_and_says_the_stream() {
        let mut cursor = cursor();
        cursor.folders.push(Folder::new(Tracked {
            name: "INBOX.Campbooks.Updates".into(),
            display: "Campbooks/Updates".into(),
            role: Role::Stream,
        }));
        let sorted = tokens(
            [(Role::Stream, "INBOX.Campbooks.Updates", false, false)],
            false,
        );
        assert_eq!(
            sorted,
            ["STREAM", "UNREAD", "folder:INBOX.Campbooks.Updates"]
        );
        let flags = flags(&sorted, &cursor);
        assert!(flags.mailbox.in_inbox && flags.labels.is_empty());
        assert_eq!(flags.provider_stream, None);
        let inbox = tokens([(Role::Inbox, "INBOX", false, false)], false);
        let archived = tokens([(Role::Archive, "Archive", false, false)], false);
        // Sorted into the folder (new, or moved there): its stream.
        assert_eq!(
            provider_stream(None, &sorted, &cursor),
            Some(Some(ProviderStream::Updates))
        );
        assert_eq!(
            provider_stream(Some(inbox.as_slice()), &sorted, &cursor),
            Some(Some(ProviderStream::Updates))
        );
        // New in the Inbox, or moved there from a stream folder: Primary.
        assert_eq!(
            provider_stream(None, &inbox, &cursor),
            Some(Some(ProviderStream::Primary))
        );
        assert_eq!(
            provider_stream(Some(sorted.as_slice()), &inbox, &cursor),
            Some(Some(ProviderStream::Primary))
        );
        // Not moved between them, archived, or sent: no word.
        assert_eq!(
            provider_stream(Some(inbox.as_slice()), &inbox, &cursor),
            None
        );
        assert_eq!(
            provider_stream(Some(sorted.as_slice()), &archived, &cursor),
            None
        );
        let sent = tokens([(Role::Sent, "INBOX.Sent", true, false)], false);
        assert_eq!(provider_stream(None, &sent, &cursor), None);
    }

    #[test]
    fn the_plan_sees_flags_the_inbox_bins_and_marks() {
        let mut cursor = cursor();
        cursor.folders.push(Folder::new(Tracked {
            name: "Campbooks.Receipts".into(),
            display: "Campbooks/Receipts".into(),
            role: Role::Stream,
        }));
        let filed = tokens(
            [
                (Role::Stream, "Campbooks.Receipts", true, true),
                (Role::User, "INBOX.Bakery", true, false),
            ],
            false,
        );
        assert_eq!(view(&filed, &cursor), ["STARRED", "INBOX", "mark:receipts"]);
        let trashed = tokens([(Role::Trash, "Trash", false, false)], false);
        assert_eq!(view(&trashed, &cursor), ["UNREAD", "TRASH"]);
        let both = tokens(
            [
                (Role::Trash, "Trash", true, false),
                (Role::Inbox, "INBOX", true, false),
            ],
            false,
        );
        assert_eq!(view(&both, &cursor), ["INBOX"]);
        let spam = tokens([(Role::Junk, "Spam", true, false)], false);
        assert_eq!(view(&spam, &cursor), ["SPAM"]);
        // The member's own filed copy does not hold it out of the trash.
        let filed_too = tokens(
            [
                (Role::Trash, "Trash", true, false),
                (Role::User, "INBOX.Bakery", true, false),
            ],
            false,
        );
        assert_eq!(view(&filed_too, &cursor), ["TRASH"]);
        assert!(super::flags(&filed_too, &cursor).mailbox.in_trash);
        // A pending change shows as made.
        let mut flags = flags(&filed, &cursor);
        overlay(
            &mut flags,
            &[
                (vec!["UNREAD".into()], vec!["STARRED".into()]),
                (vec!["TRASH".into()], vec!["SPAM".into()]),
            ],
        );
        assert!(!flags.read && !flags.starred);
        assert!(flags.mailbox.in_trash && !flags.mailbox.in_inbox);
    }

    #[test]
    fn the_cursor_reads_what_it_wrote_and_an_empty_one() {
        let cursor = cursor();
        let json = serde_json::to_value(&cursor).unwrap();
        assert_eq!(serde_json::from_value::<Cursor>(json).unwrap(), cursor);
        let empty: Cursor = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(empty.folders.is_empty() && !empty.backfilled());
        // A cursor `sync_stream` stamped keeps its stamp.
        let stamped: Cursor =
            serde_json::from_value(serde_json::json!({"synced_at": "2026-10-01T00:00:00Z"}))
                .unwrap();
        assert!(stamped.synced_at.is_some());
        assert_eq!(cursor.token(), "0:4:0");
    }
}
