//! `GET /api/v1/w/{workspace_id}/streams` — the built-in streams with how
//! many conversations each holds in the inbox, and how many are unread.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::access::visible_inboxes,
    model::{InboxId, Stream},
    scopes::ConversationsRead,
};

/// Which inbox.
#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StreamsQuery {
    /// Only this inbox; every inbox you can read when absent.
    pub inbox: Option<InboxId>,
}

/// A stream and what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StreamView {
    /// The stream.
    pub stream: Stream,
    /// Conversations in it that are still in the inbox.
    pub conversations: u32,
    /// Of those, the ones with an unread message.
    pub unread: u32,
}

/// Every built-in stream, in display order (Primary first).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StreamList {
    /// The streams, each listed even when empty.
    pub streams: Vec<StreamView>,
}

/// List the streams and their counts.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/streams",
    operation_id = "conversations.list_streams",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.conversation"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), StreamsQuery),
    responses(
        (status = OK, description = "The streams.", body = StreamList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such workspace or inbox, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Query(query): extract::Query<StreamsQuery>,
) -> Result<Json<StreamList>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let mut inboxes = visible_inboxes(&mut tx, access).await?;
    if let Some(inbox) = query.inbox {
        if !inboxes.contains(&inbox) {
            return Err(Error::NotFound);
        }
        inboxes = vec![inbox];
    }
    let rows = sqlx::query!(
        r#"
        SELECT stream, count(*)::int AS "conversations!",
               count(*) FILTER (WHERE unread_count > 0)::int AS "unread!"
          FROM conversations.conversations
         WHERE inbox_id = ANY($1) AND archived_at IS NULL AND NOT trashed
         GROUP BY stream
        "#,
        &inboxes as &[InboxId],
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let count = |n: i32| u32::try_from(n).unwrap_or_default();
    let streams = Stream::ALL
        .into_iter()
        .map(|stream| {
            let row = rows.iter().find(|row| row.stream == stream.as_str());
            StreamView {
                stream,
                conversations: row.map_or(0, |r| count(r.conversations)),
                unread: row.map_or(0, |r| count(r.unread)),
            }
        })
        .collect();
    Ok(Json(StreamList { streams }))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::{model::InboxId, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn counts_each_stream_in_order_even_when_empty(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/streams");
        let response = kit.get(&sam, &base).await;
        assert_eq!(response.status, StatusCode::OK, "{:?}", response.json);
        let streams = response.json["streams"].as_array().unwrap();
        let names: Vec<&str> = streams
            .iter()
            .map(|s| s["stream"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["primary", "updates", "receipts", "newsletters"]);
        let total: u64 = streams
            .iter()
            .map(|s| s["conversations"].as_u64().unwrap())
            .sum();
        assert_eq!(total, 3);
        assert_eq!(streams[2]["conversations"], 0);
        assert_eq!(streams[0]["unread"], 1);

        let filtered = kit.get(&sam, &format!("{base}?inbox={inbox}")).await;
        assert_eq!(filtered.json, response.json);
        let unknown = format!("{base}?inbox={}", InboxId::new());
        assert_eq!(kit.get(&sam, &unknown).await.status, StatusCode::NOT_FOUND);

        let (ana, _) = kit.member("ana", workspace);
        let theirs = kit.get(&ana, &base).await;
        assert_eq!(theirs.json["streams"][0]["conversations"], 0);
    }
}
