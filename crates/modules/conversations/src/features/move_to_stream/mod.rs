//! `POST /api/v1/w/{workspace_id}/conversations/{conversation_id}/stream` —
//! a member moves a conversation to another stream.
//!
//! The move sticks (later messages never re-sort the conversation) and is
//! recorded as a **correction**: the labelled example the stream question's
//! eval set is built from (campbooks-cloud#59), linked to the answers on
//! record. Moving back is the same operation.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use campbooks_ai::decide::signals::email::triage;

use crate::{
    domain::paths::ConversationPath,
    domain::{
        access::lock_conversation,
        conversation::{Changed, ConversationSummary, provider_message_ids, request, summary},
    },
    events::{Change, Origin, RequestedChange},
    model::{ConversationId, CorrectionId, DecisionId, MessageId, Stream},
    scopes::ConversationsWrite,
};

/// Where to move the conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveToStream {
    /// The stream.
    pub stream: Stream,
}

/// Move a conversation to another stream.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/stream",
    operation_id = "conversations.move_to_stream",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .automation()
        .assistant()
        .records(["conversations.conversation", "conversations.correction"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    request_body = MoveToStream,
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = BAD_REQUEST, description = "Not a stream.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn handler(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
    extract::Json(body): extract::Json<MoveToStream>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let inbox = lock_conversation(&state, &mut tx, access, id).await?;
    let current = sqlx::query!(
        "SELECT stream, stream_source FROM conversations.conversations WHERE id = $1",
        id as ConversationId
    )
    .fetch_one(&mut *tx)
    .await?;
    if current.stream != body.stream.as_str() {
        sqlx::query!(
            r#"UPDATE conversations.conversations
                  SET stream = $2, stream_source = 'member', updated_at = now()
                WHERE id = $1"#,
            id as ConversationId,
            body.stream.as_str(),
        )
        .execute(&mut *tx)
        .await?;
        let latest_inbound = sqlx::query_scalar!(
            r#"SELECT id AS "id: MessageId" FROM conversations.messages
                WHERE conversation_id = $1 AND direction = 'inbound'
                ORDER BY sent_at DESC, id DESC LIMIT 1"#,
            id as ConversationId
        )
        .fetch_optional(&mut *tx)
        .await?;
        let decisions = sqlx::query_scalar!(
            r#"SELECT id AS "id: DecisionId" FROM conversations.decisions
                WHERE conversation_id = $1 AND question = $2 ORDER BY created_at, id"#,
            id as ConversationId,
            triage::STREAM,
        )
        .fetch_all(&mut *tx)
        .await?;
        sqlx::query!(
            r#"
            INSERT INTO conversations.corrections
                (id, workspace_id, conversation_id, message_id, question, from_value, to_value,
                 overridden_source, decision_ids, corrected_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            CorrectionId::new() as CorrectionId,
            access.workspace.as_uuid(),
            id as ConversationId,
            latest_inbound as Option<MessageId>,
            triage::STREAM,
            current.stream,
            body.stream.as_str(),
            current.stream_source,
            &decisions as &[DecisionId],
            access.user.map(|u| *u.as_uuid()),
        )
        .execute(&mut *tx)
        .await?;
        let provider_message_ids = provider_message_ids(&mut tx, id).await?;
        let changed = Changed {
            workspace: access.workspace.to_string(),
            inbox,
            conversation: id,
            origin: Origin::Member,
            changes: vec![Change::Stream],
            provider_message_ids,
        };
        let move_to = RequestedChange::MoveToStream {
            stream: body.stream,
        };
        request(&state.events, &mut tx, &changed, &move_to).await?;
        changed.publish(&state.events, &mut tx).await?;
    }
    let summary = summary(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(summary))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_move_sticks_and_is_recorded_as_a_correction(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-3").await;
        let uri = format!("/api/v1/w/{workspace}/conversations/{id}/stream");

        let moved = kit
            .post(&sam, &uri, Some(json!({"stream": "primary"})))
            .await;
        assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
        assert_eq!(moved.json["stream"], "primary");
        assert_eq!(moved.json["stream_source"], "member");

        let (from, to, source, decisions, by): (String, String, String, i64, Option<String>) =
            sqlx::query_as(
                "SELECT from_value, to_value, overridden_source,
                        cardinality(decision_ids)::bigint, corrected_by::text
                   FROM conversations.corrections",
            )
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(
            (from.as_str(), to.as_str(), source.as_str(), decisions),
            ("newsletters", "primary", "rules", 1)
        );
        assert!(by.is_some());
        let requested = kit.events("conversations.changes_requested").await;
        assert_eq!(
            requested[0]["items"][0]["change"],
            json!({"kind": "move_to_stream", "stream": "primary"})
        );

        // The same move again records nothing new.
        kit.post(&sam, &uri, Some(json!({"stream": "primary"})))
            .await;
        let corrections: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.corrections")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(corrections, 1);

        // Later provider changes never re-sort it.
        kit.ingest(workspace, inbox, "followup").await;
        assert_eq!(
            kit.stream_of("en-3").await,
            ("primary".to_owned(), "member".to_owned())
        );

        let bad = kit.post(&sam, &uri, Some(json!({"stream": "spam"}))).await;
        assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    }
}
