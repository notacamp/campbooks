//! The Gmail messages adapter: reads one bounded page of a stream from its
//! cursor and turns it into `MessageChangeBatch` changes plus updates to the
//! adapter's index. Talks to Gmail and the blob store; writes nothing to
//! Postgres itself (the engine applies a [`Page`] in its transaction).
//!
//! - **Backfill**: the window (`after:<start>`), newest first, a page of
//!   [`LIST_PAGE`] ids per run, after recording the mailbox's history id.
//! - **Incremental**: `history.list` from the history id, up to
//!   [`MAX_CHANGED`] messages per run. A label-only change is re-emitted
//!   from the index's stored document, without downloading the message.
//! - **Resync**: when Gmail no longer has the history id, the window again;
//!   known messages are re-checked, and indexed messages in the window that
//!   Gmail no longer lists are removed.

pub mod normalize;
mod read;
pub mod write;

use campbooks_kernel::blobs::BlobRef;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use read::{ReadError, Reader};

use crate::{contract::MessageChange, domain::model::Phase, google::LabelInfo};

/// Message ids listed per backfill or resync run.
pub const LIST_PAGE: u32 = 50;

/// Most changed messages one incremental run handles.
pub const MAX_CHANGED: usize = 100;

/// Messages fetched from Gmail at once within a run.
pub const FETCH_CONCURRENCY: usize = 5;

/// Gmail quota units of `messages.get`.
pub const UNITS_PER_GET: u32 = 5;

/// The most quota units one run may spend: a list or history call plus a
/// page of fetches and label checks.
pub const UNITS_PER_RUN: u32 = 5 + UNITS_PER_GET * LIST_PAGE * 2;

/// A Gmail stream's cursor (the `streams.cursor` column).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    /// The mailbox's history id the next incremental run starts from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    /// The page of `messages.list` or `history.list` in progress.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
    /// The start of the window being listed (backfill, resync).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_start: Option<DateTime<Utc>>,
    /// The current resync pass; index rows listed in it carry it.
    #[serde(default)]
    pub generation: i32,
}

impl Cursor {
    /// The cursor as the opaque string events carry: the history id (never
    /// personal data).
    pub fn token(&self) -> String {
        self.history_id.clone().unwrap_or_default()
    }
}

/// What one run read.
#[derive(Debug, Clone)]
pub struct Page {
    /// The changes for the batch, in order.
    pub changes: Vec<MessageChange>,
    /// Updates to the index, applied with the batch.
    pub index: Vec<IndexOp>,
    /// A fresh `labels.list`, to replace the cached label names.
    pub labels: Option<Vec<LabelInfo>>,
    /// The cursor after this page.
    pub cursor: Cursor,
    /// The phase after this page.
    pub phase: Phase,
    /// Whether more is waiting (run again at once).
    pub more: bool,
    /// Whether every new message of the page was fetched; `false` when the
    /// run's byte budget ran out (the cursor then stays put).
    pub complete: bool,
}

/// One update to the adapter's index (`connectors.gmail_messages`).
#[derive(Debug, Clone)]
pub enum IndexOp {
    /// A newly emitted message: its row holds every blob it references.
    Insert {
        /// The new row's id (the blobs' owner).
        row: Uuid,
        /// Gmail's id.
        provider_message_id: String,
        /// Gmail's thread id.
        thread_id: String,
        /// Its labels.
        label_ids: Vec<String>,
        /// When Gmail received it.
        internal_date: DateTime<Utc>,
        /// The normalised message document.
        document: BlobRef,
        /// Every blob the message references (raw, bodies, attachments,
        /// document).
        blobs: Vec<BlobRef>,
        /// The resync pass that listed it.
        generation: i32,
    },
    /// A known message's labels changed (it was re-emitted) or it was seen
    /// again by a resync pass.
    Update {
        /// The row.
        row: Uuid,
        /// Its labels now.
        label_ids: Vec<String>,
        /// The resync pass that listed it.
        generation: i32,
    },
    /// The message is gone: delete the row and release its blobs.
    Remove {
        /// The row.
        row: Uuid,
    },
    /// The message is over the size limit: record it, never fetch it again.
    Oversized {
        /// Gmail's id.
        provider_message_id: String,
    },
}
