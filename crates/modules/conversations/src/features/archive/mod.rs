//! `POST …/conversations/{conversation_id}/archive` and `/unarchive` — take a
//! conversation out of the inbox, or put it back.

use axum::{Extension, Json, extract::State};
use campbooks_kernel::{
    AppState, Operation, Problem, Result, Risk, extract,
    tenancy::{Workspace, WorkspaceId},
};

use crate::{
    domain::paths::ConversationPath,
    domain::{
        conversation::ConversationSummary,
        flags::{self, Flag},
        needs::NeedSettings,
    },
    model::ConversationId,
    scopes::ConversationsWrite,
};

/// Archive a conversation.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/archive",
    operation_id = "conversations.archive",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.unarchive")
        .automation()
        .assistant()
        .records(["conversations.conversation"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn archive(
    State(state): State<AppState>,
    Extension(needs): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    flags::apply(&state, &workspace.access, id, Flag::Archive, Some(&needs))
        .await
        .map(Json)
}

/// Move a conversation back to the inbox.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/conversations/{conversation_id}/unarchive",
    operation_id = "conversations.unarchive",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.archive")
        .automation()
        .assistant()
        .records(["conversations.conversation"]).access("conversations.conversation", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("conversation_id" = ConversationId, Path, description = "The conversation."),
    ),
    responses(
        (status = OK, description = "The conversation afterwards.", body = ConversationSummary),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such conversation, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn unarchive(
    State(state): State<AppState>,
    Extension(needs): Extension<NeedSettings>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<ConversationPath>,
) -> Result<Json<ConversationSummary>> {
    let id = path.conversation_id;
    flags::apply(&state, &workspace.access, id, Flag::Unarchive, Some(&needs))
        .await
        .map(Json)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::testing;
    use serde_json::json;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    const ARCHIVED: &str = "conversations.conversation_archived";
    const UNARCHIVED: &str = "conversations.conversation_unarchived";

    #[sqlx::test(migrations = "../../../migrations")]
    async fn archiving_tracks_the_conversation_as_it_is_afterwards(db: PgPool) {
        let kit = Kit::new(db).await.tracking();
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        let base = format!("/api/v1/w/{workspace}/conversations/{id}");

        kit.post(&sam, &format!("{base}/archive"), None).await;
        // Already archived: nothing changes, nothing more is tracked.
        kit.post(&sam, &format!("{base}/archive"), None).await;

        let tracked = testing::tracked_events(&kit.db, ARCHIVED).await;
        assert_eq!(tracked.len(), 1);
        let event = &tracked[0];
        assert_eq!(event.version, 1);
        assert_eq!(event.workspace_id, Some(*workspace.as_uuid()));
        let envelope = &event.envelope;
        assert_eq!(envelope["name"], ARCHIVED);
        assert_eq!(envelope["actor"], json!({"type": "user", "id": "sam"}));
        assert_eq!(envelope["on_behalf_of"], serde_json::Value::Null);
        assert_eq!(envelope["operation"], "conversations.archive");
        assert_eq!(envelope["workspace_id"], workspace.to_string());
        let correlation = envelope["correlation"].as_str().unwrap();
        assert_eq!(correlation.len(), 32, "{correlation}");
        assert_eq!(
            envelope["plan_version"],
            campbooks_kernel::tracking::PLAN_VERSION
        );
        let conversation = &event.body["conversation"];
        assert_eq!(conversation["id"], id);
        assert_eq!(conversation["inbox_id"], inbox.to_string());
        assert_eq!(conversation["status"], "archived");
        assert!(conversation["stream"].is_string());
        assert!(conversation["message_count"].as_i64().unwrap() >= 1);
        // Ids, enums, counts and times only: never what someone wrote.
        assert!(conversation.get("subject").is_none());
        assert!(conversation.get("snippet").is_none());

        kit.post(&sam, &format!("{base}/unarchive"), None).await;
        let back = testing::tracked_events(&kit.db, UNARCHIVED).await;
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].body["conversation"]["status"], "inbox");
        assert_eq!(back[0].envelope["operation"], "conversations.unarchive");
        assert_ne!(back[0].envelope["correlation"], envelope["correlation"]);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_assistant_archiving_tracks_it_on_the_members_behalf(db: PgPool) {
        let kit = Kit::new(db).await.tracking();
        let (workspace, _, _) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;

        kit.as_assistant(
            "sam",
            workspace,
            ("conversations.archive", json!({"conversation_id": id})),
            false,
        )
        .await
        .unwrap();

        let tracked = testing::tracked_events(&kit.db, ARCHIVED).await;
        assert_eq!(tracked.len(), 1);
        let envelope = &tracked[0].envelope;
        assert_eq!(envelope["actor"], json!({"type": "assistant"}));
        assert_eq!(
            envelope["on_behalf_of"],
            json!({"type": "user", "id": "sam"})
        );
        assert_eq!(envelope["operation"], "conversations.archive");
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn with_tracking_off_nothing_is_written(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        let archived = kit
            .post(
                &sam,
                &format!("/api/v1/w/{workspace}/conversations/{id}/archive"),
                None,
            )
            .await;
        assert_eq!(archived.json["archived"], true);
        assert!(testing::tracked_events(&kit.db, ARCHIVED).await.is_empty());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn archive_moves_it_out_of_the_inbox_and_unarchive_back(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        let base = format!("/api/v1/w/{workspace}/conversations/{id}");
        let list = format!("/api/v1/w/{workspace}/conversations");

        let archived = kit.post(&sam, &format!("{base}/archive"), None).await;
        assert_eq!(archived.json["archived"], true);
        assert_eq!(
            kit.get(&sam, &list).await.json["conversations"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let archive = kit.get(&sam, &format!("{list}?archived=true")).await;
        assert_eq!(archive.json["conversations"][0]["id"], id);
        let requested = kit.events("conversations.changes_requested").await;
        assert_eq!(requested[0]["items"].as_array().unwrap().len(), 2);

        let back = kit.post(&sam, &format!("{base}/unarchive"), None).await;
        assert_eq!(back.json["archived"], false);
        assert_eq!(
            kit.get(&sam, &list).await.json["conversations"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn readers_see_but_cannot_change(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, _) = kit.sams_inbox().await;
        let (ana, ana_id) = kit.member("ana", workspace);
        sqlx::query(
            "INSERT INTO conversations.inbox_members (workspace_id, inbox_id, user_id, role)
             VALUES ($1, $2, $3, 'reader')",
        )
        .bind(workspace.as_uuid())
        .bind(inbox)
        .bind(ana_id.as_uuid())
        .execute(&db)
        .await
        .unwrap();
        let id = kit.conversation_of("en-1").await;
        let base = format!("/api/v1/w/{workspace}/conversations/{id}");
        assert_eq!(kit.get(&ana, &base).await.status, StatusCode::OK);
        for (action, body) in [
            ("archive", None),
            ("read", None),
            ("stream", Some(serde_json::json!({"stream": "updates"}))),
            ("labels", Some(serde_json::json!({"name": "Mine"}))),
        ] {
            let denied = kit.post(&ana, &format!("{base}/{action}"), body).await;
            assert_eq!(denied.status, StatusCode::FORBIDDEN, "{action}");
            assert_eq!(denied.json["code"], "inbox_read_only", "{action}");
        }
        let unchanged = kit.get(&ana, &base).await;
        assert_eq!(unchanged.json["conversation"]["archived"], false);
    }
}
