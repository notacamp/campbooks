//! `PATCH /api/v1/w/{workspace_id}/drafts/{draft_id}` — save a draft as the
//! member writes (only the fields given change; the `version` read must
//! still be current), and `DELETE` — discard it.
//!
//! Saving publishes no event: drafts are private, and autosave would fill
//! the retained event log. Dropping an attachment releases the draft's hold on its blob.

use std::collections::HashSet;

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    blobs::BlobOwner,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::Deserialize;
use serde_json::json;
use utoipa::ToSchema;

use crate::features::{
    signature,
    upload_draft_attachment::{max_attachment_bytes_of, over_limit},
};
use crate::{
    domain::{
        drafts::{
            self, DraftView, StoredAttachment, attachment_bytes, check_limits, shown_to_author,
            tidy,
        },
        format,
        paths::DraftPath,
        resources, tracked,
    },
    events::DRAFT_RECORD,
    model::{Address, AttachmentId, DraftId, DraftState, InboxId},
    scopes::ConversationsSend,
    tracking,
};
use campbooks_kernel::authz::Action;

/// The fields to change; the others stay.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DraftChanges {
    /// The version this change was made on (from the draft last read).
    pub version: u32,
    /// The `content_token` that came with the draft on screen (its last
    /// save's answer, or the draft as read). Every save keeps fields it
    /// cannot set (the inbox, the reply headers, the attachments' files),
    /// so every save names what it was shown: missing, another member's, or
    /// not the draft as it is now answers 409 `content_changed` and nothing
    /// is saved (campbooks-cloud#242).
    #[serde(default)]
    pub content_token: Option<String>,
    /// Recipients.
    #[serde(default)]
    pub to: Option<Vec<Address>>,
    /// Copied recipients.
    #[serde(default)]
    pub cc: Option<Vec<Address>>,
    /// Blind-copied recipients.
    #[serde(default)]
    pub bcc: Option<Vec<Address>>,
    /// The subject.
    #[serde(default)]
    pub subject: Option<String>,
    /// The text.
    #[serde(default)]
    pub body: Option<String>,
    /// The quoted history.
    #[serde(default)]
    pub quoted: Option<String>,
    /// The attachments to keep, forwarded or uploaded (the others are dropped).
    #[serde(default)]
    pub attachments: Option<Vec<AttachmentId>>,
    /// `false` leaves the signature off; `true` adds the author's current
    /// signature for the inbox (campbooks-cloud#393). Absent keeps it as it is.
    #[serde(default)]
    pub signature: Option<bool>,
    /// Move the draft to another inbox the author may send from
    /// (campbooks-cloud#394): it is then sent from that inbox's address. A
    /// reply keeps its `In-Reply-To` and `References` and drops the first
    /// mailbox's thread key; a signature that was on switches to the
    /// author's for the new inbox. The inbox must be in this workspace with
    /// the `sender` or `manager` role and not read-only: a `reader` gets
    /// 403 `inbox_read_only`, any other inbox 404. Choosing From sends nothing.
    #[serde(default)]
    pub inbox_id: Option<InboxId>,
}

/// Save a draft.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/drafts/{draft_id}",
    operation_id = "conversations.update_draft",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("draft_id" = DraftId, Path, description = "The draft."),
    ),
    request_body = DraftChanges,
    responses(
        (status = OK, description = "The draft, with its new version.", body = DraftView),
        (status = BAD_REQUEST, description = "A field over its limit.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read its inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Changed since that version (`draft_changed`), on its way (`draft_not_editable`), or not what you were shown (`content_changed`): read it again, then save.",
            body = Problem, content_type = "application/problem+json"),
        (status = PAYLOAD_TOO_LARGE, description = "Moving it to an inbox whose mailbox carries less than the draft's attachments (`attachments_too_large_for_mailbox`; `conversations.list_inboxes` says it as `max_attachment_bytes`): nothing moved.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<DraftPath>,
    extract::Json(changes): extract::Json<DraftChanges>,
) -> Result<Json<DraftView>> {
    let access = &workspace.access;
    let id = path.draft_id;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = drafts::load(&state, &mut tx, access, id, true, Action::Write).await?;
    if !row.state()?.editable() {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("draft_not_editable"),
        ));
    }
    if i64::from(changes.version) != i64::from(row.version) {
        return Err(Error::Conflict(crate::MESSAGES.reason("draft_changed")));
    }
    // What the save keeps is what the member was shown: the token is checked
    // against the row before the write, and only then is a new one issued
    // over what the save leaves (campbooks-cloud#242).
    shown_to_author(&state, access, &row, changes.content_token.as_deref())?;
    let (to, cc, bcc) = row.recipients()?;
    let was = (to.clone(), cc.clone(), bcc.clone());
    let [to, cc, bcc] = tidy([
        changes.to.unwrap_or(to),
        changes.cc.unwrap_or(cc),
        changes.bcc.unwrap_or(bcc),
    ]);
    let subject = changes.subject.unwrap_or_else(|| row.subject.clone());
    // The assistant's words never carry a link whose words hide where it
    // goes: only the member's own request writes `[words](url)`.
    let body = match changes.body {
        Some(text) if workspace.invoked_by.is_some() => format::literal_links(&text),
        Some(text) => text,
        None => row.body.clone(),
    };
    let quoted = changes.quoted.unwrap_or_else(|| row.quoted.clone());
    let (moved, signature) = from_and_signature(
        &state,
        &mut tx,
        access,
        &row,
        changes.inbox_id,
        changes.signature,
    )
    .await?;
    check_limits(to.len() + cc.len() + bcc.len(), &subject, &body, &quoted)?;

    let mut attachments = row.stored_attachments()?;
    let attachments_before = attachments.clone();
    if let Some(keep) = changes.attachments {
        let keep: HashSet<AttachmentId> = keep.into_iter().collect();
        let (kept, dropped): (Vec<_>, Vec<_>) =
            attachments.into_iter().partition(|a| keep.contains(&a.id));
        let still: HashSet<_> = kept.iter().map(|a| a.blob.hash).collect();
        let ws = access.workspace.to_string();
        let owner = BlobOwner::new(DRAFT_RECORD, id);
        for gone in dropped.iter().filter(|a| !still.contains(&a.blob.hash)) {
            state
                .blobs
                .detach(&mut tx, &ws, &gone.blob.hash, &owner)
                .await?;
        }
        attachments = kept;
    }

    if let Some(target) = moved {
        move_row(&state, &mut tx, id, target, &attachments).await?;
    }
    sqlx::query!(
        r#"
        UPDATE conversations.drafts
           SET to_recipients = $2, cc_recipients = $3, bcc_recipients = $4, subject = $5,
               body = $6, quoted = $7, attachments = $8, signature = $10, version = version + 1,
               state = $9, failure_reason = NULL, updated_at = now()
         WHERE id = $1
        "#,
        id as DraftId,
        json!(to),
        json!(cc),
        json!(bcc),
        subject,
        body,
        quoted,
        json!(attachments),
        DraftState::Draft.as_str(),
        signature,
    )
    .execute(&mut *tx)
    .await?;
    let draft = drafts::load(&state, &mut tx, access, id, false, Action::Read)
        .await?
        .shown(&state.binding, access)?;
    // Tracked when the save changed something (campbooks-cloud#325): an
    // autosave of what is there already tracks nothing.
    let edited = was != (to, cc, bcc)
        || subject != row.subject
        || body != row.body
        || quoted != row.quoted
        || signature != row.signature
        || moved.is_some()
        || attachments != attachments_before
        || row.state()? != DraftState::Draft;
    if edited {
        let workspace = access.workspace;
        tracked::track_draft(&state.tracker, &mut tx, id, |draft| {
            tracking::DraftUpdated { workspace, draft }
        })
        .await?;
    }
    tx.commit().await?;
    Ok(Json(draft))
}

/// Put draft `id` in `inbox`. The provider's thread key belongs to the first
/// mailbox, so it goes; the reply headers stay, and the recipient's mail
/// still threads the reply.
async fn move_row(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    id: DraftId,
    inbox: InboxId,
    attachments: &[StoredAttachment],
) -> Result<()> {
    // The new mailbox may take less than the old one (Microsoft Graph): the
    // draft does not move with more than it can send.
    let limit = max_attachment_bytes_of(state, tx, inbox).await?;
    if attachment_bytes(attachments) > limit {
        return Err(over_limit(limit));
    }
    sqlx::query("UPDATE conversations.drafts SET inbox_id = $2, thread_key = NULL WHERE id = $1")
        .bind(id)
        .bind(inbox)
        .execute(tx)
        .await?;
    Ok(())
}

/// Where the draft goes from after this save, and the signature it carries.
///
/// From is another inbox the author may send from (the catalogue decides: a
/// reader 403, anything else outside their inboxes 404); `None` when it
/// stays. The signature follows the author to the new inbox when it was on
/// (off stays off), unless the save says `true` (the author's current one
/// for the inbox) or `false` (none).
async fn from_and_signature(
    state: &AppState,
    tx: &mut sqlx::PgConnection,
    access: &campbooks_kernel::tenancy::Access,
    row: &drafts::DraftRow,
    to: Option<InboxId>,
    signed: Option<bool>,
) -> Result<(Option<InboxId>, String)> {
    let moved = to.filter(|target| *target != row.inbox_id);
    if let Some(target) = moved {
        resources::authorize(state, tx, access, Action::Send, &resources::inbox(target)).await?;
    }
    let inbox = moved.unwrap_or(row.inbox_id);
    let signature = match signed {
        Some(false) => String::new(),
        Some(true) => signature::own(tx, inbox, access.user).await?,
        None if moved.is_some() && !row.signature.is_empty() => {
            signature::own(tx, inbox, access.user).await?
        }
        None => row.signature.clone(),
    };
    Ok((moved, signature))
}

/// Discard a draft (not one on its way: cancel its send first).
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/drafts/{draft_id}",
    operation_id = "conversations.delete_draft",
    tag = "conversations",
    security(("bearer" = ["conversations:send"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["conversations.draft"]).access("conversations.draft", campbooks_kernel::authz::Action::Delete)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("draft_id" = DraftId, Path, description = "The draft."),
    ),
    responses(
        (status = NO_CONTENT, description = "Discarded."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you only read its inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such draft, or it is not yours.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "Scheduled or on its way (`draft_not_editable`): cancel its send first.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsSend>,
    extract::Path(path): extract::Path<DraftPath>,
) -> Result<StatusCode> {
    let access = &workspace.access;
    let id = path.draft_id;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let row = drafts::load(&state, &mut tx, access, id, true, Action::Delete).await?;
    // A scheduled send is taken back only by a deliberate cancel
    // (`conversations.cancel_send`, the Undo), never as a side effect of a
    // discard (campbooks-cloud#242): then it is a draft again and may go.
    if !row.state()?.editable() {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("draft_not_editable"),
        ));
    }
    // Tracked as it was, before it goes (campbooks-cloud#325).
    let workspace = access.workspace;
    tracked::track_draft(&state.tracker, &mut tx, id, |draft| {
        tracking::DraftDiscarded { workspace, draft }
    })
    .await?;
    discard(&state, &mut tx, &access.workspace.to_string(), id).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Delete draft `id` and release every blob it holds.
pub async fn discard(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace: &str,
    id: DraftId,
) -> Result<()> {
    sqlx::query!(
        "DELETE FROM conversations.drafts WHERE id = $1",
        id as DraftId
    )
    .execute(&mut **tx)
    .await?;
    state
        .blobs
        .detach_owner(tx, workspace, &BlobOwner::new(DRAFT_RECORD, id))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
