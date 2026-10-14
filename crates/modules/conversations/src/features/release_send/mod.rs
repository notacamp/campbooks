//! `conversations.release_send` — a send's undo window ended: write the
//! outgoing message document, mark the draft as sending and publish
//! `conversations.send_requested` for the connector, in one transaction.
//!
//! The job does nothing unless the draft is still scheduled at the
//! generation it was enqueued for: a cancel, a delete or a later send of
//! the same draft wins.

use campbooks_kernel::{
    JobContext, JobError, JobHandler,
    blobs::BlobOwner,
    db::BeginFor as _,
    jobs::EnqueueOptions,
    tenancy::{Access, WorkspaceId},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    domain::{drafts, format, redact},
    events::{DRAFT_RECORD, SendRequested},
    features::check_send::{CHECK_AFTER, CheckSend, CheckSendPayload},
    model::{DraftId, DraftState, SendId},
    outgoing::{OUTGOING_CONTENT_TYPE, OUTGOING_VERSION, OutgoingAttachment, OutgoingMessage},
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

        // Read the draft as whoever wrote it; it cannot change while
        // scheduled, so the document built from it stays true.
        let mut read = state.db.begin_for(workspace).await?;
        let author = sqlx::query_scalar!(
            "SELECT author FROM conversations.drafts WHERE id = $1",
            payload.draft as DraftId
        )
        .fetch_optional(&mut *read)
        .await?;
        let Some(author) = author else {
            return Ok(()); // Discarded meanwhile.
        };
        let access = Access {
            workspace,
            user: author.map(Into::into),
            role: None,
        };
        let Some(row) = drafts::find(&mut read, &access, payload.draft, false).await? else {
            return Ok(());
        };
        if !still_may_send(&state, &mut read, &access, &payload).await? {
            read.commit().await?;
            tracing::info!(draft = %payload.draft, "scheduled send not released: the author may no longer send it");
            return Ok(());
        }
        read.commit().await?;
        if row.state()? != DraftState::Scheduled || row.send_generation != payload.generation {
            return Ok(());
        }

        let send_id = SendId::new();
        let document = outgoing(&row, send_id)?;
        let rfc_message_id = document.rfc_message_id.clone();
        let bytes = serde_json::to_vec(&document).map_err(|e| JobError::fatal(redact::json(&e)))?;
        let blob = state.blobs.put(&ws, bytes, OUTGOING_CONTENT_TYPE).await?;

        let events = state.events.clone();
        let tx = ctx.tx().await?;
        let released = sqlx::query_scalar!(
            r#"
            UPDATE conversations.drafts
               SET state = $3, send_id = $4, rfc_message_id = $5, updated_at = now()
             WHERE id = $1 AND state = 'scheduled' AND send_generation = $2
            RETURNING id
            "#,
            payload.draft as DraftId,
            payload.generation,
            DraftState::Sending.as_str(),
            send_id as SendId,
            rfc_message_id,
        )
        .fetch_optional(&mut **tx)
        .await?;
        if released.is_none() {
            return Ok(()); // Cancelled while the document was written.
        }
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
                    author: author.map(Into::into),
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

/// Whether the author may still send the draft (they may have lost the
/// inbox, or it was kept read-only); when not, it goes back to a draft.
async fn still_may_send(
    state: &campbooks_kernel::AppState,
    read: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    access: &Access,
    payload: &ReleaseSendPayload,
) -> Result<bool, JobError> {
    let allowed = campbooks_kernel::authz::authorize(
        state,
        read,
        access,
        campbooks_kernel::authz::Action::Send,
        &campbooks_kernel::RecordRef::new(DRAFT_RECORD, &payload.draft),
    )
    .await?;
    if allowed == campbooks_kernel::authz::Decision::Allow {
        return Ok(true);
    }
    sqlx::query!(
        r#"UPDATE conversations.drafts
              SET state = 'draft', send_at = NULL, updated_at = now()
            WHERE id = $1 AND state = 'scheduled' AND send_generation = $2"#,
        payload.draft as DraftId,
        payload.generation,
    )
    .execute(&mut **read)
    .await?;
    Ok(false)
}

/// The outgoing message document of `row`, sent as `send_id`.
fn outgoing(row: &drafts::DraftRow, send_id: SendId) -> Result<OutgoingMessage, JobError> {
    let domain = row
        .inbox_address
        .rsplit_once('@')
        .map_or("campbooks.invalid", |(_, d)| d);
    let rfc_message_id = format!("{send_id}@{domain}");
    let (to, cc, bcc) = row.recipients()?;
    let text = if row.quoted.trim().is_empty() {
        row.body.clone()
    } else {
        format!("{}\n\n{}", row.body.trim_end(), row.quoted)
    };
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
        html: Some(format::to_html(&text)),
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
