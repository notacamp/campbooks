//! The kernel's consent-door port (ADR 0014): `identity` is the one front
//! door for every provider consent, including the mailbox consents the
//! connectors module starts (`connectors.start_connection`). A request
//! becomes `identity.start_link` with purpose `connect`, or `reconnect`
//! when it names a connection.

use std::sync::Arc;

use campbooks_kernel::{
    AppState,
    connections::{ConsentDoor, ConsentDoorPort, ConsentFuture, ConsentRequest, StartedConsent},
};

use crate::{
    deps::Deps,
    features::start_link::{self, LinkPurpose, StartLink},
};

/// Starts mailbox consents for other modules.
#[derive(Debug, Clone)]
pub struct FrontDoor {
    deps: Deps,
}

/// The port the server registers with `AppState::with_consent_door`.
pub fn consent_door(deps: Deps) -> ConsentDoorPort {
    Arc::new(FrontDoor { deps })
}

impl ConsentDoor for FrontDoor {
    fn start<'a>(&'a self, state: &'a AppState, request: ConsentRequest) -> ConsentFuture<'a> {
        Box::pin(async move {
            let purpose = if request.connection.is_some() {
                LinkPurpose::Reconnect
            } else {
                LinkPurpose::Connect
            };
            let started = start_link::execute_asking(
                state,
                &self.deps,
                &request.principal,
                StartLink {
                    purpose,
                    provider: Some(request.provider),
                    workspace_id: Some(request.workspace),
                    connection_id: request.connection,
                    return_to: None,
                    // As before the choice existed: the account becomes a
                    // way in. The Mailboxes screen asks first.
                    sign_in: None,
                },
                &request.capabilities,
            )
            .await?;
            Ok(StartedConsent {
                url: started.url,
                expires_at: started.expires_at,
            })
        })
    }
}
