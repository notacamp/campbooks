//! `GET /api/v1/w/{workspace_id}/conversations` — conversations by stream,
//! pinned first (the provider's star), then newest first, a page at a time.

use axum::{Extension, Json, extract::State};
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

use crate::config::ActionSettings;
use crate::domain::resources;
use crate::events::CONVERSATION_RECORD;
use crate::{
    domain::{
        access::visible_inboxes,
        actions,
        conversation::{ConversationSummary, SummaryRow},
        needs, rules, suggestions,
    },
    model::{ConversationId, InboxId, Stream},
    scopes::ConversationsRead,
};
use campbooks_kernel::authz::Action;

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
    /// Ignored with `sent`.
    #[serde(default)]
    pub archived: bool,
    /// `true` for the conversations you have written in, in any stream,
    /// archived or not (trash and spam left out), newest message of yours
    /// first. Nothing is pinned first. `stream` and `inbox` still narrow it.
    /// A cursor from a list that was not `sent` is refused.
    #[serde(default)]
    pub sent: bool,
    /// `next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// Page size, 1 to 100 (default 50).
    pub limit: Option<u32>,
    /// These conversations only (1 to 100 ids, comma-separated), whatever
    /// their stream or whether archived; those you cannot see are left
    /// out. Other filters and the cursor are ignored.
    pub ids: Option<String>,
}

/// A page of conversations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConversationList {
    /// Pinned first, then newest first (archived lists and `ids`: newest
    /// first; `sent`: your latest message first).
    pub conversations: Vec<ConversationSummary>,
    /// Pass it as `cursor` for the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// List conversations: pinned first, then newest first.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/conversations",
    operation_id = "conversations.list",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.conversation", "conversations.rule"]).access("conversations.conversation", campbooks_kernel::authz::Action::Read)))),
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
    Extension(settings): Extension<ActionSettings>,
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
    let ids = query.ids.as_deref().map(parse_ids).transpose()?;
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;
    if after.is_some_and(|cursor| cursor.sent != query.sent) {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_cursor")));
    }
    let access = &workspace.access;
    resources::authorize_type(&state, access, Action::Read, CONVERSATION_RECORD)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let mut inboxes = visible_inboxes(&mut tx, access).await?;
    if let Some(ids) = ids {
        let rows = fetch_ids(&mut tx, &inboxes, &ids).await?;
        let mut conversations = rows
            .into_iter()
            .map(ConversationSummary::try_from)
            .collect::<Result<Vec<_>>>()?;
        suggestions::attach(&mut tx, &mut conversations).await?;
        needs::attach(&mut tx, &mut conversations).await?;
        actions::attach(&mut tx, settings, &mut conversations).await?;
        rules::attach(&mut tx, access.user, &mut conversations).await?;
        tx.commit().await?;
        return Ok(Json(ConversationList {
            conversations,
            next_cursor: None,
        }));
    }
    if let Some(inbox) = query.inbox {
        if !inboxes.contains(&inbox) {
            return Err(Error::NotFound);
        }
        inboxes = vec![inbox];
    }
    let rows = fetch_page(&mut tx, &inboxes, &query, after, limit).await?;
    let mut conversations = rows
        .into_iter()
        .map(ConversationSummary::try_from)
        .collect::<Result<Vec<_>>>()?;
    suggestions::attach(&mut tx, &mut conversations).await?;
    needs::attach(&mut tx, &mut conversations).await?;
    actions::attach(&mut tx, settings, &mut conversations).await?;
    rules::attach(&mut tx, access.user, &mut conversations).await?;
    tx.commit().await?;
    let next_cursor = if conversations.len() > limit as usize {
        conversations.truncate(limit as usize);
        conversations.last().map(|last| {
            if query.sent {
                // Ordered by your latest message, with no pins.
                encode_sent_cursor(last.last_sent_at.unwrap_or(last.last_message_at), last.id)
            } else {
                // Archived lists ignore pins, so their cursor never carries one.
                encode_cursor(
                    last.starred && !query.archived,
                    last.last_message_at,
                    last.id,
                )
            }
        })
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
    after: Option<Cursor>,
    limit: u32,
) -> Result<Vec<SummaryRow>> {
    let stream = query.stream.map(Stream::as_str);
    let after_pinned = after.is_some_and(|cursor| cursor.pinned);
    let (after_at, after_id) = after.map(|cursor| (cursor.at, cursor.id)).unzip();
    let fetch = i64::from(limit) + 1;
    let rows = if query.sent {
        sqlx::query_as!(
            SummaryRow,
            r#"
            SELECT c.id AS "id: ConversationId", c.inbox_id AS "inbox_id: InboxId", c.subject,
                   c.stream, c.stream_source, c.last_message_at, c.message_count, c.unread_count,
                   c.starred, c.archived_at IS NOT NULL AS "archived!",
                   m.sender AS "sender?", m.snippet AS "snippet?",
                   coalesce(m.direction = 'outbound', false) AS "latest_from_you!",
                   coalesce((SELECT jsonb_agg(jsonb_build_object('id', l.id, 'name', l.name)
                                              ORDER BY l.name)
                               FROM conversations.conversation_labels cl
                               JOIN conversations.labels l ON l.id = cl.label_id
                              WHERE cl.conversation_id = c.id), '[]') AS "labels!",
                   c.last_sent_at,
                   coalesce(o.recipients -> 'to' -> 0, o.recipients -> 'cc' -> 0) AS "sent_to?"
              FROM conversations.conversations c
              LEFT JOIN LATERAL (
                  SELECT sender, snippet, direction FROM conversations.messages
                   WHERE conversation_id = c.id ORDER BY sent_at DESC, id DESC LIMIT 1
              ) m ON true
              LEFT JOIN LATERAL (
                  SELECT recipients FROM conversations.messages
                   WHERE conversation_id = c.id AND c.last_sent_at IS NOT NULL AND direction = 'outbound'
                     AND NOT in_trash AND NOT in_spam
                   ORDER BY sent_at DESC, id DESC LIMIT 1
              ) o ON true
             WHERE c.inbox_id = ANY($1) AND c.last_sent_at IS NOT NULL AND NOT c.trashed
               AND ($2::text IS NULL OR c.stream = $2)
               AND ($3::timestamptz IS NULL OR (c.last_sent_at, c.id) < ($3, $4))
             ORDER BY c.last_sent_at DESC, c.id DESC
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
    } else if query.archived {
        sqlx::query_as!(
            SummaryRow,
            r#"
            SELECT c.id AS "id: ConversationId", c.inbox_id AS "inbox_id: InboxId", c.subject,
                   c.stream, c.stream_source, c.last_message_at, c.message_count, c.unread_count,
                   c.starred, c.archived_at IS NOT NULL AS "archived!",
                   m.sender AS "sender?", m.snippet AS "snippet?",
                   coalesce(m.direction = 'outbound', false) AS "latest_from_you!",
                   coalesce((SELECT jsonb_agg(jsonb_build_object('id', l.id, 'name', l.name)
                                              ORDER BY l.name)
                               FROM conversations.conversation_labels cl
                               JOIN conversations.labels l ON l.id = cl.label_id
                              WHERE cl.conversation_id = c.id), '[]') AS "labels!",
                   c.last_sent_at,
                   coalesce(o.recipients -> 'to' -> 0, o.recipients -> 'cc' -> 0) AS "sent_to?"
              FROM conversations.conversations c
              LEFT JOIN LATERAL (
                  SELECT sender, snippet, direction FROM conversations.messages
                   WHERE conversation_id = c.id ORDER BY sent_at DESC, id DESC LIMIT 1
              ) m ON true
              LEFT JOIN LATERAL (
                  SELECT recipients FROM conversations.messages
                   WHERE conversation_id = c.id AND c.last_sent_at IS NOT NULL AND direction = 'outbound'
                     AND NOT in_trash AND NOT in_spam
                   ORDER BY sent_at DESC, id DESC LIMIT 1
              ) o ON true
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
                   coalesce(m.direction = 'outbound', false) AS "latest_from_you!",
                   coalesce((SELECT jsonb_agg(jsonb_build_object('id', l.id, 'name', l.name)
                                              ORDER BY l.name)
                               FROM conversations.conversation_labels cl
                               JOIN conversations.labels l ON l.id = cl.label_id
                              WHERE cl.conversation_id = c.id), '[]') AS "labels!",
                   c.last_sent_at,
                   coalesce(o.recipients -> 'to' -> 0, o.recipients -> 'cc' -> 0) AS "sent_to?"
              FROM conversations.conversations c
              LEFT JOIN LATERAL (
                  SELECT sender, snippet, direction FROM conversations.messages
                   WHERE conversation_id = c.id ORDER BY sent_at DESC, id DESC LIMIT 1
              ) m ON true
              LEFT JOIN LATERAL (
                  SELECT recipients FROM conversations.messages
                   WHERE conversation_id = c.id AND c.last_sent_at IS NOT NULL AND direction = 'outbound'
                     AND NOT in_trash AND NOT in_spam
                   ORDER BY sent_at DESC, id DESC LIMIT 1
              ) o ON true
             WHERE c.inbox_id = ANY($1) AND c.archived_at IS NULL AND NOT c.trashed
               AND ($2::text IS NULL OR c.stream = $2)
               AND ($3::timestamptz IS NULL
                    OR (c.starred, c.last_message_at, c.id) < ($6::bool, $3, $4))
             ORDER BY c.starred DESC, c.last_message_at DESC, c.id DESC
             LIMIT $5
            "#,
            inboxes as &[InboxId],
            stream,
            after_at,
            after_id as Option<ConversationId>,
            fetch,
            after_pinned,
        )
        .fetch_all(&mut *conn)
        .await?
    };
    Ok(rows)
}

/// `ids=a,b,c`: 1 to [`MAX_LIMIT`] conversation ids.
fn parse_ids(ids: &str) -> Result<Vec<ConversationId>> {
    let bad = || Error::BadRequest(crate::MESSAGES.reason("invalid_ids").arg("max", MAX_LIMIT));
    let ids = ids
        .split(',')
        .map(|id| id.trim().parse::<ConversationId>().map_err(|_| bad()))
        .collect::<Result<Vec<_>>>()?;
    if ids.is_empty() || ids.len() > MAX_LIMIT as usize {
        return Err(bad());
    }
    Ok(ids)
}

/// The conversations `ids` that are in `inboxes`, newest first.
async fn fetch_ids(
    conn: &mut PgConnection,
    inboxes: &[InboxId],
    ids: &[ConversationId],
) -> Result<Vec<SummaryRow>> {
    Ok(sqlx::query_as!(
        SummaryRow,
        r#"
        SELECT c.id AS "id: ConversationId", c.inbox_id AS "inbox_id: InboxId", c.subject,
               c.stream, c.stream_source, c.last_message_at, c.message_count, c.unread_count,
               c.starred, c.archived_at IS NOT NULL AS "archived!",
               m.sender AS "sender?", m.snippet AS "snippet?",
               coalesce(m.direction = 'outbound', false) AS "latest_from_you!",
               coalesce((SELECT jsonb_agg(jsonb_build_object('id', l.id, 'name', l.name)
                                          ORDER BY l.name)
                           FROM conversations.conversation_labels cl
                           JOIN conversations.labels l ON l.id = cl.label_id
                          WHERE cl.conversation_id = c.id), '[]') AS "labels!",
               c.last_sent_at,
               coalesce(o.recipients -> 'to' -> 0, o.recipients -> 'cc' -> 0) AS "sent_to?"
          FROM conversations.conversations c
          LEFT JOIN LATERAL (
              SELECT sender, snippet, direction FROM conversations.messages
               WHERE conversation_id = c.id ORDER BY sent_at DESC, id DESC LIMIT 1
          ) m ON true
          LEFT JOIN LATERAL (
              SELECT recipients FROM conversations.messages
               WHERE conversation_id = c.id AND c.last_sent_at IS NOT NULL AND direction = 'outbound'
                 AND NOT in_trash AND NOT in_spam
               ORDER BY sent_at DESC, id DESC LIMIT 1
          ) o ON true
         WHERE c.inbox_id = ANY($1) AND c.id = ANY($2)
         ORDER BY c.last_message_at DESC, c.id DESC
        "#,
        inboxes as &[InboxId],
        ids as &[ConversationId],
    )
    .fetch_all(conn)
    .await?)
}

/// Where the previous page ended: whether its last row was pinned, its
/// `last_message_at` and its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    pinned: bool,
    /// Whether it pages a `sent` list: `at` is then `last_sent_at`.
    sent: bool,
    at: DateTime<Utc>,
    id: ConversationId,
}

/// `(pinned, last_message_at, id)` of the last row of a page, opaque.
fn encode_cursor(pinned: bool, at: DateTime<Utc>, id: ConversationId) -> String {
    URL_SAFE_NO_PAD.encode(format!(
        "{}|{}|{id}",
        u8::from(pinned),
        at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
    ))
}

/// `(last_sent_at, id)` of the last row of a `sent` page, opaque, marked `s`
/// so a list of another kind refuses it.
fn encode_sent_cursor(at: DateTime<Utc>, id: ConversationId) -> String {
    URL_SAFE_NO_PAD.encode(format!(
        "s|{}|{id}",
        at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
    ))
}

/// A cursor from before pins (`at|id`) reads as not pinned: a list paged
/// across the upgrade goes on among the unpinned conversations.
fn decode_cursor(cursor: &str) -> Result<Cursor> {
    let bad = || Error::BadRequest(crate::MESSAGES.reason("invalid_cursor"));
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| bad())?;
    let text = String::from_utf8(bytes).map_err(|_| bad())?;
    let parts: Vec<&str> = text.split('|').collect();
    let (pinned, sent, at, id) = match parts.as_slice() {
        [at, id] | ["0", at, id] => (false, false, *at, *id),
        ["1", at, id] => (true, false, *at, *id),
        ["s", at, id] => (false, true, *at, *id),
        _ => return Err(bad()),
    };
    let at = DateTime::parse_from_rfc3339(at).map_err(|_| bad())?;
    Ok(Cursor {
        pinned,
        sent,
        at: at.with_timezone(&Utc),
        id: id.parse().map_err(|_| bad())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursors_round_trip_and_reject_junk() {
        let at = Utc::now();
        let id = ConversationId::new();
        for pinned in [false, true] {
            assert_eq!(
                decode_cursor(&encode_cursor(pinned, at, id)).unwrap(),
                Cursor {
                    pinned,
                    sent: false,
                    at,
                    id
                }
            );
        }
        assert!(decode_cursor("not-a-cursor").is_err());
        assert!(decode_cursor(&URL_SAFE_NO_PAD.encode("2026-01-01|nope")).is_err());
        assert!(decode_cursor(&URL_SAFE_NO_PAD.encode("2|2026-01-01T00:00:00Z|x")).is_err());
    }

    #[test]
    fn a_sent_cursor_is_marked() {
        let at = Utc::now();
        let id = ConversationId::new();
        assert_eq!(
            decode_cursor(&encode_sent_cursor(at, id)).unwrap(),
            Cursor {
                pinned: false,
                sent: true,
                at,
                id
            }
        );
    }

    #[test]
    fn a_cursor_from_before_pins_reads_as_not_pinned() {
        let at = Utc::now();
        let id = ConversationId::new();
        let legacy = URL_SAFE_NO_PAD.encode(format!(
            "{}|{id}",
            at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        ));
        assert_eq!(
            decode_cursor(&legacy).unwrap(),
            Cursor {
                pinned: false,
                sent: false,
                at,
                id
            }
        );
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
        // Sam wrote last; the newsletter's newest message is theirs.
        assert_eq!(q4["latest_from_you"], true);
        assert_eq!(all.json["conversations"][1]["latest_from_you"], false);
        assert_eq!(q4["labels"][0]["name"], "Clients");
        assert!(all.json["next_cursor"].is_null());

        let primary = kit.get(&sam, &format!("{base}?stream=primary")).await;
        assert_eq!(primary.json["conversations"].as_array().unwrap().len(), 1);
        let news = kit
            .get(&sam, &format!("{base}?stream=newsletters&inbox={inbox}"))
            .await;
        assert_eq!(news.json["conversations"][0]["stream_source"], "model");

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
    async fn pinned_conversations_stay_at_the_top_of_their_stream(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        // Pin the oldest: the Q4 plan.
        let q4 = kit.conversation_of("en-1").await;
        let pinned = kit.post(&sam, &format!("{base}/{q4}/star"), None).await;
        assert_eq!(pinned.json["starred"], true);

        let subjects = |json: &serde_json::Value| -> Vec<String> {
            json["conversations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["subject"].as_str().unwrap().to_owned())
                .collect()
        };
        let all = kit.get(&sam, &base).await;
        assert_eq!(
            subjects(&all.json),
            [
                "Q4 plan: can you review by Friday?",
                "New sign-in to your account from Linux",
                "Issue 142: five tools for calmer mornings"
            ]
        );
        let primary = kit.get(&sam, &format!("{base}?stream=primary")).await;
        assert_eq!(primary.json["conversations"][0]["starred"], true);

        // Paging crosses from the pinned to the rest without losing or repeating one.
        let mut seen = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..4 {
            let uri = match &cursor {
                Some(c) => format!("{base}?limit=1&cursor={c}"),
                None => format!("{base}?limit=1"),
            };
            let page = kit.get(&sam, &uri).await;
            assert_eq!(page.status, StatusCode::OK, "{:?}", page.json);
            seen.extend(subjects(&page.json));
            cursor = page.json["next_cursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen, subjects(&all.json));

        // Archived lists stay newest first.
        kit.post(&sam, &format!("{base}/{q4}/archive"), None).await;
        let archived = kit.get(&sam, &format!("{base}?archived=true")).await;
        assert_eq!(
            subjects(&archived.json),
            ["Q4 plan: can you review by Friday?"]
        );
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

    /// A message in a recorded batch.
    fn message(
        id: &str,
        thread: &str,
        direction: &str,
        other: (&str, &str),
        sent_at: &str,
        in_inbox: bool,
    ) -> serde_json::Value {
        let sam = serde_json::json!({"name": "Sam Rivers", "address": "sam@example.com"});
        let them = serde_json::json!({"name": other.0, "address": other.1});
        let (from, to) = if direction == "outbound" {
            (sam, them)
        } else {
            (them, sam)
        };
        serde_json::json!({"op": "upsert", "message": {
            "provider_message_id": id,
            "thread_key": thread,
            "from": from,
            "to": [to],
            "subject": format!("Subject of {thread}"),
            "sent_at": sent_at,
            "direction": direction,
            "text": "Example text.",
            "mailbox": {"in_inbox": in_inbox},
            "read": true, "starred": false
        }})
    }

    fn batch(messages: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({"changes": messages})
    }

    fn ids(json: &serde_json::Value) -> Vec<String> {
        json["conversations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap().to_owned())
            .collect()
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn sent_lists_what_you_started_even_archived_ordered_by_your_latest(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        // You start two conversations; neither is in the inbox.
        kit.ingest_value(
            workspace,
            inbox,
            &batch(vec![
                message(
                    "s-1",
                    "thr-s-1",
                    "outbound",
                    ("Clara Nunes", "clara@example.com"),
                    "2026-10-10T09:00:00Z",
                    false,
                ),
                message(
                    "s-2",
                    "thr-s-2",
                    "outbound",
                    ("Pedro Alves", "pedro@example.com"),
                    "2026-10-12T09:00:00Z",
                    false,
                ),
            ]),
        )
        .await;
        let started = kit.conversation_of("s-1").await;
        let other = kit.conversation_of("s-2").await;

        // They are not in the inbox.
        let inbox_list = kit.get(&sam, &base).await;
        assert!(!ids(&inbox_list.json).contains(&started));
        assert!(!ids(&inbox_list.json).contains(&other));

        // They are in Sent, your latest first; the Q4 thread (replied on
        // 5 October) comes after them.
        let sent = kit.get(&sam, &format!("{base}?sent=true")).await;
        assert_eq!(sent.status, StatusCode::OK, "{:?}", sent.json);
        let q4 = kit.conversation_of("en-1").await;
        assert_eq!(ids(&sent.json), [other.clone(), started.clone(), q4]);
        let first = &sent.json["conversations"][0];
        assert_eq!(first["archived"], true);
        assert_eq!(first["last_sent_at"], "2026-10-12T09:00:00Z");
        assert_eq!(first["sent_to"]["name"], "Pedro Alves");
        assert_eq!(first["sent_to"]["address"], "pedro@example.com");
        assert_eq!(first["latest_from_you"], true);

        // Their reply brings the started conversation into the inbox, and
        // Sent still orders by your message, not theirs.
        kit.ingest_value(
            workspace,
            inbox,
            &batch(vec![message(
                "s-1-reply",
                "thr-s-1",
                "inbound",
                ("Clara Nunes", "clara@example.com"),
                "2026-10-20T09:00:00Z",
                true,
            )]),
        )
        .await;
        let inbox_list = kit.get(&sam, &base).await;
        assert!(ids(&inbox_list.json).contains(&started));
        let sent = kit.get(&sam, &format!("{base}?sent=true")).await;
        assert_eq!(ids(&sent.json)[..2], [other, started]);
        let replied = &sent.json["conversations"][1];
        assert_eq!(replied["latest_from_you"], false);
        assert_eq!(replied["last_sent_at"], "2026-10-10T09:00:00Z");
        assert_eq!(replied["last_message_at"], "2026-10-20T09:00:00Z");
        assert_eq!(replied["archived"], false);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn sent_leaves_out_trash_and_spam_and_follows_streams_and_inboxes(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        kit.ingest_value(
            workspace,
            inbox,
            &batch(vec![
                message(
                    "t-1",
                    "thr-t-1",
                    "outbound",
                    ("Clara Nunes", "clara@example.com"),
                    "2026-10-10T09:00:00Z",
                    false,
                ),
                message(
                    "t-2",
                    "thr-t-2",
                    "outbound",
                    ("Pedro Alves", "pedro@example.com"),
                    "2026-10-11T09:00:00Z",
                    false,
                ),
            ]),
        )
        .await;
        let trashed = kit.conversation_of("t-1").await;
        let spammed = kit.conversation_of("t-2").await;
        let q4 = kit.conversation_of("en-1").await;
        let sent = format!("{base}?sent=true");

        assert_eq!(ids(&kit.get(&sam, &sent).await.json).len(), 3);
        kit.post(&sam, &format!("{base}/{trashed}/trash"), None)
            .await;
        kit.post(&sam, &format!("{base}/{spammed}/spam"), None)
            .await;
        assert_eq!(ids(&kit.get(&sam, &sent).await.json), [q4]);

        // Back from the trash, it is listed again.
        kit.post(&sam, &format!("{base}/{trashed}/untrash"), None)
            .await;
        assert!(ids(&kit.get(&sam, &sent).await.json).contains(&trashed));

        // Streams and inboxes narrow it.
        let news = kit.get(&sam, &format!("{sent}&stream=newsletters")).await;
        assert!(ids(&news.json).is_empty());
        let primary = kit.get(&sam, &format!("{sent}&stream=primary")).await;
        assert_eq!(ids(&primary.json).len(), 2);
        let mine = kit.get(&sam, &format!("{sent}&inbox={inbox}")).await;
        assert_eq!(ids(&mine.json).len(), 2);
        let unknown = kit
            .get(&sam, &format!("{sent}&inbox={}", InboxId::new()))
            .await;
        assert_eq!(unknown.status, StatusCode::NOT_FOUND);

        // `archived` is ignored.
        let both = kit.get(&sam, &format!("{sent}&archived=true")).await;
        assert_eq!(ids(&both.json).len(), 2);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn sent_pages_with_its_own_cursor(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        kit.ingest_value(
            workspace,
            inbox,
            &batch(vec![
                message(
                    "p-1",
                    "thr-p-1",
                    "outbound",
                    ("Clara Nunes", "clara@example.com"),
                    "2026-10-10T09:00:00Z",
                    false,
                ),
                message(
                    "p-2",
                    "thr-p-2",
                    "outbound",
                    ("Pedro Alves", "pedro@example.com"),
                    "2026-10-11T09:00:00Z",
                    false,
                ),
            ]),
        )
        .await;
        let mut seen = Vec::new();
        let mut cursor: Option<String> = None;
        let mut pages = 0;
        loop {
            let uri = match &cursor {
                Some(c) => format!("{base}?sent=true&limit=1&cursor={c}"),
                None => format!("{base}?sent=true&limit=1"),
            };
            let page = kit.get(&sam, &uri).await;
            assert_eq!(page.status, StatusCode::OK, "{:?}", page.json);
            seen.extend(ids(&page.json));
            pages += 1;
            cursor = page.json["next_cursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(pages, 3);
        let all = kit.get(&sam, &format!("{base}?sent=true")).await;
        assert_eq!(seen, ids(&all.json));

        // A cursor only pages the kind of list it came from.
        let first = kit.get(&sam, &format!("{base}?sent=true&limit=1")).await;
        let sent_cursor = first.json["next_cursor"].as_str().unwrap().to_owned();
        let wrong = kit.get(&sam, &format!("{base}?cursor={sent_cursor}")).await;
        assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
        assert_eq!(wrong.json["code"], "invalid_cursor");
        let stream_page = kit.get(&sam, &format!("{base}?limit=1")).await;
        let stream_cursor = stream_page.json["next_cursor"].as_str().unwrap().to_owned();
        let wrong = kit
            .get(&sam, &format!("{base}?sent=true&cursor={stream_cursor}"))
            .await;
        assert_eq!(wrong.status, StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn sent_stays_out_of_an_inbox_you_cannot_read(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, _) = kit.sams_inbox().await;
        let (ana, _) = kit.member("ana", workspace);
        let base = format!("/api/v1/w/{workspace}/conversations?sent=true");
        let response = kit.get(&ana, &base).await;
        assert_eq!(response.status, StatusCode::OK);
        assert!(ids(&response.json).is_empty());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_recount_restores_last_sent_at_as_the_backfill_computes_it(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        let q4 = kit.conversation_of("en-1").await;
        let stored: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
            "SELECT last_sent_at FROM conversations.conversations WHERE id = $1::uuid",
        )
        .bind(&q4)
        .fetch_one(&db)
        .await
        .unwrap();
        // Sam's only message in it is en-2, sent 5 October at 10:02.
        assert_eq!(stored.unwrap().to_rfc3339(), "2026-10-05T10:02:00+00:00");
        // The newsletter has none.
        let news = kit.conversation_of("en-3").await;
        let none: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
            "SELECT last_sent_at FROM conversations.conversations WHERE id = $1::uuid",
        )
        .bind(&news)
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(none.is_none());

        // Lose it, then touch the conversation: the recount derives it again.
        sqlx::query(
            "UPDATE conversations.conversations SET last_sent_at = NULL WHERE id = $1::uuid",
        )
        .bind(&q4)
        .execute(&db)
        .await
        .unwrap();
        kit.post(&sam, &format!("{base}/{q4}/star"), None).await;
        let again: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
            "SELECT last_sent_at FROM conversations.conversations WHERE id = $1::uuid",
        )
        .bind(&q4)
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(again, stored);
    }
}
