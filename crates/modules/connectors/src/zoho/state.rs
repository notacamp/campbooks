//! Zoho's vocabulary into the contract's. A Zoho message is in one folder,
//! read or not, flagged or not, archived or not. The index keeps that as
//! label-like tokens, so the outbound-change machinery (merge, overlay,
//! echo-free writes) is the same as Gmail's:
//!
//! `folder:<id>`, `INBOX` (in the Inbox folder and not archived), `SENT`,
//! `SPAM`, `TRASH`, `OWN` (one of the member's own folders), `UNREAD`,
//! `STARRED` (any flag), `ARCHIVED`, and `label:<id>` per Zoho label (a
//! stream mark too) when the grant has the labels scope. A message in a
//! folder the adapter does not read (Snoozed, Outbox, a folder made since
//! the last sweep) carries its folder token and labels only.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    contract::{Direction, Label, Mailbox, ProviderStream},
    domain::marks::{self, Marked},
    mail::Flags,
};

/// The prefix of a Zoho label's token.
const LABEL: &str = "label:";

/// The token of Zoho label `id`.
pub fn label_token(id: &str) -> String {
    format!("{LABEL}{id}")
}

/// The Zoho label a token names.
pub fn label_of(token: &str) -> Option<&str> {
    token.strip_prefix(LABEL)
}

/// `tokens` with the labels a listing says (`listed`), or, when it says
/// none, with the labels `known` (the index's tokens) had: a listing that
/// does not name labels changes none. Sorted.
pub fn with_labels(
    mut tokens: Vec<String>,
    listed: Option<&[String]>,
    known: Option<&[String]>,
) -> Vec<String> {
    match listed {
        Some(ids) => tokens.extend(ids.iter().map(|id| label_token(id))),
        None => tokens.extend(
            known
                .unwrap_or_default()
                .iter()
                .filter(|t| label_of(t).is_some())
                .cloned(),
        ),
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

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

/// The tokens Zoho can be asked to change: the flags of the intents, the
/// trash and spam (moves between folders), a label and a stream mark.
pub fn writable(token: &str) -> bool {
    matches!(token, "UNREAD" | "STARRED" | "INBOX" | "TRASH" | "SPAM")
        || label_of(token).is_some_and(|id| !id.is_empty())
        || marks::of_token(token).is_some()
}

/// Where a message with `tokens` is: its folder and the tokens that say
/// what kind of place that is (and whether it is archived there). What a
/// move to the trash or spam remembers, to put the message back.
pub fn location(tokens: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tokens
        .iter()
        .filter(|t| {
            t.starts_with("folder:") || matches!(t.as_str(), "INBOX" | "SENT" | "OWN" | "ARCHIVED")
        })
        .cloned()
        .collect();
    out.sort();
    out
}

/// The contract's state of a message with `tokens`, its labels named by
/// `names` (id → name): its labels other than stream marks, and for mail
/// the inbox received, its provider stream (its stream mark; Primary
/// without one; none with several). A label `names` does not know is not
/// carried.
pub fn flags_with(tokens: &[String], names: &HashMap<String, String>) -> Flags {
    let mut flags = flags(tokens);
    let name_of = |token: &str| {
        label_of(token)
            .and_then(|id| names.get(id))
            .map(String::as_str)
    };
    let mut labels: Vec<Label> = tokens
        .iter()
        .filter_map(|token| {
            let id = label_of(token)?;
            let name = names.get(id)?;
            marks::of_name(name).is_none().then(|| Label {
                key: id.to_owned(),
                name: name.clone(),
            })
        })
        .collect();
    labels.sort_by(|a, b| a.key.cmp(&b.key));
    flags.labels = labels;
    flags.provider_stream = (flags.direction == Direction::Inbound).then(|| {
        match marks::marked(&marks::marks_in(tokens, name_of)) {
            Marked::One(stream) => Some(stream),
            Marked::Several => None,
            Marked::Unmarked => Some(ProviderStream::Primary),
        }
    });
    flags
}

/// The contract's state of a message with `tokens`, without its labels or
/// stream (a grant without the labels scope, or the member's sent mail).
/// Zoho has no categories.
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
        provider_stream: None,
        stream_mark_stale: false,
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
        assert!(writable("INBOX") && writable("TRASH") && writable("SPAM"));
        assert!(!writable("folder:11") && !writable("SENT") && !writable("ARCHIVED"));
        assert_eq!(
            location(&tokens("11", FolderKind::Other, false, true, true)),
            ["ARCHIVED", "OWN", "folder:11"]
        );
    }

    #[test]
    fn labels_and_marks_are_read_by_name() {
        let names = HashMap::from([
            ("71".to_owned(), "Clients".to_owned()),
            ("72".to_owned(), "Campbooks/Newsletters".to_owned()),
            ("73".to_owned(), "Campbooks/Updates".to_owned()),
        ]);
        let inbox = tokens("11", FolderKind::Inbox, true, false, false);
        let listed = |ids: &[&str]| {
            let ids: Vec<String> = ids.iter().map(|&s| s.to_owned()).collect();
            with_labels(inbox.clone(), Some(ids.as_slice()), None)
        };
        let plain = flags_with(&listed(&["71"]), &names);
        assert_eq!(
            plain.labels,
            [Label {
                key: "71".into(),
                name: "Clients".into()
            }]
        );
        assert_eq!(plain.provider_stream, Some(Some(ProviderStream::Primary)));
        let marked = flags_with(&listed(&["71", "72"]), &names);
        assert_eq!(marked.labels.len(), 1);
        assert_eq!(
            marked.provider_stream,
            Some(Some(ProviderStream::Newsletters))
        );
        assert_eq!(
            flags_with(&listed(&["72", "73"]), &names).provider_stream,
            Some(None)
        );
        // A label not known by name is not carried.
        assert!(flags_with(&listed(&["99"]), &names).labels.is_empty());
        // Sent mail has no stream; without names (no scope) nothing is.
        let sent = with_labels(
            tokens("12", FolderKind::Sent, true, false, false),
            Some(["72".to_owned()].as_slice()),
            None,
        );
        assert_eq!(flags_with(&sent, &names).provider_stream, None);
        assert_eq!(flags(&listed(&["72"])).provider_stream, None);
        // A listing that names no labels keeps the index's.
        let known = listed(&["71"]);
        assert_eq!(
            with_labels(inbox.clone(), None, Some(known.as_slice())),
            known
        );
        assert!(writable("label:71") && writable("mark:receipts"));
        assert!(!writable("label:") && !writable("mark:someday"));
    }
}
