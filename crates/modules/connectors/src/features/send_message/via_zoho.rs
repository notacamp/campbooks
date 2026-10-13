//! Sending through Zoho Mail. Zoho takes the message as JSON (or as a reply
//! to one of its messages) and sets its own `Message-ID`, so an attempt that
//! may have gone out is looked up in the mailbox's Sent folder instead: a
//! message sent since that attempt, with the same subject and recipients,
//! that is not already another send's.

use std::collections::HashSet;

use campbooks_kernel::{JobContext, JobError, db::BeginFor as _};
use chrono::{DateTime, TimeDelta, Utc};

use super::{
    Attempt, MAX_MESSAGE_BYTES, SendMessage, Sent, Target, UNCLEAR_WAIT, finish_failed, read_blob,
    record_attempt, set_unclear,
};
use crate::{
    contract::{Address, OutgoingMessage},
    domain::{
        model::{ConnectionId, SendId, StreamId},
        schedule::{self, Rate},
    },
    mail::Fetched,
    provider::ProviderError,
    zoho::{
        OriginalFetch, ZohoMail,
        read::Cursor,
        state::{self, FolderKind},
    },
};

/// How far before an unclear attempt a sent message still counts as it
/// (clocks differ).
const CLOCK_SLACK: TimeDelta = TimeDelta::minutes(2);

/// Sent messages looked at when looking an attempt up.
const LOOKUP_PAGE: u32 = 50;

impl SendMessage {
    /// An attempt through Zoho.
    pub(super) async fn attempt_zoho(
        &self,
        ctx: &mut JobContext,
        mut target: Target,
        attempt: Attempt<'_>,
    ) -> Result<(), JobError> {
        let Attempt {
            workspace,
            send,
            stream,
            token,
            document,
            give_up,
            now,
        } = attempt;
        let (Some(zoho), Some(connection)) = (self.deps.zoho.clone(), target.connection) else {
            return finish_failed(ctx, workspace, send, &target, "not_connected").await;
        };
        let Some(centre) = target
            .provider_server
            .as_deref()
            .and_then(|server| zoho.centre(server))
        else {
            return finish_failed(ctx, workspace, send, &target, "needs_reauth").await;
        };
        self.take_budget(ctx, connection, document).await?;
        let mailbox = target.mailbox_key.clone().unwrap_or_default();
        let mail = zoho.mail(&centre, token, &mailbox);
        let sent_folder = sent_folder(&target, &mail).await;

        // An earlier attempt may have reached Zoho: look before sending.
        if target.unclear {
            let since = target.attempted_at.unwrap_or(now) - CLOCK_SLACK;
            match look_up(ctx, &mail, &sent_folder, &attempt, since).await? {
                Ok(Some(id)) => {
                    tracing::info!(%workspace, %send, "an earlier attempt was sent");
                    let sent = self
                        .fetch_sent(&mail, sent_folder.ok().flatten(), id, now)
                        .await;
                    return self
                        .finish_sent(ctx, workspace, send, &target, stream, sent, now)
                        .await;
                }
                Ok(None) => {
                    // Proven not sent.
                    set_unclear(ctx, workspace, send, false).await?;
                    target.unclear = false;
                }
                Err(err) => {
                    return Err(JobError::retry_after(
                        format!("looking up an earlier attempt: {err}"),
                        UNCLEAR_WAIT,
                    ));
                }
            }
        }
        if give_up {
            return finish_failed(ctx, workspace, send, &target, "unavailable").await;
        }

        // Attachments go up first: uploading one is not sending.
        let uploaded = match upload(ctx, &mail, workspace, document).await? {
            Ok(uploaded) => uploaded,
            Err(Upload::TooLarge) => {
                return finish_failed(ctx, workspace, send, &target, "too_large").await;
            }
            Err(Upload::Refused(err)) => {
                return self.refused(ctx, workspace, send, &target, &err).await;
            }
        };
        let body = message_json(document, uploaded);
        let parent = parent_of(ctx, workspace, stream, document).await?;

        // From here until Zoho answers, the message may be out.
        record_attempt(ctx, workspace, send, now).await?;
        target.unclear = true;
        match mail.send(&body, parent.as_deref()).await {
            Ok(answer) => {
                let sent = match answer.message_id {
                    Some(id) => {
                        self.fetch_sent(&mail, sent_folder.ok().flatten(), id, now)
                            .await
                    }
                    None => Sent {
                        id: None,
                        thread_key: None,
                        fetched: None,
                    },
                };
                self.finish_sent(ctx, workspace, send, &target, stream, sent, now)
                    .await
            }
            Err(ProviderError::Unavailable(reason)) => {
                // Zoho may have it: look it up before trying again.
                tracing::warn!(%workspace, %send, %reason, "send ended unclearly");
                Err(JobError::retry_after(
                    "the send ended unclearly; looking it up next time",
                    UNCLEAR_WAIT,
                ))
            }
            Err(err) => {
                // A definite answer: this attempt did not send it.
                set_unclear(ctx, workspace, send, false).await?;
                target.unclear = false;
                self.refused(ctx, workspace, send, &target, &err).await
            }
        }
    }

    /// Take a send's requests from the Zoho login's budget, or wait.
    async fn take_budget(
        &self,
        ctx: &JobContext,
        connection: ConnectionId,
        document: &OutgoingMessage,
    ) -> Result<(), JobError> {
        let cost = u32::try_from(document.attachments.len() + 4).unwrap_or(u32::MAX);
        let rate = Rate::per_minute(self.deps.config.zoho.requests_per_minute);
        let key = schedule::connection_budget(connection);
        match schedule::take(ctx.pool(), &key, cost, rate).await? {
            Ok(()) => Ok(()),
            Err(wait) => Err(JobError::retry_after(
                "waiting for the provider budget",
                wait,
            )),
        }
    }

    /// The sent message `id`, fetched to index and announce it. When it
    /// cannot be fetched, the send is still sent: the next sync brings it
    /// from the Sent folder.
    async fn fetch_sent(
        &self,
        mail: &ZohoMail<'_>,
        sent_folder: Option<String>,
        id: String,
        now: DateTime<Utc>,
    ) -> Sent {
        let max = self.deps.config.zoho.max_message_bytes;
        let fetched = match (sent_folder, mail.original(&id, max).await) {
            (Some(folder), Ok(OriginalFetch::Message { raw, .. })) => {
                let tokens = state::tokens(&folder, FolderKind::Sent, true, false, false);
                Some(Fetched {
                    id: id.clone(),
                    thread_key: None,
                    flags: state::flags(&tokens),
                    label_ids: tokens,
                    internal_date: now,
                    raw,
                })
            }
            (_, Err(err)) => {
                tracing::info!(error = %err, "the sent message will come with the next sync");
                None
            }
            _ => None,
        };
        Sent {
            id: Some(id),
            thread_key: None,
            fetched,
        }
    }
}

/// Why attachments could not go up.
enum Upload {
    /// Together they are larger than a message may be.
    TooLarge,
    /// Zoho refused one.
    Refused(ProviderError),
}

/// Upload `document`'s attachments: what the send names them by.
async fn upload(
    ctx: &JobContext,
    mail: &ZohoMail<'_>,
    workspace: campbooks_kernel::tenancy::WorkspaceId,
    document: &OutgoingMessage,
) -> Result<Result<Vec<serde_json::Value>, Upload>, JobError> {
    let mut uploaded = Vec::with_capacity(document.attachments.len());
    let mut total = document.text.len() + document.html.as_ref().map_or(0, String::len);
    for attachment in &document.attachments {
        let Some(bytes) = read_blob(ctx, workspace, &attachment.blob, MAX_MESSAGE_BYTES).await?
        else {
            return Ok(Err(Upload::TooLarge));
        };
        total += bytes.len();
        if total > MAX_MESSAGE_BYTES {
            return Ok(Err(Upload::TooLarge));
        }
        match mail
            .upload(&attachment.filename, &attachment.content_type, bytes)
            .await
        {
            Ok(reference) => uploaded.push(reference),
            Err(err @ ProviderError::Unavailable(_)) => {
                return Err(JobError::retry(format!("uploading an attachment: {err}")));
            }
            Err(err) => return Ok(Err(Upload::Refused(err))),
        }
    }
    Ok(Ok(uploaded))
}

/// The message of an earlier attempt in the Sent folder, if it went out.
async fn look_up(
    ctx: &JobContext,
    mail: &ZohoMail<'_>,
    sent_folder: &Result<Option<String>, ProviderError>,
    attempt: &Attempt<'_>,
    since: DateTime<Utc>,
) -> Result<Result<Option<String>, ProviderError>, JobError> {
    Ok(match sent_folder {
        Ok(Some(folder)) => {
            let others = other_sends(ctx, attempt.workspace, attempt.stream, attempt.send).await?;
            find_sent(mail, folder, attempt.document, since, &others).await
        }
        Ok(None) => Ok(None),
        Err(err) => Err(err.clone()),
    })
}

/// The mailbox's Sent folder: from the stream's cursor, else asked.
async fn sent_folder(
    target: &Target,
    mail: &ZohoMail<'_>,
) -> Result<Option<String>, ProviderError> {
    let cursor: Option<Cursor> = target
        .cursor
        .clone()
        .and_then(|c| serde_json::from_value(c).ok());
    if let Some(folder) = cursor
        .as_ref()
        .and_then(|c| c.folder_of_kind(FolderKind::Sent))
    {
        return Ok(Some(folder.to_owned()));
    }
    Ok(mail
        .folders()
        .await?
        .into_iter()
        .find(|f| FolderKind::of(&f.kind, &f.path) == Some(FolderKind::Sent))
        .map(|f| f.id))
}

/// The provider ids other sends of `stream` already claimed.
async fn other_sends(
    ctx: &JobContext,
    workspace: campbooks_kernel::tenancy::WorkspaceId,
    stream: StreamId,
    send: SendId,
) -> Result<HashSet<String>, JobError> {
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let ids = sqlx::query_scalar!(
        r#"SELECT provider_message_id AS "id!" FROM connectors.sends
            WHERE stream_id = $1 AND id <> $2 AND provider_message_id IS NOT NULL"#,
        stream as StreamId,
        send as SendId,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ids.into_iter().collect())
}

/// A message of the Sent folder `folder` that is `document`: sent since
/// `since`, same subject, every recipient of it, not another send's.
async fn find_sent(
    mail: &ZohoMail<'_>,
    folder: &str,
    document: &OutgoingMessage,
    since: DateTime<Utc>,
    others: &HashSet<String>,
) -> Result<Option<String>, ProviderError> {
    let since = since.timestamp_millis();
    let subject = clean(&document.subject);
    let wanted: Vec<String> = document
        .to
        .iter()
        .map(|a| a.address.trim().to_lowercase())
        .collect();
    let listed = mail.list(folder, 1, LOOKUP_PAGE).await?;
    Ok(listed
        .into_iter()
        .find(|m| {
            let recipients = m.recipients();
            !others.contains(&m.message_id)
                && m.received_time.is_some_and(|t| t >= since)
                && m.subject() == subject
                && wanted.iter().all(|a| recipients.contains(a))
        })
        .map(|m| m.message_id))
}

/// The Zoho message `document` answers, when the stream has it: a reply is
/// sent as Zoho's reply to it, so Zoho threads it at the recipients.
async fn parent_of(
    ctx: &JobContext,
    workspace: campbooks_kernel::tenancy::WorkspaceId,
    stream: StreamId,
    document: &OutgoingMessage,
) -> Result<Option<String>, JobError> {
    let Some(parent) = document.in_reply_to.as_deref() else {
        return Ok(None);
    };
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let found = sqlx::query_scalar!(
        r#"SELECT provider_message_id FROM connectors.messages
            WHERE stream_id = $1 AND rfc_message_id = $2
            ORDER BY internal_date DESC LIMIT 1"#,
        stream as StreamId,
        parent,
    )
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(found)
}

/// A header-like value with every control character turned into a space.
fn clean(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Zoho's comma-separated address list.
fn list(addresses: &[Address]) -> String {
    addresses
        .iter()
        .map(|a| clean(&a.address))
        .collect::<Vec<_>>()
        .join(",")
}

/// The JSON Zoho sends `document` from, with `attachments` uploaded.
fn message_json(
    document: &OutgoingMessage,
    attachments: Vec<serde_json::Value>,
) -> serde_json::Value {
    let (content, format) = match &document.html {
        Some(html) => (html.clone(), "html"),
        None => (document.text.clone(), "plaintext"),
    };
    let mut body = serde_json::json!({
        "fromAddress": clean(&document.from.address),
        "toAddress": list(&document.to),
        "subject": clean(&document.subject),
        "content": content,
        "mailFormat": format,
        "askReceipt": "no",
        "encoding": "UTF-8",
    });
    if !document.cc.is_empty() {
        body["ccAddress"] = list(&document.cc).into();
    }
    if !document.bcc.is_empty() {
        body["bccAddress"] = list(&document.bcc).into();
    }
    if !attachments.is_empty() {
        body["attachments"] = attachments.into();
    }
    body
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::blobs::BlobRef;

    use super::*;
    use crate::{contract::OutgoingAttachment, domain::model::InboxId};

    #[test]
    fn the_json_carries_the_document() {
        let person = |address: &str| Address {
            name: Some("Someone".into()),
            address: address.into(),
        };
        let document = OutgoingMessage {
            version: 1,
            inbox: InboxId::new(),
            rfc_message_id: "s1@example.com".into(),
            from: person("sam@example.net"),
            to: vec![person("maya@northwind.example"), person("leo@example.com")],
            cc: vec![person("joana@example.com")],
            bcc: vec![],
            subject: "Re: Plano\r\nBcc: evil@example.com".into(),
            date: Utc::now(),
            in_reply_to: Some("q4@studio.example".into()),
            references: vec![],
            thread_key: None,
            text: "Done.".into(),
            html: Some("<p>Done.</p>".into()),
            attachments: vec![OutgoingAttachment {
                filename: "plan.pdf".into(),
                content_type: "application/pdf".into(),
                blob: BlobRef {
                    hash: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
                        .parse()
                        .unwrap(),
                    size: 9,
                    content_type: "application/pdf".into(),
                },
            }],
        };
        let json = message_json(&document, vec![serde_json::json!({"storeName": "1"})]);
        assert_eq!(json["fromAddress"], "sam@example.net");
        assert_eq!(json["toAddress"], "maya@northwind.example,leo@example.com");
        assert_eq!(json["ccAddress"], "joana@example.com");
        assert!(json.get("bccAddress").is_none());
        // A line break in the subject cannot add a header.
        assert_eq!(json["subject"], "Re: Plano  Bcc: evil@example.com");
        assert_eq!(json["mailFormat"], "html");
        assert_eq!(json["content"], "<p>Done.</p>");
        assert_eq!(json["attachments"][0]["storeName"], "1");
    }
}
