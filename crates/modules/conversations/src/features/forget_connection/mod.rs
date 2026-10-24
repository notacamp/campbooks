//! `conversations.forget_connection` ← `connectors.connection_removed`: the
//! mail a removed connection brought in is deleted, or kept read-only, as
//! the event's `mail` says (the owner's decision, campbooks-cloud#117;
//! `contracts/connectors.connection_removed.v1.schema.json`).
//!
//! - **Delete** (the default): each inbox it fed goes with its messages,
//!   attachments, conversations, labels, decisions, corrections, drafts,
//!   memberships and taught rules (with their runs, which stop); the files they held are let go (the kernel's sweep deletes
//!   them once nothing else holds them); the inbox is remembered as removed,
//!   so a late batch or a replayed grant never brings it back; and
//!   `conversations.inbox_removed` tells search to forget it and the change
//!   feed that it is gone.
//! - **Keep**: each inbox is frozen read-only (`detached_at`): nothing syncs
//!   into it, nothing is written or sent from it (scheduled sends go back to
//!   drafts, which their authors may still delete), its taught rules stop
//!   acting (their runs are cancelled; the member's words, what they changed
//!   and the dismissed suggestions are purged), its members keep reading.
//!
//! Idempotent: a replay finds nothing left to do.

use campbooks_kernel::{
    AppState, Event, EventHandler, JobContext, JobError, blobs::BlobOwner, tenancy::WorkspaceId,
};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::tracked,
    events::{
        ConnectionRemoved, DRAFT_RECORD, InboxChanged, InboxRemoved, MESSAGE_RECORD, MailChoice,
    },
    features::apply_changes::{ATTACHMENT_RECORD, BATCH_RECORD},
    model::{ConnectionId, InboxId},
    tracking,
};

/// Deletes or freezes a removed connection's inboxes.
pub struct ForgetConnection;

impl EventHandler for ForgetConnection {
    type Event = ConnectionRemoved;
    const NAME: &'static str = "conversations.forget_connection";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<ConnectionRemoved>) -> Result<(), JobError> {
        let removed = event.payload;
        let workspace: WorkspaceId = removed
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("connection_removed names an invalid workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        // The inboxes it fed: those the event names, and any still pointing
        // at the connection.
        let inboxes = sqlx::query_scalar!(
            r#"SELECT id AS "id: InboxId" FROM conversations.inboxes
                WHERE id = ANY($1) OR connection_id = $2
                ORDER BY id"#,
            &removed.inbox_ids as &[InboxId],
            removed.connection as ConnectionId,
        )
        .fetch_all(&mut **tx)
        .await?;
        for inbox in inboxes {
            match removed.mail {
                MailChoice::Delete => delete_inbox(&state, tx, workspace, inbox).await?,
                MailChoice::Keep => keep_inbox(&state, tx, workspace, inbox).await?,
            }
        }
        // Inboxes it fed that were never created (or are gone already) are
        // remembered too, so a late grant cannot create them.
        if removed.mail == MailChoice::Delete {
            sqlx::query!(
                r#"INSERT INTO conversations.removed_inboxes (inbox_id, workspace_id)
                   SELECT id, $2 FROM unnest($1::uuid[]) AS ids(id)
                   ON CONFLICT DO NOTHING"#,
                &removed.inbox_ids as &[InboxId],
                workspace.as_uuid(),
            )
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }
}

/// Delete `inbox` and everything in it, in `tx`.
pub async fn delete_inbox(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    inbox: InboxId,
) -> Result<(), JobError> {
    let ws = workspace.to_string();
    // Lock the inbox first: a batch being applied holds it `FOR SHARE`, so
    // this waits for it to commit and then sees every row it wrote (and a
    // batch that comes later finds the inbox gone and is dropped). Reading
    // first and deleting later could miss its files.
    sqlx::query_scalar!(
        "SELECT id FROM conversations.inboxes WHERE id = $1 FOR UPDATE",
        inbox as InboxId
    )
    .fetch_optional(&mut **tx)
    .await?;
    let messages = sqlx::query_scalar!(
        "SELECT id FROM conversations.messages WHERE inbox_id = $1",
        inbox as InboxId
    )
    .fetch_all(&mut **tx)
    .await?;
    let attachments = sqlx::query_scalar!(
        r#"SELECT a.id FROM conversations.attachments a
             JOIN conversations.messages m ON m.id = a.message_id
            WHERE m.inbox_id = $1"#,
        inbox as InboxId
    )
    .fetch_all(&mut **tx)
    .await?;
    let drafts = sqlx::query_scalar!(
        "SELECT id FROM conversations.drafts WHERE inbox_id = $1",
        inbox as InboxId
    )
    .fetch_all(&mut **tx)
    .await?;
    let blobs = &state.blobs;
    let mut released = 0;
    released += blobs
        .detach_owners(tx, &ws, MESSAGE_RECORD, &messages)
        .await?;
    released += blobs
        .detach_owners(tx, &ws, ATTACHMENT_RECORD, &attachments)
        .await?;
    released += blobs.detach_owners(tx, &ws, DRAFT_RECORD, &drafts).await?;
    released += blobs
        .detach_owner(tx, &ws, &BlobOwner::new(BATCH_RECORD, inbox))
        .await?;
    // Everything else goes with the inbox (foreign keys cascade): members,
    // conversations, messages, attachments, labels, decisions, corrections
    // and drafts.
    let deleted = sqlx::query!(
        "DELETE FROM conversations.inboxes WHERE id = $1",
        inbox as InboxId
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    sqlx::query!(
        r#"INSERT INTO conversations.removed_inboxes (inbox_id, workspace_id)
           VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
        inbox as InboxId,
        workspace.as_uuid(),
    )
    .execute(&mut **tx)
    .await?;
    if deleted > 0 {
        state
            .events
            .publish(
                tx,
                &InboxRemoved {
                    workspace: ws,
                    inbox,
                },
            )
            .await?;
        state
            .tracker
            .track(
                tx,
                &tracking::InboxClosed {
                    workspace,
                    inbox_id: *inbox.as_uuid(),
                    mail: tracking::ClosedMail::Deleted,
                    message_count: tracked::count(messages.len()),
                    draft_count: tracked::count(drafts.len()),
                },
            )
            .await?;
    }
    tracing::info!(
        %inbox,
        messages = messages.len(),
        drafts = drafts.len(),
        files_released = released,
        "inbox deleted with its connection"
    );
    Ok(())
}

/// Freeze `inbox` read-only, in `tx`.
async fn keep_inbox(
    state: &AppState,
    tx: &mut Transaction<'static, Postgres>,
    workspace: WorkspaceId,
    inbox: InboxId,
) -> Result<(), JobError> {
    // The update waits for a batch being applied (it holds the inbox
    // `FOR SHARE`); a later batch finds the inbox frozen and is dropped.
    let frozen = sqlx::query!(
        r#"UPDATE conversations.inboxes SET detached_at = now()
            WHERE id = $1 AND detached_at IS NULL"#,
        inbox as InboxId
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if frozen > 0 {
        let (messages, drafts): (i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM conversations.messages WHERE inbox_id = $1),
                    (SELECT count(*) FROM conversations.drafts WHERE inbox_id = $1)",
        )
        .bind(inbox)
        .fetch_one(&mut **tx)
        .await?;
        state
            .tracker
            .track(
                tx,
                &tracking::InboxClosed {
                    workspace,
                    inbox_id: *inbox.as_uuid(),
                    mail: tracking::ClosedMail::Kept,
                    message_count: messages,
                    draft_count: drafts,
                },
            )
            .await?;
        // Nothing is sent from it: scheduled sends go back to drafts (their
        // release jobs act only at the generation they were given).
        sqlx::query!(
            r#"UPDATE conversations.drafts
                  SET state = 'draft', send_at = NULL, send_generation = send_generation + 1,
                      version = version + 1, updated_at = now()
                WHERE inbox_id = $1 AND state = 'scheduled'"#,
            inbox as InboxId
        )
        .execute(&mut **tx)
        .await?;
        // Its rules act no more: their runs stop, and what only served
        // acting again goes (the member's words, what each rule changed, the
        // suggestions turned down). The rules stay listed, paused.
        // Runs before rules, in id order (`rule_runs::lock_running`).
        sqlx::query!(
            r#"SELECT u.id FROM conversations.rule_runs u
                WHERE u.state = 'running'
                  AND u.rule_id IN (SELECT id FROM conversations.rules WHERE inbox_id = $1)
                ORDER BY u.id FOR UPDATE OF u"#,
            inbox as InboxId
        )
        .fetch_all(&mut **tx)
        .await?;
        sqlx::query!(
            r#"UPDATE conversations.rule_runs SET state = 'cancelled', finished_at = now()
                WHERE state = 'running'
                  AND rule_id IN (SELECT id FROM conversations.rules WHERE inbox_id = $1)"#,
            inbox as InboxId
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            "UPDATE conversations.rules SET origin_text = NULL WHERE inbox_id = $1",
            inbox as InboxId
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            r#"UPDATE conversations.rule_applications SET before = '{}'::jsonb
                WHERE rule_id IN (SELECT id FROM conversations.rules WHERE inbox_id = $1)"#,
            inbox as InboxId
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            "DELETE FROM conversations.rule_dismissals WHERE inbox_id = $1",
            inbox as InboxId
        )
        .execute(&mut **tx)
        .await?;
        crate::features::rule_notices::forget_inbox(tx, inbox).await?;
        state
            .events
            .publish(
                tx,
                &InboxChanged {
                    workspace: workspace.to_string(),
                    inbox,
                },
            )
            .await?;
        tracing::info!(%inbox, "inbox kept read-only after its connection was removed");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
