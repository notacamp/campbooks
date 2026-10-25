//! The copies, in the mailbox's Sent folder, of messages Campbooks sent
//! through its SMTP server (campbooks-cloud#313): made by the stream's run,
//! over its session, once the read is done, so a copy that will not go never
//! holds up inbound mail.
//!
//! For each copy still to be made (`connectors.sends.sent_copy = 'pending'`,
//! the oldest first, within the run's byte budget): the Sent folder
//! (SPECIAL-USE `\Sent`, else the folder the reader found by name); the
//! server's `APPENDLIMIT` and the adapter's largest message; a look in Sent
//! for its `Message-ID` (a server that keeps a copy itself, or an earlier
//! `APPEND` cut short, must not leave a second; a Sent folder that cannot be
//! looked in gets nothing appended); then `APPEND … (\Seen) {n}`, a
//! synchronising literal through the session's bounded framer. With UIDPLUS,
//! `APPENDUID` says where the copy went. Each copy's end is recorded as it
//! ends, in a transaction of its own, so a run cut short loses none. Nothing
//! here touches the send: it was sent. Logs carry ids, codes and counts only.

use std::time::Duration;

use campbooks_kernel::{
    blobs::{BlobOwner, BlobRef, BlobStore},
    db::BeginFor as _,
    tenancy::WorkspaceId,
};
use futures_util::StreamExt as _;
use sqlx::PgPool;

use super::{
    folders::Role,
    session::{Limits, Session, logged_code, quoted},
    state::{self, Cursor, Location, LocationOp, Place},
    syntax::{Code, Untagged},
    write::Trouble,
};
use crate::{
    config::ImapConfig,
    domain::{
        health,
        model::{ConnectionId, SendId, StreamId},
        removal::SENT_COPY_OWNER,
    },
    mail::ReadError,
};

/// Most copies one run makes.
pub const COPIES_PER_RUN: i64 = 10;

/// Runs a copy may meet trouble in (busy, a cut session, a Sent folder that
/// will not open) before it is given up.
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
    /// Already in Sent: the server keeps its own copy, or an earlier
    /// `APPEND` went through.
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
    /// Copies appended.
    pub appended: usize,
    /// What stopped it, if anything did (the read was already done).
    pub trouble: Option<Trouble>,
}

/// Everything making copies needs.
pub struct Appender<'a> {
    /// The run's signed-in session.
    pub session: &'a mut Session,
    /// The pool (runtime role), for recording each copy's end.
    pub pool: &'a PgPool,
    /// The blob store (the bytes to append).
    pub blobs: &'a BlobStore,
    /// The workspace.
    pub workspace: WorkspaceId,
    /// The stream's connection (locked first, removal's order).
    pub connection: ConnectionId,
    /// The stream.
    pub stream: StreamId,
    /// The stream's cursor (its folders), as the read left it.
    pub cursor: &'a Cursor,
    /// The adapter's settings.
    pub config: &'a ImapConfig,
}

/// What looking in Sent found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    /// A message with the `Message-ID` is there.
    Found,
    /// None is.
    Absent,
    /// Sent could not be looked in: nothing may be appended.
    Unknown,
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

    /// Make `pending`'s copies, recording each as it ends, and fail the
    /// copies in `broken` (whose bytes cannot be read back). Errors are the
    /// database's only: what the server did, and its trouble, is recorded.
    pub async fn append(
        mut self,
        pending: Vec<Pending>,
        broken: &[SendId],
    ) -> Result<Copied, ReadError> {
        let mut copied = Copied::default();
        for send in broken {
            self.record(*send, "", &End::Failed("copy_unreadable"), 0)
                .await?;
        }
        if pending.is_empty() {
            return Ok(copied);
        }
        let small = self.small();
        let caps = match self.session.capabilities(small).await {
            Ok(capabilities) => Caps::of(&capabilities),
            // A session the read left unusable: every copy waits, uncounted.
            Err(err) => {
                copied.trouble = Some(Trouble::Wire(err));
                return Ok(copied);
            }
        };
        let Some(sent) = self.cursor.folder_of(Role::Sent).cloned() else {
            // Not listed yet: later. Listed, none: skipped.
            if !self.cursor.folders.is_empty() {
                for copy in &pending {
                    let end = End::Skipped("no_sent_folder");
                    self.record(copy.send, &copy.key, &end, copy.attempts)
                        .await?;
                }
            }
            return Ok(copied);
        };
        let Some(name) = quoted(&sent.name) else {
            for copy in &pending {
                let end = End::Skipped("folder_name_refused");
                self.record(copy.send, &copy.key, &end, copy.attempts)
                    .await?;
            }
            return Ok(copied);
        };
        let largest = caps
            .append_limit
            .unwrap_or(u64::MAX)
            .min(self.config.max_message_bytes);
        // Whether Sent is open (read-only) for looking: `None` before trying.
        let mut examined: Option<bool> = None;
        let mut sent_bytes: u64 = 0;
        for copy in pending {
            // Within the run's byte budget, after one copy at least.
            if copied.appended > 0 && sent_bytes >= self.config.run_budget_bytes {
                break;
            }
            let end = self.copy(&copy, &sent, &name, largest, &mut examined).await;
            let end = match end {
                Ok(end) => end,
                Err(trouble) => {
                    copied.trouble = Some(trouble);
                    End::Later { counted: true }
                }
            };
            if let End::Appended(_) = end {
                copied.appended += 1;
                sent_bytes = sent_bytes.saturating_add(copy.raw.size);
            }
            self.record(copy.send, &copy.key, &end, copy.attempts)
                .await?;
            if copied.trouble.is_some() {
                break;
            }
        }
        if copied.trouble.is_none() && examined == Some(true) && caps.unselect {
            // Leave no folder open (`EXAMINE` already expunges nothing).
            match self.session.command("UNSELECT", small, |_| {}).await {
                Ok(done) if done.is_busy() => copied.trouble = Some(Trouble::Busy),
                Ok(_) => {}
                Err(err) => copied.trouble = Some(Trouble::Wire(err)),
            }
        }
        Ok(copied)
    }

    /// Make one copy in Sent (`sent`, its name quoted as `name`), within
    /// `largest` bytes: how it ended, or the trouble that stopped it.
    async fn copy(
        &mut self,
        copy: &Pending,
        sent: &state::Folder,
        name: &str,
        largest: u64,
        examined: &mut Option<bool>,
    ) -> Result<End, Trouble> {
        if copy.raw.size > largest {
            return Ok(End::Skipped("too_large"));
        }
        let Some(bytes) = self.bytes(&copy.raw, largest).await else {
            return Ok(End::Failed("copy_unreadable"));
        };
        match self
            .look(name, copy.rfc_message_id.as_deref(), examined)
            .await?
        {
            Look::Found => return Ok(End::Found),
            // Whether it is there already cannot be told: no second copy.
            Look::Unknown => return Ok(End::Later { counted: true }),
            Look::Absent => {}
        }
        Ok(match self.append_one(name, &bytes).await? {
            Ok(code) => End::Appended(appended_at(
                code.as_ref(),
                &sent.name,
                sent.uidvalidity,
                &copy.key,
            )),
            Err(code) => {
                tracing::info!(code, "the IMAP server refused a sent copy");
                End::Failed("append_refused")
            }
        })
    }

    /// Record, in a transaction of its own, how send `send`'s copy (of the
    /// message with key `key`) ended: its state, the bytes it held released
    /// once it is done, and where an appended copy went. A copy that met
    /// trouble a [`MAX_ATTEMPTS`]th time is given up. Whether it was recorded
    /// (the copy was still pending).
    async fn record(
        &self,
        send: SendId,
        key: &str,
        end: &End,
        attempts: i32,
    ) -> Result<bool, ReadError> {
        let (outcome, reason) = match end {
            End::Appended(_) => ("appended", None),
            End::Found => ("found", None),
            End::Skipped(reason) => ("skipped", Some(*reason)),
            End::Failed(reason) => ("failed", Some(*reason)),
            End::Later { counted: false } => return Ok(false),
            End::Later { counted: true } if attempts + 1 >= MAX_ATTEMPTS => {
                ("failed", Some("append_gave_up"))
            }
            End::Later { counted: true } => ("pending", None),
        };
        let mut tx = self.pool.begin_for(self.workspace).await?;
        // The connection before the stream (removal's order).
        health::lock(&mut tx, self.connection).await?;
        let recorded = sqlx::query(
            "UPDATE connectors.sends
                SET sent_copy = $3, sent_copy_reason = $4,
                    sent_copy_raw = CASE WHEN $3 = 'pending' THEN sent_copy_raw END,
                    sent_copy_attempts = sent_copy_attempts
                        + CASE WHEN $5 THEN 1 ELSE 0 END
              WHERE id = $1 AND stream_id = $2 AND sent_copy = 'pending'",
        )
        .bind(send)
        .bind(self.stream)
        .bind(outcome)
        .bind(reason)
        .bind(matches!(end, End::Later { counted: true }))
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        if recorded && outcome != "pending" {
            self.blobs
                .detach_owner(
                    &mut tx,
                    &self.workspace.to_string(),
                    &BlobOwner::new(SENT_COPY_OWNER, send),
                )
                .await?;
        }
        if let (true, End::Appended(Some((place, location)))) = (recorded, end) {
            // A place recorded for another message keeps it: the read looks.
            let other: Option<String> = sqlx::query_scalar(
                "SELECT message_key FROM connectors.imap_locations
                  WHERE stream_id = $1 AND folder = $2 AND uidvalidity = $3 AND uid = $4",
            )
            .bind(self.stream)
            .bind(&place.folder)
            .bind(i64::from(place.uidvalidity))
            .bind(i64::from(place.uid))
            .fetch_optional(&mut *tx)
            .await?;
            if other.is_none_or(|other| other == key) {
                let put = LocationOp::Put(place.clone(), location.clone());
                state::apply(&mut tx, self.workspace, self.stream, vec![put]).await?;
            }
        }
        tx.commit().await?;
        Ok(recorded)
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
    /// `Message-ID` `id`, opening it read-only the first time. A Sent folder
    /// that will not open is [`Look::Unknown`]; an id no quoted string can
    /// carry is not looked for ([`Look::Absent`]).
    async fn look(
        &mut self,
        name: &str,
        id: Option<&str>,
        examined: &mut Option<bool>,
    ) -> Result<Look, Trouble> {
        let Some(id) = id.and_then(quoted) else {
            return Ok(Look::Absent);
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
            return Ok(Look::Unknown);
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
        Ok(match (done.is_ok(), found > 0) {
            (true, true) => Look::Found,
            (true, false) => Look::Absent,
            (false, _) => Look::Unknown,
        })
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
