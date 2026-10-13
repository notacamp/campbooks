//! The kernel's invitation port: whether an address has a pending
//! invitation to any workspace. The identity module asks before it creates
//! an account on an invite-only instance (ADR 0014).
//!
//! Invitations are workspace-owned (row-level security keyed on the
//! workspace). This lookup crosses workspaces, so it reads through its own
//! policy: rows whose address equals `campbooks.invitee_email`, set for one
//! transaction only.

use std::sync::Arc;

use campbooks_kernel::signup::{Invitations, InvitationsPort, InvitedFuture};
use sqlx::PgPool;

/// The port the server registers.
pub fn invitations() -> InvitationsPort {
    Arc::new(PendingInvitations)
}

/// Pending invitations as this module records them.
#[derive(Debug, Clone, Copy, Default)]
pub struct PendingInvitations;

impl Invitations for PendingInvitations {
    fn pending_for<'a>(&'a self, db: &'a PgPool, email: &'a str) -> InvitedFuture<'a> {
        Box::pin(async move {
            let email = email.trim().to_lowercase();
            let mut tx = db.begin().await?;
            sqlx::query("SELECT set_config('campbooks.invitee_email', $1, true)")
                .bind(&email)
                .execute(&mut *tx)
                .await?;
            let invited: bool = sqlx::query_scalar(
                "SELECT EXISTS (
                     SELECT 1 FROM workspaces.invitations
                      WHERE email = $1 AND accepted_at IS NULL AND expires_at > now())",
            )
            .bind(&email)
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(invited)
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{
        tenancy::{UserId, WorkspaceId},
        testing,
    };
    use sqlx::PgPool;

    use super::*;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_address_is_invited_while_an_invitation_waits(db: PgPool) {
        let ws = WorkspaceId::new();
        let user = UserId::new();
        sqlx::query("INSERT INTO workspaces.users (id, idp_subject) VALUES ($1, 'ana')")
            .bind(user)
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO workspaces.workspaces (id, name, kind, accent)
             VALUES ($1, 'Shop', 'business', 'business')",
        )
        .bind(ws)
        .execute(&db)
        .await
        .unwrap();
        for (email, days) in [("bea@example.com", 1), ("old@example.com", -1)] {
            sqlx::query(
                "INSERT INTO workspaces.invitations
                     (id, workspace_id, email, role, token_hash, expires_at, invited_by)
                 VALUES (gen_random_uuid(), $1, $2, 'member', decode(md5($2), 'hex'),
                         now() + make_interval(days => $3), $4)",
            )
            .bind(ws)
            .bind(email)
            .bind(days)
            .bind(user)
            .execute(&db)
            .await
            .unwrap();
        }
        // As the server queries: the runtime role, no workspace set.
        let app = testing::app_role_pool(&db).await;
        let port = invitations();
        assert!(port.pending_for(&app, " Bea@Example.com").await.unwrap());
        assert!(
            !port.pending_for(&app, "old@example.com").await.unwrap(),
            "expired"
        );
        assert!(!port.pending_for(&app, "cid@example.com").await.unwrap());
    }
}
