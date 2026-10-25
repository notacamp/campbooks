//! A Microsoft stream's cursor, and a message's state in the contract's
//! terms (campbooks-cloud#316).
//!
//! A Graph message is in one folder, read or not, flagged or not, with its
//! Outlook categories. The index keeps that as tokens like IMAP's: the
//! place its folder is (`INBOX`, `STREAM`, `SENT`, `JUNK`, `TRASH`,
//! `ARCHIVE`, `USER`, the IMAP adapter's [`Role`]s), `folder:<id>` (its
//! folder, whatever it is), `UNREAD`, `STARRED` and `category:<name>` per
//! category. A folder is a role as IMAP's are: the well-known ones by
//! Graph's own names, a member's folder shown as `Campbooks/<Stream>` a
//! stream folder, any other a label.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::graph::MessageState;
use crate::{
    contract::{Direction, Label, Mailbox, ProviderStream},
    domain::marks::{self, Marked},
    imap::folders::Role,
    mail::Flags,
};

/// The prefix of a folder's token.
const FOLDER: &str = "folder:";

/// The prefix of a category's token.
const CATEGORY: &str = "category:";

/// The message is unread.
pub const UNREAD: &str = "UNREAD";

/// The message is flagged.
pub const STARRED: &str = "STARRED";

/// A Microsoft stream's cursor (the `streams.cursor` column).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cursor {
    /// The folders read, Inbox first.
    pub folders: Vec<Folder>,
    /// The folders never read (Drafts, Outbox, …), as of the last listing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
    /// When the folders were last listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listed_at: Option<DateTime<Utc>>,
    /// The start of the window a pass (the backfill, a resync) reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_start: Option<DateTime<Utc>>,
    /// The pass; index rows a pass listed carry it.
    pub generation: i32,
    /// When the first sync (the backfill) completed (set by the run,
    /// `sync_stream`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<DateTime<Utc>>,
    /// Messages that left a folder, still to be looked up (moved or gone).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lookups: Vec<String>,
    /// The last listing may have left folders out (the folder cap, a
    /// listing cut short, a well-known folder Graph did not name this
    /// time): a message whose folder is not read is never taken for gone.
    #[serde(skip_serializing_if = "is_false")]
    pub partial: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "serde's signature")]
const fn is_false(b: &bool) -> bool {
    !*b
}

/// A folder the stream reads, and where its delta is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Graph's id.
    pub id: String,
    /// Its name as shown: under its parents (`Projects/Bakery`), the
    /// Inbox's own folders without `Inbox/`.
    pub display: String,
    /// What it is.
    pub role: Role,
    /// The rest of a round under way (`$skiptoken`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip: Option<String>,
    /// Where the next round starts (`$deltatoken`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    /// The messages its first round has listed so far, while one is under
    /// way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listed: Option<u32>,
    /// Its last first round listed as many messages as Graph lists with a
    /// `$filter` at most: it may have left some out, so none of its messages
    /// is taken for gone for not being listed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub truncated: bool,
}

impl Folder {
    /// A folder not read yet.
    pub fn new(id: String, display: String, role: Role) -> Self {
        Self {
            id,
            display,
            role,
            skip: None,
            delta: None,
            listed: None,
            truncated: false,
        }
    }

    /// Its first round is done.
    pub fn round_done(&self) -> bool {
        self.skip.is_none() && self.delta.is_some()
    }
}

impl Cursor {
    /// The cursor as the opaque string events carry (numbers only).
    pub fn token(&self) -> String {
        let done = self.folders.iter().filter(|f| f.round_done()).count();
        format!("{}:{}:{done}", self.generation, self.folders.len())
    }

    /// Folder `id`, when it is read.
    pub fn folder(&self, id: &str) -> Option<&Folder> {
        self.folders.iter().find(|f| f.id == id)
    }

    /// Whether `id` is a folder the last listing saw.
    pub fn knows(&self, id: &str) -> bool {
        self.folder(id).is_some() || self.skipped.iter().any(|s| s == id)
    }

    /// The first folder read with `role`.
    pub fn folder_of_role(&self, role: Role) -> Option<&Folder> {
        self.folders.iter().find(|f| f.role == role)
    }

    /// The stream folder read for `stream` (`Campbooks/<Stream>`, under any
    /// parent), the first by name when there are several.
    pub fn stream_folder(&self, stream: ProviderStream) -> Option<&Folder> {
        self.folders
            .iter()
            .filter(|f| f.role == Role::Stream && marks::of_name(&f.display) == Some(stream))
            .min_by(|a, b| a.display.cmp(&b.display).then_with(|| a.id.cmp(&b.id)))
    }

    /// The stream that folder `id` files, when it is a stream folder read.
    fn stream_of(&self, id: &str) -> Option<ProviderStream> {
        self.folder(id)
            .filter(|f| f.role == Role::Stream)
            .and_then(|f| marks::of_name(&f.display))
    }

    /// Start a new pass: every folder read again from the window, the
    /// generation up.
    pub fn restart(&mut self) {
        for folder in &mut self.folders {
            folder.skip = None;
            folder.delta = None;
            folder.listed = None;
            folder.truncated = false;
        }
        self.generation = self.generation.saturating_add(1);
        self.window_start = None;
        self.lookups.clear();
    }
}

/// The token of folder `id`.
pub fn folder_token(id: &str) -> String {
    format!("{FOLDER}{id}")
}

/// The folder a message's tokens put it in.
pub fn folder_of(tokens: &[String]) -> Option<&str> {
    tokens.iter().find_map(|t| t.strip_prefix(FOLDER))
}

/// The tokens of a message in folder `folder` (as `cursor` knows it) with
/// `state`. Sorted.
pub fn tokens(cursor: &Cursor, folder: &str, state: &MessageState) -> Vec<String> {
    let mut tokens = vec![folder_token(folder)];
    if let Some(role) = cursor.folder(folder).map(|f| f.role) {
        tokens.push(role.token().to_owned());
    }
    if !state.read {
        tokens.push(UNREAD.to_owned());
    }
    if state.flagged {
        tokens.push(STARRED.to_owned());
    }
    tokens.extend(
        state
            .categories
            .iter()
            .map(|name| format!("{CATEGORY}{name}")),
    );
    tokens.sort();
    tokens.dedup();
    tokens
}

/// `tokens` as an outbound change's plan sees them (`write.rs`,
/// campbooks-cloud#317): read (`UNREAD`) and starred as they are; `INBOX`
/// in the Inbox or a stream folder; `TRASH` in Deleted Items; `SPAM` in Junk
/// Email; and the mark (`mark:updates`) of each stream its stream folder or
/// a stream category says. Sorted.
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
    if has(Role::Trash.token()) {
        view.push("TRASH".to_owned());
    }
    if has(Role::Junk.token()) {
        view.push("SPAM".to_owned());
    }
    view.extend(streams_in(tokens, cursor).into_iter().map(marks::token));
    view.sort();
    view
}

/// The streams `tokens` mark: a stream folder's, and a category named like
/// a stream mark (`Campbooks/Updates`).
fn streams_in(tokens: &[String], cursor: &Cursor) -> BTreeSet<ProviderStream> {
    tokens
        .iter()
        .filter_map(|t| {
            t.strip_prefix(FOLDER)
                .and_then(|id| cursor.stream_of(id))
                .or_else(|| t.strip_prefix(CATEGORY).and_then(marks::of_name))
        })
        .collect()
}

/// The contract's state of a message with `tokens`, its folders named as
/// `cursor` shows them: in the inbox in the Inbox or a stream folder, in
/// spam in Junk Email, in the trash in Deleted Items, read unless `UNREAD`,
/// starred when flagged, outbound in Sent Items; labelled with the member's
/// folder it is in and its categories (never a stream mark). Its provider
/// stream: none for sent mail; the one its stream folder or categories
/// mark (none for several); Primary for unmarked mail in the Inbox;
/// otherwise no word.
pub fn flags(tokens: &[String], cursor: &Cursor) -> Flags {
    let has = |token: &str| tokens.iter().any(|t| t == token);
    let in_inbox = has(Role::Inbox.token()) || has(Role::Stream.token());
    let direction = if has(Role::Sent.token()) {
        Direction::Outbound
    } else {
        Direction::Inbound
    };
    let mut labels: Vec<Label> = tokens
        .iter()
        .filter_map(|t| {
            if let Some(id) = t.strip_prefix(FOLDER) {
                let folder = cursor.folder(id).filter(|f| f.role == Role::User)?;
                return Some(Label {
                    key: folder.id.clone(),
                    name: folder.display.clone(),
                });
            }
            let name = t.strip_prefix(CATEGORY)?;
            marks::of_name(name).is_none().then(|| Label {
                key: name.to_owned(),
                name: name.to_owned(),
            })
        })
        .collect();
    labels.sort_by(|a, b| a.key.cmp(&b.key));
    let provider_stream = if direction == Direction::Outbound {
        None
    } else {
        let marked = streams_in(tokens, cursor);
        match marks::marked(&marked) {
            Marked::One(stream) => Some(Some(stream)),
            Marked::Several => Some(None),
            Marked::Unmarked if has(Role::Inbox.token()) => Some(Some(ProviderStream::Primary)),
            Marked::Unmarked => None,
        }
    };
    Flags {
        mailbox: Mailbox {
            in_inbox,
            in_spam: has(Role::Junk.token()),
            in_trash: has(Role::Trash.token()),
        },
        read: !has(UNREAD),
        starred: has(STARRED),
        direction,
        categories: Vec::new(),
        labels,
        provider_stream,
        stream_mark_stale: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor() -> Cursor {
        Cursor {
            folders: vec![
                Folder::new("in".into(), "Inbox".into(), Role::Inbox),
                Folder::new("st".into(), "Campbooks/Updates".into(), Role::Stream),
                Folder::new("se".into(), "Sent Items".into(), Role::Sent),
                Folder::new("ju".into(), "Junk Email".into(), Role::Junk),
                Folder::new("tr".into(), "Deleted Items".into(), Role::Trash),
                Folder::new("ar".into(), "Archive".into(), Role::Archive),
                Folder::new("cl".into(), "Clients".into(), Role::User),
            ],
            skipped: vec!["dr".into()],
            ..Cursor::default()
        }
    }

    fn state(read: bool, flagged: bool, categories: &[&str]) -> MessageState {
        MessageState {
            folder: None,
            conversation: None,
            received: None,
            read,
            draft: false,
            flagged,
            categories: categories.iter().map(|c| (*c).to_owned()).collect(),
        }
    }

    #[test]
    fn tokens_say_where_and_how_a_message_is() {
        let cursor = cursor();
        let inbox = tokens(&cursor, "in", &state(false, true, &["Red"]));
        assert_eq!(
            inbox,
            ["INBOX", "STARRED", "UNREAD", "category:Red", "folder:in"]
        );
        let shown = flags(&inbox, &cursor);
        assert!(shown.mailbox.in_inbox && !shown.read && shown.starred);
        assert_eq!(shown.direction, Direction::Inbound);
        assert_eq!(shown.provider_stream, Some(Some(ProviderStream::Primary)));
        assert_eq!(
            shown.labels,
            [Label {
                key: "Red".into(),
                name: "Red".into()
            }]
        );

        let sent = flags(&tokens(&cursor, "se", &state(true, false, &[])), &cursor);
        assert_eq!(sent.direction, Direction::Outbound);
        assert_eq!(sent.provider_stream, None);
        let junk = flags(&tokens(&cursor, "ju", &state(true, false, &[])), &cursor);
        assert!(junk.mailbox.in_spam && !junk.mailbox.in_inbox);
        let trash = flags(&tokens(&cursor, "tr", &state(true, false, &[])), &cursor);
        assert!(trash.mailbox.in_trash);
        let archived = flags(&tokens(&cursor, "ar", &state(true, false, &[])), &cursor);
        assert!(!archived.mailbox.in_inbox && archived.provider_stream.is_none());
        let filed = flags(&tokens(&cursor, "cl", &state(true, false, &[])), &cursor);
        assert!(!filed.mailbox.in_inbox);
        assert_eq!(
            filed.labels,
            [Label {
                key: "cl".into(),
                name: "Clients".into()
            }]
        );
        // A folder not read (Drafts) or not listed: its token only.
        assert_eq!(
            tokens(&cursor, "dr", &state(true, false, &[])),
            ["folder:dr"]
        );
    }

    #[test]
    fn stream_folders_and_categories_mark_a_stream() {
        let cursor = cursor();
        let filed = flags(&tokens(&cursor, "st", &state(true, false, &[])), &cursor);
        assert!(filed.mailbox.in_inbox);
        assert_eq!(filed.provider_stream, Some(Some(ProviderStream::Updates)));
        assert!(filed.labels.is_empty());
        let marked = flags(
            &tokens(&cursor, "in", &state(true, false, &["Campbooks/Receipts"])),
            &cursor,
        );
        assert_eq!(marked.provider_stream, Some(Some(ProviderStream::Receipts)));
        assert!(marked.labels.is_empty(), "a mark is not a label");
        let both = flags(
            &tokens(&cursor, "st", &state(true, false, &["Campbooks/Receipts"])),
            &cursor,
        );
        assert_eq!(both.provider_stream, Some(None));
    }

    #[test]
    fn the_plan_sees_the_inbox_the_bins_and_the_marks() {
        let cursor = cursor();
        let view_of =
            |folder: &str, state: &MessageState| view(&tokens(&cursor, folder, state), &cursor);
        assert_eq!(
            view_of("in", &state(false, true, &["Red"])),
            ["INBOX", "STARRED", "UNREAD"]
        );
        assert_eq!(
            view_of("st", &state(true, false, &[])),
            ["INBOX", "mark:updates"]
        );
        assert_eq!(
            view_of("in", &state(true, false, &["Campbooks/Receipts"])),
            ["INBOX", "mark:receipts"]
        );
        assert_eq!(view_of("tr", &state(true, false, &[])), ["TRASH"]);
        assert_eq!(view_of("ju", &state(true, false, &[])), ["SPAM"]);
        assert!(view_of("ar", &state(true, false, &[])).is_empty());
        assert_eq!(cursor.folder_of_role(Role::Archive).unwrap().id, "ar");
        assert_eq!(
            cursor.stream_folder(ProviderStream::Updates).unwrap().id,
            "st"
        );
        assert!(cursor.stream_folder(ProviderStream::Receipts).is_none());
    }

    #[test]
    fn a_new_pass_reads_every_folder_again() {
        let mut cursor = cursor();
        cursor.folders[0].delta = Some("d".into());
        cursor.folders[1].skip = Some("s".into());
        cursor.lookups.push("m1".into());
        assert!(cursor.folders[0].round_done() && !cursor.folders[1].round_done());
        assert_eq!(cursor.token(), "0:7:1");
        cursor.restart();
        assert_eq!(cursor.generation, 1);
        assert!(
            cursor
                .folders
                .iter()
                .all(|f| f.skip.is_none() && f.delta.is_none())
        );
        assert!(cursor.lookups.is_empty());
        assert!(cursor.knows("dr") && cursor.knows("in") && !cursor.knows("zz"));
    }
}
