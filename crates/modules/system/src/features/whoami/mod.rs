//! `system.whoami` — who does the API take the caller to be?
//!
//! The reference **action** (ADR 0008): declared once with
//! [`Action`], it becomes `POST /api/v1/actions/system.whoami` with its
//! schemas and scope in the OpenAPI document, and an entry in the
//! declarations catalogue. Read-only, so no idempotency key or undo.

use campbooks_kernel::{
    Action, ActionContext, Result,
    auth::{PrincipalKind, Scope},
    declare::ActionDecl,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::scopes::SystemRead;

/// The action, for the module's declarations.
pub fn action() -> ActionDecl {
    Action::<WhoamiInput, WhoamiOutput>::new("system.whoami")
        .describe(
            "Who the API takes the caller to be: the subject, client, scopes and workspace \
             of the access token the request carries.",
        )
        .scope::<SystemRead>()
        .handler(whoami)
}

/// No input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WhoamiInput {}

/// The caller, as the access token describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WhoamiOutput {
    /// The token's subject (or, for a client acting on its own behalf, its
    /// client id).
    #[schema(example = "8d6f0d5e-2c55-4f7e-9a8e-1b8c1f3f2a10")]
    pub subject: String,
    /// A person signed in through a client, or a client on its own behalf.
    pub kind: CallerKind,
    /// The client the token was issued to, when it names one.
    #[schema(example = "campbooks-web")]
    pub client_id: Option<String>,
    /// The scopes the token grants, sorted.
    #[schema(example = json!(["openid", "system:read"]))]
    pub scopes: Vec<String>,
    /// The workspace a client is bound to; none for people.
    pub workspace: Option<String>,
}

/// Who a token was issued to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallerKind {
    /// A person, signed in through a client.
    User,
    /// A client acting on its own behalf.
    Client,
}

async fn whoami(ctx: ActionContext, _: WhoamiInput) -> Result<WhoamiOutput> {
    let principal = ctx.principal();
    Ok(WhoamiOutput {
        subject: principal.subject.clone(),
        kind: match principal.kind {
            PrincipalKind::User => CallerKind::User,
            PrincipalKind::Client => CallerKind::Client,
        },
        client_id: principal.client_id.clone(),
        scopes: principal.scopes.iter().map(Scope::to_string).collect(),
        workspace: principal.workspace.clone(),
    })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::testing::{self, TestIdp};
    use serde_json::json;

    #[tokio::test]
    async fn reports_the_token_it_was_called_with() {
        let idp = TestIdp::start().await;
        let app = testing::app(crate::module(), idp.state(testing::unreachable_pool()));
        let token = idp.token(&idp.claims("system:read"));

        let response = testing::call_action(&app, "system.whoami", Some(&token), &json!({})).await;

        assert_eq!(response.status, StatusCode::OK, "{}", response.json);
        assert_eq!(
            response.json,
            json!({"result": {
                "subject": "test-user",
                "kind": "user",
                "client_id": "campbooks-web",
                "scopes": ["openid", "system:read"],
                "workspace": null,
            }})
        );
    }

    #[tokio::test]
    async fn requires_system_read() {
        let idp = TestIdp::start().await;
        let app = testing::app(crate::module(), idp.state(testing::unreachable_pool()));

        let anonymous = testing::call_action(&app, "system.whoami", None, &json!({})).await;
        assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);

        let token = idp.token(&idp.claims("other:read"));
        let unscoped = testing::call_action(&app, "system.whoami", Some(&token), &json!({})).await;
        assert_eq!(unscoped.status, StatusCode::FORBIDDEN);
        assert_eq!(unscoped.json["detail"], "insufficient_scope");
    }
}
