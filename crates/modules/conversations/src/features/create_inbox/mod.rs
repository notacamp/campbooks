//! `conversations.create_inbox` — a connection gained the `messages`
//! capability: `connectors.capability_granted` names the inbox the connector
//! minted and the account behind it (a connection-account v1 blob), and this
//! creates the inbox with the granting member as its `manager`
//! (`contracts/connectors.capability_granted.v1.schema.json`). Idempotent:
//! a replayed event changes nothing.

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    domain::{blobs::read_bytes, redact},
    events::{AccountDocument, CapabilityGranted},
    model::{Channel, ConnectionId, InboxId, InboxRole},
};

/// Largest account document read, in bytes.
const MAX_ACCOUNT_BYTES: usize = 64 * 1024;

/// Creates the inbox a `messages` grant feeds.
pub struct CreateInbox;

impl EventHandler for CreateInbox {
    type Event = CapabilityGranted;
    const NAME: &'static str = "conversations.create_inbox";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<CapabilityGranted>) -> Result<(), JobError> {
        let granted = event.payload;
        let Some(inbox) = granted.inbox_id else {
            return Ok(());
        };
        if granted.capability != "messages" {
            return Ok(());
        }
        let workspace: WorkspaceId = granted
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("capability_granted names an invalid workspace"))?;
        let (bytes, cut) = read_bytes(
            &ctx.state().blobs,
            &granted.workspace,
            &granted.account,
            MAX_ACCOUNT_BYTES,
        )
        .await?;
        if cut {
            return Err(JobError::fatal(
                "the account document is larger than the limit",
            ));
        }
        let account: AccountDocument = serde_json::from_slice(&bytes).map_err(|e| {
            JobError::fatal(format!(
                "the account document does not parse: {}",
                redact::json(&e)
            ))
        })?;
        if account.version != 1 {
            return Err(JobError::fatal(
                "the account document's version is not supported",
            ));
        }

        let tx = ctx.tx().await?;
        sqlx::query!(
            r#"
            INSERT INTO conversations.inboxes
                (id, workspace_id, channel, address, display_name, connection_id, owner)
            SELECT $1, $2, $3, $4, $5, $6, $7
             WHERE NOT EXISTS (SELECT 1 FROM conversations.removed_inboxes WHERE inbox_id = $1)
            ON CONFLICT DO NOTHING
            "#,
            inbox as InboxId,
            workspace.as_uuid(),
            Channel::Email.as_str(),
            account.address.to_lowercase(),
            account.display_name,
            granted.connection as ConnectionId,
            granted.granted_by.as_uuid(),
        )
        .execute(&mut **tx)
        .await?;
        // The member is added to the inbox with this id, whether it was just
        // created or already there (a replay, an incremental consent). An
        // inbox for the same address under another id is left alone: its
        // batches wait for an inbox that is never created, which shows up as
        // a pending `apply_changes`.
        let added = sqlx::query!(
            r#"
            INSERT INTO conversations.inbox_members (workspace_id, inbox_id, user_id, role)
            SELECT $1, i.id, $3, $4 FROM conversations.inboxes i
             WHERE i.id = $2 AND i.detached_at IS NULL
            ON CONFLICT DO NOTHING
            "#,
            workspace.as_uuid(),
            inbox as InboxId,
            granted.granted_by.as_uuid(),
            InboxRole::Manager.as_str(),
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        tracing::info!(%inbox, added_member = added > 0, "inbox for a messages grant");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::tenancy::{UserId, WorkspaceId};
    use sqlx::PgPool;

    use crate::{
        events::{CapabilityGranted, MessagesChanged},
        model::{ConnectionId, InboxId},
        testkit::Kit,
    };

    async fn grant(
        kit: &Kit,
        workspace: WorkspaceId,
        inbox: InboxId,
        sam: UserId,
        capability: &str,
    ) {
        let ws = workspace.to_string();
        let account = kit
            .state
            .blobs
            .put(
                &ws,
                serde_json::to_vec(&serde_json::json!({
                    "version": 1, "provider": "google", "address": "Sam@Example.com",
                    "address_verified": true, "display_name": "Sam Rivers"
                }))
                .unwrap(),
                "application/json",
            )
            .await
            .unwrap();
        let mut tx = kit.state.db.begin().await.unwrap();
        kit.state
            .events
            .publish(
                &mut tx,
                &CapabilityGranted {
                    workspace: ws,
                    connection: ConnectionId::new(),
                    capability: capability.to_owned(),
                    granted_by: sam,
                    inbox_id: Some(inbox),
                    account,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        kit.run_jobs().await;
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_messages_grant_creates_the_inbox_and_its_batches_flow_in(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let workspace = WorkspaceId::new();
        let (token, sam) = kit.member("sam", workspace);
        let inbox = InboxId::new();
        grant(&kit, workspace, inbox, sam, "messages").await;
        // A replay, or an incremental consent, changes nothing.
        grant(&kit, workspace, inbox, sam, "messages").await;

        let inboxes = kit
            .get(&token, &format!("/api/v1/w/{workspace}/inboxes"))
            .await;
        let listed = inboxes.json["inboxes"].as_array().unwrap();
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(listed[0]["id"], inbox.to_string());
        assert_eq!(listed[0]["address"], "sam@example.com");
        assert_eq!(listed[0]["display_name"], "Sam Rivers");
        assert_eq!(listed[0]["role"], "manager");

        // The connector's batch, announced by `connectors.messages_changed`.
        let batch = kit.batch(workspace, inbox, "en").await;
        let ws = workspace.to_string();
        let blob = kit
            .state
            .blobs
            .put(
                &ws,
                serde_json::to_vec(&batch).unwrap(),
                crate::ingest::BATCH_CONTENT_TYPE,
            )
            .await
            .unwrap();
        let mut tx = kit.state.db.begin().await.unwrap();
        kit.state
            .events
            .publish(
                &mut tx,
                &MessagesChanged {
                    workspace: ws,
                    inbox_id: inbox,
                    batch: blob,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        kit.run_jobs().await;
        assert_eq!(kit.threads(inbox).await.len(), 4);
        let list = kit
            .get(&token, &format!("/api/v1/w/{workspace}/conversations"))
            .await;
        assert_eq!(list.json["conversations"].as_array().unwrap().len(), 3);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn other_capabilities_create_no_inbox(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let workspace = WorkspaceId::new();
        let (_, sam) = kit.member("sam", workspace);
        grant(&kit, workspace, InboxId::new(), sam, "calendar").await;
        let inboxes: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.inboxes")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(inboxes, 0);
    }
}
