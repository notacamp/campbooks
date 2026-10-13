//! A member leaving a workspace, by their own choice or removed by a
//! manager: shared by `remove_member` and `leave_workspace`.

use campbooks_kernel::{
    AppState, Error, Result,
    db::BeginFor as _,
    tenancy::{Role, UserId, WorkspaceId},
};

use super::{
    access::{keep_an_owner, lock_workspace, role_of},
    security::{self, SecurityKind},
};
use crate::events::MemberLeft;

/// Who is acting on a departure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Departure {
    /// The member leaves.
    Leave,
    /// A manager (with their role) removes the member.
    RemovedBy(UserId, Role),
}

/// Take `member` out of `workspace`. Refuses to remove the last owner (409
/// `last_owner`); an admin may not remove an owner (403).
pub async fn depart(
    state: &AppState,
    workspace: WorkspaceId,
    member: UserId,
    how: Departure,
) -> Result<()> {
    let mut tx = state.db.begin_for(workspace).await?;
    lock_workspace(&mut tx, workspace).await?;
    let role = role_of(&mut tx, workspace, member)
        .await?
        .ok_or(Error::NotFound)?;
    if let Departure::RemovedBy(_, actor_role) = how
        && role == Role::Owner
        && actor_role != Role::Owner
    {
        return Err(Error::Forbidden);
    }
    keep_an_owner(&mut tx, workspace, role).await?;
    sqlx::query!(
        "DELETE FROM workspaces.members WHERE workspace_id = $1 AND user_id = $2",
        workspace as WorkspaceId,
        member as UserId,
    )
    .execute(&mut *tx)
    .await?;
    let (actor, kind, removed_by) = match how {
        Departure::Leave => (member, SecurityKind::MemberLeft, None),
        Departure::RemovedBy(actor, _) => (actor, SecurityKind::MemberRemoved, Some(actor)),
    };
    security::record(
        &mut tx,
        Some(workspace),
        Some(actor),
        kind,
        serde_json::json!({ "member": member, "role": role }),
    )
    .await?;
    state
        .events
        .publish(
            &mut tx,
            &MemberLeft {
                workspace: workspace.to_string(),
                user: member,
                removed_by,
            },
        )
        .await?;
    tx.commit().await?;
    Ok(())
}
