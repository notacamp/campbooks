//! `GET /api/v1/w/{workspace_id}/inboxes` — the inboxes the caller sees.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    db::BeginFor as _,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::resources;
use crate::events::INBOX_RECORD;
use crate::{
    domain::access::visible_inboxes,
    model::{Channel, InboxId, InboxRole},
    scopes::ConversationsRead,
};
use campbooks_kernel::authz::Action;

/// An inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct InboxView {
    /// The inbox.
    pub id: InboxId,
    /// Its channel.
    pub channel: Channel,
    /// Its address.
    #[schema(example = "office@example.com")]
    pub address: String,
    /// Its display name.
    pub display_name: Option<String>,
    /// The caller's access to it; `None` for a client token.
    pub role: Option<InboxRole>,
    /// Unread inbound messages in conversations still in the inbox.
    pub unread_count: u32,
    /// Kept as read-only history after its connection was removed: no
    /// longer synced, nothing can be written or sent from it.
    pub read_only: bool,
}

/// The inboxes, by address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct InboxList {
    /// The inboxes.
    pub inboxes: Vec<InboxView>,
}

/// List the inboxes you can read.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/inboxes",
    operation_id = "conversations.list_inboxes",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.inbox"]).access("conversations.inbox", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The inboxes.", body = InboxList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace, or you are not a member.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
) -> Result<Json<InboxList>> {
    let access = &workspace.access;
    resources::authorize_type(&state, access, Action::Read, INBOX_RECORD)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let visible = visible_inboxes(&mut tx, access).await?;
    let rows = sqlx::query!(
        r#"
        SELECT i.id AS "id: InboxId", i.channel, i.address, i.display_name,
               i.detached_at IS NOT NULL AS "read_only!",
               (SELECT m.role FROM conversations.inbox_members m
                 WHERE m.inbox_id = i.id AND m.user_id = $2) AS role,
               (SELECT coalesce(sum(c.unread_count), 0)::int FROM conversations.conversations c
                 WHERE c.inbox_id = i.id AND c.archived_at IS NULL AND NOT c.trashed)
                 AS "unread_count!"
          FROM conversations.inboxes i
         WHERE i.id = ANY($1)
         ORDER BY i.address
        "#,
        &visible as &[InboxId],
        access.user.map(|u| *u.as_uuid()),
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let inboxes = rows
        .into_iter()
        .map(|row| {
            Ok(InboxView {
                id: row.id,
                channel: row.channel.parse()?,
                address: row.address,
                display_name: row.display_name,
                role: row.role.as_deref().map(str::parse).transpose()?,
                unread_count: u32::try_from(row.unread_count).unwrap_or_default(),
                read_only: row.read_only,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Json(InboxList { inboxes }))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::tenancy::WorkspaceId;
    use sqlx::PgPool;

    use crate::{model::InboxRole, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn users_see_their_inboxes_and_bare_clients_none(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let (ana, ana_id) = kit.member("ana", workspace);
        kit.inbox(
            workspace,
            "Office@Example.com",
            &[(ana_id, InboxRole::Reader)],
        )
        .await;

        let uri = format!("/api/v1/w/{workspace}/inboxes");
        let mine = kit.get(&sam, &uri).await;
        assert_eq!(mine.status, StatusCode::OK);
        let inboxes = mine.json["inboxes"].as_array().unwrap();
        assert_eq!(inboxes.len(), 1);
        assert_eq!(inboxes[0]["address"], "sam@example.com");
        assert_eq!(inboxes[0]["role"], "manager");
        assert_eq!(inboxes[0]["unread_count"], 3);
        assert_eq!(
            kit.get(&ana, &uri).await.json["inboxes"][0]["address"],
            "office@example.com"
        );

        // A client acting for no member gets no inbox (ADR 0017): a client
        // a member registered arrives as that member instead.
        let client = kit.client(workspace, "conversations:read");
        assert_eq!(kit.get(&client, &uri).await.status, StatusCode::FORBIDDEN);

        let elsewhere = format!("/api/v1/w/{}/inboxes", WorkspaceId::new());
        assert_eq!(
            kit.get(&sam, &elsewhere).await.status,
            StatusCode::NOT_FOUND
        );
        assert_eq!(kit.get("", &uri).await.status, StatusCode::UNAUTHORIZED);
    }
}
