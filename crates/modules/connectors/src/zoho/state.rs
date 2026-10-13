//! Zoho's vocabulary into the contract's. A Zoho message is in one folder,
//! read or not, flagged or not, archived or not. The index keeps that as
//! label-like tokens, so the outbound-change machinery (merge, overlay,
//! echo-free writes) is the same as Gmail's:
//!
//! `folder:<id>`, `INBOX` (in the Inbox folder and not archived), `SENT`,
//! `SPAM`, `TRASH`, `OWN` (one of the member's own folders), `UNREAD`,
//! `STARRED` (any flag), `ARCHIVED`. A message in a folder the adapter does
//! not read (Snoozed, Outbox, a folder made since the last sweep) carries
//! its folder token only.

use serde::{Deserialize, Serialize};

use crate::{
    contract::{Direction, Mailbox},
    mail::Flags,
};

/// What a folder is to the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderKind {
    /// The Inbox.
    Inbox,
    /// Sent mail.
    Sent,
    /// Spam.
    Spam,
    /// The trash.
    Trash,
    /// One of the member's own folders.
    Other,
    /// A folder the adapter does not read (drafts, outbox, snoozed), or one
    /// it has not seen yet. Never a place to move mail out of.
    Untracked,
}

impl FolderKind {
    /// The kind of a folder of `folder_type` at `path`; `None` for folders
    /// the adapter never reads (drafts, templates, outbox, snoozed).
    pub fn of(folder_type: &str, path: &str) -> Option<Self> {
        let path = path.trim_matches('/');
        match folder_type.to_ascii_lowercase().as_str() {
            "inbox" if path.eq_ignore_ascii_case("inbox") => Some(Self::Inbox),
            "sent" => Some(Self::Sent),
            "spam" => Some(Self::Spam),
            "trash" => Some(Self::Trash),
            "drafts" | "templates" | "outbox" | "snoozed" => None,
            _ => Some(Self::Other),
        }
    }
}

/// The token of folder `id`.
pub fn folder_token(id: &str) -> String {
    format!("folder:{id}")
}

/// The folder a message's tokens put it in.
pub fn folder_of(tokens: &[String]) -> Option<&str> {
    tokens.iter().find_map(|t| t.strip_prefix("folder:"))
}

/// The tokens of a message in folder `folder` of `kind`, sorted.
pub fn tokens(
    folder: &str,
    kind: FolderKind,
    read: bool,
    flagged: bool,
    archived: bool,
) -> Vec<String> {
    let mut tokens = vec![folder_token(folder)];
    let mut push = |token: &str, on: bool| {
        if on {
            tokens.push(token.to_owned());
        }
    };
    push("INBOX", kind == FolderKind::Inbox && !archived);
    push("SENT", kind == FolderKind::Sent);
    push("SPAM", kind == FolderKind::Spam);
    push("TRASH", kind == FolderKind::Trash);
    push("OWN", kind == FolderKind::Other);
    push("UNREAD", !read);
    push("STARRED", flagged);
    push("ARCHIVED", archived);
    tokens.sort();
    tokens
}

/// `tokens` with the message archived in place: out of the inbox.
pub fn archived(tokens: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tokens.iter().filter(|t| *t != "INBOX").cloned().collect();
    if !out.iter().any(|t| t == "ARCHIVED") {
        out.push("ARCHIVED".to_owned());
    }
    out.sort();
    out
}

/// The tokens Zoho can be asked to change: the flags of the intents.
pub fn writable(token: &str) -> bool {
    matches!(token, "UNREAD" | "STARRED" | "INBOX")
}

/// The contract's state of a message with `tokens`. Zoho has no categories,
/// and its labels are not carried.
pub fn flags(tokens: &[String]) -> Flags {
    let has = |token: &str| tokens.iter().any(|t| t == token);
    let in_inbox = has("INBOX");
    Flags {
        mailbox: Mailbox {
            in_inbox,
            in_spam: has("SPAM"),
            in_trash: has("TRASH"),
        },
        read: !has("UNREAD"),
        starred: has("STARRED"),
        direction: if has("SENT") && !in_inbox {
            Direction::Outbound
        } else {
            Direction::Inbound
        },
        categories: Vec::new(),
        labels: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_read_by_type_and_path() {
        assert_eq!(FolderKind::of("Inbox", "/Inbox"), Some(FolderKind::Inbox));
        // Zoho calls some of the member's own folders `Inbox` too.
        assert_eq!(FolderKind::of("Inbox", "/Clients"), Some(FolderKind::Other));
        assert_eq!(FolderKind::of("Sent", "/Sent"), Some(FolderKind::Sent));
        assert_eq!(FolderKind::of("Trash", "/Trash"), Some(FolderKind::Trash));
        for skipped in ["Drafts", "Templates", "Outbox", "Snoozed"] {
            assert_eq!(FolderKind::of(skipped, "/x"), None, "{skipped}");
        }
    }

    #[test]
    fn tokens_say_where_and_how_a_message_is() {
        let own = tokens("14", FolderKind::Other, true, false, false);
        assert_eq!(own, ["OWN", "folder:14"]);
        assert_eq!(
            tokens("15", FolderKind::Untracked, true, false, false),
            ["folder:15"]
        );
        let inbox = tokens("11", FolderKind::Inbox, false, true, false);
        assert_eq!(inbox, ["INBOX", "STARRED", "UNREAD", "folder:11"]);
        let state = flags(&inbox);
        assert!(state.mailbox.in_inbox && !state.read && state.starred);
        assert_eq!(state.direction, Direction::Inbound);
        assert_eq!(folder_of(&inbox), Some("11"));

        let gone = archived(&inbox);
        assert_eq!(gone, ["ARCHIVED", "STARRED", "UNREAD", "folder:11"]);
        assert!(!flags(&gone).mailbox.in_inbox);

        let sent = flags(&tokens("12", FolderKind::Sent, true, false, false));
        assert_eq!(sent.direction, Direction::Outbound);
        let trash = flags(&tokens("13", FolderKind::Trash, true, false, false));
        assert!(trash.mailbox.in_trash && !trash.mailbox.in_inbox);
        assert!(writable("INBOX") && !writable("TRASH") && !writable("folder:11"));
    }
}
