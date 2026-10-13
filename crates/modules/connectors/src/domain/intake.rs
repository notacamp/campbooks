//! The kernel's grant-intake port (ADR 0014): a mailbox grant the identity
//! module obtained at sign-in becomes a connection through the same seam as
//! this module's own consent flow ([`create_or_refresh_connection`]).

use campbooks_kernel::{
    AppState, Error,
    auth::{Principal, PrincipalKind},
    connections::{AdoptFuture, GrantIntake, GrantIntakePort, ProviderGrant as Grant},
};
use secrecy::ExposeSecret as _;

use super::{
    grants::{ProviderGrant, create_or_refresh_connection},
    model::{Capability, Provider},
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
            let principal = Principal {
                subject: grant.subject.clone(),
                kind: PrincipalKind::User,
                client_id: None,
                scopes: std::collections::BTreeSet::new(),
                workspace: None,
                email: None,
                email_verified: false,
                name: None,
                expires_at: None,
            };
            let access = state
                .membership
                .resolve(&state.db, &principal, grant.workspace)
                .await?
                .ok_or(Error::NotFound)?;
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
                    extending: None,
                },
            )
            .await?;
            tracing::info!(workspace = %grant.workspace, %user, connection = %view.id, "a sign-in grant became a connection");
            Ok(view.id.to_string())
        })
    }
}
