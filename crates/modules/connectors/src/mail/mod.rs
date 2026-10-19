//! What the mail adapters share: a parsed raw message, the state the
//! contract carries, the index rows of what was emitted, and storing a
//! fetched message's blobs. Provider vocabulary stays in each adapter
//! (`gmail/`, `zoho/`).

pub mod parse;

use campbooks_kernel::blobs::{BlobError, BlobRef, BlobStore};
use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use uuid::Uuid;

use crate::{
    contract::{Attachment, Category, Direction, Label, Mailbox, NormalizedMessage, ProviderStream},
    provider::ProviderError,
};

/// A message's state at the provider, in the contract's terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flags {
    pub mailbox: Mailbox,
    pub read: bool,
    pub starred: bool,
    pub direction: Direction,
    pub categories: Vec<Category>,
    pub labels: Vec<Label>,
    /// The stream it is in at the provider (`NormalizedMessage`), when the
    /// adapter can tell.
    pub provider_stream: Option<Option<ProviderStream>>,
    /// Its stream mark is stale (`NormalizedMessage`).
    pub stream_mark_stale: bool,
}

/// Why a page could not be read (or a message stored).
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// The provider refused or failed.
    #[error(transparent)]
    Provider(#[from] ProviderError),
    /// The database failed. Shown by kind only: a database error's message
    /// can quote the row it failed on.
    #[error("{}", campbooks_kernel::redact::sqlx(.0))]
    Database(#[from] sqlx::Error),
    /// The blob store failed.
    #[error("blob store: {0}")]
    Blob(#[from] BlobError),
    /// Stored data did not decode: what and where, never the data.
    #[error("{0}")]
    Corrupt(String),
    /// A query of the module failed.
    #[error(transparent)]
    Kernel(#[from] campbooks_kernel::Error),
}

/// One update to the adapters' index (`connectors.messages`).
#[derive(Debug, Clone)]
pub enum IndexOp {
    /// A newly emitted message: its row holds every blob it references.
    Insert {
        /// The new row's id (the blobs' owner).
        row: Uuid,
        /// The provider's id.
        provider_message_id: String,
        /// The provider's thread id (empty when it has none).
        thread_id: String,
        /// Its `Message-ID`, without angle brackets.
        rfc_message_id: Option<String>,
        /// Its labels (Gmail) or state tokens (Zoho).
        label_ids: Vec<String>,
        /// When the provider received it.
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
        /// The provider's id.
        provider_message_id: String,
    },
    /// The provider failed on this one message: skip it, and try again
    /// later, backing off with each failure.
    Skipped {
        /// The provider's id.
        provider_message_id: String,
        /// The time of the run.
        at: DateTime<Utc>,
    },
}

/// The provider that received a message: whose `Authentication-Results`
/// header is trusted (its authserv-id), and no one else's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receiver {
    /// Gmail (`mx.google.com`).
    Gmail,
    /// Zoho Mail, in any of its data centres.
    Zoho,
}

impl Receiver {
    /// The authserv-ids the provider stamps its own results with. An id not
    /// listed here is not trusted, so an unknown one reads as no result at
    /// all.
    ///
    /// Zoho trusts none yet: nobody has seen which `Authentication-Results`
    /// (if any, rather than only `ARC-Authentication-Results`) Zoho adds on
    /// top of a received message, and if it adds none, a header naming its
    /// servers would be one the sender wrote. TODO(campbooks-cloud#132):
    /// list Zoho's ids once a recorded Zoho message shows them, with a test
    /// over that recording.
    pub const fn authserv_ids(self) -> &'static [&'static str] {
        match self {
            Self::Gmail => &["mx.google.com"],
            Self::Zoho => &[],
        }
    }
}

/// A fetched message, as [`store_message`] stores it.
#[derive(Debug, Clone)]
pub struct Fetched {
    /// The provider's id.
    pub id: String,
    /// The provider's thread id: the contract's thread key, when the
    /// provider has a stable one.
    pub thread_key: Option<String>,
    /// Its labels or state tokens, for the index.
    pub label_ids: Vec<String>,
    /// When the provider received it.
    pub internal_date: DateTime<Utc>,
    /// The RFC 5322 bytes.
    pub raw: Vec<u8>,
    /// Its state, in the contract's terms.
    pub flags: Flags,
    /// The provider that received it.
    pub receiver: Receiver,
}

/// Store a fetched message's blobs (its raw bytes, bodies, attachments and
/// normalised document) in `workspace` and build its upsert and index row:
/// for a message read from a provider, or one just sent through it.
pub async fn store_message(
    store: &BlobStore,
    workspace: &str,
    fetched: Fetched,
    generation: i32,
) -> Result<(NormalizedMessage, IndexOp), ReadError> {
    let parsed = parse::parse(&fetched.raw, fetched.receiver);
    let mut blobs = Vec::new();
    let raw_blob = store.put(workspace, fetched.raw, "message/rfc822").await?;
    blobs.push(raw_blob.clone());
    let text = match parsed.text {
        Some(text) => {
            let blob = store
                .put(workspace, text.into_bytes(), "text/plain; charset=utf-8")
                .await?;
            blobs.push(blob.clone());
            Some(blob)
        }
        None => None,
    };
    let html = match parsed.html {
        Some(html) => {
            let blob = store
                .put(workspace, html.into_bytes(), "text/html; charset=utf-8")
                .await?;
            blobs.push(blob.clone());
            Some(blob)
        }
        None => None,
    };
    let mut attachments = Vec::with_capacity(parsed.attachments.len());
    for attachment in parsed.attachments {
        let blob = store
            .put(workspace, attachment.bytes, &attachment.content_type)
            .await?;
        blobs.push(blob.clone());
        attachments.push(Attachment {
            filename: attachment.filename,
            content_type: attachment.content_type,
            blob,
            inline: attachment.inline,
            content_id: attachment.content_id,
        });
    }
    let flags = fetched.flags;
    let message = NormalizedMessage {
        provider_message_id: fetched.id.clone(),
        thread_key: fetched.thread_key.clone(),
        rfc_message_id: parsed.rfc_message_id.clone(),
        in_reply_to: parsed.in_reply_to,
        references: parsed.references,
        from: parsed.from,
        to: parsed.to,
        cc: parsed.cc,
        reply_to: parsed.reply_to,
        subject: parsed.subject,
        sent_at: parsed.sent_at.unwrap_or(fetched.internal_date),
        direction: flags.direction,
        headers: parsed.headers,
        raw: Some(raw_blob),
        text,
        html,
        attachments,
        mailbox: flags.mailbox,
        labels: flags.labels,
        categories: flags.categories,
        read: flags.read,
        starred: flags.starred,
        provider_stream: flags.provider_stream,
        stream_mark_stale: flags.stream_mark_stale,
    };
    let document_bytes = serde_json::to_vec(&message).map_err(|e| {
        ReadError::Corrupt(format!(
            "encoding a message document: {}",
            campbooks_kernel::redact::json(&e)
        ))
    })?;
    let document = store
        .put(workspace, document_bytes, "application/json")
        .await?;
    blobs.push(document.clone());
    let op = IndexOp::Insert {
        row: Uuid::now_v7(),
        provider_message_id: fetched.id,
        thread_id: fetched.thread_key.unwrap_or_default(),
        rfc_message_id: parsed.rfc_message_id,
        label_ids: fetched.label_ids,
        internal_date: fetched.internal_date,
        document,
        blobs,
        generation,
    };
    Ok((message, op))
}

/// Overwrite a stored message's state with `flags`.
pub fn apply_flags(message: &mut NormalizedMessage, flags: &Flags) {
    message.mailbox = flags.mailbox;
    message.read = flags.read;
    message.starred = flags.starred;
    message.direction = flags.direction;
    message.categories.clone_from(&flags.categories);
    message.labels.clone_from(&flags.labels);
    message.provider_stream = flags.provider_stream;
    message.stream_mark_stale = flags.stream_mark_stale;
}

/// A stored message document of `workspace`.
pub async fn document(
    store: &BlobStore,
    workspace: &str,
    blob: &BlobRef,
) -> Result<NormalizedMessage, ReadError> {
    let mut stream = store.get(workspace, blob).await?;
    let mut bytes = Vec::with_capacity(usize::try_from(blob.size).unwrap_or(0));
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
    }
    serde_json::from_slice(&bytes).map_err(|e| {
        ReadError::Corrupt(format!(
            "a stored message document: {}",
            campbooks_kernel::redact::json(&e)
        ))
    })
}

/// An index row's document reference.
pub fn document_ref(hash: &str, size: i64) -> Result<BlobRef, ReadError> {
    Ok(BlobRef {
        hash: hash
            .parse()
            .map_err(|_| ReadError::Corrupt("an index row's document hash".into()))?,
        size: u64::try_from(size).unwrap_or(0),
        content_type: "application/json".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{blobs::BlobStore, config::BlobsConfig, tenancy::WorkspaceId};
    use sqlx::PgPool;

    use super::*;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_corrupt_document_is_reported_without_its_content(db: PgPool) {
        let store = BlobStore::memory(&BlobsConfig::default(), db);
        let workspace = WorkspaceId::new().to_string();
        // A message document with mail where a flag belongs: serde's own
        // message quotes it.
        let corrupt = br#"{"read": "Q4 plan from maya@northwind-studio.example"}"#;
        let raw = serde_json::from_slice::<NormalizedMessage>(corrupt).unwrap_err();
        assert!(raw.to_string().contains("maya@"), "{raw}");
        let blob = store
            .put(&workspace, corrupt.as_slice(), "application/json")
            .await
            .unwrap();
        let err = document(&store, &workspace, &blob).await.unwrap_err();
        // What sync jobs store in `last_error` and log.
        for shown in [err.to_string(), format!("{err:?}")] {
            assert!(!shown.contains("maya"), "{shown}");
            assert!(!shown.contains("Q4"), "{shown}");
        }
        assert!(
            err.to_string()
                .starts_with("a stored message document: Data error at line 1"),
            "{err}"
        );
    }
}
