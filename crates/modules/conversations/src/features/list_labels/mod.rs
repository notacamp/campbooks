//! `GET /api/v1/w/{workspace_id}/labels` — the labels of the inboxes the
//! caller can read: what a rule may put on a conversation or test for.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result,
    db::BeginFor as _,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    domain::access::visible_inboxes,
    model::{InboxId, LabelId},
    scopes::ConversationsRead,
};

/// A label of an inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct InboxLabel {
    /// The label.
    pub id: LabelId,
    /// Its inbox.
    pub inbox: InboxId,
    /// Its name.
    #[schema(example = "Clients")]
    pub name: String,
}

/// Labels, by inbox then name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LabelList {
    /// The labels.
    pub labels: Vec<InboxLabel>,
}

/// List the labels of the inboxes you can read.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/labels",
    operation_id = "conversations.list_labels",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.inbox"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace.")),
    responses(
        (status = OK, description = "The labels.", body = LabelList),
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
) -> Result<Json<LabelList>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    let labels = sqlx::query_as!(
        InboxLabel,
        r#"SELECT id AS "id: LabelId", inbox_id AS "inbox: InboxId", name
             FROM conversations.labels
            WHERE inbox_id = ANY($1)
            ORDER BY inbox_id, name"#,
        &inboxes as &[InboxId],
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(LabelList { labels }))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::{model::InboxRole, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn labels_of_readable_inboxes_only(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let (ana, ana_id) = kit.member("ana", workspace);
        kit.inbox(workspace, "board@example.com", &[(ana_id, InboxRole::Manager)])
            .await;
        sqlx::query(
            "INSERT INTO conversations.labels (id, workspace_id, inbox_id, name)
             SELECT gen_random_uuid(), workspace_id, id, 'Board'
               FROM conversations.inboxes WHERE address = 'board@example.com'",
        )
        .execute(&kit.db)
        .await
        .unwrap();
        let uri = format!("/api/v1/w/{workspace}/labels");

        let sams = kit.get(&sam, &uri).await;
        assert_eq!(sams.status, StatusCode::OK);
        let names: Vec<_> = sams.json["labels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|label| label["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["Clients"]);
        assert_eq!(sams.json["labels"][0]["inbox"], inbox.to_string());

        let anas = kit.get(&ana, &uri).await;
        assert_eq!(anas.json["labels"][0]["name"], "Board");
        assert_eq!(anas.json["labels"].as_array().unwrap().len(), 1);
    }
}
