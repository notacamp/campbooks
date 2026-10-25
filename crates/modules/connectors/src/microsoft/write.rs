//! Making a Microsoft stream's pending outbound changes (campbooks-cloud#317),
//! before its run reads: merge them (the shared plan, over the index's tokens
//! as [`state::view`] shows them), read each message's state and version at
//! Graph, turn the change into a patch and a move, make them, and say how
//! each ended with the tokens the message has now, for the run to record
//! before it reads.
//!
//! - `UNREAD` / `STARRED`: `isRead` / `flag.flagStatus`, patched with
//!   `If-Match` the version just read.
//! - `INBOX` off: from the Inbox or a stream folder to Archive; on: from
//!   Archive or a member's folder to the Inbox.
//! - `TRASH` on: to Deleted Items, where the message was kept (bound into
//!   the change, as Zoho's); off: back there while it is read and not a bin,
//!   else (mail the inbox received) to the Inbox. `SPAM` on: to Junk Email;
//!   off: (mail the inbox received) to the Inbox.
//! - `mark:<stream>`: mail in the inbox to that stream's folder
//!   (`Campbooks/<Stream>`) when the mailbox has one, else to the Inbox with
//!   the category `Campbooks/<Stream>`; every mark off: to the Inbox, without
//!   stream categories. Other categories are kept.
//!
//! A token the change touches that Graph changed since the last read to
//! something other than what the member asks for is Graph's: the change is
//! `superseded` (`changed_at_provider`). A 412 re-reads the message once.
//! A place the cursor does not know is never guessed (`not_movable`).
//! Nothing here deletes a message or a folder, or empties one. Logs carry
//! counts only.

use std::collections::HashSet;

use super::{
    Microsoft,
    graph::MessageState,
    graph_write::{Current, Moved, Patch, Patched},
    state::{self, Cursor},
};
use crate::{
    contract::ProviderStream,
    domain::{
        marks,
        outbound::{Backlog, LabelChange, Outcome, State},
        push::{Fate, Plan, Push, Results},
    },
    imap::folders::Role,
    provider::ProviderError,
};

/// Most messages one run writes: the longest prefix of the backlog that
/// touches no more ([`bounded`]); the rest wait for the run after, at once.
pub const MAX_MESSAGES: usize = 50;

/// Graph requests one message's change may take: a read, a patch, a read
/// again after a conflict, a move.
pub const REQUESTS_PER_MESSAGE: u32 = 4;

/// Whether a change may put on or take off `token` at Microsoft: read,
/// starred, the inbox, the trash, spam and stream marks only.
pub fn writable(token: &str) -> bool {
    matches!(token, "UNREAD" | "STARRED" | "INBOX" | "TRASH" | "SPAM")
        || marks::of_token(token).is_some()
}

/// The Graph requests a push of `messages` messages may make.
pub fn requests(messages: usize) -> u32 {
    u32::try_from(messages.min(MAX_MESSAGES))
        .unwrap_or(u32::MAX)
        .saturating_mul(REQUESTS_PER_MESSAGE)
}

/// `backlog` cut to its longest prefix (in recorded order) that touches at
/// most [`MAX_MESSAGES`] messages: every change left out is newer than the
/// last one kept, so the run is followed by another at once.
pub fn bounded(backlog: &Backlog) -> Backlog {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut take = 0;
    for change in &backlog.pending {
        let message = change.provider_message_id.as_str();
        if !seen.contains(message) {
            if seen.len() >= MAX_MESSAGES {
                break;
            }
            seen.insert(message);
        }
        take += 1;
    }
    let mut kept = backlog.clone();
    kept.pending.truncate(take);
    kept
}

/// What a push did, for the run to record before it reads.
#[derive(Debug, Default)]
pub struct Pushed {
    /// How each change ended, and the index tokens written messages have.
    pub push: Push,
    /// Messages a move may have given another id: the read looks them up.
    pub lookups: Vec<String>,
    /// A move found no folder where the cursor has one: the read lists the
    /// folders again.
    pub relist: bool,
}

/// What one message's change asks of Graph, as the message is now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Wanted {
    patch: Patch,
    /// The folder it moves to, when it moves.
    folder: Option<String>,
    /// Where it was, when the change puts it in the trash: so taking it out
    /// puts it back there.
    origin: Option<Vec<String>>,
    /// Part of the change has nowhere to go: that part is not made.
    unmovable: bool,
}

/// Whether a change moves the message: to or from the inbox, the trash,
/// spam, or a stream.
fn moves(change: &LabelChange) -> bool {
    let (add, remove) = change;
    add.iter()
        .chain(remove)
        .any(|t| matches!(t.as_str(), "INBOX" | "TRASH" | "SPAM") || marks::of_token(t).is_some())
}

/// What `change` (the plan's tokens to put on and take off) asks of a
/// message whose state at Graph is `now`, in a mailbox whose folders are
/// `cursor`'s (`restore`: where it was before Campbooks put it in the
/// trash, as bound).
#[allow(
    clippy::too_many_lines,
    reason = "one table of intents to Graph's calls, read top to bottom"
)]
fn wanted(
    change: &LabelChange,
    now: &MessageState,
    cursor: &Cursor,
    restore: Option<&[String]>,
) -> Wanted {
    let (add, remove) = change;
    let adds = |token: &str| add.iter().any(|l| l == token);
    let removes = |token: &str| remove.iter().any(|l| l == token);
    let mut wanted = Wanted::default();
    if adds(state::UNREAD) && now.read {
        wanted.patch.read = Some(false);
    } else if removes(state::UNREAD) && !now.read {
        wanted.patch.read = Some(true);
    }
    if adds(state::STARRED) && !now.flagged {
        wanted.patch.flagged = Some(true);
    } else if removes(state::STARRED) && now.flagged {
        wanted.patch.flagged = Some(false);
    }
    let here = now.folder.as_deref().and_then(|id| cursor.folder(id));
    let role = here.map(|f| f.role);
    let in_inbox = matches!(role, Some(Role::Inbox | Role::Stream));
    let of_role = |role: Role| cursor.folder_of_role(role).map(|f| f.id.as_str());
    let inbox = of_role(Role::Inbox);
    let mark = add.iter().find_map(|t| marks::of_token(t));
    let stream_move = mark.is_some() || remove.iter().any(|t| marks::of_token(t).is_some());
    // Where mail going to the inbox goes: its stream's folder, else the
    // Inbox.
    let inbox_place = match mark {
        Some(stream) if stream != ProviderStream::Primary => cursor
            .stream_folder(stream)
            .map(|f| f.id.as_str())
            .or(inbox),
        _ => inbox,
    };
    // `None`: no move asked; `Some(None)`: asked, with nowhere to go.
    let to: Option<Option<&str>> = if here.is_none() {
        // In a folder the reader does not read (or none Graph named): no
        // place is guessed.
        moves(change).then_some(None)
    } else if adds("TRASH") {
        (role != Some(Role::Trash)).then(|| of_role(Role::Trash))
    } else if adds("SPAM") {
        (role != Some(Role::Junk)).then(|| of_role(Role::Junk))
    } else if removes("TRASH") && role == Some(Role::Trash) {
        let back = restore.and_then(state::folder_of).filter(|id| {
            cursor
                .folder(id)
                .is_some_and(|f| !matches!(f.role, Role::Trash | Role::Junk))
        });
        Some(back.or(if adds("INBOX") { inbox_place } else { None }))
    } else if removes("SPAM") && role == Some(Role::Junk) {
        Some(if adds("INBOX") { inbox_place } else { None })
    } else if removes("INBOX") && in_inbox {
        Some(of_role(Role::Archive))
    } else if adds("INBOX") && !in_inbox {
        Some(match role {
            Some(Role::Archive | Role::User) => inbox_place,
            _ => None,
        })
    } else if stream_move {
        Some(if in_inbox { inbox_place } else { None })
    } else {
        None
    };
    match to {
        Some(None) => wanted.unmovable = true,
        Some(Some(folder)) if Some(folder) != now.folder.as_deref() => {
            wanted.folder = Some(folder.to_owned());
        }
        // No move asked, or already there.
        _ => {}
    }
    if wanted.folder.is_some() && adds("TRASH") && role != Some(Role::Junk) {
        // Where Graph says it is, never only what the index holds.
        wanted.origin = now
            .folder
            .as_deref()
            .map(|folder| vec![state::folder_token(folder)]);
    }
    // A stream is the folder the message ends in, else its category.
    let ends_in = wanted
        .folder
        .as_deref()
        .or(now.folder.as_deref())
        .and_then(|id| cursor.folder(id));
    let ends_in_inbox = ends_in.is_some_and(|f| matches!(f.role, Role::Inbox | Role::Stream));
    if stream_move && !wanted.unmovable && ends_in_inbox {
        let by_folder = ends_in.is_some_and(|f| f.role == Role::Stream);
        let mut categories: Vec<String> = now
            .categories
            .iter()
            .filter(|c| marks::of_name(c).is_none())
            .cloned()
            .collect();
        if let Some(stream) = mark.filter(|s| *s != ProviderStream::Primary)
            && !by_folder
        {
            categories.push(marks::name(stream).to_owned());
        }
        categories.sort();
        categories.dedup();
        if categories != now.categories {
            wanted.patch.categories = Some(categories);
        }
    }
    wanted
}

/// Whether Graph changed, since the index was read (`before`, the plan's
/// view of it), a token of `wants` (each with whether the member wants it
/// on) to something other than what the member asks for (`now`, the plan's
/// view of Graph's state).
fn overtaken(wants: &[(String, bool)], before: &[String], now: &[String]) -> bool {
    wants.iter().any(|(token, present)| {
        let was = before.contains(token);
        let is = now.contains(token);
        was != is && is != *present
    })
}

/// The index's tokens of a message in state `state`, in the folder it names
/// (else `fallback`, the folder the index has).
fn tokens_of(cursor: &Cursor, state: &MessageState, fallback: &str) -> Vec<String> {
    state::tokens(cursor, state.folder.as_deref().unwrap_or(fallback), state)
}

/// How one message's write ended.
#[derive(Debug)]
enum Wrote {
    /// Made, or Graph had it so already: the tokens it has now.
    Made {
        tokens: Vec<String>,
        /// Part of the change had nowhere to go.
        unmovable: bool,
        /// Where a message put in the trash was.
        origin: Option<Vec<String>>,
        /// A move's answer named another id, or none.
        renamed: bool,
        /// Graph had changed it since the last read.
        changed: bool,
    },
    /// Graph changed what the change touches since the last read: its
    /// state (the tokens) stands.
    Overtaken { tokens: Vec<String> },
    /// Graph refused it; the tokens it has now, when known.
    Refused {
        tokens: Option<Vec<String>>,
        /// A move found no such folder.
        relist: bool,
    },
    /// Graph no longer has the message.
    Gone,
}

/// Trouble that stopped the push, and the tokens of a message part written
/// when it struck (its patch made, its move not).
#[derive(Debug)]
struct Stop {
    trouble: ProviderError,
    tokens: Option<Vec<String>>,
}

/// One message's change, as the push makes it.
struct One<'a> {
    message: &'a str,
    change: &'a LabelChange,
    /// The tokens the change won, each with whether it is wanted on.
    wants: &'a [(String, bool)],
    /// The index's tokens.
    indexed: &'a [String],
    /// Where it was before Campbooks put it in the trash, as bound.
    restore: Option<&'a [String]>,
}

/// Make `one` at Graph.
#[allow(
    clippy::too_many_lines,
    reason = "one message's write read top to bottom: read, check, patch, move"
)]
async fn write_one(
    microsoft: &Microsoft,
    token: &str,
    cursor: &Cursor,
    one: &One<'_>,
) -> Result<Wrote, Stop> {
    let mut indexed = one.indexed.to_vec();
    indexed.sort();
    let before = state::view(&indexed, cursor);
    let fallback = state::folder_of(&indexed).unwrap_or_default().to_owned();
    let Some(mut current) = fetch(microsoft, token, one.message).await? else {
        return Ok(Wrote::Gone);
    };
    let mut conflicts = 0;
    loop {
        let now_tokens = tokens_of(cursor, &current.state, &fallback);
        if overtaken(one.wants, &before, &state::view(&now_tokens, cursor)) {
            return Ok(Wrote::Overtaken { tokens: now_tokens });
        }
        let changed = now_tokens != indexed;
        let asked = wanted(one.change, &current.state, cursor, one.restore);
        let mut state = current.state.clone();
        if !asked.patch.is_empty() {
            let Some(etag) = current.etag.as_deref() else {
                tracing::info!("Graph gave no version of a message; it is not patched");
                return Ok(Wrote::Refused {
                    tokens: Some(now_tokens),
                    relist: false,
                });
            };
            match microsoft
                .patch(token, one.message, etag, &asked.patch)
                .await
            {
                Ok(Patched::Done) => asked.patch.apply(&mut state),
                Ok(Patched::Conflict) if conflicts == 0 => {
                    // Changed since it was read: read it again, and decide
                    // again.
                    conflicts += 1;
                    current = match fetch(microsoft, token, one.message).await? {
                        Some(current) => current,
                        None => return Ok(Wrote::Gone),
                    };
                    continue;
                }
                Ok(Patched::Conflict) => return Ok(Wrote::Overtaken { tokens: now_tokens }),
                Ok(Patched::Gone) => return Ok(Wrote::Gone),
                Err(ProviderError::Unexpected(_) | ProviderError::NotFound) => {
                    tracing::info!("Graph refused a change to one message");
                    return Ok(Wrote::Refused {
                        tokens: Some(now_tokens),
                        relist: false,
                    });
                }
                // Made or not, the next run reads it again first.
                Err(trouble) => {
                    return Err(Stop {
                        trouble,
                        tokens: None,
                    });
                }
            }
        }
        let mut renamed = false;
        if let Some(folder) = &asked.folder {
            match microsoft.move_message(token, one.message, folder).await {
                Ok(Moved::To(id)) => {
                    state.folder = Some(folder.clone());
                    renamed = id != one.message;
                }
                Ok(Moved::Unknown) => {
                    state.folder = Some(folder.clone());
                    renamed = true;
                }
                // The message was just read: it is the folder Graph does not
                // find.
                Ok(Moved::Gone) => {
                    return Ok(Wrote::Refused {
                        tokens: Some(tokens_of(cursor, &state, &fallback)),
                        relist: true,
                    });
                }
                Err(ProviderError::Unexpected(_) | ProviderError::NotFound) => {
                    tracing::info!("Graph refused to move one message");
                    return Ok(Wrote::Refused {
                        tokens: Some(tokens_of(cursor, &state, &fallback)),
                        relist: false,
                    });
                }
                Err(trouble) => {
                    return Err(Stop {
                        trouble,
                        tokens: Some(tokens_of(cursor, &state, &fallback)),
                    });
                }
            }
        }
        let moved = asked.folder.is_some();
        return Ok(Wrote::Made {
            tokens: tokens_of(cursor, &state, &fallback),
            unmovable: asked.unmovable,
            origin: asked.origin.filter(|_| moved),
            renamed,
            changed,
        });
    }
}

/// Message `id`'s state and version; `Ok(None)` when Graph no longer has
/// it. A refusal reads as gone from this push's view (nothing is written);
/// trouble stops the push.
async fn fetch(microsoft: &Microsoft, token: &str, id: &str) -> Result<Option<Current>, Stop> {
    match microsoft.current(token, id).await {
        Ok(found) => Ok(found),
        Err(ProviderError::Unexpected(_)) => {
            tracing::info!("Graph refused to read one message before a change");
            Ok(None)
        }
        Err(trouble) => Err(Stop {
            trouble,
            tokens: None,
        }),
    }
}

/// A change that only moves its message (to or from the inbox, the trash,
/// spam or a stream) and had nowhere to go ends `skipped` (`not_movable`),
/// not `applied`, and Graph's state is shown again; no place to put it back
/// is kept.
fn not_movable(backlog: &Backlog, done: &Results, outcome: &mut Outcome) {
    let place_only = |labels: &[String]| {
        labels.iter().all(|t| {
            matches!(t.as_str(), "INBOX" | "TRASH" | "SPAM") || marks::of_token(t).is_some()
        })
    };
    for change in &backlog.pending {
        let message = &change.provider_message_id;
        let labels: Vec<String> = change.add.iter().chain(&change.remove).cloned().collect();
        if !done.unmovable.contains(message) || !place_only(&labels) {
            continue;
        }
        if let Some(end) = outcome.ends.iter_mut().find(|e| e.id == change.id)
            && end.state == State::Applied
        {
            end.state = State::Skipped;
            end.reason = Some("not_movable");
            outcome.reemit.push(message.clone());
            outcome.restore.retain(|(row, _)| *row != change.id);
        }
    }
}

/// Push `backlog` through Graph (`microsoft`, with `token`), in the mailbox
/// whose folders are `cursor`'s.
#[allow(
    clippy::too_many_lines,
    reason = "one push read top to bottom: plan, each message's write, ends"
)]
pub async fn push(
    microsoft: &Microsoft,
    token: &str,
    backlog: &Backlog,
    cursor: &Cursor,
) -> Pushed {
    let mut pushed = Pushed::default();
    let mut viewed = backlog.clone();
    viewed.labels = backlog
        .labels
        .iter()
        .map(|(id, tokens)| (id.clone(), state::view(tokens, cursor)))
        .collect();
    let mut plan = Plan::merge(&viewed);
    let mut done = Results::default();
    let mut trouble = None;
    'messages: for (change, messages) in plan.groups() {
        for message in messages {
            let Some(indexed) = backlog.labels.get(&message) else {
                // Not carried: the plan has it gone.
                continue;
            };
            let wants: Vec<(String, bool)> = plan
                .wants
                .get(&message)
                .into_iter()
                .flatten()
                .filter(|(label, _)| {
                    plan.fates.get(&(message.clone(), (*label).clone())) == Some(&Fate::Write)
                })
                .map(|(label, want)| (label.clone(), want.present))
                .collect();
            let one = One {
                message: &message,
                change: &change,
                wants: &wants,
                indexed,
                restore: backlog.restore.get(&message).map(Vec::as_slice),
            };
            let outcome = &mut pushed.push.outcome;
            match write_one(microsoft, token, cursor, &one).await {
                Ok(Wrote::Made {
                    tokens,
                    unmovable,
                    origin,
                    renamed,
                    changed,
                }) => {
                    done.written.insert(message.clone());
                    if unmovable {
                        done.unmovable.insert(message.clone());
                    }
                    if let Some(origin) = origin {
                        done.restore.insert(message.clone(), origin);
                    }
                    if renamed {
                        pushed.lookups.push(message.clone());
                    }
                    if changed {
                        // Graph's own change since the last read is shown
                        // too.
                        outcome.reemit.push(message.clone());
                    }
                    outcome.index.push((message, tokens));
                }
                Ok(Wrote::Overtaken { tokens }) => {
                    for (label, _) in &wants {
                        plan.fates
                            .insert((message.clone(), label.clone()), Fate::Overtaken);
                    }
                    outcome.index.push((message, tokens));
                }
                Ok(Wrote::Refused { tokens, relist }) => {
                    done.refused.insert(message.clone());
                    pushed.relist |= relist;
                    if let Some(tokens) = tokens {
                        outcome.index.push((message, tokens));
                    }
                }
                Ok(Wrote::Gone) => {
                    done.gone.insert(message);
                }
                Err(stop) => {
                    if let Some(tokens) = stop.tokens {
                        outcome.index.push((message, tokens));
                    }
                    trouble = Some(stop.trouble);
                    break 'messages;
                }
            }
        }
    }
    plan.atomic();
    let outcome = &mut pushed.push.outcome;
    plan.ends(&viewed, &done, outcome);
    plan.restores(&done, outcome);
    not_movable(&viewed, &done, outcome);
    outcome.index.sort();
    outcome.reemit.sort();
    outcome.reemit.dedup();
    pushed.lookups.sort();
    pushed.lookups.dedup();
    tracing::info!(
        changes = backlog.pending.len(),
        written = done.written.len(),
        refused = done.refused.len(),
        gone = done.gone.len(),
        stopped = trouble.is_some(),
        "Microsoft outbound changes made"
    );
    pushed.push.trouble = trouble;
    pushed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{domain::outbound::Pending, microsoft::state::Folder};

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|&s| s.to_owned()).collect()
    }

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
            ..Cursor::default()
        }
    }

    fn at(folder: &str, read: bool, categories: &[&str]) -> MessageState {
        MessageState {
            folder: Some(folder.to_owned()),
            conversation: None,
            received: None,
            read,
            draft: false,
            flagged: false,
            categories: list(categories),
        }
    }

    fn change(add: &[&str], remove: &[&str]) -> LabelChange {
        (list(add), list(remove))
    }

    fn moved_to(folder: &str) -> Wanted {
        Wanted {
            folder: Some(folder.to_owned()),
            ..Wanted::default()
        }
    }

    #[test]
    fn flags_and_moves_become_graph_calls() {
        let cursor = cursor();
        let unread_inbox = at("in", false, &[]);
        let made = |c: LabelChange, now: &MessageState| wanted(&c, now, &cursor, None);
        assert_eq!(
            made(change(&[], &["UNREAD"]), &unread_inbox).patch,
            Patch {
                read: Some(true),
                ..Patch::default()
            }
        );
        // Already so: nothing to call.
        assert_eq!(
            made(change(&["UNREAD"], &[]), &unread_inbox),
            Wanted::default()
        );
        assert_eq!(
            made(change(&["STARRED"], &[]), &unread_inbox).patch.flagged,
            Some(true)
        );
        assert_eq!(made(change(&[], &["INBOX"]), &unread_inbox), moved_to("ar"));
        assert_eq!(
            made(change(&[], &["INBOX"]), &at("st", true, &[])),
            moved_to("ar")
        );
        assert_eq!(
            made(change(&["INBOX"], &[]), &at("ar", true, &[])),
            moved_to("in")
        );
        assert_eq!(
            made(change(&["INBOX"], &[]), &at("cl", true, &[])),
            moved_to("in")
        );
        // Sent mail, spam and the trash never go to the inbox this way.
        for folder in ["se", "ju", "tr"] {
            assert!(
                made(change(&["INBOX"], &[]), &at(folder, true, &[])).unmovable,
                "{folder}"
            );
        }
        assert_eq!(
            made(change(&["SPAM"], &["INBOX", "TRASH"]), &unread_inbox),
            moved_to("ju")
        );
        // Unknown folders are never moved out of, nor guessed.
        assert!(made(change(&[], &["INBOX"]), &at("zz", true, &[])).unmovable);
        let no_archive = Cursor {
            folders: cursor
                .folders
                .iter()
                .filter(|f| f.role != Role::Archive)
                .cloned()
                .collect(),
            ..Cursor::default()
        };
        assert!(wanted(&change(&[], &["INBOX"]), &unread_inbox, &no_archive, None).unmovable);
    }

    #[test]
    fn the_trash_remembers_where_a_message_was() {
        let cursor = cursor();
        let trashed = wanted(
            &change(&["TRASH"], &["SPAM"]),
            &at("cl", true, &[]),
            &cursor,
            None,
        );
        assert_eq!(trashed.folder.as_deref(), Some("tr"));
        assert_eq!(trashed.origin, Some(list(&["folder:cl"])));
        let in_trash = at("tr", true, &[]);
        let back = |restore: Option<&[String]>, inbound: bool| {
            let add: &[&str] = if inbound { &["INBOX"] } else { &[] };
            wanted(&change(add, &["TRASH"]), &in_trash, &cursor, restore)
        };
        let was_clients = list(&["folder:cl"]);
        assert_eq!(back(Some(&was_clients), true), moved_to("cl"));
        assert_eq!(back(Some(&was_clients), false), moved_to("cl"));
        // Nowhere remembered: received mail to the Inbox, sent mail stays.
        assert_eq!(back(None, true), moved_to("in"));
        assert!(back(None, false).unmovable);
        // A folder gone since, or a bin: never guessed.
        let was_gone = list(&["folder:gone"]);
        let was_junk = list(&["folder:ju"]);
        assert_eq!(back(Some(&was_gone), true), moved_to("in"));
        assert_eq!(back(Some(&was_junk), true), moved_to("in"));
        // Out of spam: received mail to the Inbox only.
        let in_junk = at("ju", true, &[]);
        assert_eq!(
            wanted(&change(&["INBOX"], &["SPAM"]), &in_junk, &cursor, None),
            moved_to("in")
        );
        assert!(wanted(&change(&[], &["SPAM"]), &in_junk, &cursor, None).unmovable);
    }

    #[test]
    fn streams_go_to_their_folder_else_a_category() {
        let cursor = cursor();
        let to = |stream: &str| {
            let (add, remove) = marks::with_mark(
                Vec::new(),
                Vec::new(),
                ProviderStream::parse(stream).and_then(marks::zoho_mark),
            );
            (add, remove)
        };
        let in_inbox = at("in", true, &["Red"]);
        // Updates has its folder: moved there.
        assert_eq!(
            wanted(&to("updates"), &in_inbox, &cursor, None),
            moved_to("st")
        );
        // Receipts has none: kept in the Inbox, with its category.
        let receipts = wanted(&to("receipts"), &at("st", true, &["Red"]), &cursor, None);
        assert_eq!(receipts.folder.as_deref(), Some("in"));
        assert_eq!(
            receipts.patch.categories,
            Some(list(&["Campbooks/Receipts", "Red"]))
        );
        // Primary: the Inbox, without stream categories.
        let primary = wanted(
            &to("primary"),
            &at("in", true, &["Campbooks/Newsletters", "Red"]),
            &cursor,
            None,
        );
        assert_eq!(primary.folder, None);
        assert_eq!(primary.patch.categories, Some(list(&["Red"])));
        // Archived mail has no place in a stream.
        assert!(wanted(&to("updates"), &at("ar", true, &[]), &cursor, None).unmovable);
        // Already there: nothing to call.
        assert_eq!(
            wanted(
                &to("receipts"),
                &at("in", true, &["Campbooks/Receipts"]),
                &cursor,
                None
            ),
            Wanted::default()
        );
    }

    #[test]
    fn graph_s_later_change_wins_unless_it_is_the_member_s() {
        let wants = vec![("STARRED".to_owned(), false)];
        // Flagged in Outlook since the read; the member wants it off.
        assert!(overtaken(
            &wants,
            &list(&["INBOX"]),
            &list(&["INBOX", "STARRED"])
        ));
        // Unchanged since the read.
        assert!(!overtaken(
            &wants,
            &list(&["INBOX", "STARRED"]),
            &list(&["INBOX", "STARRED"])
        ));
        // Graph already did what the member asks.
        let read = vec![("UNREAD".to_owned(), false)];
        assert!(!overtaken(&read, &list(&["UNREAD"]), &list(&[])));
        // A change to another token is not this change's.
        assert!(!overtaken(
            &read,
            &list(&["UNREAD"]),
            &list(&["STARRED", "UNREAD"])
        ));
    }

    #[test]
    fn a_run_takes_at_most_its_messages() {
        let pending = |n: usize| Pending {
            id: uuid::Uuid::now_v7(),
            provider_message_id: format!("m{n}"),
            add: Vec::new(),
            remove: list(&["UNREAD"]),
            stamp: None,
            attempts: 0,
            created_at: chrono::Utc::now(),
        };
        let mut backlog = Backlog::default();
        for n in 0..MAX_MESSAGES {
            backlog.pending.push(pending(n));
        }
        // Another change of a message taken, then a new message.
        backlog.pending.push(pending(0));
        backlog.pending.push(pending(MAX_MESSAGES));
        backlog.pending.push(pending(1));
        let kept = bounded(&backlog);
        assert_eq!(kept.pending.len(), MAX_MESSAGES + 1);
        assert_eq!(kept.last(), Some(backlog.pending[MAX_MESSAGES].id));
        assert_eq!(requests(MAX_MESSAGES * 3), 200);
        assert!(writable("mark:updates") && writable("TRASH"));
        assert!(!writable("category:Red") && !writable("folder:in"));
    }
}
