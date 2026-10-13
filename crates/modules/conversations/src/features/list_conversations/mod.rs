//! `GET /api/v1/w/{workspace_id}/conversations` — conversations by stream,
//! newest first, a page at a time.

use axum::{Json, extract::State};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        access::visible_inboxes,
        conversation::{ConversationSummary, SummaryRow},
    },
    model::{ConversationId, InboxId, Stream},
    scopes::ConversationsRead,
};

/// Page size when none is asked for.
pub const DEFAULT_LIMIT: u32 = 50;
/// Largest page.
pub const MAX_LIMIT: u32 = 100;

/// Which conversations, and which page.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
    /// Only this stream; every stream when absent.
    pub stream: Option<Stream>,
    /// Only this inbox; every inbox you can read when absent.
    pub inbox: Option<InboxId>,
    /// `true` for archived conversations instead of those in the inbox.
    #[serde(default)]
    pub archived: bool,
    /// `next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// Page size, 1 to 100 (default 50).
    pub limit: Option<u32>,
}

/// A page of conversations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConversationList {
    /// Newest first.
    pub conversations: Vec<ConversationSummary>,
    /// Pass it as `cursor` for the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// List conversations, newest first.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations",
    operation_id = "conversations.list",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.conversation"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), ListQuery),
    responses(
        (status = OK, description = "A page of conversations.", body = ConversationList),
        (status = BAD_REQUEST, description = "A malformed cursor or limit.", body = Problem,
            content_type = "application/problem+json"),
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
    extract::Query(query): extract::Query<ListQuery>,
) -> Result<Json<ConversationList>> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(Error::BadRequest(
            crate::MESSAGES
                .reason("invalid_limit")
                .arg("max", MAX_LIMIT),
        ));
    }
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let mut inboxes = visible_inboxes(&mut tx, access).await?;
    if let Some(inbox) = query.inbox {
        if !inboxes.contains(&inbox) {
            return Err(Error::NotFound);
        }
        inboxes = vec![inbox];
    }
    let rows = fetch_page(&mut tx, &inboxes, &query, after, limit).await?;
    tx.commit().await?;
    let mut conversations = rows
        .into_iter()
        .map(ConversationSummary::try_from)
        .collect::<Result<Vec<_>>>()?;
    let next_cursor = if conversations.len() > limit as usize {
        conversations.truncate(limit as usize);
        conversations
            .last()
            .map(|last| encode_cursor(last.last_message_at, last.id))
    } else {
        None
    };
    Ok(Json(ConversationList {
        conversations,
        next_cursor,
    }))
}

/// One page (plus one row, to tell whether there is a next page).
async fn fetch_page(
    conn: &mut PgConnection,
    inboxes: &[InboxId],
    query: &ListQuery,
    after: Option<(DateTime<Utc>, ConversationId)>,
    limit: u32,
) -> Result<Vec<SummaryRow>> {
    let stream = query.stream.map(Stream::as_str);
    let (after_at, after_id) = after.unzip();
    let fetch = i64::from(limit) + 1;
    let rows = if query.archived {
        sqlx::query_as!(
            SummaryRow,
            r#"
            SELECT c.id AS "id: ConversationId", c.inbox_id AS "inbox_id: InboxId", c.subject,
                   c.stream, c.stream_source, c.last_message_at, c.message_count, c.unread_count,
                   c.starred, c.archived_at IS NOT NULL AS "archived!",
                   m.sender AS "sender?", m.snippet AS "snippet?",
                   coalesce((SELECT jsonb_agg(jsonb_build_object('id', l.id, 'name', l.name)
                                              ORDER BY l.name)
                               FROM conversations.conversation_labels cl
                               JOIN conversations.labels l ON l.id = cl.label_id
                              WHERE cl.conversation_id = c.id), '[]') AS "labels!"
              FROM conversations.conversations c
              LEFT JOIN LATERAL (
                  SELECT sender, snippet FROM conversations.messages
                   WHERE conversation_id = c.id ORDER BY sent_at DESC, id DESC LIMIT 1
              ) m ON true
             WHERE c.inbox_id = ANY($1) AND c.archived_at IS NOT NULL AND NOT c.trashed
               AND ($2::text IS NULL OR c.stream = $2)
               AND ($3::timestamptz IS NULL OR (c.last_message_at, c.id) < ($3, $4))
             ORDER BY c.last_message_at DESC, c.id DESC
             LIMIT $5
            "#,
            inboxes as &[InboxId],
            stream,
            after_at,
            after_id as Option<ConversationId>,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
    } else {
        sqlx::query_as!(
            SummaryRow,
            r#"
            SELECT c.id AS "id: ConversationId", c.inbox_id AS "inbox_id: InboxId", c.subject,
                   c.stream, c.stream_source, c.last_message_at, c.message_count, c.unread_count,
                   c.starred, c.archived_at IS NOT NULL AS "archived!",
                   m.sender AS "sender?", m.snippet AS "snippet?",
                   coalesce((SELECT jsonb_agg(jsonb_build_object('id', l.id, 'name', l.name)
                                              ORDER BY l.name)
                               FROM conversations.conversation_labels cl
                               JOIN conversations.labels l ON l.id = cl.label_id
                              WHERE cl.conversation_id = c.id), '[]') AS "labels!"
              FROM conversations.conversations c
              LEFT JOIN LATERAL (
                  SELECT sender, snippet FROM conversations.messages
                   WHERE conversation_id = c.id ORDER BY sent_at DESC, id DESC LIMIT 1
              ) m ON true
             WHERE c.inbox_id = ANY($1) AND c.archived_at IS NULL AND NOT c.trashed
               AND ($2::text IS NULL OR c.stream = $2)
               AND ($3::timestamptz IS NULL OR (c.last_message_at, c.id) < ($3, $4))
             ORDER BY c.last_message_at DESC, c.id DESC
             LIMIT $5
            "#,
            inboxes as &[InboxId],
            stream,
            after_at,
            after_id as Option<ConversationId>,
            fetch,
        )
        .fetch_all(&mut *conn)
        .await?
    };
    Ok(rows)
}

/// `(last_message_at, id)` of the last row of a page, opaque.
fn encode_cursor(at: DateTime<Utc>, id: ConversationId) -> String {
    URL_SAFE_NO_PAD.encode(format!(
        "{}|{id}",
        at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
    ))
}

fn decode_cursor(cursor: &str) -> Result<(DateTime<Utc>, ConversationId)> {
    let bad = || Error::BadRequest(crate::MESSAGES.reason("invalid_cursor"));
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| bad())?;
    let text = String::from_utf8(bytes).map_err(|_| bad())?;
    let (at, id) = text.split_once('|').ok_or_else(bad)?;
    let at = DateTime::parse_from_rfc3339(at).map_err(|_| bad())?;
    Ok((at.with_timezone(&Utc), id.parse().map_err(|_| bad())?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip_and_reject_junk() {
        let at = Utc::now();
        let id = ConversationId::new();
        assert_eq!(decode_cursor(&encode_cursor(at, id)).unwrap(), (at, id));
        assert!(decode_cursor("not-a-cursor").is_err());
        assert!(decode_cursor(&URL_SAFE_NO_PAD.encode("2026-01-01|nope")).is_err());
    }
}

#[cfg(test)]
mod http_tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::{model::InboxId, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn lists_by_stream_newest_first_a_page_at_a_time(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");

        let all = kit.get(&sam, &base).await;
        assert_eq!(all.status, StatusCode::OK, "{:?}", all.json);
        let subjects: Vec<&str> = all.json["conversations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["subject"].as_str().unwrap())
            .collect();
        assert_eq!(
            subjects,
            [
                "New sign-in to your account from Linux",
                "Issue 142: five tools for calmer mornings",
                "Q4 plan: can you review by Friday?"
            ]
        );
        let q4 = &all.json["conversations"][2];
        assert_eq!(q4["message_count"], 2);
        assert_eq!(q4["latest_from"]["address"], "sam@example.com");
        assert_eq!(q4["labels"][0]["name"], "Clients");
        assert!(all.json["next_cursor"].is_null());

        let primary = kit.get(&sam, &format!("{base}?stream=primary")).await;
        assert_eq!(primary.json["conversations"].as_array().unwrap().len(), 1);
        let news = kit
            .get(&sam, &format!("{base}?stream=newsletters&inbox={inbox}"))
            .await;
        assert_eq!(news.json["conversations"][0]["stream_source"], "rules");

        let first = kit.get(&sam, &format!("{base}?limit=2")).await;
        assert_eq!(first.json["conversations"].as_array().unwrap().len(), 2);
        let cursor = first.json["next_cursor"].as_str().unwrap().to_owned();
        let second = kit
            .get(&sam, &format!("{base}?limit=2&cursor={cursor}"))
            .await;
        assert_eq!(
            second.json["conversations"][0]["subject"],
            "Q4 plan: can you review by Friday?"
        );
        assert!(second.json["next_cursor"].is_null());

        let archived = kit.get(&sam, &format!("{base}?archived=true")).await;
        assert!(
            archived.json["conversations"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let junk = kit.get(&sam, &format!("{base}?cursor=junk")).await;
        assert_eq!(junk.status, StatusCode::BAD_REQUEST);
        assert_eq!(junk.json["code"], "invalid_cursor");
        assert_eq!(
            kit.get(&sam, &format!("{base}?limit=0")).await.status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            kit.get(&sam, &format!("{base}?stream=spam")).await.status,
            StatusCode::BAD_REQUEST
        );
        let unknown = format!("{base}?inbox={}", InboxId::new());
        assert_eq!(kit.get(&sam, &unknown).await.status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn other_members_do_not_see_an_inbox_they_are_not_in(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, _) = kit.sams_inbox().await;
        let (ana, _) = kit.member("ana", workspace);
        let base = format!("/api/v1/w/{workspace}/conversations");
        let response = kit.get(&ana, &base).await;
        assert_eq!(response.status, StatusCode::OK);
        assert!(
            response.json["conversations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let filtered = kit.get(&ana, &format!("{base}?inbox={inbox}")).await;
        assert_eq!(filtered.status, StatusCode::NOT_FOUND);
    }
}
