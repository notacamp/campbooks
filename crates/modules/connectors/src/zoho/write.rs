//! Making a Zoho stream's pending outbound changes: merge them (the shared
//! plan), turn each message's change into Zoho's `updatemessage` modes, and
//! say how each ended. Zoho keeps no history, so changes carry no stamp and
//! apply in the order they were recorded.
//!
//! - `UNREAD`: `markAsUnread` / `markAsRead`;
//! - `STARRED`: `setFlag` `important` / `flag_not_set`;
//! - `INBOX` off: `archiveMails`; on: `unArchiveMails` for an archived
//!   message, `moveMessage` to the Inbox folder for one in another folder.
//! - `TRASH` on: `moveMessage` to the account's Trash folder; `SPAM` on:
//!   `moveToSpam`, Zoho's report (it moves the message to Spam). Either
//!   remembers where the message was (its folder, and whether it was
//!   archived there).
//! - `label:<id>` on: `applyLabel`; off: `removeLabel`. A stream mark's
//!   token is its label, created first (`POST labels`) when the account has
//!   none of its name (`domain/marks.rs`).
//! - `TRASH` or `SPAM` off: `moveMessage` back to the folder it was in
//!   (archived again if it was). Zoho's own "not spam" mode is documented
//!   two ways (`moveNotSpam`, `markNotSpam`) and is not used. A folder the
//!   account's listing did not show, or a message with nowhere remembered,
//!   is never guessed: that part of the change is left unmade
//!   (`not_movable`).
//!
//! Messages with the same mode go in one call; a call Zoho refuses as a
//! whole is retried one message at a time to find the one that refuses.

use std::collections::{BTreeMap, HashSet};

use super::{ZohoLabel, ZohoMail, state};
use crate::{
    contract::ProviderStream,
    domain::{
        marks::{self, MarkIds},
        outbound::{Backlog, LabelChange, apply_labels},
        push::{Plan, Push, Results, retry_or_give_up, without_marks},
    },
    provider::ProviderError,
};

/// Most messages in one `updatemessage` call.
pub const UPDATE_MAX: usize = 100;

/// One `updatemessage` call: its mode and the fields besides the ids.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Op {
    MarkRead,
    MarkUnread,
    Flag(&'static str),
    Archive,
    Unarchive,
    MoveTo(String),
    /// Report as spam (Zoho moves it to Spam).
    Spam,
    /// Put label `0` on.
    ApplyLabel(String),
    /// Take label `0` off.
    RemoveLabel(String),
    /// Archive again where it was put back (after [`Op::MoveTo`]: calls go
    /// in this order).
    Rearchive,
}

impl Op {
    fn mode(&self) -> &'static str {
        match self {
            Self::MarkRead => "markAsRead",
            Self::MarkUnread => "markAsUnread",
            Self::Flag(_) => "setFlag",
            Self::Archive | Self::Rearchive => "archiveMails",
            Self::Unarchive => "unArchiveMails",
            Self::MoveTo(_) => "moveMessage",
            Self::Spam => "moveToSpam",
            Self::ApplyLabel(_) => "applyLabel",
            Self::RemoveLabel(_) => "removeLabel",
        }
    }

    fn extra(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut extra = serde_json::Map::new();
        match self {
            Self::Flag(flag) => {
                extra.insert("flagid".into(), (*flag).into());
            }
            Self::MoveTo(folder) => {
                let id = folder
                    .parse::<u64>()
                    .map_or_else(|_| folder.clone().into(), serde_json::Value::from);
                extra.insert("destfolderId".into(), id);
                // Archived messages move too.
                extra.insert("isArchive".into(), true.into());
            }
            Self::ApplyLabel(label) | Self::RemoveLabel(label) => {
                let id = label
                    .parse::<u64>()
                    .map_or_else(|_| label.clone().into(), serde_json::Value::from);
                extra.insert("labelId".into(), vec![id].into());
                if matches!(self, Self::ApplyLabel(_)) {
                    // Archived messages are labelled too.
                    extra.insert("isArchive".into(), true.into());
                }
            }
            _ => {}
        }
        extra
    }
}

/// Zoho's calls a push of `messages` messages may make: about one per
/// message and mode, at most.
pub fn requests(messages: usize) -> u32 {
    u32::try_from(messages.saturating_mul(2)).unwrap_or(u32::MAX)
}

/// The folders a push may move messages to, from the account's listing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Folders<'a> {
    /// The Inbox folder.
    pub inbox: Option<&'a str>,
    /// The Trash folder.
    pub trash: Option<&'a str>,
    /// The Spam folder.
    pub spam: Option<&'a str>,
    /// Every folder the listing shows: a remembered folder is used only
    /// while it is still one of them.
    pub listed: &'a [&'a str],
}

/// What a change does to a message's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bin {
    /// Into the trash.
    Trash,
    /// Reported as spam.
    Spam,
    /// Out of the trash or spam, back where it was.
    Out,
}

fn bin(change: &LabelChange, current: &[String]) -> Option<Bin> {
    let (add, remove) = change;
    let has = |token: &str| current.iter().any(|t| t == token);
    let adds = |token: &str| add.iter().any(|l| l == token);
    let removes = |token: &str| remove.iter().any(|l| l == token);
    if adds("TRASH") && !has("TRASH") {
        Some(Bin::Trash)
    } else if adds("SPAM") && !has("SPAM") {
        Some(Bin::Spam)
    } else if (removes("TRASH") && has("TRASH")) || (removes("SPAM") && has("SPAM")) {
        Some(Bin::Out)
    } else {
        None
    }
}

/// Where a message `change` takes out of the trash or spam goes, as the
/// location tokens it will have: out of the trash, back to the folder it was
/// in (`restore`) while the listing still shows that folder and it is
/// neither the trash nor spam; otherwise, for mail the inbox received (the
/// change adds `INBOX`), the Inbox folder; otherwise nowhere (`None`): never
/// a guess.
fn destination(
    change: &LabelChange,
    current: &[String],
    folders: &Folders<'_>,
    restore: Option<&[String]>,
) -> Option<Vec<String>> {
    let (add, remove) = change;
    let from_trash = remove.iter().any(|l| l == "TRASH") && current.iter().any(|t| t == "TRASH");
    if from_trash && let Some(place) = restore {
        let folder = state::folder_of(place);
        let still_there = folder.is_some_and(|f| folders.listed.contains(&f));
        let a_bin = folder.is_some() && (folder == folders.trash || folder == folders.spam);
        if still_there && !a_bin {
            return Some(place.to_vec());
        }
    }
    match folders.inbox {
        Some(inbox) if add.iter().any(|l| l == "INBOX") => {
            Some(vec!["INBOX".to_owned(), state::folder_token(inbox)])
        }
        _ => None,
    }
}

/// The calls that make `change` on a message whose tokens are `current`,
/// and whether it asks for something Zoho must not be asked: putting back
/// in the inbox a message that is not the member's to move there (their own
/// sent mail, spam, the trash, a folder the adapter does not read), a move
/// to a folder the listing did not show, or out of the trash or spam with
/// nowhere remembered (`restore`, the place it was in before). Only an
/// archived message is unarchived, only one in the member's own folders is
/// moved to the Inbox, and a message taken out of the trash or spam goes
/// back to the folder it was in, archived again if it was.
fn ops(
    change: &LabelChange,
    current: &[String],
    folders: &Folders<'_>,
    restore: Option<&[String]>,
) -> (Vec<Op>, bool) {
    let (add, remove) = change;
    let inbox = folders.inbox;
    let has = |token: &str| current.iter().any(|t| t == token);
    let adds = |token: &str| add.iter().any(|l| l == token);
    let removes = |token: &str| remove.iter().any(|l| l == token);
    let mut ops = Vec::new();
    let mut unmovable = false;
    if adds("UNREAD") {
        ops.push(Op::MarkUnread);
    }
    if removes("UNREAD") {
        ops.push(Op::MarkRead);
    }
    if adds("STARRED") {
        ops.push(Op::Flag("important"));
    }
    if removes("STARRED") {
        ops.push(Op::Flag("flag_not_set"));
    }
    for label in add
        .iter()
        .filter(|l| !has(l.as_str()))
        .filter_map(|l| state::label_of(l))
    {
        ops.push(Op::ApplyLabel(label.to_owned()));
    }
    for label in remove
        .iter()
        .filter(|l| has(l.as_str()))
        .filter_map(|l| state::label_of(l))
    {
        ops.push(Op::RemoveLabel(label.to_owned()));
    }
    let moved = bin(change, current);
    match moved {
        Some(Bin::Trash) => match folders.trash {
            Some(folder) => ops.push(Op::MoveTo(folder.to_owned())),
            None => unmovable = true,
        },
        Some(Bin::Spam) => ops.push(Op::Spam),
        Some(Bin::Out) => {
            let to = destination(change, current, folders, restore);
            match to.as_deref().and_then(state::folder_of) {
                Some(folder) => {
                    ops.push(Op::MoveTo(folder.to_owned()));
                    if to
                        .as_ref()
                        .is_some_and(|t| t.iter().any(|token| token == "ARCHIVED"))
                    {
                        ops.push(Op::Rearchive);
                    }
                }
                None => unmovable = true,
            }
        }
        None => {}
    }
    if moved.is_some() {
        return (ops, unmovable);
    }
    if removes("INBOX") && has("INBOX") {
        ops.push(Op::Archive);
    }
    if adds("INBOX") && !has("INBOX") {
        let folder = state::folder_of(current);
        let in_inbox_folder = folder.is_some() && folder == inbox;
        match inbox {
            Some(inbox) if has("OWN") => {
                ops.push(Op::MoveTo(inbox.to_owned()));
                if has("ARCHIVED") {
                    ops.push(Op::Unarchive);
                }
            }
            Some(_) if in_inbox_folder && has("ARCHIVED") => ops.push(Op::Unarchive),
            _ => unmovable = true,
        }
    }
    (ops, unmovable)
}

/// `change` without the moves Zoho was not asked to make: back to the
/// inbox, into or out of the trash or spam.
fn without_inbox(change: &LabelChange) -> LabelChange {
    let (add, remove) = change;
    let moves = |l: &&String| matches!(l.as_str(), "INBOX" | "TRASH" | "SPAM");
    (
        add.iter().filter(|l| !moves(l)).cloned().collect(),
        remove.iter().filter(|l| !moves(l)).cloned().collect(),
    )
}

/// The tokens a message has once `change` is made: what the index keeps, so
/// the next listing compares equal and nothing is emitted for the write.
/// `restore` is where a message taken out of the trash or spam goes back to.
pub fn after(
    current: &[String],
    change: &LabelChange,
    folders: &Folders<'_>,
    restore: Option<&[String]>,
) -> Vec<String> {
    let (add, remove) = change;
    let inbox = folders.inbox;
    let moved = bin(change, current);
    let mut tokens = apply_labels(current, std::slice::from_ref(change));
    // The flags and labels a move between folders keeps.
    let flags = |tokens: &mut Vec<String>| {
        tokens
            .retain(|t| matches!(t.as_str(), "UNREAD" | "STARRED") || state::label_of(t).is_some());
    };
    match moved {
        Some(Bin::Trash | Bin::Spam) => {
            // In that folder, and nothing else.
            let (token, folder) = if moved == Some(Bin::Trash) {
                ("TRASH", folders.trash)
            } else {
                ("SPAM", folders.spam)
            };
            flags(&mut tokens);
            tokens.push(token.to_owned());
            if let Some(folder) = folder {
                tokens.push(state::folder_token(folder));
            }
        }
        Some(Bin::Out) => {
            // Where it went (nowhere when it could not be moved).
            flags(&mut tokens);
            tokens.extend(destination(change, current, folders, restore).unwrap_or_default());
        }
        None => {
            if remove.iter().any(|l| l == "INBOX") && current.iter().any(|t| t == "INBOX") {
                tokens = state::archived(&tokens);
            }
            if add.iter().any(|l| l == "INBOX") {
                tokens.retain(|t| t != "ARCHIVED" && t != "OWN");
                if let Some(inbox) = inbox {
                    tokens.retain(|t| !t.starts_with("folder:"));
                    tokens.push(state::folder_token(inbox));
                }
            }
        }
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// `stream`'s mark at Zoho, created: or found, when the account already
/// has a label of its name the cache did not know.
async fn create_mark(
    mail: &ZohoMail<'_>,
    stream: ProviderStream,
) -> Result<ZohoLabel, ProviderError> {
    let name = marks::name(stream);
    match mail.create_label(name).await {
        Ok(label) => Ok(label),
        Err(ProviderError::Unexpected(refused)) => mail
            .labels()
            .await?
            .into_iter()
            .find(|label| marks::of_name(&label.name()) == Some(stream))
            .ok_or(ProviderError::Unexpected(refused)),
        Err(other) => Err(other),
    }
}

/// Every change of `backlog` pending again (or given up), after `trouble`.
fn stopped(backlog: &Backlog, trouble: ProviderError, push: &mut Push) {
    for change in &backlog.pending {
        let end = retry_or_give_up(
            change.id,
            change.attempts,
            &mut push.outcome.reemit,
            &change.provider_message_id,
        );
        push.outcome.ends.push(end);
    }
    push.trouble = Some(trouble);
}

/// Push `backlog` through the mail account `mail`, whose Inbox, Trash and
/// Spam folders are `folders` and whose stream marks are `marks` (as the
/// cache knows them).
pub async fn push(
    mail: &ZohoMail<'_>,
    backlog: &Backlog,
    folders: &Folders<'_>,
    marks: &MarkIds,
) -> Push {
    let mut push = Push::default();
    // The marks the changes put on, created where Zoho has none yet.
    let mut marks = marks.clone();
    let mut unmade = BTreeMap::new();
    for stream in marks::missing(backlog, &marks) {
        match create_mark(mail, stream).await {
            Ok(label) => {
                let name = label.name();
                marks.insert(stream, label.label_id.clone());
                push.created.push((label.label_id, name));
            }
            // The grant may not make labels (no labels scope), or Zoho
            // refused that one: the changes needing it are not made.
            Err(ProviderError::Unauthorized) => {
                unmade.insert(stream, "scope_missing");
            }
            Err(ProviderError::Unexpected(_) | ProviderError::NotFound) => {
                unmade.insert(stream, "refused");
            }
            Err(trouble) => {
                stopped(backlog, trouble, &mut push);
                return push;
            }
        }
    }
    let backlog = &marks::resolve(
        &without_marks(backlog, &unmade, &mut push.outcome),
        &marks,
        state::label_token,
    );
    let mut plan = Plan::merge(backlog);
    // Zoho keeps no history: the index is what is known, but for labels,
    // which are read only through listings and can be behind.
    let unsure: HashSet<(String, String)> = plan
        .wants
        .iter()
        .flat_map(|(message, labels)| {
            labels
                .keys()
                .filter(|label| state::label_of(label).is_some())
                .map(move |label| (message.clone(), label.clone()))
        })
        .collect();
    plan.in_place(backlog, Some(&unsure));
    plan.atomic();
    let mut calls: BTreeMap<Op, Vec<String>> = BTreeMap::new();
    let mut nothing_to_do = Vec::new();
    let mut done = Results::default();
    for (change, messages) in plan.groups() {
        for message in messages {
            let current = backlog.labels.get(&message).cloned().unwrap_or_default();
            let restore = backlog.restore.get(&message).map(Vec::as_slice);
            let (wanted, unmovable) = ops(&change, &current, folders, restore);
            if bin(&change, &current) == Some(Bin::Trash) && !unmovable {
                // Where it was, so untrash puts it back there.
                done.restore
                    .insert(message.clone(), state::location(&current));
            }
            if unmovable {
                done.unmovable.insert(message.clone());
                if wanted.is_empty() {
                    continue;
                }
            }
            if wanted.is_empty() {
                // Already as wanted at Zoho (an archive of a message out of
                // the inbox): nothing to call, and it counts as made.
                nothing_to_do.push(message);
                continue;
            }
            for op in wanted {
                calls.entry(op).or_default().push(message.clone());
            }
        }
    }
    let mut failed: HashSet<String> = HashSet::new();
    for (op, messages) in &calls {
        if let Some(trouble) = call(mail, op, messages, &mut failed, &mut done).await {
            push.trouble = Some(trouble);
            break;
        }
    }
    if push.trouble.is_none() {
        for messages in calls.values() {
            for message in messages {
                if !failed.contains(message) {
                    done.written.insert(message.clone());
                }
            }
        }
        done.written.extend(nothing_to_do);
    }
    if let Some(trouble) = push.trouble.take() {
        stopped(backlog, trouble, &mut push);
        return push;
    }
    plan.ends(backlog, &done, &mut push.outcome);
    plan.restores(&done, &mut push.outcome);
    for message in &done.written {
        let current = backlog.labels.get(message).cloned().unwrap_or_default();
        let mut change = plan.change_of(message);
        if done.unmovable.contains(message) {
            change = without_inbox(&change);
        }
        let restore = backlog.restore.get(message).map(Vec::as_slice);
        let tokens = after(&current, &change, folders, restore);
        push.outcome.index.push((message.clone(), tokens));
    }
    push.outcome.index.sort();
    push.outcome.reemit.sort();
    push.outcome.reemit.dedup();
    push
}

/// Make `op` for `messages`, in chunks; a chunk Zoho refuses as a whole is
/// retried one by one. Messages that fail land in `failed` and `done`'s
/// `refused` or `gone`. Returns the trouble that stopped it.
async fn call(
    mail: &ZohoMail<'_>,
    op: &Op,
    messages: &[String],
    failed: &mut HashSet<String>,
    done: &mut Results,
) -> Option<ProviderError> {
    let extra = op.extra();
    for chunk in messages.chunks(UPDATE_MAX) {
        match refusal(op, mail.update(op.mode(), chunk, &extra).await) {
            Ok(()) => {}
            Err(ProviderError::NotFound | ProviderError::Unexpected(_)) if chunk.len() > 1 => {
                for message in chunk {
                    if let Some(trouble) = call_one(mail, op, message, &extra, failed, done).await {
                        return Some(trouble);
                    }
                }
            }
            Err(ProviderError::NotFound) => {
                failed.extend(chunk.iter().cloned());
                done.gone.extend(chunk.iter().cloned());
            }
            Err(ProviderError::Unexpected(_)) => {
                tracing::info!("Zoho refused a change to one message");
                failed.extend(chunk.iter().cloned());
                done.refused.extend(chunk.iter().cloned());
            }
            Err(trouble) => return Some(trouble),
        }
    }
    None
}

/// A label change Zoho refuses the grant (it may lack the labels scope) is
/// a refusal of that change, never trouble with the grant.
fn refusal(op: &Op, result: Result<(), ProviderError>) -> Result<(), ProviderError> {
    match result {
        Err(ProviderError::Unauthorized)
            if matches!(op, Op::ApplyLabel(_) | Op::RemoveLabel(_)) =>
        {
            Err(ProviderError::Unexpected("labels refused".into()))
        }
        other => other,
    }
}

async fn call_one(
    mail: &ZohoMail<'_>,
    op: &Op,
    message: &str,
    extra: &serde_json::Map<String, serde_json::Value>,
    failed: &mut HashSet<String>,
    done: &mut Results,
) -> Option<ProviderError> {
    match refusal(
        op,
        mail.update(op.mode(), std::slice::from_ref(&message.to_owned()), extra)
            .await,
    ) {
        Ok(()) => None,
        Err(ProviderError::NotFound) => {
            failed.insert(message.to_owned());
            done.gone.insert(message.to_owned());
            None
        }
        Err(ProviderError::Unexpected(_)) => {
            tracing::info!("Zoho refused a change to one message");
            failed.insert(message.to_owned());
            done.refused.insert(message.to_owned());
            None
        }
        Err(trouble) => Some(trouble),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|&s| s.to_owned()).collect()
    }

    #[test]
    fn changes_become_zoho_modes() {
        let folders = Folders {
            inbox: Some("1"),
            trash: Some("5"),
            spam: Some("4"),
            listed: &["1", "4", "5", "7"],
        };
        let in_inbox = list(&["INBOX", "UNREAD", "folder:1"]);
        let change = |add: &[&str], remove: &[&str]| (list(add), list(remove));
        let made = |c: LabelChange, current: &[String]| ops(&c, current, &folders, None);
        assert_eq!(
            made(change(&[], &["UNREAD"]), &in_inbox),
            (vec![Op::MarkRead], false)
        );
        assert_eq!(
            made(change(&["STARRED"], &[]), &in_inbox),
            (vec![Op::Flag("important")], false)
        );
        assert_eq!(
            made(change(&[], &["INBOX"]), &in_inbox),
            (vec![Op::Archive], false)
        );
        let archived = list(&["ARCHIVED", "folder:1"]);
        assert_eq!(
            made(change(&["INBOX"], &[]), &archived),
            (vec![Op::Unarchive], false)
        );
        let own = list(&["OWN", "folder:7"]);
        assert_eq!(
            made(change(&["INBOX"], &[]), &own),
            (vec![Op::MoveTo("1".into())], false)
        );
        // Archiving what is not in the inbox needs no call.
        assert_eq!(made(change(&[], &["INBOX"]), &own), (vec![], false));
        // The member's sent mail, spam, the trash and folders the adapter
        // does not read never move to the Inbox.
        for current in [
            list(&["SENT", "folder:3"]),
            list(&["SPAM", "folder:4"]),
            list(&["TRASH", "folder:5"]),
            list(&["folder:6"]),
        ] {
            assert_eq!(
                made(change(&["INBOX"], &[]), &current),
                (vec![], true),
                "{current:?}"
            );
        }
        // The rest of a change to such a message is still made.
        assert_eq!(
            made(
                change(&["INBOX", "STARRED"], &[]),
                &list(&["SENT", "folder:3"])
            ),
            (vec![Op::Flag("important")], true)
        );
        // Already in the inbox: nothing to do.
        assert_eq!(made(change(&["INBOX"], &[]), &in_inbox), (vec![], false));
    }

    #[test]
    fn trash_and_spam_move_and_come_back_where_they_were() {
        let folders = Folders {
            inbox: Some("1"),
            trash: Some("5"),
            spam: Some("4"),
            listed: &["1", "3", "4", "5", "7"],
        };
        let change = |add: &[&str], remove: &[&str]| (list(add), list(remove));
        let made = |c: LabelChange, current: &[String], restore: Option<&[String]>| {
            ops(&c, current, &folders, restore)
        };
        let in_inbox = list(&["INBOX", "UNREAD", "folder:1"]);
        let trashed = list(&["TRASH", "UNREAD", "folder:5"]);
        let spam = list(&["SPAM", "folder:4"]);
        // Inbound mail out of the trash or spam carries INBOX; sent mail not.
        let untrash = change(&["INBOX"], &["TRASH"]);
        let untrash_sent = change(&[], &["TRASH"]);
        let unspam = change(&["INBOX"], &["SPAM"]);
        let unspam_sent = change(&[], &["SPAM"]);
        assert_eq!(
            made(change(&["TRASH"], &["SPAM"]), &in_inbox, None),
            (vec![Op::MoveTo("5".into())], false)
        );
        // Spam is Zoho's report, which moves the message to Spam itself.
        assert_eq!(
            made(change(&["SPAM"], &["TRASH"]), &in_inbox, None),
            (vec![Op::Spam], false)
        );
        // An archived message or one in an own folder goes too.
        assert_eq!(
            made(change(&["TRASH"], &[]), &list(&["OWN", "folder:7"]), None),
            (vec![Op::MoveTo("5".into())], false)
        );
        // Out of the trash: back to the folder it was in, archived again if
        // it was, even for sent mail.
        let was_archived_own = list(&["ARCHIVED", "OWN", "folder:7"]);
        let was_sent = list(&["SENT", "folder:3"]);
        assert_eq!(
            made(untrash.clone(), &trashed, Some(was_archived_own.as_slice())),
            (vec![Op::MoveTo("7".into()), Op::Rearchive], false)
        );
        assert_eq!(
            made(untrash_sent.clone(), &trashed, Some(was_sent.as_slice())),
            (vec![Op::MoveTo("3".into())], false)
        );
        // Nowhere remembered: received mail goes to the Inbox, sent mail
        // stays where it is.
        assert_eq!(
            made(untrash.clone(), &trashed, None),
            (vec![Op::MoveTo("1".into())], false)
        );
        assert_eq!(made(untrash_sent.clone(), &trashed, None), (vec![], true));
        // A folder deleted since, or one that was the spam: the same.
        let was_deleted = list(&["OWN", "folder:9"]);
        let was_spam = list(&["folder:4"]);
        assert_eq!(
            made(untrash.clone(), &trashed, Some(was_deleted.as_slice())),
            (vec![Op::MoveTo("1".into())], false)
        );
        assert_eq!(
            made(untrash_sent.clone(), &trashed, Some(was_deleted.as_slice())),
            (vec![], true)
        );
        assert_eq!(
            made(untrash.clone(), &trashed, Some(was_spam.as_slice())),
            (vec![Op::MoveTo("1".into())], false)
        );
        // Out of spam: received mail to the Inbox, never anywhere remembered;
        // sent mail is not moved.
        assert_eq!(
            made(unspam.clone(), &spam, Some(was_archived_own.as_slice())),
            (vec![Op::MoveTo("1".into())], false)
        );
        assert_eq!(made(unspam_sent, &spam, None), (vec![], true));
        // Not in spam: nothing taken out, and never out of the trash.
        assert_eq!(made(unspam, &trashed, None), (vec![], true));
        // Already there: nothing to call.
        assert_eq!(
            made(change(&["TRASH"], &[]), &trashed, None),
            (vec![], false)
        );
        // A Trash folder the listing did not show is never guessed.
        let unknown = Folders {
            inbox: Some("1"),
            ..Folders::default()
        };
        assert_eq!(
            ops(
                &change(&["TRASH", "STARRED"], &[]),
                &in_inbox,
                &unknown,
                None
            ),
            (vec![Op::Flag("important")], true)
        );
        assert_eq!(
            without_inbox(&change(&["TRASH", "STARRED"], &["UNREAD", "SPAM"])),
            change(&["STARRED"], &["UNREAD"])
        );
    }

    #[test]
    fn the_index_holds_where_trash_and_spam_put_it() {
        let folders = Folders {
            inbox: Some("1"),
            trash: Some("5"),
            spam: Some("4"),
            listed: &["1", "3", "4", "5", "7"],
        };
        let change = |add: &[&str], remove: &[&str]| (list(add), list(remove));
        let in_inbox = list(&["INBOX", "UNREAD", "folder:1"]);
        let trashed = list(&["TRASH", "UNREAD", "folder:5"]);
        let untrash = change(&["INBOX"], &["TRASH"]);
        let was_archived_own = list(&["ARCHIVED", "OWN", "folder:7"]);
        // What the index keeps.
        assert_eq!(
            after(&in_inbox, &change(&["TRASH"], &["SPAM"]), &folders, None),
            list(&["TRASH", "UNREAD", "folder:5"])
        );
        assert_eq!(
            after(&in_inbox, &change(&["SPAM"], &["TRASH"]), &folders, None),
            list(&["SPAM", "UNREAD", "folder:4"])
        );
        assert_eq!(
            after(
                &trashed,
                &untrash,
                &folders,
                Some(was_archived_own.as_slice())
            ),
            list(&["ARCHIVED", "OWN", "UNREAD", "folder:7"])
        );
        assert_eq!(
            after(&trashed, &untrash, &folders, None),
            list(&["INBOX", "UNREAD", "folder:1"])
        );
        assert_eq!(state::location(&in_inbox), list(&["INBOX", "folder:1"]));
    }

    #[test]
    fn the_index_holds_what_was_written() {
        let folders = Folders {
            inbox: Some("1"),
            ..Folders::default()
        };
        let inbox = &folders;
        assert_eq!(
            after(
                &list(&["INBOX", "UNREAD", "folder:1"]),
                &(vec![], list(&["INBOX"])),
                inbox,
                None
            ),
            list(&["ARCHIVED", "UNREAD", "folder:1"])
        );
        assert_eq!(
            after(
                &list(&["OWN", "folder:7"]),
                &(list(&["INBOX"]), vec![]),
                inbox,
                None
            ),
            list(&["INBOX", "folder:1"])
        );
        assert_eq!(
            after(
                &list(&["ARCHIVED", "folder:1"]),
                &(list(&["INBOX", "STARRED"]), vec![]),
                inbox,
                None
            ),
            list(&["INBOX", "STARRED", "folder:1"])
        );
    }
}
