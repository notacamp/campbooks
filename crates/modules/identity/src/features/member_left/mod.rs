//! `identity.revoke_departed_consents` ← `workspaces.member_left`: the apps
//! a departing member allowed in that workspace lose their access (the
//! consents and every refresh token), and the workspace clients they
//! registered are removed (their tokens act for that member, and are
//! refused from the moment the member is gone). Their access tokens already
//! stop at the next membership check.
//!
//! The event alone is no proof that the member left: any transaction
//! working in the workspace could write it (campbooks-cloud#211), and
//! revoking an app's refresh tokens reaches past the workspace. Before
//! acting, the handler asks the workspaces module (the membership port), in
//! the delivery's transaction, whether they are a member there; while they
//! are, nothing happens (someone removed and added back before this ran
//! keeps the apps they had allowed).

use campbooks_kernel::{
    Event, EventConsumer, JobContext, JobError,
    tenancy::{UserId, WorkspaceId},
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    domain::consents,
    events::{ConsentRevoked, MemberLeft},
};

/// Revokes a departed member's consents in that workspace.
#[derive(Debug, Clone, Copy, Default)]
pub struct RevokeDepartedConsents;

impl EventConsumer for RevokeDepartedConsents {
    type Event = MemberLeft;
    const NAME: &'static str = "identity.revoke_departed_consents";
    const QUEUE: &'static str = "events";

    async fn handle(&self, ctx: &mut JobContext, event: Event<MemberLeft>) -> Result<(), JobError> {
        let workspace: Uuid = event
            .payload
            .workspace
            .parse()
            .map_err(|_| JobError::fatal("member_left without a workspace id"))?;
        let now = Utc::now();
        let state = ctx.state().clone();
        let tx = ctx.tx().await?;
        let member = state
            .membership
            .role_held(
                &mut **tx,
                WorkspaceId::from_uuid(workspace),
                UserId::from_uuid(event.payload.user),
            )
            .await
            .map_err(|e| JobError::retry(e.to_string()))?;
        if member.is_some() {
            tracing::warn!("member_left names someone who is still a member; not acted on");
            return Ok(());
        }
        let revoked = consents::revoke_in_workspace(
            tx,
            workspace,
            Some(event.payload.user),
            "member_left",
            now,
        )
        .await
        .map_err(|e| JobError::retry(e.to_string()))?;
        for consent in revoked {
            publish(ctx, consent, "member_left").await?;
        }
        if let Some(subject) = event.payload.subject.as_deref() {
            remove_their_clients(ctx, workspace, subject, event.occurred_at, now).await?;
        }
        Ok(())
    }
}

/// Remove the workspace clients `subject` registered in `workspace`, with
/// everything they were given.
async fn remove_their_clients(
    ctx: &mut JobContext,
    workspace: Uuid,
    subject: &str,
    before: chrono::DateTime<Utc>,
    now: chrono::DateTime<Utc>,
) -> Result<(), JobError> {
    let state = ctx.state().clone();
    let tx = ctx.tx().await?;
    let clients: Vec<String> = sqlx::query_scalar(
        "DELETE FROM identity.clients
          WHERE kind = 'workspace' AND workspace_id = $1 AND created_by::text = $2
            AND created_at <= $3
          RETURNING client_id",
    )
    .bind(workspace)
    .bind(subject)
    .bind(before)
    .fetch_all(&mut **tx)
    .await?;
    let mut revoked = Vec::new();
    for client in clients {
        let ended =
            crate::domain::workspace_clients::revoke_everything(tx, &client, "member_left", now)
                .await
                .map_err(|e| JobError::retry(e.to_string()))?;
        state
            .events
            .publish(
                tx,
                &crate::events::ClientRemoved {
                    workspace: workspace.to_string(),
                    client: client.clone(),
                },
            )
            .await
            .map_err(|e| JobError::retry(e.to_string()))?;
        revoked.extend(ended.into_iter().map(|(account, bound)| consents::Revoked {
            account_id: account,
            client_id: client.clone(),
            bound_workspace_id: bound,
        }));
    }
    for consent in revoked {
        publish(ctx, consent, "member_left").await?;
    }
    Ok(())
}

/// Announce that `consent` ended for `reason`.
pub(crate) async fn publish(
    ctx: &mut JobContext,
    consent: consents::Revoked,
    reason: &str,
) -> Result<(), JobError> {
    let state = ctx.state().clone();
    let tx = ctx.tx().await?;
    state
        .events
        .publish(
            tx,
            &ConsentRevoked {
                account: consent.account_id,
                client: consent.client_id,
                workspace: consent.bound_workspace_id.map(|w| w.to_string()),
                reason: reason.to_owned(),
            },
        )
        .await
        .map_err(|e| JobError::retry(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::db::BeginFor as _;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::testkit::{HOME, Kit, MEMBER_IN_HOME};

    async fn client(db: &PgPool, id: &str, created_by: Uuid) {
        sqlx::query(
            "INSERT INTO identity.clients
                 (client_id, kind, workspace_id, name, redirect_uris, grant_types, auth_method,
                  secret_hash, allowed_scopes, created_by, created_at)
             VALUES ($1, 'workspace', $2, 'Sync', '{}', '{client_credentials}',
                     'client_secret_basic', '\\x00'::bytea, '{conversations:read}', $3, now())",
        )
        .bind(id)
        .bind(HOME)
        .bind(created_by)
        .execute(db)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_clients_a_departing_member_registered_go_with_them(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (leaving, staying) = (Uuid::now_v7(), Uuid::now_v7());
        client(&db, "cb_theirs", leaving).await;
        client(&db, "cb_others", staying).await;
        let home = campbooks_kernel::tenancy::WorkspaceId::from_uuid(HOME);
        let mut tx = kit.state.db.begin_for(home).await.unwrap();
        kit.state
            .events
            .publish(
                &mut tx,
                &crate::events::MemberLeft {
                    workspace: HOME.to_string(),
                    user: MEMBER_IN_HOME,
                    subject: Some(leaving.to_string()),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        kit.run_jobs().await;
        let left: Vec<String> =
            sqlx::query_scalar("SELECT client_id FROM identity.clients WHERE kind = 'workspace'")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(left, ["cb_others"]);
        let removed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM kernel.events WHERE name = 'identity.client_removed'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(removed, 1);
    }
}
