//! The kernel's grant-intake port (ADR 0014): a mailbox grant the identity
//! module obtained (at sign-up, or through its front door for a connect or
//! a reconnect) becomes a connection through the one seam,
//! [`create_or_refresh_connection`]. The port also tells the identity module
//! which connection a reconnect is for, and refuses anyone but its grantor.

use campbooks_kernel::{
    AppState, Error,
    auth::{Principal, PrincipalKind},
    connections::{
        AdoptFuture, GrantIntake, GrantIntakePort, ProviderGrant as Grant, ReconnectTarget,
        TargetFuture,
    },
    db::BeginFor as _,
    tenancy::{Access, WorkspaceId},
};
use secrecy::ExposeSecret as _;

use super::{
    connections,
    grants::{ProviderGrant, create_or_refresh_connection},
    model::{Capability, ConnectionId, Provider},
};
use crate::{MESSAGES, deps::Deps, google};

/// Takes sign-in grants into connections.
#[derive(Debug, Clone)]
pub struct Intake {
    deps: Deps,
}

/// The port the server registers with `AppState::with_grant_intake`.
pub fn grant_intake(deps: Deps) -> GrantIntakePort {
    std::sync::Arc::new(Intake { deps })
}

impl GrantIntake for Intake {
    fn mailbox_scopes(&self, provider: &str) -> Vec<String> {
        match (provider, &self.deps.google) {
            ("google", Some(_)) => google::capability_scope(Capability::Messages)
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            _ => Vec::new(),
        }
    }

    fn adopt<'a>(&'a self, state: &'a AppState, grant: Grant) -> AdoptFuture<'a> {
        Box::pin(async move {
            let provider = match grant.provider.as_str() {
                "google" if self.deps.google.is_some() => Provider::Google,
                _ => return Err(Error::Conflict(MESSAGES.reason("provider_not_configured"))),
            };
            let capabilities: Vec<Capability> = google::granted_capabilities(&grant.scopes)
                .into_iter()
                .filter(|c| google::capability_scope(*c).is_some())
                .collect();
            if capabilities.is_empty() {
                return Err(Error::Denied(MESSAGES.reason("consent_incomplete")));
            }
            let access = member(state, &grant.subject, grant.workspace).await?;
            let user = access.user.ok_or(Error::NotFound)?;
            let view = create_or_refresh_connection(
                state,
                &self.deps,
                &ProviderGrant {
                    workspace: grant.workspace,
                    user,
                    provider,
                    account_subject: grant.account_subject,
                    address: grant.address.trim().to_lowercase(),
                    address_verified: grant.address_verified,
                    display_name: grant.display_name,
                    capabilities,
                    tokens: google::TokenGrant {
                        access_token: grant.access_token.expose_secret().to_owned(),
                        expires_in: grant.expires_in.and_then(|s| i64::try_from(s).ok()),
                        refresh_token: grant
                            .refresh_token
                            .as_ref()
                            .map(|t| t.expose_secret().to_owned()),
                        scope: Some(grant.scopes.join(" ")),
                    },
                    extending: grant
                        .extending
                        .as_deref()
                        .map(|id| id.parse::<ConnectionId>().map_err(|_| Error::NotFound))
                        .transpose()?,
                },
            )
            .await?;
            tracing::info!(workspace = %grant.workspace, %user, connection = %view.id, "a sign-in grant became a connection");
            Ok(view.id.to_string())
        })
    }

    fn reconnect_target<'a>(
        &'a self,
        state: &'a AppState,
        subject: &'a str,
        workspace: WorkspaceId,
        connection: &'a str,
    ) -> TargetFuture<'a> {
        Box::pin(async move {
            let id: ConnectionId = connection.parse().map_err(|_| Error::NotFound)?;
            let access = member(state, subject, workspace).await?;
            let user = access.user.ok_or(Error::NotFound)?;
            let mut tx = state.db.begin_for(workspace).await?;
            let found = connections::load(&mut tx, id).await?;
            tx.commit().await?;
            // Only the grantor consents to their grant again.
            let connection = found
                .filter(|c| c.granted_by == user)
                .ok_or(Error::NotFound)?;
            Ok(ReconnectTarget {
                provider: connection.provider.as_str().to_owned(),
                address: connection.address,
            })
        })
    }
}

/// The membership of the person with token subject `subject` in
/// `workspace`; 404 when they are not a member.
async fn member(state: &AppState, subject: &str, workspace: WorkspaceId) -> Result<Access, Error> {
    let principal = Principal {
        subject: subject.to_owned(),
        kind: PrincipalKind::User,
        client_id: None,
        scopes: std::collections::BTreeSet::new(),
        workspace: None,
        email: None,
        email_verified: false,
        name: None,
        expires_at: None,
        auth_time: None,
    };
    state
        .membership
        .resolve(&state.db, &principal, workspace)
        .await?
        .ok_or(Error::NotFound)
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{
        Error,
        connections::ProviderGrant,
        tenancy::{Role, WorkspaceId},
    };
    use secrecy::SecretString;
    use sqlx::PgPool;

    use super::grant_intake;
    use crate::testkit::{ACCOUNT_SUB, Kit};

    fn grant(workspace: WorkspaceId, subject: &str, provider: &str) -> ProviderGrant {
        ProviderGrant {
            workspace,
            subject: subject.to_owned(),
            provider: provider.to_owned(),
            account_subject: ACCOUNT_SUB.to_owned(),
            address: "Sam@Example.com".to_owned(),
            address_verified: true,
            display_name: Some("Sam Rivera".to_owned()),
            scopes: vec![
                "openid".to_owned(),
                "email".to_owned(),
                crate::google::GMAIL_SCOPE.to_owned(),
            ],
            access_token: SecretString::from("fake-access"),
            refresh_token: Some(SecretString::from("fake-refresh")),
            expires_in: Some(3600),
            extending: None,
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_sign_in_grant_becomes_the_members_connection(db: PgPool) {
        let kit = Kit::new(db).await;
        let ws = WorkspaceId::new();
        let ana = kit.user("ana");
        let ana_id = kit.join(&ana, ws, Role::Owner);
        let intake = grant_intake(kit.deps.clone());
        assert_eq!(
            intake.mailbox_scopes("google"),
            [crate::google::GMAIL_SCOPE]
        );
        assert!(intake.mailbox_scopes("zoho").is_empty());

        let id = intake
            .adopt(&kit.state, grant(ws, "ana", "google"))
            .await
            .unwrap();
        let list = kit
            .get(&ana.token, &format!("/api/v1/w/{ws}/connections"))
            .await;
        let connection = &list.json["connections"][0];
        assert_eq!(connection["id"], id);
        assert_eq!(connection["address"], "sam@example.com");
        assert_eq!(connection["granted_by"], ana_id.to_string());
        assert_eq!(connection["capabilities"], serde_json::json!(["messages"]));
        assert_eq!(
            kit.events_of(ws).await,
            [
                "connectors.connection_added",
                "connectors.capability_granted"
            ]
        );

        // The same account again refreshes it.
        let again = intake
            .adopt(&kit.state, grant(ws, "ana", "google"))
            .await
            .unwrap();
        assert_eq!(again, id);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_stranger_or_an_unknown_provider_is_refused(db: PgPool) {
        let kit = Kit::new(db).await;
        let ws = WorkspaceId::new();
        let intake = grant_intake(kit.deps.clone());
        assert!(matches!(
            intake.adopt(&kit.state, grant(ws, "bea", "google")).await,
            Err(Error::NotFound)
        ));
        let ana = kit.user("ana");
        kit.join(&ana, ws, Role::Owner);
        assert!(matches!(
            intake.adopt(&kit.state, grant(ws, "ana", "zoho")).await,
            Err(Error::Conflict(_))
        ));
        let mut no_mail = grant(ws, "ana", "google");
        no_mail.scopes.pop();
        assert!(matches!(
            intake.adopt(&kit.state, no_mail).await,
            Err(Error::Denied(_))
        ));
    }
}
