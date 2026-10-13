//! `conversations.forget_member` ← `workspaces.member_left`: a member who
//! leaves (or is removed) loses every inbox membership and their drafts are
//! deleted, and they stop owning the inboxes their connections fed (which
//! the connectors module removes, deleting or keeping their mail). Coming
//! back to the workspace never brings any of it back. Idempotent.

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    events::{DRAFT_RECORD, InboxChanged, MemberLeft},
    model::InboxId,
};

/// Takes a departed member out of the workspace's inboxes.
pub struct ForgetMember;

impl EventHandler for ForgetMember {
    type Event = MemberLeft;
    const NAME: &'static str = "conversations.forget_member";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<MemberLeft>) -> Result<(), JobError> {
        let left = event.payload;
        let workspace: WorkspaceId = left
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("member_left names an invalid workspace"))?;
        let ws = workspace.to_string();
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let drafts = sqlx::query_scalar!(
            "DELETE FROM conversations.drafts WHERE author = $1 RETURNING id",
            left.user.as_uuid()
        )
        .fetch_all(&mut **tx)
        .await?;
        state
            .blobs
            .detach_owners(tx, &ws, DRAFT_RECORD, &drafts)
            .await?;
        let mut inboxes = sqlx::query_scalar!(
            r#"DELETE FROM conversations.inbox_members WHERE user_id = $1
               RETURNING inbox_id AS "inbox: InboxId""#,
            left.user.as_uuid()
        )
        .fetch_all(&mut **tx)
        .await?;
        inboxes.extend(
            sqlx::query_scalar!(
                r#"UPDATE conversations.inboxes SET owner = NULL WHERE owner = $1
                   RETURNING id AS "id: InboxId""#,
                left.user.as_uuid()
            )
            .fetch_all(&mut **tx)
            .await?,
        );
        inboxes.sort_unstable();
        inboxes.dedup();
        for inbox in &inboxes {
            state
                .events
                .publish(
                    tx,
                    &InboxChanged {
                        workspace: ws.clone(),
                        inbox: *inbox,
                    },
                )
                .await?;
        }
        tracing::info!(
            inboxes = inboxes.len(),
            drafts = drafts.len(),
            "a departed member's inbox access and drafts removed"
        );
        Ok(())
    }
}
