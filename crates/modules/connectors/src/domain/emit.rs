//! Emitting what an adapter read (or sent): its index rows, and the
//! change batch announced to the owning module, in the caller's
//! transaction.

use campbooks_kernel::{
    JobError,
    blobs::{BlobOwner, BlobRef},
    tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};

use crate::{
    domain::{
        model::{InboxId, StreamId},
        removal::{BATCH_OWNER, MESSAGE_OWNER},
    },
    events::MessagesChanged,
    mail::IndexOp,
};

/// Apply the adapter's index updates, holding and releasing blobs.
pub async fn apply_index(
    tx: &mut Transaction<'static, Postgres>,
    state: &campbooks_kernel::AppState,
    workspace: WorkspaceId,
    stream: StreamId,
    ops: Vec<IndexOp>,
) -> Result<(), JobError> {
    let key = workspace.to_string();
    for op in ops {
        match op {
            IndexOp::Insert {
                row,
                provider_message_id,
                thread_id,
                rfc_message_id,
                label_ids,
                internal_date,
                document,
                blobs,
                generation,
            } => {
                let inserted = sqlx::query_scalar!(
                    r#"INSERT INTO connectors.messages
                           (id, workspace_id, stream_id, provider_message_id, thread_id, label_ids,
                            internal_date, document_hash, document_size, listed_generation,
                            rfc_message_id)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                       ON CONFLICT (stream_id, provider_message_id) DO UPDATE
                          SET label_ids = EXCLUDED.label_ids,
                              listed_generation = EXCLUDED.listed_generation
                    RETURNING id"#,
                    row,
                    workspace as WorkspaceId,
                    stream as StreamId,
                    provider_message_id,
                    thread_id,
                    &label_ids,
                    internal_date,
                    document.hash.to_string(),
                    i64::try_from(document.size).unwrap_or(i64::MAX),
                    generation,
                    rfc_message_id,
                )
                .fetch_one(&mut **tx)
                .await?;
                let owner = BlobOwner::new(MESSAGE_OWNER, inserted);
                for blob in &blobs {
                    state.blobs.attach(tx, &key, blob, &owner).await?;
                }
            }
            IndexOp::Update {
                row,
                label_ids,
                generation,
            } => {
                sqlx::query!(
                    r#"UPDATE connectors.messages
                          SET label_ids = $2, listed_generation = greatest(listed_generation, $3),
                              reemit = false
                        WHERE id = $1"#,
                    row,
                    &label_ids,
                    generation,
                )
                .execute(&mut **tx)
                .await?;
            }
            IndexOp::Remove { row } => {
                state
                    .blobs
                    .detach_owner(tx, &key, &BlobOwner::new(MESSAGE_OWNER, row))
                    .await?;
                sqlx::query!("DELETE FROM connectors.messages WHERE id = $1", row)
                    .execute(&mut **tx)
                    .await?;
            }
            IndexOp::Oversized {
                provider_message_id,
            } => {
                sqlx::query!(
                    r#"INSERT INTO connectors.oversized (workspace_id, stream_id, provider_message_id)
                       VALUES ($1, $2, $3)
                       ON CONFLICT DO NOTHING"#,
                    workspace as WorkspaceId,
                    stream as StreamId,
                    provider_message_id,
                )
                .execute(&mut **tx)
                .await?;
            }
        }
    }
    Ok(())
}

/// Record a batch, hold its blob and announce it.
#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the values are distinct"
)]
pub async fn store_batch(
    tx: &mut Transaction<'static, Postgres>,
    state: &campbooks_kernel::AppState,
    workspace: WorkspaceId,
    stream: StreamId,
    inbox: InboxId,
    batch: &BlobRef,
    cursor: &str,
    changes: usize,
    now: DateTime<Utc>,
) -> Result<(), JobError> {
    let id = uuid::Uuid::now_v7();
    sqlx::query!(
        r#"INSERT INTO connectors.batches
               (id, workspace_id, stream_id, batch_hash, batch_size, cursor, changes, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        id,
        workspace as WorkspaceId,
        stream as StreamId,
        batch.hash.to_string(),
        i64::try_from(batch.size).unwrap_or(i64::MAX),
        cursor,
        i32::try_from(changes).unwrap_or(i32::MAX),
        now,
    )
    .execute(&mut **tx)
    .await?;
    let key = workspace.to_string();
    state
        .blobs
        .attach(tx, &key, batch, &BlobOwner::new(BATCH_OWNER, id))
        .await?;
    state
        .events
        .publish(
            tx,
            &MessagesChanged {
                workspace: key,
                inbox_id: inbox,
                batch: batch.clone(),
                cursor: cursor.to_owned(),
            },
        )
        .await?;
    Ok(())
}
