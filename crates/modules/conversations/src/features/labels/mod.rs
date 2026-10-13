//! `POST /api/v1/w/{workspace_id}/conversations/{conversation_id}/labels` and
//! `DELETE …/labels/{label_id}` — a member's own marks on a conversation.
//! Labels belong to the conversation's inbox; adding one by a new name
//! creates it.

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    tenancy::{Access, Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;

use crate::{
    domain::paths::{ConversationPath, LabelPath},
    domain::{
        access::lock_conversation,
        conversation::{Changed, LabelView, provider_message_ids, request},
    },
    events::{Change, Origin, RequestedChange},
    model::{ConversationId, InboxId, LabelId},
    scopes::ConversationsWrite,
};

/// Longest label name, in characters.
pub const MAX_NAME_CHARS: usize = 100;

/// The label to add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AddLabel {
    /// Its name; created in the conversation's inbox when new.
    #[schema(example = "Accountant")]
    pub name: String,
}

/// Put a label on a conversation.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/labels",
    operation_id = "conversations.add_label",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.remove_label")
        .automation()
        .assistant()
        .records(["conversations.conversation", "conversations.label"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    request_body = AddLabel,
    responses(
        (status = OK, description = "The label, now on the conversation.", body = LabelView),
        (status = BAD_REQUEST, description = "An empty or too long name.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn add(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Json(body): extract::Json<AddLabel>,
) -> Result<Json<LabelView>> {
    let id = path.conversation_id;
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("invalid_label_name")
                .arg("max", MAX_NAME_CHARS),
        ));
    }
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let inbox = lock_conversation(&mut tx, access, id).await?;
    let label = sqlx::query!(
        r#"
        WITH created AS (
            INSERT INTO conversations.labels (id, workspace_id, inbox_id, name)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (inbox_id, name) DO NOTHING
            RETURNING id, name, provider_key
        )
        SELECT id AS "id!: LabelId", name AS "name!", provider_key FROM created
        UNION ALL
        SELECT id, name, provider_key FROM conversations.labels WHERE inbox_id = $3 AND name = $4
        LIMIT 1
        "#,
        LabelId::new() as LabelId,
        access.workspace.as_uuid(),
        inbox as InboxId,
        name,
    )
    .fetch_one(&mut *tx)
    .await?;
    let linked = sqlx::query!(
        r#"INSERT INTO conversations.conversation_labels (workspace_id, conversation_id, label_id)
           VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"#,
        access.workspace.as_uuid(),
        id as ConversationId,
        label.id as LabelId,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if linked > 0 {
        let change = RequestedChange::AddLabel {
            label: label.id,
            provider_key: label.provider_key,
        };
        announce(&state, &mut tx, access, inbox, id, &change).await?;
    }
    tx.commit().await?;
    Ok(Json(LabelView {
        id: label.id,
        name: label.name,
    }))
}

/// Take a label off a conversation.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/labels/{label_id}",
    operation_id = "conversations.remove_label",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.add_label")
        .automation()
        .assistant()
        .records(["conversations.conversation", "conversations.label"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
        ("label_id" = LabelId, Path, description = "The label."),
    ),
    responses(
        (status = NO_CONTENT, description = "The label is off the conversation."),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or the label is not on it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn remove(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<LabelPath>,
) -> Result<StatusCode> {
    let (id, label) = (path.conversation_id, path.label_id);
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let inbox = lock_conversation(&mut tx, access, id).await?;
    let removed = sqlx::query_scalar!(
        r#"
        DELETE FROM conversations.conversation_labels cl
         USING conversations.labels l
         WHERE cl.conversation_id = $1 AND cl.label_id = $2 AND l.id = cl.label_id
        RETURNING l.provider_key
        "#,
        id as ConversationId,
        label as LabelId,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    let change = RequestedChange::RemoveLabel {
        label,
        provider_key: removed,
    };
    announce(&state, &mut tx, access, inbox, id, &change).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn announce(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    access: &Access,
    inbox: InboxId,
    id: ConversationId,
    change: &RequestedChange,
) -> Result<()> {
    let provider_message_ids = provider_message_ids(tx, id).await?;
    let changed = Changed {
        workspace: access.workspace.to_string(),
        inbox,
        conversation: id,
        origin: Origin::Member,
        changes: vec![Change::Labels],
        provider_message_ids,
    };
    request(&state.events, tx, &changed, change).await?;
    changed.publish(&state.events, tx).await
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn labels_are_added_by_name_and_removed(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        let base = format!("/api/v1/w/{workspace}/conversations/{id}/labels");

        let added = kit
            .post(&sam, &base, Some(json!({"name": " Accountant "})))
            .await;
        assert_eq!(added.status, StatusCode::OK, "{:?}", added.json);
        assert_eq!(added.json["name"], "Accountant");
        let label = added.json["id"].as_str().unwrap().to_owned();
        // Adding it again changes nothing.
        let again = kit
            .post(&sam, &base, Some(json!({"name": "Accountant"})))
            .await;
        assert_eq!(again.json["id"], label);
        let requested = kit.events("conversations.changes_requested").await;
        assert_eq!(requested.len(), 1);
        assert_eq!(requested[0]["items"][0]["change"]["kind"], "add_label");
        assert!(
            requested[0]["items"][0]["change"]
                .get("provider_key")
                .is_none()
        );

        // The provider's own label carries its key.
        let clients = kit
            .get(&sam, &format!("/api/v1/w/{workspace}/conversations/{id}"))
            .await
            .json["conversation"]["labels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["name"] == "Clients")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let removed = kit.delete(&sam, &format!("{base}/{clients}")).await;
        assert_eq!(removed.status, StatusCode::NO_CONTENT);
        let requested = kit.events("conversations.changes_requested").await;
        assert_eq!(
            requested[1]["items"][0]["change"],
            json!({"kind": "remove_label", "label": clients, "provider_key": "Label_7"})
        );
        assert_eq!(
            kit.delete(&sam, &format!("{base}/{clients}")).await.status,
            StatusCode::NOT_FOUND
        );

        let empty = kit.post(&sam, &base, Some(json!({"name": "  "}))).await;
        assert_eq!(empty.status, StatusCode::BAD_REQUEST);
    }
}
