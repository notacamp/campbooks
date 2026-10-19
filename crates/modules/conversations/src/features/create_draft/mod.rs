//! `POST /api/v1/w/{workspace_id}/drafts` — start a message: a new one in
//! an inbox, or a reply, reply all or forward of a message. For a reply or
//! forward the server fills the recipients, the subject, the threading
//! headers and (forward) the attachments; the caller may give any field to
//! use instead. The quoted history is the client's to write.
//!
//! A nudge (campbooks-cloud#293) is a reply all of the message a waiting
//! need comes from, to the person waited on, everyone else reply all would
//! address copied, the inbox's own address in neither; only while the
//! conversation waits on someone (else 409 `not_waiting`).

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    blobs::{BlobOwner, BlobRef},
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;
use serde_json::json;
use utoipa::ToSchema;

use crate::domain::resources;
use crate::{
    domain::{
        access::visible_inboxes,
        drafts::{self, DraftView, MAX_REFERENCES, StoredAttachment, check_limits, decode, tidy},
    },
    events::DRAFT_RECORD,
    model::{Address, AttachmentId, ConversationId, DraftId, DraftKind, InboxId, MessageId},
    scopes::ConversationsSend,
};
use campbooks_kernel::authz::Action;

/// A message to start.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewDraft {
    /// What it answers.
    pub kind: DraftKind,
    /// The inbox to send from (a new message).
    #[serde(default)]
    pub inbox_id: Option<InboxId>,
    /// The message replied to or forwarded (a reply or forward).
    #[serde(default)]
    pub message_id: Option<MessageId>,
    /// Recipients, instead of the filled ones.
    #[serde(default)]
    pub to: Option<Vec<Address>>,
    /// Copied recipients, instead of the filled ones.
    #[serde(default)]
    pub cc: Option<Vec<Address>>,
    /// Blind-copied recipients.
    #[serde(default)]
    pub bcc: Option<Vec<Address>>,
    /// The subject, instead of the filled one.
    #[serde(default)]
    pub subject: Option<String>,
    /// The text.
    #[serde(default)]
    pub body: Option<String>,
    /// The quoted history (written by the client: an attribution line and
    /// the message quoted with `> `, or a forwarded-message header).
    #[serde(default)]
    pub quoted: Option<String>,
}

/// Start a draft.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/drafts",
    operation_id = "conversations.create_draft",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.delete_draft")
        .assistant()
        .records(["conversations.draft"]).access("conversations.inbox", campbooks_kernel::authz::Action::Send)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    request_body = NewDraft,
    responses(
        (status = CREATED, description = "The draft.", body = DraftView),
        (status = BAD_REQUEST, description = "A missing inbox or message, or a field over its limit.",
            body = Problem, content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read this inbox (`inbox_read_only`).",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such inbox or message, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "A nudge for a conversation that waits on nobody (`not_waiting`).",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Json(body): extract::Json<NewDraft>,
) -> Result<(StatusCode, Json<DraftView>)> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let filled = match body.kind {
        DraftKind::New => {
            let inbox = body
                .inbox_id
                .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("draft_needs_inbox")))?;
            let inboxes = visible_inboxes(&mut tx, access).await?;
            if !inboxes.contains(&inbox) {
                return Err(Error::NotFound);
            }
            Filled::new(inbox)
        }
        DraftKind::Nudge => {
            let message = body
                .message_id
                .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("draft_needs_message")))?;
            nudge(&mut tx, access, message).await?
        }
        kind => {
            let message = body
                .message_id
                .ok_or_else(|| Error::BadRequest(crate::MESSAGES.reason("draft_needs_message")))?;
            fill(&mut tx, access, kind, message).await?
        }
    };
    resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Send,
        &resources::inbox(filled.inbox),
    )
    .await?;

    // A nudge goes where the server addresses it, whatever the client sent:
    // the member changes recipients in the composer afterwards, as with any
    // draft.
    let (to, cc) = if body.kind == DraftKind::Nudge {
        (filled.to, filled.cc)
    } else {
        (body.to.unwrap_or(filled.to), body.cc.unwrap_or(filled.cc))
    };
    let [to, cc, bcc] = tidy([to, cc, body.bcc.unwrap_or_default()]);
    let subject = body.subject.unwrap_or(filled.subject);
    let text = body.body.unwrap_or_default();
    let quoted = body.quoted.unwrap_or_default();
    check_limits(to.len() + cc.len() + bcc.len(), &subject, &text, &quoted)?;

    let id = DraftId::new();
    sqlx::query!(
        r#"
        INSERT INTO conversations.drafts
            (id, workspace_id, inbox_id, author, kind, conversation_id, source_message_id,
             to_recipients, cc_recipients, bcc_recipients, subject, body, quoted, in_reply_to,
             reference_ids, thread_key, attachments)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
        "#,
        id as DraftId,
        access.workspace.as_uuid(),
        filled.inbox as InboxId,
        access.user.map(|u| *u.as_uuid()),
        body.kind.as_str(),
        filled.conversation as Option<ConversationId>,
        filled.message as Option<MessageId>,
        json!(to),
        json!(cc),
        json!(bcc),
        subject,
        text,
        quoted,
        filled.in_reply_to,
        &filled.references,
        filled.thread_key,
        json!(filled.attachments),
    )
    .execute(&mut *tx)
    .await?;
    let ws = access.workspace.to_string();
    let owner = BlobOwner::new(DRAFT_RECORD, id);
    for attachment in &filled.attachments {
        state
            .blobs
            .attach(&mut tx, &ws, &attachment.blob, &owner)
            .await?;
    }
    let draft = drafts::load(&state, &mut tx, access, id, false, Action::Read)
        .await?
        .shown(&state.binding, access)?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(draft)))
}

/// What the server fills in.
struct Filled {
    inbox: InboxId,
    conversation: Option<ConversationId>,
    message: Option<MessageId>,
    to: Vec<Address>,
    cc: Vec<Address>,
    subject: String,
    in_reply_to: Option<String>,
    references: Vec<String>,
    thread_key: Option<String>,
    attachments: Vec<StoredAttachment>,
}

impl Filled {
    fn new(inbox: InboxId) -> Self {
        Self {
            inbox,
            conversation: None,
            message: None,
            to: Vec::new(),
            cc: Vec::new(),
            subject: String::new(),
            in_reply_to: None,
            references: Vec::new(),
            thread_key: None,
            attachments: Vec::new(),
        }
    }
}

/// `messages.recipients`.
#[derive(Deserialize)]
struct Recipients {
    #[serde(default)]
    to: Vec<Address>,
    #[serde(default)]
    cc: Vec<Address>,
    #[serde(default)]
    reply_to: Vec<Address>,
}

/// Fill a reply, reply all or forward of `message`.
async fn fill(
    conn: &mut sqlx::PgConnection,
    access: &campbooks_kernel::tenancy::Access,
    kind: DraftKind,
    message: MessageId,
) -> Result<Filled> {
    let inboxes = visible_inboxes(conn, access).await?;
    let row = sqlx::query!(
        r#"
        SELECT m.inbox_id AS "inbox: InboxId", m.conversation_id AS "conversation: ConversationId",
               m.direction, m.rfc_message_id, m.reference_ids, m.sender, m.recipients, m.subject,
               c.thread_key, i.address AS inbox_address
          FROM conversations.messages m
          JOIN conversations.conversations c ON c.id = m.conversation_id
          JOIN conversations.inboxes i ON i.id = m.inbox_id
         WHERE m.id = $1 AND m.inbox_id = ANY($2)
        "#,
        message as MessageId,
        &inboxes as &[InboxId],
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(Error::NotFound)?;
    let sender: Address = decode(row.sender)?;
    let recipients: Recipients = decode(row.recipients)?;
    let ours = |a: &Address| a.address.trim().eq_ignore_ascii_case(&row.inbox_address);
    let outbound = row.direction == "outbound";
    let (to, cc) = match kind {
        DraftKind::Reply | DraftKind::ReplyAll => {
            let mut to = if outbound {
                recipients.to.clone()
            } else if recipients.reply_to.is_empty() {
                vec![sender]
            } else {
                recipients.reply_to.clone()
            };
            let mut cc = Vec::new();
            if kind == DraftKind::ReplyAll {
                if !outbound {
                    to.extend(recipients.to.iter().cloned());
                }
                cc.extend(recipients.cc.iter().cloned());
            }
            to.retain(|a| !ours(a));
            cc.retain(|a| !ours(a));
            (to, cc)
        }
        DraftKind::Forward | DraftKind::New | DraftKind::Nudge => (Vec::new(), Vec::new()),
    };
    let mut references = row.reference_ids;
    if let Some(id) = &row.rfc_message_id {
        references.push(id.clone());
    }
    let skip = references.len().saturating_sub(MAX_REFERENCES);
    let references: Vec<String> = references.into_iter().skip(skip).collect();
    let attachments = if kind == DraftKind::Forward {
        forwarded(conn, message).await?
    } else {
        Vec::new()
    };
    Ok(Filled {
        inbox: row.inbox,
        conversation: Some(row.conversation),
        message: Some(message),
        to,
        cc,
        subject: prefixed(kind, &row.subject),
        in_reply_to: row.rfc_message_id,
        references,
        thread_key: row.thread_key,
        attachments,
    })
}

/// The open wait of a conversation: its message, and the person's address
/// and name.
const WAIT: &str = "SELECT message_id, awaited_address, awaited_name FROM conversations.needs \
      WHERE conversation_id = $1 AND state = 'open' AND reason IN ('waiting', 'nudge') \
        AND awaited_address IS NOT NULL";

/// Fill a nudge from `message`'s conversation (campbooks-cloud#293): a reply
/// all of its waiting need's message, the person waited on in `To` and
/// everyone else reply all would address in `Cc`. 404 when the caller cannot
/// see `message`, 409 `not_waiting` when the conversation waits on nobody.
async fn nudge(
    conn: &mut sqlx::PgConnection,
    access: &campbooks_kernel::tenancy::Access,
    message: MessageId,
) -> Result<Filled> {
    let seen = fill(conn, access, DraftKind::ReplyAll, message).await?;
    let wait: Option<(MessageId, String, Option<String>)> = sqlx::query_as(WAIT)
        .bind(seen.conversation)
        .fetch_optional(&mut *conn)
        .await?;
    let (waited, address, name) =
        wait.ok_or_else(|| Error::Conflict(crate::MESSAGES.reason("not_waiting")))?;
    let mut filled = if waited == message {
        seen
    } else {
        fill(conn, access, DraftKind::ReplyAll, waited).await?
    };
    // Never a nudge to the inbox itself (mail it sent itself, filed as
    // received): nothing to wait for.
    let inbox: String =
        sqlx::query_scalar("SELECT address FROM conversations.inboxes WHERE id = $1")
            .bind(filled.inbox)
            .fetch_one(&mut *conn)
            .await?;
    if address.trim().eq_ignore_ascii_case(inbox.trim()) {
        return Err(Error::Conflict(crate::MESSAGES.reason("not_waiting")));
    }
    let awaited = Address {
        name: name.filter(|n| !n.trim().is_empty()),
        address,
    };
    let mut cc: Vec<Address> = std::mem::take(&mut filled.to);
    cc.append(&mut filled.cc);
    cc.retain(|a| {
        !a.address
            .trim()
            .eq_ignore_ascii_case(awaited.address.trim())
    });
    filled.to = vec![awaited];
    filled.cc = cc;
    Ok(filled)
}

/// The attachments of `message` a forward carries (not the inline ones).
async fn forwarded(
    conn: &mut sqlx::PgConnection,
    message: MessageId,
) -> Result<Vec<StoredAttachment>> {
    let rows = sqlx::query!(
        r#"
        SELECT id AS "id: AttachmentId", filename, content_type, size, blob
          FROM conversations.attachments
         WHERE message_id = $1 AND NOT inline
         ORDER BY position
        "#,
        message as MessageId,
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(StoredAttachment {
                id: row.id,
                filename: row.filename,
                content_type: row.content_type,
                size: u64::try_from(row.size).unwrap_or_default(),
                blob: decode::<BlobRef>(row.blob)?,
            })
        })
        .collect()
}

/// `subject` with `Re: ` or `Fwd: ` in front, unless it has one already.
fn prefixed(kind: DraftKind, subject: &str) -> String {
    let subject = subject.trim();
    let lower = subject.to_lowercase();
    match kind {
        DraftKind::Reply | DraftKind::ReplyAll if !lower.starts_with("re:") => {
            format!("Re: {subject}")
        }
        DraftKind::Forward if !lower.starts_with("fwd:") && !lower.starts_with("fw:") => {
            format!("Fwd: {subject}")
        }
        _ => subject.to_owned(),
    }
}

#[cfg(test)]
mod tests;
