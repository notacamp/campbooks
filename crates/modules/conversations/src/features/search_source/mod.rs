//! Conversations as a search source (ADR 0016): the kernel's
//! `SearchSource` port for the group type `conversations.conversation`,
//! registered by the server. A conversation is the group (in its inbox);
//! its messages are the entries.
//!
//! An entry's stamp is made of the hashes of the blobs its text is read from
//! (participants document, text and HTML parts). Blobs are content-addressed,
//! so the stamp changes exactly when the words do, and a flag change never
//! makes search read a body again. An entry read from HTML also carries
//! [`HTML_TEXT_VERSION`], so a change in how HTML becomes text reads those
//! bodies again.

use std::{collections::HashMap, fmt::Write as _, sync::Arc};

use campbooks_kernel::{
    Error, RecordRef, Result,
    blobs::{BlobError, BlobRef, BlobStore},
    search::{
        EntryHead, EntryText, Facet, Facets, Group, Page, Person, PersonRole, SearchSource,
        SourceContext, SourceFuture, facet,
    },
};
use serde::Deserialize;
use sqlx::PgConnection;

use crate::{
    documents::ParticipantsDocument,
    domain::{blobs::read_text, plain_text},
    events::{CONVERSATION_RECORD, INBOX_RECORD, MESSAGE_RECORD},
    model::{Address, ConversationId, MessageId},
};

/// The most of a participants document read, in bytes.
const MAX_PARTICIPANTS_BYTES: usize = 64 * 1024;
/// The most of a text part read before quotes are stripped, in bytes.
const MAX_TEXT_BYTES: usize = 128 * 1024;
/// The most of an HTML part read, in bytes.
const MAX_HTML_BYTES: usize = 512 * 1024;
/// How HTML becomes text ([`plain_text::from_html`]): part of the stamp of
/// an entry read from HTML. 2: words after `<head>` with no `<body>` tag are
/// read (campbooks-cloud#218).
const HTML_TEXT_VERSION: u32 = 2;

/// The port, for the server to register.
pub fn search_source() -> Arc<dyn SearchSource> {
    Arc::new(ConversationsSource)
}

/// This module's conversations, as search reads them.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConversationsSource;

/// The `bodies` column: blob refs of the parts.
#[derive(Debug, Default, Deserialize)]
struct Bodies {
    #[serde(default)]
    text: Option<BlobRef>,
    #[serde(default)]
    html: Option<BlobRef>,
}

impl Bodies {
    fn parse(value: serde_json::Value) -> Self {
        serde_json::from_value(value).unwrap_or_default()
    }
}

fn hash_of(blob: Option<&BlobRef>) -> String {
    blob.map_or_else(|| "-".to_owned(), |blob| blob.hash.to_string())
}

fn mailbox(in_inbox: bool, in_spam: bool, in_trash: bool) -> &'static str {
    if in_trash {
        "trash"
    } else if in_spam {
        "spam"
    } else if in_inbox {
        "inbox"
    } else {
        "archive"
    }
}

impl SearchSource for ConversationsSource {
    fn group_type(&self) -> &'static str {
        CONVERSATION_RECORD
    }

    fn container_type(&self) -> &'static str {
        INBOX_RECORD
    }

    fn group<'a>(&'a self, cx: SourceContext<'a>, id: &'a str) -> SourceFuture<'a, Option<Group>> {
        Box::pin(async move {
            let Ok(id) = id.parse::<ConversationId>() else {
                return Ok(None);
            };
            group(cx.conn, id).await
        })
    }

    fn texts<'a>(
        &'a self,
        cx: SourceContext<'a>,
        entries: &'a [RecordRef],
    ) -> SourceFuture<'a, Vec<EntryText>> {
        Box::pin(async move {
            let ids: Vec<MessageId> = entries
                .iter()
                .filter(|entry| entry.record_type == MESSAGE_RECORD)
                .filter_map(|entry| entry.id.parse().ok())
                .collect();
            texts(cx.conn, cx.blobs, &cx.workspace.to_string(), &ids).await
        })
    }

    fn page<'a>(
        &'a self,
        cx: SourceContext<'a>,
        after: Option<&'a str>,
        limit: u32,
    ) -> SourceFuture<'a, Page> {
        Box::pin(async move {
            let after = after.and_then(|after| after.parse::<ConversationId>().ok());
            let ids = sqlx::query_scalar!(
                r#"SELECT id AS "id: ConversationId" FROM conversations.conversations
                    WHERE ($1::uuid IS NULL OR id > $1) ORDER BY id LIMIT $2"#,
                after as Option<ConversationId>,
                i64::from(limit),
            )
            .fetch_all(&mut *cx.conn)
            .await?;
            let next = (ids.len() == limit as usize)
                .then(|| ids.last().map(ToString::to_string))
                .flatten();
            Ok(Page {
                groups: ids.iter().map(ToString::to_string).collect(),
                next,
            })
        })
    }
}

async fn group(conn: &mut PgConnection, id: ConversationId) -> Result<Option<Group>> {
    let Some(row) = sqlx::query!(
        r#"SELECT inbox_id::text AS "inbox!", subject, stream, last_message_at, message_count,
                  unread_count, starred, archived_at IS NOT NULL AS "archived!"
             FROM conversations.conversations WHERE id = $1"#,
        id as ConversationId,
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(None);
    };
    let labels = sqlx::query!(
        r#"SELECT l.id::text AS "id!", l.name
             FROM conversations.conversation_labels cl
             JOIN conversations.labels l ON l.id = cl.label_id
            WHERE cl.conversation_id = $1 ORDER BY l.name"#,
        id as ConversationId,
    )
    .fetch_all(&mut *conn)
    .await?;
    let messages = sqlx::query!(
        r#"SELECT m.id AS "id: MessageId", m.sent_at, m.direction, m.in_inbox, m.in_spam,
                  m.in_trash, m.bodies, m.participants,
                  (SELECT count(*) FROM conversations.attachments a
                    WHERE a.message_id = m.id AND NOT a.inline) AS "attachments!"
             FROM conversations.messages m
            WHERE m.conversation_id = $1 ORDER BY m.sent_at, m.id"#,
        id as ConversationId,
    )
    .fetch_all(&mut *conn)
    .await?;
    let entries = messages
        .into_iter()
        .map(|message| {
            let bodies = Bodies::parse(message.bodies);
            let participants: Option<BlobRef> = serde_json::from_value(message.participants).ok();
            let mut stamp = format!(
                "{}:{}:{}",
                hash_of(participants.as_ref()),
                hash_of(bodies.text.as_ref()),
                hash_of(bodies.html.as_ref()),
            );
            // Its words come from its HTML (no text part).
            if bodies.text.is_none() && bodies.html.is_some() {
                let _ = write!(stamp, ":h{HTML_TEXT_VERSION}");
            }
            let facets: Facets = [
                (
                    facet::MAILBOX.to_owned(),
                    Facet::Text(
                        mailbox(message.in_inbox, message.in_spam, message.in_trash).to_owned(),
                    ),
                ),
                (facet::DIRECTION.to_owned(), Facet::Text(message.direction)),
                (
                    facet::ATTACHMENTS.to_owned(),
                    Facet::Count(message.attachments),
                ),
            ]
            .into();
            EntryHead {
                record: RecordRef::new(MESSAGE_RECORD, &message.id),
                occurred_at: message.sent_at,
                facets,
                stamp,
            }
        })
        .collect();
    let facets: Facets = [
        (facet::STREAM.to_owned(), Facet::Text(row.stream)),
        (facet::UNREAD.to_owned(), Facet::Bool(row.unread_count > 0)),
        (facet::STARRED.to_owned(), Facet::Bool(row.starred)),
        (facet::ARCHIVED.to_owned(), Facet::Bool(row.archived)),
        (
            facet::LABELS.to_owned(),
            Facet::Texts(labels.iter().map(|label| label.id.clone()).collect()),
        ),
        (
            facet::COUNT.to_owned(),
            Facet::Count(i64::from(row.message_count)),
        ),
    ]
    .into();
    Ok(Some(Group {
        container: RecordRef::new(INBOX_RECORD, &row.inbox),
        occurred_at: row.last_message_at,
        title: row.subject,
        keywords: labels.into_iter().map(|label| label.name).collect(),
        facets,
        entries,
    }))
}

async fn texts(
    conn: &mut PgConnection,
    blobs: &BlobStore,
    workspace: &str,
    ids: &[MessageId],
) -> Result<Vec<EntryText>> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id: MessageId", bodies, participants
             FROM conversations.messages WHERE id = ANY($1)"#,
        ids as &[MessageId],
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut files: HashMap<MessageId, Vec<String>> = HashMap::new();
    for file in sqlx::query!(
        r#"SELECT message_id AS "message_id: MessageId", filename
             FROM conversations.attachments
            WHERE message_id = ANY($1) AND NOT inline ORDER BY message_id, position"#,
        ids as &[MessageId],
    )
    .fetch_all(&mut *conn)
    .await?
    {
        files
            .entry(file.message_id)
            .or_default()
            .push(file.filename);
    }
    let mut texts = Vec::with_capacity(rows.len());
    for row in rows {
        let bodies = Bodies::parse(row.bodies);
        let people = match serde_json::from_value::<BlobRef>(row.participants) {
            Ok(blob) => people(blobs, workspace, &blob).await?,
            Err(_) => Vec::new(),
        };
        let text = text(blobs, workspace, &bodies).await?;
        texts.push(EntryText {
            record: RecordRef::new(MESSAGE_RECORD, &row.id),
            people,
            text,
            attachments: files.remove(&row.id).unwrap_or_default(),
        });
    }
    Ok(texts)
}

/// The text at `blob`, or nothing when the blob is gone (the message is
/// being removed; its refresh follows).
async fn read(blobs: &BlobStore, workspace: &str, blob: &BlobRef, max: usize) -> Result<String> {
    match read_text(blobs, workspace, blob, max).await {
        Ok((text, _)) => Ok(text),
        Err(BlobError::NotFound) => Ok(String::new()),
        Err(e) => Err(Error::from(e)),
    }
}

async fn people(blobs: &BlobStore, workspace: &str, blob: &BlobRef) -> Result<Vec<Person>> {
    let document = read(blobs, workspace, blob, MAX_PARTICIPANTS_BYTES).await?;
    let Ok(document) = serde_json::from_str::<ParticipantsDocument>(&document) else {
        return Ok(Vec::new());
    };
    let person = |role, address: &Address| Person {
        role,
        name: address
            .name
            .as_ref()
            .filter(|name| !name.trim().is_empty())
            .cloned(),
        address: address.address.to_lowercase(),
    };
    Ok(std::iter::once(person(PersonRole::From, &document.from))
        .chain(document.to.iter().map(|a| person(PersonRole::To, a)))
        .chain(document.cc.iter().map(|a| person(PersonRole::Cc, a)))
        .collect())
}

async fn text(blobs: &BlobStore, workspace: &str, bodies: &Bodies) -> Result<String> {
    let raw = match (&bodies.text, &bodies.html) {
        (Some(text), _) => read(blobs, workspace, text, MAX_TEXT_BYTES).await?,
        (None, Some(html)) => {
            plain_text::from_html(&read(blobs, workspace, html, MAX_HTML_BYTES).await?)
        }
        (None, None) => String::new(),
    };
    Ok(plain_text::own_words(&raw))
}

#[cfg(test)]
mod tests;
