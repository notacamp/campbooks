//! The kernel's grant-intake port (ADR 0014): a mailbox grant the identity
//! module obtained (at sign-up, or through its front door for a connect or
//! a reconnect) becomes a connection through the one seam,
//! [`create_or_refresh_connection`]. The port also tells the identity module
//! which connection a reconnect is for, and refuses anyone but its grantor.

use campbooks_kernel::{
    AppState, Error,
    auth::{Principal, PrincipalKind},
    connections::{
        AdoptFuture, ConnectionOfFuture, GrantIntake, GrantIntakePort, ProviderGrant as Grant,
        ReconnectTarget, TargetFuture,
    },
    db::BeginFor as _,
    tenancy::{Access, UserId, WorkspaceId},
};
use secrecy::ExposeSecret as _;

use super::{
    grants::{Mailbox, ProviderGrant, Secret, create_or_refresh_connection},
    model::{Capability, ConnectionId, Health, Provider},
};
use crate::{MESSAGES, deps::Deps, google, provider::TokenGrant, zoho};

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
        match provider {
            "google" if self.deps.google.is_some() => {
                google::capability_scope(Capability::Messages)
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect()
            }
            "zoho" if self.deps.zoho.is_some() => zoho::MAIL_SCOPES.map(ToOwned::to_owned).to_vec(),
            _ => Vec::new(),
        }
    }

    fn adopt<'a>(&'a self, state: &'a AppState, grant: Grant) -> AdoptFuture<'a> {
        Box::pin(async move {
            let access = member(state, &grant.subject, grant.workspace).await?;
            let user = access.user.ok_or(Error::NotFound)?;
            let address = grant.address.trim().to_lowercase();
            let (provider, capabilities, provider_server, mailboxes) = match grant.provider.as_str()
            {
                "google" if self.deps.google.is_some() => {
                    let capabilities: Vec<Capability> = google::granted_capabilities(&grant.scopes)
                        .into_iter()
                        .filter(|c| google::capability_scope(*c).is_some())
                        .collect();
                    let mailbox = Mailbox {
                        key: String::new(),
                        address: address.clone(),
                        address_verified: grant.address_verified,
                        display_name: grant.display_name.clone(),
                    };
                    (Provider::Google, capabilities, None, vec![mailbox])
                }
                "zoho" if self.deps.zoho.is_some() => {
                    let granted = zoho::MAIL_SCOPES
                        .iter()
                        .all(|s| grant.scopes.iter().any(|g| g == s));
                    if !granted {
                        return Err(Error::Denied(MESSAGES.reason("consent_incomplete")));
                    }
                    let (server, mailboxes) = self.zoho_mailboxes(&grant, &address).await?;
                    (
                        Provider::Zoho,
                        vec![Capability::Messages],
                        Some(server),
                        mailboxes,
                    )
                }
                _ => return Err(Error::Conflict(MESSAGES.reason("provider_not_configured"))),
            };
            if capabilities.is_empty() {
                return Err(Error::Denied(MESSAGES.reason("consent_incomplete")));
            }
            let view = create_or_refresh_connection(
                state,
                &self.deps,
                &ProviderGrant {
                    workspace: grant.workspace,
                    user,
                    provider,
                    account_subject: grant.account_subject,
                    address,
                    address_verified: grant.address_verified,
                    display_name: grant.display_name,
                    capabilities,
                    secret: Secret::Tokens(TokenGrant {
                        access_token: grant.access_token.expose_secret().to_owned(),
                        expires_in: grant.expires_in.and_then(|s| i64::try_from(s).ok()),
                        refresh_token: grant
                            .refresh_token
                            .as_ref()
                            .map(|t| t.expose_secret().to_owned()),
                        scope: Some(grant.scopes.join(" ")),
                    }),
                    imap: None,
                    grantor_subject: Some(grant.subject.clone()),
                    extending: grant
                        .extending
                        .as_deref()
                        .map(|id| id.parse::<ConnectionId>().map_err(|_| Error::NotFound))
                        .transpose()?,
                    provider_server,
                    mailboxes,
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
            let found = sqlx::query!(
                r#"SELECT provider, address, account_subject, health
                     FROM connectors.connections
                    WHERE id = $1 AND granted_by = $2"#,
                id as ConnectionId,
                user as UserId,
            )
            .fetch_optional(&mut *tx)
            .await?;
            tx.commit().await?;
            // Only the grantor consents to their grant again.
            let found = found.ok_or(Error::NotFound)?;
            Ok(ReconnectTarget {
                provider: found.provider,
                address: found.address,
                account_subject: found.account_subject,
                needs_reauth: found.health == Health::NeedsReauth.as_str(),
            })
        })
    }

    fn connection_of<'a>(
        &'a self,
        state: &'a AppState,
        subject: &'a str,
        workspace: WorkspaceId,
        provider: &'a str,
        account_subject: &'a str,
    ) -> ConnectionOfFuture<'a> {
        Box::pin(async move {
            // Someone who left the workspace grants nothing there.
            let user = match member(state, subject, workspace).await {
                Ok(access) => access.user,
                Err(Error::NotFound) => None,
                Err(error) => return Err(error),
            };
            let Some(user) = user else {
                return Ok(None);
            };
            let mut tx = state.db.begin_for(workspace).await?;
            let found = sqlx::query_scalar!(
                r#"SELECT id AS "id: ConnectionId"
                     FROM connectors.connections
                    WHERE workspace_id = $1 AND provider = $2 AND account_subject = $3
                      AND granted_by = $4"#,
                workspace as WorkspaceId,
                provider,
                account_subject,
                user as UserId,
            )
            .fetch_optional(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(found.map(|id| id.to_string()))
        })
    }
}

impl Intake {
    /// The data centre of a Zoho grant (its validated issuer) and the
    /// login's mail accounts, read with the grant's access token.
    async fn zoho_mailboxes(
        &self,
        grant: &Grant,
        login: &str,
    ) -> Result<(String, Vec<Mailbox>), Error> {
        let zoho = self
            .deps
            .zoho
            .as_ref()
            .ok_or_else(|| Error::Conflict(MESSAGES.reason("provider_not_configured")))?;
        let centre = zoho
            .centre(&grant.issuer)
            .ok_or_else(|| Error::Conflict(MESSAGES.reason("unknown_data_centre")))?;
        let accounts = zoho
            .mail(&centre, grant.access_token.expose_secret(), "")
            .accounts()
            .await
            .map_err(|err| {
                tracing::warn!(error = %err, "could not list a Zoho login's mail accounts");
                Error::Unavailable(MESSAGES.reason("provider_unavailable"))
            })?;
        let mut mailboxes: Vec<Mailbox> = Vec::new();
        for account in accounts {
            let Some(address) = account.address() else {
                continue;
            };
            if mailboxes.iter().any(|m| m.address == address) {
                continue;
            }
            mailboxes.push(Mailbox {
                // A Zoho-hosted mailbox is Zoho's to vouch for; an outside
                // address fetched into Zoho only when it is the login's own.
                address_verified: account.is_zoho_mailbox()
                    || (address == login && grant.address_verified),
                display_name: account.name().or_else(|| grant.display_name.clone()),
                key: account.account_id,
                address,
            });
        }
        if mailboxes.is_empty() {
            return Err(Error::Conflict(MESSAGES.reason("no_mailbox")));
        }
        Ok((centre.issuer, mailboxes))
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
        issued_at: None,
        auth_time: None,
        sign_in: None,
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
            issuer: "https://accounts.google.com".to_owned(),
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
        assert_eq!(intake.mailbox_scopes("zoho"), crate::zoho::MAIL_SCOPES);
        assert!(intake.mailbox_scopes("microsoft").is_empty());

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
            intake
                .adopt(&kit.state, grant(ws, "ana", "microsoft"))
                .await,
            Err(Error::Conflict(_))
        ));
        // A Zoho grant without its mail scopes connects nothing.
        assert!(matches!(
            intake.adopt(&kit.state, grant(ws, "ana", "zoho")).await,
            Err(Error::Denied(_))
        ));
        let mut no_mail = grant(ws, "ana", "google");
        no_mail.scopes.pop();
        assert!(matches!(
            intake.adopt(&kit.state, no_mail).await,
            Err(Error::Denied(_))
        ));
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn identity_learns_whether_a_grantors_connection_exists(db: PgPool) {
        let kit = Kit::new(db).await;
        let ws = WorkspaceId::new();
        let ana = kit.user("ana");
        kit.join(&ana, ws, Role::Owner);
        let bea = kit.user("bea");
        kit.join(&bea, ws, Role::Member);
        let intake = grant_intake(kit.deps.clone());
        let of = |subject: &'static str, workspace: WorkspaceId, account: &'static str| {
            let intake = intake.clone();
            let state = kit.state.clone();
            async move {
                intake
                    .connection_of(&state, subject, workspace, "google", account)
                    .await
                    .unwrap()
            }
        };
        assert_eq!(of("ana", ws, ACCOUNT_SUB).await, None);

        let id = intake
            .adopt(&kit.state, grant(ws, "ana", "google"))
            .await
            .unwrap();
        assert_eq!(of("ana", ws, ACCOUNT_SUB).await, Some(id));
        // Not for another member, another account, another workspace, or
        // someone who is no member there.
        assert_eq!(of("bea", ws, ACCOUNT_SUB).await, None);
        assert_eq!(of("ana", ws, "someone-else").await, None);
        assert_eq!(of("ana", WorkspaceId::new(), ACCOUNT_SUB).await, None);
        assert_eq!(of("cid", ws, ACCOUNT_SUB).await, None);
    }
}
