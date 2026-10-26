//! A member who leaves loses the pages shared with them (`pages.forget_member`).
//!
//! Their grants go. Their private pages stay as they are: nobody else can
//! read them, and data is never deleted without being asked to (the
//! workspace's purge removes them in the end). Pages they made in the
//! workspace place stay too. If they are a member again by the time this
//! runs, nothing goes: what they were given since is theirs.

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    domain::{PageId, notion},
    events::MemberLeft,
    features::share::lock_member,
};

/// `workspaces.member_left` → delete the grants made to the member.
pub struct ForgetMember;

impl EventHandler for ForgetMember {
    type Event = MemberLeft;
    const NAME: &'static str = "pages.forget_member";
    const QUEUE: &'static str = "default";

    async fn handle(ctx: &mut JobContext, event: Event<MemberLeft>) -> Result<(), JobError> {
        let left = event.payload;
        let workspace: WorkspaceId = left
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("member_left names an invalid workspace"))?;
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        // After any share call that checked the membership before it ended.
        lock_member(tx, workspace, left.user).await?;
        let back = state.membership.role_held(tx, workspace, left.user).await?;
        if back.is_some() {
            return Ok(());
        }
        let removed = sqlx::query("DELETE FROM pages.grants WHERE member_id = $1")
            .bind(left.user)
            .execute(&mut **tx)
            .await?
            .rows_affected();
        tracing::info!(grants = removed, "a departed member's page grants removed");
        // The pages kept in step with their Notion account stop: nothing is
        // deleted there, and the pages stay.
        let linked: Vec<PageId> = sqlx::query_scalar(
            "SELECT page_id FROM pages.external_links
              WHERE created_by = $1 AND state <> 'stopped' ORDER BY page_id",
        )
        .bind(left.user)
        .fetch_all(&mut **tx)
        .await?;
        for page in linked {
            notion::stop(&state, tx, workspace, page, notion::StopReason::MemberLeft).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
