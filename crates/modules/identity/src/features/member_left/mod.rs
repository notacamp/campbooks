//! `identity.revoke_departed_consents` ← `workspaces.member_left`: the apps
//! a departing member allowed in that workspace lose their access (the
//! consents and every refresh token). Their access tokens already stop at
//! the next membership check.

use campbooks_kernel::{Event, EventConsumer, JobContext, JobError};
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
        let tx = ctx.tx().await?;
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
        Ok(())
    }
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
