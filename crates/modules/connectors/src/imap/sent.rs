//! The copies, in the mailbox's Sent folder, of messages Campbooks sent
//! through its SMTP server (campbooks-cloud#313): made by the stream's run,
//! over its session, after the write-backs and before the read, so the
//! read finds each copy where it was recorded and announces nothing.
//!
//! For each copy still to be made (`connectors.sends.sent_copy = 'pending'`,
//! the oldest first): the Sent folder (SPECIAL-USE `\Sent`, else the folder
//! the reader found by name); the server's `APPENDLIMIT` and the adapter's
//! largest message; a look in Sent for its `Message-ID` (a server that keeps
//! a copy itself must not get a second); then `APPEND … (\Seen) {n}`, a
//! synchronising literal through the session's bounded framer. With UIDPLUS,
//! `APPENDUID` says where the copy went. Nothing here touches the send: it
//! was sent. Logs carry ids, codes and counts only.

use std::time::Duration;

use campbooks_kernel::{
    blobs::{BlobRef, BlobStore},
    tenancy::WorkspaceId,
};
use futures_util::StreamExt as _;

use super::{
    folders::Role,
    session::{Limits, Session, logged_code, quoted},
    state::{Cursor, Location, Place},
    syntax::{Code, Untagged},
    write::Trouble,
};
use crate::{config::ImapConfig, domain::model::SendId};

/// Most copies one run makes.
pub const COPIES_PER_RUN: i64 = 10;

/// Runs a copy may meet trouble in (busy, a cut session) before it is given
/// up.
pub const MAX_ATTEMPTS: i32 = 3;

/// Most UIDs a look for a copy reads.
const MAX_FOUND: usize = 1_000;

/// Bytes a second an `APPEND` is given beyond its base time.
const BYTES_PER_SECOND: usize = 256 * 1024;

/// A sent message whose copy is still to be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// The send.
    pub send: SendId,
    /// The index's key of the sent message.
    pub key: String,
    /// Its `Message-ID`, without angle brackets, when the index has it.
    pub rfc_message_id: Option<String>,
    /// The bytes to append (the message as sent, Bcc kept).
    pub raw: BlobRef,
    /// Runs that met trouble with it so far.
    pub attempts: i32,
}

/// How a copy ended this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// Appended; where, when the server said (`APPENDUID`) and the place
    /// can be trusted.
    Appended(Option<(Place, Location)>),
    /// Already in Sent: the server keeps its own copy.
    Found,
    /// Not to be made (a stable code): no Sent folder, too large, a folder
    /// name no command can carry.
    Skipped(&'static str),
    /// The server refused it, or it met trouble too often (a stable code).
    Failed(&'static str),
    /// To be made by a later run; `counted` when it met trouble this run.
    Later {
        /// It met trouble this run (towards [`MAX_ATTEMPTS`]).
        counted: bool,
    },
}

/// What a run did with its copies.
#[derive(Debug, Default)]
pub struct Copied {
    /// How each ended, in order.
    pub ends: Vec<(Pending, End)>,
    /// What stopped it, if anything did: the run stops too.
    pub trouble: Option<Trouble>,
}

/// Everything making copies needs.
pub struct Appender<'a> {
    /// The run's signed-in session.
    pub session: &'a mut Session,
    /// The blob store (the bytes to append).
    pub blobs: &'a BlobStore,
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The stream's cursor (its folders).
    pub cursor: &'a Cursor,
    /// The adapter's settings.
    pub config: &'a ImapConfig,
}

/// What the server offers that copying reads.
#[derive(Debug, Clone, Copy, Default)]
struct Caps {
    unselect: bool,
    append_limit: Option<u64>,
}

impl Caps {
    fn of(capabilities: &[String]) -> Self {
        Self {
            unselect: capabilities.iter().any(|c| c == "UNSELECT"),
            append_limit: capabilities.iter().find_map(|c| {
                c.strip_prefix("APPENDLIMIT=")
                    .and_then(|n| n.parse::<u64>().ok())
            }),
        }
    }
}

/// The location `code` (an `APPEND`'s `OK`) names for `key`: `APPENDUID`
/// with a non-zero UIDVALIDITY that, when the folder was read, is its own,
/// and one non-zero UID.
pub fn appended_at(
    code: Option<&Code>,
    folder: &str,
    uidvalidity: Option<u32>,
    key: &str,
) -> Option<(Place, Location)> {
    let code = code.filter(|c| c.name == "APPENDUID")?;
    let at = u32::try_from(code.number?).ok().filter(|v| *v != 0)?;
    if uidvalidity.is_some_and(|known| known != at) {
        return None;
    }
    let [uids] = code.sets.as_slice() else {
        return None;
    };
    let [uid] = uids.as_slice() else {
        return None;
    };
    Some((
        Place {
            folder: folder.to_owned(),
            uidvalidity: at,
            uid: *uid,
        },
        Location {
            key: key.to_owned(),
            seen: true,
            flagged: false,
            email_id: None,
            fingerprint: None,
        },
    ))
}

impl Appender<'_> {
    fn timeout(&self) -> Duration {
        Duration::from_secs(u64::from(self.config.timeout_seconds))
    }

    fn small(&self) -> Limits {
        Limits {
            response: 64 * 1024,
            responses: 1_000,
            total: 1024 * 1024,
            time: self.timeout(),
        }
    }

    /// Make `pending`'s copies. Errors never: what the server did, and its
    /// trouble, is in the result.
    pub async fn append(mut self, pending: Vec<Pending>) -> Copied {
        let mut copied = Copied::default();
        if pending.is_empty() {
            return copied;
        }
        let small = self.small();
        let caps = match self.session.capabilities(small).await {
            Ok(capabilities) => Caps::of(&capabilities),
            Err(err) => return later(pending, Trouble::Wire(err)),
        };
        let Some(sent) = self.cursor.folder_of(Role::Sent).cloned() else {
            // Not listed yet (the first run): later. Listed, none: skipped.
            let end = if self.cursor.folders.is_empty() {
                End::Later { counted: false }
            } else {
                End::Skipped("no_sent_folder")
            };
            copied.ends = pending.into_iter().map(|p| (p, end.clone())).collect();
            return copied;
        };
        let Some(name) = quoted(&sent.name) else {
            copied.ends = pending
                .into_iter()
                .map(|p| (p, End::Skipped("folder_name_refused")))
                .collect();
            return copied;
        };
        let largest = caps
            .append_limit
            .unwrap_or(u64::MAX)
            .min(self.config.max_message_bytes);
        // Whether Sent is open (read-only) for looking: `None` before trying.
        let mut examined: Option<bool> = None;
        let mut rest = pending.into_iter();
        for copy in rest.by_ref() {
            if copy.raw.size > largest {
                copied.ends.push((copy, End::Skipped("too_large")));
                continue;
            }
            let Some(bytes) = self.bytes(&copy.raw, largest).await else {
                copied.ends.push((copy, End::Failed("copy_unreadable")));
                continue;
            };
            match self
                .look(&name, copy.rfc_message_id.as_deref(), &mut examined)
                .await
            {
                Ok(true) => {
                    copied.ends.push((copy, End::Found));
                    continue;
                }
                Ok(false) => {}
                Err(trouble) => {
                    copied.ends.push((copy, End::Later { counted: true }));
                    copied.trouble = Some(trouble);
                    break;
                }
            }
            match self.append_one(&name, &bytes).await {
                Ok(Ok(code)) => {
                    let at = appended_at(code.as_ref(), &sent.name, sent.uidvalidity, &copy.key);
                    copied.ends.push((copy, End::Appended(at)));
                }
                Ok(Err(code)) => {
                    tracing::info!(code, "the IMAP server refused a sent copy");
                    copied.ends.push((copy, End::Failed("append_refused")));
                }
                Err(trouble) => {
                    copied.ends.push((copy, End::Later { counted: true }));
                    copied.trouble = Some(trouble);
                    break;
                }
            }
        }
        // What a stop left unmade waits for the next run, uncounted.
        copied
            .ends
            .extend(rest.map(|p| (p, End::Later { counted: false })));
        if copied.trouble.is_none() && examined == Some(true) && caps.unselect {
            // Leave no folder open (`EXAMINE` already expunges nothing).
            match self.session.command("UNSELECT", small, |_| {}).await {
                Ok(done) if done.is_busy() => copied.trouble = Some(Trouble::Busy),
                Ok(_) => {}
                Err(err) => copied.trouble = Some(Trouble::Wire(err)),
            }
        }
        copied
    }

    /// The bytes of `blob`, at most `max`; `None` when larger or unreadable.
    async fn bytes(&self, blob: &BlobRef, max: u64) -> Option<Vec<u8>> {
        let max = usize::try_from(max).unwrap_or(usize::MAX);
        let mut stream = match self.blobs.get(&self.workspace.to_string(), blob).await {
            Ok(stream) => stream,
            Err(err) => {
                tracing::warn!(error = %campbooks_kernel::redact::error(&err), "a sent copy could not be read");
                return None;
            }
        };
        let mut bytes = Vec::with_capacity(usize::try_from(blob.size).unwrap_or(0).min(max));
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.ok()?);
            if bytes.len() > max {
                return None;
            }
        }
        Some(bytes)
    }

    /// Whether Sent (`name`, quoted) already holds a message with
    /// `Message-ID` `id`, opening it read-only the first time. An id no
    /// quoted string can carry, or a Sent folder that will not open, is not
    /// looked for.
    async fn look(
        &mut self,
        name: &str,
        id: Option<&str>,
        examined: &mut Option<bool>,
    ) -> Result<bool, Trouble> {
        let Some(id) = id.and_then(quoted) else {
            return Ok(false);
        };
        if examined.is_none() {
            let small = self.small();
            let done = self
                .session
                .command(&format!("EXAMINE {name}"), small, |_| {})
                .await
                .map_err(Trouble::Wire)?;
            if done.is_busy() {
                return Err(Trouble::Busy);
            }
            *examined = Some(done.is_ok());
        }
        if *examined != Some(true) {
            return Ok(false);
        }
        let limits = Limits {
            response: 256 * 1024,
            responses: 100,
            total: 1024 * 1024,
            time: self.timeout(),
        };
        let mut found = 0_usize;
        let done = self
            .session
            .command(
                &format!("UID SEARCH HEADER Message-ID {id}"),
                limits,
                |untagged| {
                    if let Untagged::Search(uids) = untagged {
                        found = found.saturating_add(uids.len()).min(MAX_FOUND);
                    }
                },
            )
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        Ok(done.is_ok() && found > 0)
    }

    /// `APPEND` `bytes` to Sent (`name`, quoted), `\Seen`: the `OK`'s
    /// response code, or the logged code of a refusal; trouble when the
    /// server is busy or the session was cut.
    async fn append_one(
        &mut self,
        name: &str,
        bytes: &[u8],
    ) -> Result<Result<Option<Code>, &'static str>, Trouble> {
        let extra = u64::try_from(bytes.len() / BYTES_PER_SECOND).unwrap_or(u64::MAX);
        let limits = Limits {
            response: 64 * 1024,
            responses: 100,
            total: 1024 * 1024,
            time: self.timeout() * 2 + Duration::from_secs(extra),
        };
        let done = self
            .session
            .append(name, "\\Seen", bytes, limits)
            .await
            .map_err(Trouble::Wire)?;
        if done.is_busy() {
            return Err(Trouble::Busy);
        }
        if done.is_ok() {
            Ok(Ok(done.code))
        } else {
            Ok(Err(logged_code(done.code.as_ref())))
        }
    }
}

/// Every copy left for a later run after `trouble` stopped the run before
/// any was tried: the first counted, the rest not.
fn later(pending: Vec<Pending>, trouble: Trouble) -> Copied {
    let ends = pending
        .into_iter()
        .enumerate()
        .map(|(n, p)| (p, End::Later { counted: n == 0 }))
        .collect();
    Copied {
        ends,
        trouble: Some(trouble),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(name: &str, number: Option<u64>, sets: Vec<Vec<u32>>) -> Code {
        Code {
            name: name.to_owned(),
            number,
            sets,
        }
    }

    #[test]
    fn appenduid_places_the_copy_only_when_it_can_be_trusted() {
        let ok = code("APPENDUID", Some(7), vec![vec![42]]);
        let (place, location) = appended_at(Some(&ok), "Sent", Some(7), "mid:a").unwrap();
        assert_eq!(
            (place.folder.as_str(), place.uidvalidity, place.uid),
            ("Sent", 7, 42)
        );
        assert!(location.seen && !location.flagged);
        assert_eq!(location.key, "mid:a");
        // A folder not read yet takes the server's UIDVALIDITY.
        assert!(appended_at(Some(&ok), "Sent", None, "mid:a").is_some());
        // Another UIDVALIDITY, zero, several UIDs, none, another code: none.
        assert!(appended_at(Some(&ok), "Sent", Some(8), "mid:a").is_none());
        for bad in [
            code("APPENDUID", Some(0), vec![vec![42]]),
            code("APPENDUID", Some(7), vec![vec![42, 43]]),
            code("APPENDUID", Some(7), vec![]),
            code("APPENDUID", None, vec![vec![42]]),
            code("COPYUID", Some(7), vec![vec![42]]),
        ] {
            assert!(
                appended_at(Some(&bad), "Sent", Some(7), "mid:a").is_none(),
                "{bad:?}"
            );
        }
        assert!(appended_at(None, "Sent", Some(7), "mid:a").is_none());
    }

    #[test]
    fn appendlimit_is_read_from_the_capabilities() {
        let caps = Caps::of(&[
            "IMAP4REV1".into(),
            "APPENDLIMIT=1000".into(),
            "UNSELECT".into(),
        ]);
        assert_eq!(caps.append_limit, Some(1000));
        assert!(caps.unselect);
        // `APPENDLIMIT` without a number is per folder: no instance-wide one.
        assert_eq!(Caps::of(&["APPENDLIMIT".into()]).append_limit, None);
    }
}
