//! Making a Zoho stream's pending outbound changes: merge them (the shared
//! plan), turn each message's change into Zoho's `updatemessage` modes, and
//! say how each ended. Zoho keeps no history, so changes carry no stamp and
//! apply in the order they were recorded.
//!
//! - `UNREAD`: `markAsUnread` / `markAsRead`;
//! - `STARRED`: `setFlag` `important` / `flag_not_set`;
//! - `INBOX` off: `archiveMails`; on: `unArchiveMails` for an archived
//!   message, `moveMessage` to the Inbox folder for one in another folder.
//!
//! Messages with the same mode go in one call; a call Zoho refuses as a
//! whole is retried one message at a time to find the one that refuses.

use std::collections::{BTreeMap, HashSet};

use super::{ZohoMail, state};
use crate::{
    domain::{
        outbound::{Backlog, LabelChange, apply_labels},
        push::{Plan, Push, Results, retry_or_give_up},
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
}

impl Op {
    fn mode(&self) -> &'static str {
        match self {
            Self::MarkRead => "markAsRead",
            Self::MarkUnread => "markAsUnread",
            Self::Flag(_) => "setFlag",
            Self::Archive => "archiveMails",
            Self::Unarchive => "unArchiveMails",
            Self::MoveTo(_) => "moveMessage",
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

/// The calls that make `change` on a message whose tokens are `current`,
/// and whether it asks for something Zoho must not be asked: putting back
/// in the inbox a message that is not the member's to move there (their own
/// sent mail, spam, the trash, a folder the adapter does not read). Only an
/// archived message is unarchived, and only one in the member's own folders
/// is moved to the Inbox.
fn ops(change: &LabelChange, current: &[String], inbox: Option<&str>) -> (Vec<Op>, bool) {
    let (add, remove) = change;
    let has = |token: &str| current.iter().any(|t| t == token);
    let mut ops = Vec::new();
    if add.iter().any(|l| l == "UNREAD") {
        ops.push(Op::MarkUnread);
    }
    if remove.iter().any(|l| l == "UNREAD") {
        ops.push(Op::MarkRead);
    }
    if add.iter().any(|l| l == "STARRED") {
        ops.push(Op::Flag("important"));
    }
    if remove.iter().any(|l| l == "STARRED") {
        ops.push(Op::Flag("flag_not_set"));
    }
    if remove.iter().any(|l| l == "INBOX") && has("INBOX") {
        ops.push(Op::Archive);
    }
    let mut unmovable = false;
    if add.iter().any(|l| l == "INBOX") && !has("INBOX") {
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

/// `change` without putting the message back in the inbox.
fn without_inbox(change: &LabelChange) -> LabelChange {
    let (add, remove) = change;
    (
        add.iter().filter(|l| *l != "INBOX").cloned().collect(),
        remove.clone(),
    )
}

/// The tokens a message has once `change` is made: what the index keeps, so
/// the next listing compares equal and nothing is emitted for the write.
pub fn after(current: &[String], change: &LabelChange, inbox: Option<&str>) -> Vec<String> {
    let (add, remove) = change;
    let mut tokens = apply_labels(current, std::slice::from_ref(change));
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
    tokens.sort();
    tokens.dedup();
    tokens
}

/// Push `backlog` through the mail account `mail`, whose Inbox folder is
/// `inbox`.
pub async fn push(mail: &ZohoMail<'_>, backlog: &Backlog, inbox: Option<&str>) -> Push {
    let mut push = Push::default();
    let mut plan = Plan::merge(backlog);
    plan.atomic();
    let mut calls: BTreeMap<Op, Vec<String>> = BTreeMap::new();
    let mut nothing_to_do = Vec::new();
    let mut done = Results::default();
    for (change, messages) in plan.groups() {
        for message in messages {
            let current = backlog.labels.get(&message).cloned().unwrap_or_default();
            let (wanted, unmovable) = ops(&change, &current, inbox);
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
    if push.trouble.is_some() {
        for change in &backlog.pending {
            let end = retry_or_give_up(
                change.id,
                change.attempts,
                &mut push.outcome.reemit,
                &change.provider_message_id,
            );
            push.outcome.ends.push(end);
        }
        return push;
    }
    plan.ends(backlog, &done, &mut push.outcome);
    for message in &done.written {
        let current = backlog.labels.get(message).cloned().unwrap_or_default();
        let mut change = plan.change_of(message);
        if done.unmovable.contains(message) {
            change = without_inbox(&change);
        }
        let tokens = after(&current, &change, inbox);
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
        match mail.update(op.mode(), chunk, &extra).await {
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

async fn call_one(
    mail: &ZohoMail<'_>,
    op: &Op,
    message: &str,
    extra: &serde_json::Map<String, serde_json::Value>,
    failed: &mut HashSet<String>,
    done: &mut Results,
) -> Option<ProviderError> {
    match mail
        .update(op.mode(), std::slice::from_ref(&message.to_owned()), extra)
        .await
    {
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
        let inbox = Some("1");
        let in_inbox = list(&["INBOX", "UNREAD", "folder:1"]);
        let change = |add: &[&str], remove: &[&str]| (list(add), list(remove));
        let made = |c: LabelChange, current: &[String]| ops(&c, current, inbox);
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
    fn the_index_holds_what_was_written() {
        let inbox = Some("1");
        assert_eq!(
            after(
                &list(&["INBOX", "UNREAD", "folder:1"]),
                &(vec![], list(&["INBOX"])),
                inbox
            ),
            list(&["ARCHIVED", "UNREAD", "folder:1"])
        );
        assert_eq!(
            after(
                &list(&["OWN", "folder:7"]),
                &(list(&["INBOX"]), vec![]),
                inbox
            ),
            list(&["INBOX", "folder:1"])
        );
        assert_eq!(
            after(
                &list(&["ARCHIVED", "folder:1"]),
                &(list(&["INBOX", "STARRED"]), vec![]),
                inbox
            ),
            list(&["INBOX", "STARRED", "folder:1"])
        );
    }
}
