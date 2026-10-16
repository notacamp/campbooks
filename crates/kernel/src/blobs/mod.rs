//! Blob storage: message bodies, attachments and files, outside Postgres
//! (ADR 0011).
//!
//! Bytes live in an object store — a directory on local disk (the self-host
//! default) or any S3-compatible bucket — **content-addressed per
//! workspace**: an object's key is `<workspace>/<aa>/<sha256>`, so the same
//! attachment arriving in three inboxes of one workspace is stored once.
//! Postgres keeps a catalogue (`kernel.blobs`) and the records holding each
//! blob (`kernel.blob_refs`); a scheduled sweeper deletes what nothing holds
//! any more.
//!
//! ```ignore
//! // Store the bytes (streamed, hashed and size-checked on the way in)…
//! let blob = state.blobs.put(workspace, BlobBody::stream(body), "message/rfc822").await?;
//! // …and attach them to the record that holds them, in that record's
//! // transaction. Keep the `BlobRef` on the record.
//! let mut tx = state.db.begin_for(workspace).await?;
//! /* insert the message, with blob.hash / blob.size / blob.content_type */
//! state.blobs.attach(&mut tx, workspace, &blob, &BlobOwner::new("mail.message", id)).await?;
//! tx.commit().await?;
//! // Read them back as a stream, verified against the hash.
//! let bytes = state.blobs.get(workspace, &blob).await?;
//! // When the record goes, detach in its transaction; the sweeper deletes
//! // the object once nothing holds it and the grace period has passed.
//! state.blobs.detach(&mut tx, workspace, &blob.hash, &owner).await?;
//! ```
//!
//! **Access control is the caller's.** The store never serves a blob by URL;
//! a module's download endpoint checks that the caller may see the record,
//! then streams [`BlobStore::get`]. A content type is stored as given and
//! never trusted: serve downloads as attachments with `nosniff`.
//!
//! **Deleting a workspace** calls [`BlobStore::purge_workspace`] (the
//! `workspaces` module's `workspaces.purge_workspace` job does, when the
//! deleted workspace's grace period ends).

mod store;
mod sweep;
mod verify;

#[cfg(test)]
mod tests;

use std::{borrow::Cow, fmt, io, pin::Pin, str::FromStr};

use bytes::Bytes;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use store::BlobStore;
pub use sweep::{SWEEP_EVERY, SweepBlobs, SweepStats};

/// A stream of bytes: what [`BlobStore::get`] returns and what
/// [`BlobBody::stream`] takes (with [`io::Error`] as its error).
pub type ByteStream<E> = Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>;

/// The content type recorded when none (or an unusable one) is given.
pub const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

/// SHA-256 of a blob's bytes: its address within a workspace. Displayed,
/// parsed and serialized as 64 lower-case hex digits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlobHash([u8; 32]);

impl BlobHash {
    /// The SHA-256 of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        let mut hasher = verify::Hasher::new();
        hasher.update(bytes);
        hasher.finish()
    }

    /// The raw 32 bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlobHash({self})")
    }
}

/// The text is not 64 hex digits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a blob hash is 64 hexadecimal digits (SHA-256)")]
pub struct ParseBlobHashError;

impl FromStr for BlobHash {
    type Err = ParseBlobHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ParseBlobHashError);
        }
        let mut out = [0_u8; 32];
        for (byte, pair) in out.iter_mut().zip(s.as_bytes().chunks_exact(2)) {
            let hex = std::str::from_utf8(pair).map_err(|_| ParseBlobHashError)?;
            *byte = u8::from_str_radix(hex, 16).map_err(|_| ParseBlobHashError)?;
        }
        Ok(Self(out))
    }
}

impl Serialize for BlobHash {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for BlobHash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// A stored blob, as [`BlobStore::put`] returns it. Records that hold a blob
/// keep this (hash, size and content type) and read the bytes back with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobRef {
    /// SHA-256 of the bytes.
    pub hash: BlobHash,
    /// Length in bytes.
    pub size: u64,
    /// The media type the uploader gave, lower-cased; not trusted.
    pub content_type: String,
}

/// The record holding a blob: its type, `<module>.<record>` (for example
/// `mail.message`), and its id. Other modules' records are named, never
/// joined.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlobOwner {
    kind: Cow<'static, str>,
    id: Uuid,
}

impl BlobOwner {
    /// The record `id` of type `kind` (`<module>.<record>`).
    pub fn new(kind: impl Into<Cow<'static, str>>, id: impl Into<Uuid>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }

    /// The record's type.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The record's id.
    pub fn id(&self) -> Uuid {
        self.id
    }
}

/// What to store: bytes already in memory, or a stream that is written as it
/// arrives (no whole-file buffering).
pub enum BlobBody {
    /// Bytes in memory.
    Bytes(Bytes),
    /// Bytes as they arrive; an `Err` item aborts the upload.
    Stream(ByteStream<io::Error>),
}

impl BlobBody {
    /// A streamed body.
    pub fn stream(stream: impl Stream<Item = Result<Bytes, io::Error>> + Send + 'static) -> Self {
        Self::Stream(Box::pin(stream))
    }
}

impl fmt::Debug for BlobBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bytes(bytes) => write!(f, "BlobBody::Bytes({} bytes)", bytes.len()),
            Self::Stream(_) => f.write_str("BlobBody::Stream(..)"),
        }
    }
}

impl From<Bytes> for BlobBody {
    fn from(bytes: Bytes) -> Self {
        Self::Bytes(bytes)
    }
}

impl From<Vec<u8>> for BlobBody {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Bytes(bytes.into())
    }
}

impl From<&'static [u8]> for BlobBody {
    fn from(bytes: &'static [u8]) -> Self {
        Self::Bytes(Bytes::from_static(bytes))
    }
}

/// What the uploader knows about the bytes in advance (a provider's digest
/// or `Content-Length`). A body that does not match is rejected and nothing
/// is stored, so a truncated download never becomes a blob.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Expected {
    /// The SHA-256 the bytes must have.
    pub hash: Option<BlobHash>,
    /// The length the bytes must have.
    pub size: Option<u64>,
}

/// An object's size and age, from [`BlobStore::head`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlobInfo {
    /// Length in bytes.
    pub size: u64,
    /// When the object was last written.
    pub last_modified: chrono::DateTime<chrono::Utc>,
}

/// Why a blob operation failed. Messages never contain blob contents or
/// credentials.
#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    /// The workspace id cannot be part of an object key.
    #[error("invalid workspace id for blob storage")]
    InvalidWorkspace,
    /// The owner type is not `<module>.<record>`.
    #[error("blob owner type `{0}` must be `<module>.<record>` (lower-case snake case)")]
    InvalidOwner(String),
    /// The body is larger than `CAMPBOOKS_BLOBS__MAX_OBJECT_BYTES`.
    #[error("the file is larger than the {limit}-byte limit")]
    TooLarge {
        /// The limit, in bytes.
        limit: u64,
    },
    /// The body's length or SHA-256 differs from what was [`Expected`]
    /// (for example, a truncated download).
    #[error("the uploaded bytes do not match the expected {0}")]
    Mismatch(&'static str),
    /// The body stream failed.
    #[error("reading the upload failed: {0}")]
    Read(#[source] io::Error),
    /// No such blob in this workspace.
    #[error("blob not found")]
    NotFound,
    /// Attaching to a blob that is not (or no longer) stored: put it again.
    #[error("blob is not stored in this workspace; put it again before attaching it")]
    NotStored,
    /// The stored object does not match its hash or size: it was damaged in
    /// storage. Reported at the end of a [`BlobStore::get`] stream.
    #[error("stored blob {0} is corrupt")]
    Corrupt(BlobHash),
    /// No blob store is configured in this process.
    #[error("blob storage is not configured")]
    NotConfigured,
    /// The object store failed.
    #[error("object store: {0}")]
    Store(#[from] object_store::Error),
    /// The database failed.
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
}

/// For handlers: missing blobs are 404s; bad input is a 400; storage and
/// database failures are internal.
impl From<BlobError> for crate::Error {
    fn from(err: BlobError) -> Self {
        match err {
            BlobError::NotFound => Self::NotFound,
            BlobError::TooLarge { limit } => Self::BadRequest(
                crate::i18n::MESSAGES
                    .reason("file_too_large")
                    .arg("limit", limit),
            ),
            BlobError::Mismatch(_) => {
                Self::BadRequest(crate::i18n::MESSAGES.reason("upload_mismatch"))
            }
            BlobError::Read(_) => Self::BadRequest(crate::i18n::MESSAGES.reason("upload_failed")),
            other => Self::internal(other),
        }
    }
}

/// For jobs: bad input and missing blobs are permanent; the rest is retried.
impl From<BlobError> for crate::JobError {
    fn from(err: BlobError) -> Self {
        match err {
            BlobError::Store(_) | BlobError::Database(_) | BlobError::Read(_) => {
                Self::retry(err.to_string())
            }
            other => Self::fatal(other.to_string()),
        }
    }
}

/// A workspace id is used as the first segment of object keys: 1 to 64
/// ASCII letters, digits or hyphens (UUIDs qualify). Underscore-prefixed
/// top-level names stay free for the store's own use (`_staging`).
fn check_workspace(workspace: &str) -> Result<(), BlobError> {
    let ok = (1..=64).contains(&workspace.len())
        && workspace
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(BlobError::InvalidWorkspace)
    }
}

fn check_owner(owner: &BlobOwner) -> Result<(), BlobError> {
    if owner.kind.len() <= 128 && crate::jobs::is_dotted(&owner.kind) {
        Ok(())
    } else {
        Err(BlobError::InvalidOwner(owner.kind.clone().into_owned()))
    }
}

/// A media type as stored: trimmed and lower-cased if it looks like
/// `type/subtype[; params]` in printable ASCII, else
/// [`DEFAULT_CONTENT_TYPE`]. Stored, never trusted.
fn normalize_content_type(content_type: &str) -> String {
    let trimmed = content_type.trim();
    let essence = trimmed.split(';').next().unwrap_or_default().trim();
    let plausible = (3..=255).contains(&trimmed.len())
        && trimmed.bytes().all(|b| (0x20..0x7f).contains(&b))
        && essence
            .split_once('/')
            .is_some_and(|(kind, sub)| is_token(kind) && is_token(sub));
    if plausible {
        trimmed.to_ascii_lowercase()
    } else {
        DEFAULT_CONTENT_TYPE.to_owned()
    }
}

/// An RFC 9110 token.
fn is_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}
