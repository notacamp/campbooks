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
use crate::events::{MailChoice, MemberLeft};

/// Who is acting on a departure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Departure {
    /// The member leaves: the mail of the mailboxes they connected is
    /// deleted.
    Leave,
    /// A manager (with their role) removes the member, deleting or keeping
    /// their mail.
    RemovedBy(UserId, Role, MailChoice),
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
    if let Departure::RemovedBy(_, actor_role, _) = how
        && role == Role::Owner
        && actor_role != Role::Owner
    {
        return Err(Error::Forbidden);
    }
    keep_an_owner(&mut tx, workspace, role).await?;
    let subject = sqlx::query_scalar!(
        "SELECT idp_subject FROM workspaces.users WHERE id = $1",
        member as UserId,
    )
    .fetch_optional(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM workspaces.members WHERE workspace_id = $1 AND user_id = $2",
        workspace as WorkspaceId,
        member as UserId,
    )
    .execute(&mut *tx)
    .await?;
    let (actor, kind, removed_by, mail) = match how {
        Departure::Leave => (member, SecurityKind::MemberLeft, None, MailChoice::Delete),
        Departure::RemovedBy(actor, _, mail) => {
            (actor, SecurityKind::MemberRemoved, Some(actor), mail)
        }
    };
    security::record(
        &mut tx,
        Some(workspace),
        Some(actor),
        kind,
        serde_json::json!({ "member": member, "role": role, "mail": mail }),
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
                mail,
                subject,
            },
        )
        .await?;
    tx.commit().await?;
    Ok(())
}
