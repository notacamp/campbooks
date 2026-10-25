//! Looking for an address check's message in the Inbox of the connection
//! that sent it (campbooks-cloud#314), over the stream run's own session,
//! and removing it once found.
//!
//! Found means: the Inbox, opened read-only, answers `UID SEARCH HEADER
//! Message-ID` with at most [`MAX_CANDIDATES`] UIDs, and the `Message-ID`
//! fetched for one of them is exactly the expected one. A server that
//! answers every search with hits finds nothing that way.
//!
//! Removed, only by its UID, only with the write paths of #312: with
//! UIDPLUS, read-write, its `Message-ID` checked again and no other message
//! of the Inbox marked `\Deleted`, `UID STORE +FLAGS.SILENT (\Deleted)` then
//! `UID EXPUNGE` of that UID (the mark taken off again when the expunge
//! fails); else, with MOVE, `UID MOVE` of that UID to the stream's Trash;
//! else left where it is. Never `EXPUNGE` without UIDs, `CLOSE`, `DELETE`
//! or `RENAME`; the Inbox is left with `UNSELECT`, or `EXAMINE` of it.
//! Logs carry counts and codes only.

use std::{collections::HashSet, time::Duration};

use super::{
    read::sequence,
    session::{Done, Limits, Session, quoted},
    syntax::Untagged,
    write::{Trouble, expunges, guard_expunged},
};

/// Most UIDs a search may name: more is a lying answer, and counts as none.
pub const MAX_CANDIDATES: usize = 5;

/// Most UIDs a `UID SEARCH DELETED` answer is read into.
const MAX_DELETED: usize = 100_000;

/// The Inbox, as a quoted string.
const INBOX: &str = "\"INBOX\"";

/// How the found message went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// Expunged by its UID (UIDPLUS).
    Expunged,
    /// Moved to Trash by its UID (MOVE).
    Trashed,
    /// Left in the Inbox: no safe way to remove it.
    Left,
}

/// What the look found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// The message, and what became of it.
    Yes(Removal),
    /// The message, but removing it, the server said it expunged more than
    /// that one message: the run stops there (`imap_expunged_more`).
    Hostile,
    /// Not there.
    No,
}

/// One look for one check's message.
pub struct Look<'a> {
    /// The run's signed-in session.
    pub session: &'a mut Session,
    /// What is searched for in `Message-ID` (`cb-check-<code>`).
    pub search: &'a str,
    /// The whole expected `Message-ID` (`<cb-check-<code>@<domain>>`).
    pub message_id: &'a str,
    /// The stream's Trash folder, as the server spells it, when it has one.
    pub trash: Option<&'a str>,
    /// How long one command may take.
    pub timeout: Duration,
}

impl Look<'_> {
    /// Look in the Inbox, and remove what is found.
    pub async fn run(mut self) -> Result<Found, Trouble> {
        let capabilities = self
            .session
            .capabilities(self.small())
            .await
            .map_err(Trouble::Wire)?;
        let has = |name: &str| capabilities.iter().any(|c| c == name);
        let (uidplus, moves, unselect) = (has("UIDPLUS"), has("MOVE"), has("UNSELECT"));
        let opened = self.command(&format!("EXAMINE {INBOX}")).await?;
        if opened.is_busy() {
            return Err(Trouble::Busy);
        }
        if !opened.is_ok() {
            return Ok(Found::No);
        }
        let found = match self.candidates().await? {
            Some(uids) => self.matching(&uids).await?,
            None => None,
        };
        let Some(uid) = found else {
            self.deselect(unselect).await?;
            return Ok(Found::No);
        };
        let removal = match self.remove(uid, uidplus, moves).await {
            Err(Trouble::Hostile(_)) => {
                // No folder stays open, even after a server that did more
                // than it was asked.
                let _ = self.deselect(unselect).await;
                return Ok(Found::Hostile);
            }
            other => other,
        };
        // Left as found whatever the removal did: no folder stays open.
        let left = self.deselect(unselect).await;
        let removal = removal?;
        left?;
        tracing::info!(
            removal = match removal {
                Removal::Expunged => "expunged",
                Removal::Trashed => "trashed",
                Removal::Left => "left",
            },
            "an address check's message was found"
        );
        Ok(Found::Yes(removal))
    }

    /// Bounds of a command with a short answer.
    fn small(&self) -> Limits {
        Limits {
            response: 64 * 1024,
            responses: 1_000,
            total: 1024 * 1024,
            time: self.timeout,
        }
    }

    async fn command(&mut self, command: &str) -> Result<Done, Trouble> {
        let limits = self.small();
        self.session
            .command(command, limits, |_| {})
            .await
            .map_err(Trouble::Wire)
    }

    /// The UIDs of the open Inbox whose `Message-ID` holds the search term;
    /// `None` for none, a refusal, or more than [`MAX_CANDIDATES`].
    async fn candidates(&mut self) -> Result<Option<Vec<u32>>, Trouble> {
        let Some(term) = quoted(self.search) else {
            return Ok(None);
        };
        let mut uids = Vec::new();
        let mut over = false;
        let limits = self.small();
        let done = self
            .session
            .command(
                &format!("UID SEARCH HEADER Message-ID {term}"),
                limits,
                |untagged| {
                    if let Untagged::Search(found) = untagged {
                        if uids.len().saturating_add(found.len()) > MAX_CANDIDATES {
                            over = true;
                        } else {
                            uids.extend(found);
                        }
                    }
                },
            )
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        if over {
            tracing::warn!("an IMAP server named too many messages for an address check");
        }
        uids.sort_unstable();
        uids.dedup();
        Ok((done.is_ok() && !over && !uids.is_empty()).then_some(uids))
    }

    /// The first of `uids` (of the open folder) whose `Message-ID` is
    /// exactly the expected one, as the server sends its header.
    async fn matching(&mut self, uids: &[u32]) -> Result<Option<u32>, Trouble> {
        let asked: HashSet<u32> = uids.iter().copied().collect();
        let mut headers: Vec<(u32, Vec<u8>)> = Vec::new();
        let limits = self.small();
        let done = self
            .session
            .command(
                &format!(
                    "UID FETCH {} (UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)])",
                    sequence(uids)
                ),
                limits,
                |untagged| {
                    if let Untagged::Fetch(fetch) = untagged
                        && let (Some(uid), Some(header)) = (fetch.uid, fetch.header.as_deref())
                        && asked.contains(&uid)
                    {
                        headers.push((uid, header.to_vec()));
                    }
                },
            )
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        if !done.is_ok() {
            return Ok(None);
        }
        Ok(headers
            .iter()
            .find(|(_, header)| {
                message_id(header).is_some_and(|id| same(id.as_bytes(), self.message_id.as_bytes()))
            })
            .map(|(uid, _)| *uid))
    }

    /// Remove `uid` of the Inbox by its UID only, or leave it.
    async fn remove(&mut self, uid: u32, uidplus: bool, moves: bool) -> Result<Removal, Trouble> {
        let trash = self.trash.and_then(quoted);
        if !uidplus && (!moves || trash.is_none()) {
            return Ok(Removal::Left);
        }
        let selected = self.command(&format!("SELECT {INBOX}")).await?;
        if selected.is_busy() {
            return Err(Trouble::Busy);
        }
        // Read-write now: the same UID must still be the same message.
        if !selected.is_ok() || self.matching(&[uid]).await? != Some(uid) {
            return Ok(Removal::Left);
        }
        if uidplus {
            match self.expunge(uid).await? {
                Some(removal) => return Ok(removal),
                None if !moves => return Ok(Removal::Left),
                None => {}
            }
        }
        let Some(trash) = trash.filter(|_| moves) else {
            return Ok(Removal::Left);
        };
        let mut expunged = 0_usize;
        let limits = self.small();
        let done = self
            .session
            .command(&format!("UID MOVE {uid} {trash}"), limits, |untagged| {
                expunged = expunged.saturating_add(expunges(&untagged));
            })
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        if !done.is_ok() {
            return Ok(Removal::Left);
        }
        guard_expunged(expunged, 1)?;
        Ok(Removal::Trashed)
    }

    /// Mark `uid` `\Deleted` and expunge it by its UID (UIDPLUS), unless
    /// another message of the Inbox is marked already (a server that
    /// expunged more than asked would take it along): `None` when it was
    /// not made (nothing changed).
    async fn expunge(&mut self, uid: u32) -> Result<Option<Removal>, Trouble> {
        let Some(deleted) = self.deleted().await? else {
            return Ok(None);
        };
        if deleted.iter().any(|other| *other != uid) {
            tracing::info!("the Inbox holds messages marked deleted elsewhere: no expunge there");
            return Ok(None);
        }
        let marked = self
            .command(&format!("UID STORE {uid} +FLAGS.SILENT (\\Deleted)"))
            .await?;
        if marked.is_busy() {
            return Err(Trouble::Busy);
        }
        if !marked.is_ok() {
            return Ok(None);
        }
        let mut expunged = 0_usize;
        let limits = self.small();
        let done = self
            .session
            .command(&format!("UID EXPUNGE {uid}"), limits, |untagged| {
                expunged = expunged.saturating_add(expunges(&untagged));
            })
            .await
            .map_err(Trouble::Wire)?;
        if done.is_ok() {
            guard_expunged(expunged, 1)?;
            return Ok(Some(Removal::Expunged));
        }
        // Not expunged: the mark this look made comes off again.
        if !deleted.contains(&uid) {
            let _ = self
                .command(&format!("UID STORE {uid} -FLAGS.SILENT (\\Deleted)"))
                .await?;
        }
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        Ok(None)
    }

    /// The UIDs of the open folder marked `\Deleted`; `None` when the
    /// server will not say, or says more than is read.
    async fn deleted(&mut self) -> Result<Option<HashSet<u32>>, Trouble> {
        let mut found = HashSet::new();
        let mut over = false;
        let limits = Limits {
            response: 16 * 1024 * 1024,
            responses: 1_000,
            total: 32 * 1024 * 1024,
            time: self.timeout * 2,
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

    /// Leave no folder open, expunging nothing: `UNSELECT` where offered,
    /// else `EXAMINE` of the Inbox. Never `CLOSE`.
    async fn deselect(&mut self, unselect: bool) -> Result<(), Trouble> {
        let command = if unselect {
            "UNSELECT".to_owned()
        } else {
            format!("EXAMINE {INBOX}")
        };
        let done = self.command(&command).await?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        Ok(())
    }
}

/// The one `Message-ID` of a header block, unfolded and trimmed; `None`
/// when there is none, or more than one.
fn message_id(header: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(header);
    let mut found: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.starts_with([' ', '\t']) {
            if let Some(value) = current.as_mut() {
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some(value) = current.take() {
            found.push(value);
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("message-id")
        {
            current = Some(value.trim().to_owned());
        }
    }
    if let Some(value) = current {
        found.push(value);
    }
    match found.as_slice() {
        [one] => Some(one.trim().to_owned()),
        _ => None,
    }
}

/// Whether `a` and `b` are the same bytes, in time that does not depend on
/// where they differ.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_one_message_id_of_a_header() {
        assert_eq!(
            message_id(b"Message-ID: <cb-check-ab@bakery.example>\r\n\r\n").as_deref(),
            Some("<cb-check-ab@bakery.example>")
        );
        assert_eq!(
            message_id(b"message-id:\r\n <cb-check-ab@bakery.example>\r\n\r\n").as_deref(),
            Some("<cb-check-ab@bakery.example>")
        );
        // None, or two: not one message's.
        assert_eq!(message_id(b"Subject: hi\r\n\r\n"), None);
        assert_eq!(
            message_id(b"Message-ID: <a@x.example>\r\nMessage-ID: <b@x.example>\r\n\r\n"),
            None
        );
    }

    #[test]
    fn comparison_is_exact() {
        assert!(same(b"<cb-check-ab@x>", b"<cb-check-ab@x>"));
        assert!(!same(b"<cb-check-ab@x>", b"<cb-check-ac@x>"));
        assert!(!same(b"<cb-check-ab@x>", b"<cb-check-ab@x> "));
    }
}
