//! An IMAP stream's cursor, where its messages are, and the state that
//! follows from it.
//!
//! A message is one message wherever it is: its key (the index's provider
//! message id) is its normalised `Message-ID`, and each copy of it in a
//! folder is a **location** (folder, UIDVALIDITY, UID) with its own `\Seen`
//! and `\Flagged` (`connectors.imap_locations`). Its state is the union of
//! its locations, kept in the index as tokens like Zoho's: the places it is
//! in (`INBOX`, `SENT`, `JUNK`, `TRASH`, `ARCHIVE`, `USER`), `folder:<name>`
//! per folder of the member's own, `UNREAD` when no copy is seen, `STARRED`
//! when one is flagged, and `SELF` when the mailbox sent it.

use std::collections::HashMap;

use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

use super::folders::{self, Role, Tracked};
use crate::{
    contract::{Direction, Label, Mailbox},
    domain::model::StreamId,
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

    /// Whether every folder has read its window.
    pub fn backfilled(&self) -> bool {
        !self.folders.is_empty() && self.folders.iter().all(|f| f.backfilled)
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
        if role == Role::User {
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

/// The contract's state of a message with `tokens`, its folders named as
/// `cursor` shows them: in the Inbox when a copy is; in spam (or the
/// trash) only when every copy is; read when a copy is seen; starred when
/// one is flagged; sent when a copy is in Sent (or the mailbox sent it and
/// no copy is in the Inbox); labelled with the member's folders it is in.
/// IMAP has no categories and no provider streams.
pub fn flags(tokens: &[String], cursor: &Cursor) -> Flags {
    let has = |token: &str| tokens.iter().any(|t| t == token);
    let places = [
        Role::Inbox,
        Role::Sent,
        Role::Junk,
        Role::Trash,
        Role::Archive,
        Role::User,
    ]
    .iter()
    .filter(|role| has(role.token()))
    .count();
    let only = |role: Role| has(role.token()) && places == 1;
    let in_inbox = has(Role::Inbox.token());
    let names: HashMap<&str, &str> = cursor
        .folders
        .iter()
        .map(|f| (f.name.as_str(), f.display.as_str()))
        .collect();
    let mut labels: Vec<Label> = tokens
        .iter()
        .filter_map(|t| t.strip_prefix(FOLDER))
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
            in_spam: only(Role::Junk),
            in_trash: only(Role::Trash),
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
    for (place, location) in puts.drain(..) {
        folder.push(place.folder);
        uidvalidity.push(i64::from(place.uidvalidity));
        uid.push(i64::from(place.uid));
        key.push(location.key);
        seen.push(location.seen);
        flagged.push(location.flagged);
        email_id.push(location.email_id);
    }
    sqlx::query(
        "INSERT INTO connectors.imap_locations
             (workspace_id, stream_id, folder, uidvalidity, uid, message_key, seen, flagged, email_id)
         SELECT $1::uuid, $2::uuid, l.*
           FROM unnest($3::text[], $4::bigint[], $5::bigint[], $6::text[], $7::bool[], $8::bool[],
                       $9::text[]) AS l
         ON CONFLICT (stream_id, folder, uidvalidity, uid) DO UPDATE
            SET message_key = EXCLUDED.message_key, seen = EXCLUDED.seen,
                flagged = EXCLUDED.flagged, email_id = EXCLUDED.email_id",
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
