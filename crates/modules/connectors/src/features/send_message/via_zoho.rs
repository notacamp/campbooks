//! Sending through Zoho Mail. Zoho takes the message as JSON (or as a reply
//! to one of its messages) and sets its own `Message-ID`, so an attempt that
//! may have gone out is looked up in the mailbox's Sent folder and Outbox
//! instead: a message sent within the attempt's window (from just before it
//! to the HTTP timeout after it), with the same subject, To, Cc, attachment
//! flag and start of text, that is not already another send's. Exactly one
//! such message proves the send; none proves it was not sent; several leave
//! it unclear. A mailbox whose Sent folder cannot be found proves nothing.

use std::collections::{BTreeSet, HashSet};

use campbooks_kernel::{JobContext, JobError, db::BeginFor as _};
use chrono::{DateTime, TimeDelta, Utc};

use super::{
    Attempt, MAX_MESSAGE_BYTES, SendMessage, Sent, Target, UNCLEAR_WAIT, finish_failed, read_blob,
    record_attempt, set_unclear, unsent,
};
use crate::{
    contract::{Address, OutgoingMessage},
    domain::{
        model::{ConnectionId, SendId, StreamId},
        schedule::{self, Rate},
    },
    mail::Fetched,
    provider::{HTTP_TIMEOUT, ProviderError},
    zoho::{
        Listed, OriginalFetch, ZohoMail,
        read::Cursor,
        state::{self, FolderKind},
    },
};

/// How far around an attempt's window a sent message still counts as it
/// (Zoho's clock and ours differ a little).
const CLOCK_SLACK: TimeDelta = TimeDelta::seconds(30);

/// Characters of text compared between a sent message and the document
/// (letters and digits only: Zoho's summary is made from the HTML).
const TEXT_PREFIX: usize = 40;

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
            stands,
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

        // An earlier attempt may have reached Zoho: look before sending.
        if target.unclear {
            let at = target.attempted_at.unwrap_or(now);
            match self.look_up(ctx, &mail, &attempt, at).await? {
                Lookup::Found(id, sent_folder) => {
                    tracing::info!(%workspace, %send, "an earlier attempt was sent");
                    let sent = self.fetch_sent(&mail, Some(sent_folder), id, now).await;
                    return self
                        .finish_sent(ctx, workspace, send, &target, stream, sent, now)
                        .await;
                }
                Lookup::NotSent => {
                    // Proven not sent.
                    set_unclear(ctx, workspace, send, false).await?;
                    target.unclear = false;
                }
                Lookup::Unclear(why) => {
                    return Err(JobError::retry_after(
                        format!("looking up an earlier attempt: {why}"),
                        UNCLEAR_WAIT,
                    ));
                }
            }
        }
        if give_up || !stands {
            return unsent(ctx, workspace, send, &target, stands).await;
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
        record_attempt(ctx.pool(), workspace, send, target.attempted_at, now).await?;
        target.unclear = true;
        match mail.send(&body, parent.as_deref()).await {
            Ok(answer) => {
                let sent_folder = folders(&target, &mail).await.ok().map(|f| f.sent);
                let id = match answer.message_id {
                    Some(id) => Some(id),
                    // No id in the answer: claim the Sent copy, so no later
                    // look-up takes it for another send.
                    None => match self.look_up(ctx, &mail, &attempt, now).await? {
                        Lookup::Found(id, _) => Some(id),
                        Lookup::NotSent | Lookup::Unclear(_) => None,
                    },
                };
                let sent = match id {
                    Some(id) => self.fetch_sent(&mail, sent_folder, id, now).await,
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
                    receiver: crate::mail::Receiver::Zoho,
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

/// What looking an attempt up found.
#[derive(Debug, PartialEq, Eq)]
enum Lookup {
    /// Exactly one message is the attempt's: its id, and the Sent folder.
    Found(String, String),
    /// None is: it was not sent.
    NotSent,
    /// Nothing can be concluded (several match, or Zoho could not say).
    Unclear(String),
}

impl SendMessage {
    /// Look for the attempt made at `at` in the Sent folder and Outbox.
    async fn look_up(
        &self,
        ctx: &JobContext,
        mail: &ZohoMail<'_>,
        attempt: &Attempt<'_>,
        at: DateTime<Utc>,
    ) -> Result<Lookup, JobError> {
        let folders = match folders_of(mail).await {
            Ok(folders) => folders,
            Err(why) => return Ok(Lookup::Unclear(why)),
        };
        let others = other_sends(ctx, attempt.workspace, attempt.stream, attempt.send).await?;
        let mut found = Vec::new();
        for folder in std::iter::once(&folders.sent).chain(folders.outbox.as_ref()) {
            let listed = match mail.list(folder, 1, LOOKUP_PAGE).await {
                Ok(listed) => listed,
                Err(err) => return Ok(Lookup::Unclear(err.to_string())),
            };
            found.extend(
                listed
                    .into_iter()
                    .filter(|m| {
                        !others.contains(&m.message_id) && is_attempt(m, attempt.document, at)
                    })
                    .map(|m| m.message_id),
            );
        }
        Ok(match found.as_slice() {
            [] => Lookup::NotSent,
            [one] => Lookup::Found(one.clone(), folders.sent),
            _ => Lookup::Unclear("several sent messages match".to_owned()),
        })
    }
}

/// The folders a sent message can be in.
struct SentFolders {
    sent: String,
    outbox: Option<String>,
}

/// The mailbox's Sent folder (from the stream's cursor, else asked).
async fn folders(target: &Target, mail: &ZohoMail<'_>) -> Result<SentFolders, String> {
    let cursor: Option<Cursor> = target
        .cursor
        .clone()
        .and_then(|c| serde_json::from_value(c).ok());
    if let Some(sent) = cursor
        .as_ref()
        .and_then(|c| c.folder_of_kind(FolderKind::Sent))
    {
        return Ok(SentFolders {
            sent: sent.to_owned(),
            outbox: None,
        });
    }
    folders_of(mail).await
}

/// The mailbox's Sent folder and Outbox, asked. No Sent folder is an error:
/// it proves nothing about a send.
async fn folders_of(mail: &ZohoMail<'_>) -> Result<SentFolders, String> {
    let folders = mail.folders().await.map_err(|e| e.to_string())?;
    let of = |kind: &str| {
        folders
            .iter()
            .find(|f| f.kind.eq_ignore_ascii_case(kind))
            .map(|f| f.id.clone())
    };
    let sent = of("Sent").ok_or_else(|| "the mailbox has no Sent folder".to_owned())?;
    Ok(SentFolders {
        sent,
        outbox: of("Outbox"),
    })
}

/// Whether sent message `m` is `document`'s attempt made at `at`.
fn is_attempt(m: &Listed, document: &OutgoingMessage, at: DateTime<Utc>) -> bool {
    let timeout = TimeDelta::from_std(HTTP_TIMEOUT).unwrap_or(TimeDelta::MAX);
    let window =
        (at - CLOCK_SLACK).timestamp_millis()..=(at + timeout + CLOCK_SLACK).timestamp_millis();
    // A copy without a time cannot be placed in the window.
    let Some(time) = m.time() else {
        return false;
    };
    let set = |list: &[Address]| -> BTreeSet<String> {
        list.iter()
            .map(|a| a.address.trim().to_lowercase())
            .collect()
    };
    window.contains(&time)
        && m.subject() == clean(&document.subject)
        && m.recipients().into_iter().collect::<BTreeSet<_>>() == set(&document.to)
        && m.copied().into_iter().collect::<BTreeSet<_>>() == set(&document.cc)
        && m.attached() != document.attachments.is_empty()
        && same_start(&m.summary(), &document.text)
}

/// Whether `summary` (Zoho's, made from the HTML) and `text` start alike:
/// their first [`TEXT_PREFIX`] letters and digits, lower-cased. A summary
/// Zoho left empty compares with nothing.
fn same_start(summary: &str, text: &str) -> bool {
    let start = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .take(TEXT_PREFIX)
            .collect()
    };
    let (a, b) = (start(summary), start(text));
    if a.is_empty() {
        return true;
    }
    let n = a.chars().count().min(b.chars().count());
    n > 0 && a.chars().take(n).eq(b.chars().take(n))
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
