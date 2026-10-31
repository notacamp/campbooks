//! `conversations.release_send` — a send's undo window ended: write the
//! outgoing message document, mark the draft as sending and publish
//! `conversations.send_requested` for the connector, in one transaction.
//!
//! The job does nothing unless the draft is still scheduled at the
//! generation it was enqueued for: a cancel, a delete or a later send of
//! the same draft wins.
//!
//! Nor unless the draft is still bound to its author's request
//! (campbooks-cloud#227): `send_draft` signed the workspace, draft,
//! generation, author, inbox and everything that goes out with a key the
//! database never holds, and the job verifies that before it writes
//! anything. A draft planted or edited by SQL is left as it is, with a
//! WARN, and is not retried. The send it releases is signed in turn
//! (`drafts.send_mac`), and the outbound-requests port verifies that before
//! the connector sends.
//!
//! Both name where the send goes (campbooks-cloud#242): the connection,
//! stream and mailbox the inbox resolved to when the author asked. The job
//! resolves it again and releases only when it is the same; a stream
//! repointed by SQL meanwhile leaves the draft as it is, with a WARN.

use campbooks_kernel::{
    JobContext, JobError, JobHandler,
    blobs::BlobOwner,
    db::BeginFor as _,
    jobs::EnqueueOptions,
    outbound::Resolved,
    tenancy::{Access, UserId, WorkspaceId},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    domain::{binding, drafts, format, redact, tracked},
    events::{DRAFT_RECORD, SendRequested},
    features::check_send::{CHECK_AFTER, CheckSend, CheckSendPayload},
    model::{DraftId, DraftState, SendId},
    outgoing::{OUTGOING_CONTENT_TYPE, OUTGOING_VERSION, OutgoingAttachment, OutgoingMessage},
    tracking,
};

/// The job's payload: ids only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseSendPayload {
    /// The draft.
    pub draft: DraftId,
    /// The send it was enqueued for.
    pub generation: i32,
}

/// Releases scheduled sends.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseSend;

impl JobHandler for ReleaseSend {
    type Payload = ReleaseSendPayload;
    const NAME: &'static str = "conversations.release_send";
    const QUEUE: &'static str = "default";

    async fn run(&self, ctx: &mut JobContext, payload: ReleaseSendPayload) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("release_send enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let ws = workspace.to_string();

        let mut read = state.db.begin_for(workspace).await?;
        let released = releasable(&state, &mut read, workspace, &payload).await?;
        read.commit().await?;
        let Some((row, author, at)) = released else {
            return Ok(());
        };

        let send_id = SendId::new();
        let document = outgoing(&row, send_id)?;
        let rfc_message_id = document.rfc_message_id.clone();
        let bytes = serde_json::to_vec(&document).map_err(|e| JobError::fatal(redact::json(&e)))?;
        let blob = state.blobs.put(&ws, bytes, OUTGOING_CONTENT_TYPE).await?;
        // The send as stored, bound for the connector (the port verifies it).
        let stored = binding::StoredSend {
            workspace,
            send: send_id,
            inbox: row.inbox_id,
            author,
            document: &blob,
            // Where the author's request was verified to go
            // (campbooks-cloud#242): the connector checks it again.
            destination: binding::Bound::at(at.as_ref()),
        };
        let Some(send_mac) = stored.sign(&state.binding) else {
            return Ok(()); // No key: `releasable` already refused.
        };

        let events = state.events.clone();
        let tx = ctx.tx().await?;
        // Again where the send is marked: the author's membership is held
        // until this commits, so a removal either comes after the send or
        // stops it.
        if !still_may_send(&state, tx, workspace, author, &payload).await? {
            return Ok(());
        }
        let marked = Marked {
            send_id,
            rfc_message_id: &rfc_message_id,
            blob: &blob,
            send_mac: &send_mac,
        };
        if !mark_sending(tx, &payload, &marked).await? {
            return Ok(()); // Cancelled while the document was written.
        }
        tracked::track_draft(&state.tracker, tx, payload.draft, |draft| {
            tracking::DraftReleased { workspace, draft }
        })
        .await?;
        state
            .blobs
            .attach(tx, &ws, &blob, &BlobOwner::new(DRAFT_RECORD, payload.draft))
            .await?;
        events
            .publish(
                tx,
                &SendRequested {
                    workspace: ws.clone(),
                    inbox_id: row.inbox_id,
                    draft: payload.draft,
                    send_id,
                    message: blob,
                    author,
                },
            )
            .await?;
        // If nothing ever comes back, stop waiting (an hour on).
        campbooks_kernel::jobs::enqueue_to::<CheckSend>(
            tx,
            &CheckSendPayload {
                draft: payload.draft,
                send_id,
            },
            EnqueueOptions::for_workspace(ws.clone())
                .unique(format!("check:{send_id}"))
                .run_at(Utc::now() + CHECK_AFTER),
        )
        .await?;
        tracing::info!(%workspace, draft = %payload.draft, %send_id, "send released");
        Ok(())
    }
}

/// The draft of `payload`, its author and its inbox's destination, read in
/// `read` as whoever wrote it, when it may be released: still scheduled at
/// the payload's generation, bound to its author's request
/// (campbooks-cloud#227) for the destination its inbox resolves to now
/// (campbooks-cloud#242), and its author may still send it. It cannot change while scheduled, so the
/// document built from it stays true. `None` otherwise; nothing is written
/// before the binding is verified, and a draft whose binding does not
/// verify is left as it is.
async fn releasable(
    state: &campbooks_kernel::AppState,
    read: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    workspace: WorkspaceId,
    payload: &ReleaseSendPayload,
) -> Result<Option<Released>, JobError> {
    let author = sqlx::query_scalar!(
        "SELECT author FROM conversations.drafts WHERE id = $1",
        payload.draft as DraftId
    )
    .fetch_optional(&mut **read)
    .await?;
    let Some(author) = author else {
        return Ok(None); // Discarded meanwhile.
    };
    let author: Option<UserId> = author.map(Into::into);
    let access = Access {
        workspace,
        user: author,
        role: None,
    };
    let Some(row) = drafts::find(read, &access, payload.draft, false).await? else {
        return Ok(None);
    };
    if row.state()? != DraftState::Scheduled || row.send_generation != payload.generation {
        return Ok(None);
    }
    let mac: Option<Option<Vec<u8>>> =
        sqlx::query_scalar("SELECT request_mac FROM conversations.drafts WHERE id = $1")
            .bind(payload.draft)
            .fetch_optional(&mut **read)
            .await?;
    // A row that does not even decode was not written by `send_draft`.
    let mac = mac.flatten();
    // Where the inbox's mail goes out now: the request holds only for the
    // destination it was signed for (campbooks-cloud#242).
    let at = state
        .destinations
        .destination(read, row.inbox_id.into())
        .await?;
    let holds = binding::content_hash(&row).is_ok_and(|content| {
        binding::SendRequest {
            workspace,
            draft: payload.draft,
            generation: payload.generation,
            author,
            inbox: row.inbox_id,
            content: &content,
            destination: binding::Bound::Before,
        }
        .holds_at(&state.binding, mac.as_deref(), at.as_ref())
    });
    if !holds {
        tracing::warn!(
            %workspace,
            draft = %payload.draft,
            "a scheduled send is not bound to its author's request, or not to where its inbox \
             sends from now; it is not released"
        );
        // Not silent: it stops as a failed draft that says why (a newer
        // generation is never touched).
        stop_scheduled(
            state,
            read,
            workspace,
            payload,
            (
                "destination_changed",
                tracking::UnscheduleReason::DestinationChanged,
            ),
        )
        .await?;
        return Ok(None);
    }
    if !still_may_send(state, read, workspace, author, payload).await? {
        return Ok(None);
    }
    Ok(Some((row, author, at)))
}

/// A draft that may be released: its row, its author and where its inbox
/// sends from.
type Released = (drafts::DraftRow, Option<UserId>, Option<Resolved>);

/// Whether the author may still send the draft: still a member of the
/// workspace (held until `tx` ends) and allowed to send it (they may have
/// lost the inbox, or it was kept read-only). When not, it stops as a failed
/// draft that says why.
async fn still_may_send(
    state: &campbooks_kernel::AppState,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    workspace: WorkspaceId,
    author: Option<campbooks_kernel::tenancy::UserId>,
    payload: &ReleaseSendPayload,
) -> Result<bool, JobError> {
    if author_may_send(state, tx, workspace, author, payload.draft).await? {
        return Ok(true);
    }
    stop_scheduled(
        state,
        tx,
        workspace,
        payload,
        (
            "sender_may_not_send",
            tracking::UnscheduleReason::SenderMayNotSend,
        ),
    )
    .await?;
    tracing::info!(draft = %payload.draft, "scheduled send not released: the author may no longer send it");
    Ok(false)
}

/// Stop a scheduled send that will not be released: the draft becomes
/// `failed` with `reason` (the client says it in words) and no time, and the
/// stop is tracked. Only the generation the job was enqueued for is touched,
/// and only while it is still scheduled, so a cancel or a newer send wins.
/// Nothing is sent. Also what the sweep of lost releases does.
pub(crate) async fn stop_scheduled(
    state: &campbooks_kernel::AppState,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    workspace: WorkspaceId,
    payload: &ReleaseSendPayload,
    (reason, why): (&'static str, tracking::UnscheduleReason),
) -> Result<bool, JobError> {
    let stopped = sqlx::query(
        "UPDATE conversations.drafts
            SET state = 'failed', failure_reason = $3, send_at = NULL, updated_at = now()
          WHERE id = $1 AND state = 'scheduled' AND send_generation = $2",
    )
    .bind(payload.draft)
    .bind(payload.generation)
    .bind(reason)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if stopped > 0 {
        tracked::track_draft(&state.tracker, tx, payload.draft, |draft| {
            tracking::DraftUnscheduled {
                workspace,
                draft,
                reason: why,
            }
        })
        .await?;
    }
    Ok(stopped > 0)
}

/// What a released send is marked with.
struct Marked<'a> {
    send_id: SendId,
    rfc_message_id: &'a str,
    blob: &'a campbooks_kernel::blobs::BlobRef,
    /// The send's binding (`binding::StoredSend`).
    send_mac: &'a [u8],
}

/// Mark the draft as being sent as `marked.send_id`, written as
/// `marked.blob`, in `tx`: `false` when it is no longer scheduled at the
/// payload's generation (cancelled meanwhile). The document is recorded
/// with the send, bound by `send_mac`: the connector asks for it
/// (`outbound_requests`) and sends this one, never one an event names. The
/// author's request is spent: its binding is cleared.
async fn mark_sending(
    tx: &mut sqlx::PgConnection,
    payload: &ReleaseSendPayload,
    marked: &Marked<'_>,
) -> Result<bool, JobError> {
    let send_document =
        serde_json::to_value(marked.blob).map_err(|e| JobError::fatal(redact::json(&e)))?;
    let released = sqlx::query_scalar!(
        r#"
            UPDATE conversations.drafts
               SET state = $3, send_id = $4, rfc_message_id = $5, send_document = $6,
                   updated_at = now()
             WHERE id = $1 AND state = 'scheduled' AND send_generation = $2
            RETURNING id
            "#,
        payload.draft as DraftId,
        payload.generation,
        DraftState::Sending.as_str(),
        marked.send_id as SendId,
        marked.rfc_message_id,
        send_document,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if released.is_none() {
        return Ok(false);
    }
    sqlx::query("UPDATE conversations.drafts SET send_mac = $2, request_mac = NULL WHERE id = $1")
        .bind(payload.draft)
        .bind(marked.send_mac)
        .execute(&mut *tx)
        .await?;
    Ok(true)
}

/// Whether `author` may still send `draft`: a member of `workspace` (their
/// membership held until `conn`'s transaction ends) who is allowed to send
/// it (they may have lost the inbox, or it was kept read-only). A draft
/// with no author (a client token's) is never sent on its own. Asked again
/// by the connector, through the outbound-requests port, before it sends.
pub(crate) async fn author_may_send(
    state: &campbooks_kernel::AppState,
    conn: &mut sqlx::PgConnection,
    workspace: WorkspaceId,
    author: Option<campbooks_kernel::tenancy::UserId>,
    draft: DraftId,
) -> campbooks_kernel::Result<bool> {
    let Some(author) = author else {
        return Ok(false);
    };
    let Some(role) = state.membership.role_held(conn, workspace, author).await? else {
        return Ok(false);
    };
    let access = Access {
        workspace,
        user: Some(author),
        role: Some(role),
    };
    Ok(campbooks_kernel::authz::authorize(
        state,
        conn,
        &access,
        campbooks_kernel::authz::Action::Send,
        &campbooks_kernel::RecordRef::new(DRAFT_RECORD, &draft),
    )
    .await?
        == campbooks_kernel::authz::Decision::Allow)
}

/// The outgoing message document of `row`, sent as `send_id`.
fn outgoing(row: &drafts::DraftRow, send_id: SendId) -> Result<OutgoingMessage, JobError> {
    let domain = row
        .inbox_address
        .rsplit_once('@')
        .map_or("campbooks.invalid", |(_, d)| d);
    let rfc_message_id = format!("{send_id}@{domain}");
    let (to, cc, bcc) = row.recipients()?;
    let (html, text) = format::message(&row.body, &row.quoted);
    Ok(OutgoingMessage {
        version: OUTGOING_VERSION,
        inbox: row.inbox_id,
        send_id,
        rfc_message_id,
        from: row.from(),
        to,
        cc,
        bcc,
        subject: row.subject.clone(),
        date: Utc::now(),
        in_reply_to: row.in_reply_to.clone(),
        references: row.reference_ids.clone(),
        thread_key: row.thread_key.clone(),
        html: Some(html),
        text,
        attachments: row
            .stored_attachments()?
            .into_iter()
            .map(|a| OutgoingAttachment {
                filename: a.filename,
                content_type: a.content_type,
                blob: a.blob,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests;
