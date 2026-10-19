//! `GET /api/v1/w/{workspace_id}/conversations` — conversations by stream,
//! pinned first (the provider's star), then newest first, a page at a time;
//! or one of the inbox's other places (Sent, Archive, Spam, Trash; campbooks-cloud#240).

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
        conversation::{ConversationSummary, SUMMARY_SELECT, SummaryRow},
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

/// Which of the inbox's places a list shows. The streams are the inbox;
/// the other views sit beside them and are not streams.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MailView {
    /// Conversations in the inbox (by stream with `stream`), pinned first.
    #[default]
    Inbox,
    /// Every conversation the inbox wrote in, outside spam and the trash, in
    /// any stream, archived or not, by the inbox's latest message.
    Sent,
    /// Conversations out of the inbox, outside spam and the trash.
    Archive,
    /// Conversations reported as spam.
    Spam,
    /// Conversations in the trash.
    Trash,
}

/// How a view filters and orders its list: fixed SQL fragments only.
struct Order {
    /// The `WHERE` condition on `c`.
    filter: &'static str,
    /// The column pages are keyed on and sorted by, newest first.
    at: &'static str,
    /// Pinned conversations come first (the inbox only).
    pinned: bool,
}

impl MailView {
    const fn order(self) -> Order {
        match self {
            Self::Inbox => Order {
                filter: "c.archived_at IS NULL AND NOT c.trashed",
                at: "c.last_message_at",
                pinned: true,
            },
            Self::Sent => Order {
                filter: "c.last_sent_at IS NOT NULL AND NOT c.trashed",
                at: "c.last_sent_at",
                pinned: false,
            },
            Self::Archive => Order {
                filter: "c.archived_at IS NOT NULL AND NOT c.trashed",
                at: "c.last_message_at",
                pinned: false,
            },
            Self::Spam => Order {
                filter: "c.trashed AND c.spam",
                at: "c.last_message_at",
                pinned: false,
            },
            Self::Trash => Order {
                filter: "c.trashed AND NOT c.spam",
                at: "c.last_message_at",
                pinned: false,
            },
        }
    }
}

/// Which conversations, and which page.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListQuery {
    /// Only this stream; every stream when absent.
    pub stream: Option<Stream>,
    /// Only this inbox; every inbox you can read when absent.
    pub inbox: Option<InboxId>,
    /// Which place: the inbox (default), Sent, Archive, Spam or Trash.
    pub view: Option<MailView>,
    /// Deprecated: use `view=archive`. `true` lists the archive, as
    /// `view=archive` does; with another `view` it is a bad request.
    #[serde(default)]
    pub archived: bool,
    /// `next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// Page size, 1 to 100 (default 50).
    pub limit: Option<u32>,
    /// These conversations only (1 to 100 ids, comma-separated), whatever
    /// their stream, view or whether archived; those you cannot see are
    /// left out. Other filters and the cursor are ignored.
    pub ids: Option<String>,
}

impl ListQuery {
    /// The view asked for, `archived=true` read as `view=archive`.
    fn view(&self) -> Result<MailView> {
        match (self.archived, self.view) {
            (false, view) => Ok(view.unwrap_or_default()),
            (true, None | Some(MailView::Archive)) => Ok(MailView::Archive),
            (true, Some(_)) => Err(Error::BadRequest(crate::MESSAGES.reason("invalid_view"))),
        }
    }
}

/// A page of conversations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ConversationList {
    /// The inbox: pinned first, then newest first. Sent: by the inbox's
    /// latest message. Archive, Spam, Trash and `ids`: newest first.
    pub conversations: Vec<ConversationSummary>,
    /// Pass it as `cursor` for the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// List conversations: the inbox pinned first, then newest first, or another view.
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
        (status = BAD_REQUEST, description = "A malformed cursor or limit, or `archived=true` with another view.", body = Problem,
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
    let view = query.view()?;
    let ids = query.ids.as_deref().map(parse_ids).transpose()?;
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;
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
    let order = view.order();
    let rows = fetch_page(&mut tx, &inboxes, query.stream, &order, after, limit).await?;
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
            // Only the inbox puts pins first, so only its cursor carries
            // one; Sent pages on the inbox's latest message.
            let at = if view == MailView::Sent {
                last.last_sent_at.unwrap_or(last.last_message_at)
            } else {
                last.last_message_at
            };
            encode_cursor(last.starred && order.pinned, at, last.id)
        })
    } else {
        None
    };
    Ok(Json(ConversationList {
        conversations,
        next_cursor,
    }))
}

/// One page of a view (plus one row, to tell whether there is a next page),
/// in `stream` when given. The SQL is built from the view's fixed fragments
/// only; every value is bound.
async fn fetch_page(
    conn: &mut PgConnection,
    inboxes: &[InboxId],
    stream: Option<Stream>,
    order: &Order,
    after: Option<Cursor>,
    limit: u32,
) -> Result<Vec<SummaryRow>> {
    let Order { filter, at, pinned } = *order;
    let (keyset, sort) = if pinned {
        (
            format!("(c.starred, {at}, c.id) < ($6::bool, $3, $4)"),
            format!("c.starred DESC, {at} DESC, c.id DESC"),
        )
    } else {
        (
            format!("({at}, c.id) < ($3, $4)"),
            format!("{at} DESC, c.id DESC"),
        )
    };
    let sql = format!(
        "{SUMMARY_SELECT}
         WHERE c.inbox_id = ANY($1) AND {filter}
           AND ($2::text IS NULL OR c.stream = $2)
           AND ($3::timestamptz IS NULL OR {keyset})
         ORDER BY {sort}
         LIMIT $5"
    );
    let (after_at, after_id) = after.map(|cursor| (cursor.at, cursor.id)).unzip();
    let query = sqlx::query_as::<_, SummaryRow>(sqlx::AssertSqlSafe(sql))
        .bind(inboxes)
        .bind(stream.map(Stream::as_str))
        .bind(after_at)
        .bind(after_id)
        .bind(i64::from(limit) + 1);
    let query = if pinned {
        query.bind(after.is_some_and(|cursor| cursor.pinned))
    } else {
        query
    };
    Ok(query.fetch_all(&mut *conn).await?)
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
    Ok(sqlx::query_as::<_, SummaryRow>(sqlx::AssertSqlSafe(format!(
        "{SUMMARY_SELECT}
         WHERE c.inbox_id = ANY($1) AND c.id = ANY($2)
         ORDER BY c.last_message_at DESC, c.id DESC"
    )))
    .bind(inboxes)
    .bind(ids)
    .fetch_all(conn)
    .await?)
}

/// Where the previous page ended: whether its last row was pinned, the time
/// its view pages on (`last_sent_at` in Sent, else `last_message_at`) and its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cursor {
    pinned: bool,
    at: DateTime<Utc>,
    id: ConversationId,
}

/// `(pinned, time, id)` of the last row of a page, opaque.
fn encode_cursor(pinned: bool, at: DateTime<Utc>, id: ConversationId) -> String {
    URL_SAFE_NO_PAD.encode(format!(
        "{}|{}|{id}",
        u8::from(pinned),
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
    let (pinned, at, id) = match parts.as_slice() {
        [at, id] | ["0", at, id] => (false, *at, *id),
        ["1", at, id] => (true, *at, *id),
        _ => return Err(bad()),
    };
    let at = DateTime::parse_from_rfc3339(at).map_err(|_| bad())?;
    Ok(Cursor {
        pinned,
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
                Cursor { pinned, at, id }
            );
        }
        assert!(decode_cursor("not-a-cursor").is_err());
        assert!(decode_cursor(&URL_SAFE_NO_PAD.encode("2026-01-01|nope")).is_err());
        assert!(decode_cursor(&URL_SAFE_NO_PAD.encode("2|2026-01-01T00:00:00Z|x")).is_err());
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
                at,
                id
            }
        );
    }
}

#[cfg(test)]
mod http_tests {
    use axum::http::StatusCode;
    use campbooks_kernel::tenancy::WorkspaceId;
    use serde_json::{Value, json};
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

    /// A recorded message Sam sent in `thread` at `at` (RFC 3339) to Marta
    /// Ferreira, copying two others, with its `mailbox` flags.
    fn sent(id: &str, thread: &str, subject: &str, at: &str, mailbox: Value) -> Value {
        json!({"op": "upsert", "message": {
            "provider_message_id": id, "thread_key": thread,
            "from": {"name": "Sam Rivers", "address": "sam@example.com"},
            "to": [{"name": "Marta Ferreira", "address": "marta@example.com"}],
            "cc": [{"name": "Leo Park", "address": "leo@example.com"},
                   {"address": "ana@example.com"}],
            "subject": subject, "sent_at": at, "direction": "outbound",
            "text": "Here are the figures.",
            "mailbox": mailbox, "read": true, "starred": false
        }})
    }

    /// Maya's answer in the Q4 thread at `at`.
    fn maya_replies(id: &str, at: &str) -> Value {
        json!({"op": "upsert", "message": {
            "provider_message_id": id, "thread_key": "thr-en-q4",
            "in_reply_to": "q4-plan-2@example.com",
            "from": {"name": "Maya Okafor", "address": "maya@northwind-studio.example"},
            "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
            "subject": "Re: Q4 plan: can you review by Friday?", "sent_at": at,
            "direction": "inbound", "text": "Thanks, that helps.",
            "mailbox": {"in_inbox": true}, "read": false, "starred": false
        }})
    }

    /// Sam's inbox plus a conversation Sam started (archived on arrival:
    /// none of it is in the inbox).
    async fn with_one_started(kit: &Kit) -> (WorkspaceId, InboxId, String) {
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let started = sent(
            "mine-1",
            "thr-mine-1",
            "Figures for the board",
            "2026-10-07T09:00:00Z",
            json!({"in_inbox": false}),
        );
        kit.ingest_value(workspace, inbox, &json!({"changes": [started]}))
            .await;
        (workspace, inbox, sam)
    }

    /// The subjects of the list at `uri`, in order.
    async fn subjects(kit: &Kit, token: &str, uri: &str) -> Vec<String> {
        let page = kit.get(token, uri).await;
        assert_eq!(page.status, StatusCode::OK, "{uri}: {:?}", page.json);
        page.json["conversations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["subject"].as_str().unwrap().to_owned())
            .collect()
    }

    const Q4: &str = "Q4 plan: can you review by Friday?";
    const NEWS: &str = "Issue 142: five tools for calmer mornings";
    const SIGN_IN: &str = "New sign-in to your account from Linux";
    const STARTED: &str = "Figures for the board";

    #[sqlx::test(migrations = "../../../migrations")]
    async fn sent_lists_every_conversation_i_wrote_in(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = with_one_started(&kit).await;
        let base = format!("/api/v1/w/{workspace}/conversations");

        // The one Sam started, then the Q4 plan Sam replied to; never the
        // ones Sam only received.
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=sent")).await,
            [STARTED, Q4]
        );
        let page = kit.get(&sam, &format!("{base}?view=sent")).await;
        let started = &page.json["conversations"][0];
        assert_eq!(started["archived"], true);
        assert_eq!(started["last_sent_at"], "2026-10-07T09:00:00Z");
        assert_eq!(started["sent_to"][0]["name"], "Marta Ferreira");
        assert_eq!(started["sent_to"].as_array().unwrap().len(), 3);
        let q4 = &page.json["conversations"][1];
        assert_eq!(q4["last_sent_at"], "2026-10-05T10:02:00Z");
        assert_eq!(q4["sent_to"][0]["name"], "Maya Okafor");
        // Mail Sam starts stays out of the inbox until someone replies.
        assert!(
            !subjects(&kit, &sam, &base)
                .await
                .contains(&STARTED.to_owned())
        );
        // A stream still filters, when one is asked for.
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=sent&stream=newsletters")).await,
            Vec::<String>::new()
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn sent_says_when_they_replied_since(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = with_one_started(&kit).await;
        let uri = format!("/api/v1/w/{workspace}/conversations?view=sent");
        let replied = |json: &Value| -> Vec<bool> {
            json["conversations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["replied_since_sent"].as_bool().unwrap())
                .collect()
        };
        assert_eq!(replied(&kit.get(&sam, &uri).await.json), [false, false]);

        kit.ingest_value(
            workspace,
            inbox,
            &json!({"changes": [maya_replies("en-5", "2026-10-08T07:30:00Z")]}),
        )
        .await;
        // Still sorted by Sam's latest message: the Q4 plan stays second.
        let page = kit.get(&sam, &uri).await;
        assert_eq!(page.json["conversations"][1]["subject"], Q4);
        assert_eq!(replied(&page.json), [false, true]);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn sent_leaves_out_spam_and_trash(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = with_one_started(&kit).await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        let q4 = kit.conversation_of("en-1").await;
        let started = kit.conversation_of("mine-1").await;
        kit.post(&sam, &format!("{base}/{q4}/trash"), None).await;
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=sent")).await,
            [STARTED]
        );
        kit.post(&sam, &format!("{base}/{started}/spam"), None)
            .await;
        assert!(
            subjects(&kit, &sam, &format!("{base}?view=sent"))
                .await
                .is_empty()
        );
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=trash")).await,
            [Q4]
        );
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=spam")).await,
            [STARTED]
        );
        // Restored, it is back in Sent.
        kit.post(&sam, &format!("{base}/{q4}/untrash"), None).await;
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=sent")).await,
            [Q4]
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn archive_leaves_out_spam_and_trash(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        let news = kit.conversation_of("en-3").await;
        let sign_in = kit.conversation_of("en-4").await;
        for id in [&news, &sign_in] {
            kit.post(&sam, &format!("{base}/{id}/archive"), None).await;
        }
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=archive")).await,
            [SIGN_IN, NEWS]
        );
        kit.post(&sam, &format!("{base}/{sign_in}/trash"), None)
            .await;
        kit.post(&sam, &format!("{base}/{news}/spam"), None).await;
        assert!(
            subjects(&kit, &sam, &format!("{base}?view=archive"))
                .await
                .is_empty()
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn spam_and_trash_are_separate_views(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        let news = kit.conversation_of("en-3").await;
        let sign_in = kit.conversation_of("en-4").await;
        let reported = kit.post(&sam, &format!("{base}/{news}/spam"), None).await;
        assert_eq!(reported.json["trashed"], true);
        assert_eq!(reported.json["spam"], true);
        let deleted = kit
            .post(&sam, &format!("{base}/{sign_in}/trash"), None)
            .await;
        assert_eq!(deleted.json["trashed"], true);
        assert_eq!(deleted.json["spam"], false);

        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=spam")).await,
            [NEWS]
        );
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=trash")).await,
            [SIGN_IN]
        );
        assert_eq!(subjects(&kit, &sam, &base).await, [Q4]);
        assert!(
            subjects(&kit, &sam, &format!("{base}?view=archive"))
                .await
                .is_empty()
        );
        // Deleting what is in spam moves it to Trash.
        kit.post(&sam, &format!("{base}/{news}/trash"), None).await;
        assert!(
            subjects(&kit, &sam, &format!("{base}?view=spam"))
                .await
                .is_empty()
        );
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=trash")).await,
            [SIGN_IN, NEWS]
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn not_spam_comes_back_to_the_inbox(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        let news = kit.conversation_of("en-3").await;
        // Archived first: Not spam still puts received mail in the inbox,
        // as a provider's own "Not spam" does.
        kit.post(&sam, &format!("{base}/{news}/archive"), None)
            .await;
        kit.post(&sam, &format!("{base}/{news}/spam"), None).await;
        let back = kit
            .post(&sam, &format!("{base}/{news}/not-spam"), None)
            .await;
        assert_eq!(back.json["archived"], false);
        assert_eq!(back.json["spam"], false);
        assert_eq!(back.json["trashed"], false);
        assert!(subjects(&kit, &sam, &base).await.contains(&NEWS.to_owned()));
        assert!(
            subjects(&kit, &sam, &format!("{base}?view=spam"))
                .await
                .is_empty()
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn restore_brings_trash_back_where_it_was(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        let news = kit.conversation_of("en-3").await;
        let sign_in = kit.conversation_of("en-4").await;
        // One in the inbox, one archived; both deleted, then restored.
        kit.post(&sam, &format!("{base}/{news}/archive"), None)
            .await;
        for id in [&news, &sign_in] {
            kit.post(&sam, &format!("{base}/{id}/trash"), None).await;
        }
        assert_eq!(subjects(&kit, &sam, &base).await, [Q4]);
        for id in [&news, &sign_in] {
            kit.post(&sam, &format!("{base}/{id}/untrash"), None).await;
        }
        assert_eq!(subjects(&kit, &sam, &base).await, [SIGN_IN, Q4]);
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=archive")).await,
            [NEWS]
        );
        assert!(
            subjects(&kit, &sam, &format!("{base}?view=trash"))
                .await
                .is_empty()
        );
    }

    /// `last_sent_at` of the conversation holding `message`.
    async fn last_sent_at(kit: &Kit, message: &str) -> Option<String> {
        sqlx::query_scalar(
            "SELECT to_char(c.last_sent_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI')
               FROM conversations.conversations c
               JOIN conversations.messages m ON m.conversation_id = c.id
              WHERE m.provider_message_id = $1",
        )
        .bind(message)
        .fetch_one(&kit.db)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn recount_keeps_last_sent_at(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, _) = kit.sams_inbox().await;
        let later = |mailbox: Value| {
            sent(
                "en-6",
                "thr-en-q4",
                "Re: Q4 plan: can you review by Friday?",
                "2026-10-06T12:00:00Z",
                mailbox,
            )
        };
        kit.ingest_value(
            workspace,
            inbox,
            &json!({"changes": [later(json!({"in_inbox": true}))]}),
        )
        .await;
        assert_eq!(
            last_sent_at(&kit, "en-1").await.as_deref(),
            Some("2026-10-06T12:00")
        );
        // That message moved to the trash at the provider: back to the one before.
        kit.ingest_value(
            workspace,
            inbox,
            &json!({"changes": [later(json!({"in_inbox": false, "in_trash": true}))]}),
        )
        .await;
        assert_eq!(
            last_sent_at(&kit, "en-1").await.as_deref(),
            Some("2026-10-05T10:02")
        );
        // And without any: none.
        let mut reply = sent(
            "en-2",
            "thr-en-q4",
            "Re: Q4 plan: can you review by Friday?",
            "2026-10-05T10:02:00Z",
            json!({"in_inbox": false, "in_spam": true}),
        );
        reply["message"]["to"] =
            json!([{"name": "Maya Okafor", "address": "maya@northwind-studio.example"}]);
        kit.ingest_value(workspace, inbox, &json!({"changes": [reply]}))
            .await;
        assert_eq!(last_sent_at(&kit, "en-1").await, None);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn the_migration_backfills(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, _, sam) = with_one_started(&kit).await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        let news = kit.conversation_of("en-3").await;
        kit.post(&sam, &format!("{base}/{news}/spam"), None).await;
        // As before the migration: neither column known.
        sqlx::query("UPDATE conversations.conversations SET last_sent_at = NULL, spam = false")
            .execute(&db)
            .await
            .unwrap();
        let migration = std::fs::read_to_string(format!(
            "{}/../../../migrations/20261022070000_conversations__mail_views.sql",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let start = migration.find("-- seed begin").unwrap();
        let end = migration.find("-- seed end").unwrap();
        // The migration's own text, from the repository, twice: it is idempotent.
        for _ in 0..2 {
            sqlx::raw_sql(sqlx::AssertSqlSafe(migration[start..end].to_owned()))
                .execute(&db)
                .await
                .unwrap();
        }
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=sent")).await,
            [STARTED, Q4]
        );
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=spam")).await,
            [NEWS]
        );
        assert_eq!(
            last_sent_at(&kit, "mine-1").await.as_deref(),
            Some("2026-10-07T09:00")
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_view_cursor_pages_through(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = with_one_started(&kit).await;
        let other = sent(
            "mine-2",
            "thr-mine-2",
            "Studio keys",
            "2026-10-06T18:00:00Z",
            json!({"in_inbox": false}),
        );
        kit.ingest_value(workspace, inbox, &json!({"changes": [other]}))
            .await;
        let base = format!("/api/v1/w/{workspace}/conversations?view=sent&limit=2");
        let first = kit.get(&sam, &base).await;
        let cursor = first.json["next_cursor"].as_str().unwrap().to_owned();
        assert_eq!(subjects(&kit, &sam, &base).await, [STARTED, "Studio keys"]);
        let rest = kit.get(&sam, &format!("{base}&cursor={cursor}")).await;
        assert_eq!(rest.json["conversations"][0]["subject"], Q4);
        assert_eq!(rest.json["conversations"].as_array().unwrap().len(), 1);
        assert!(rest.json["next_cursor"].is_null());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn archived_true_still_lists_the_archive(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = with_one_started(&kit).await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        let news = kit.conversation_of("en-3").await;
        kit.post(&sam, &format!("{base}/{news}/archive"), None)
            .await;
        let deprecated = subjects(&kit, &sam, &format!("{base}?archived=true")).await;
        assert_eq!(deprecated, [STARTED, NEWS]);
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?view=archive")).await,
            deprecated
        );
        assert_eq!(
            subjects(&kit, &sam, &format!("{base}?archived=true&view=archive")).await,
            deprecated
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn archived_and_another_view_is_a_bad_request(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        for view in ["inbox", "sent", "spam", "trash"] {
            let refused = kit
                .get(&sam, &format!("{base}?archived=true&view={view}"))
                .await;
            assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{view}");
            assert_eq!(refused.json["code"], "invalid_view", "{view}");
        }
        assert_eq!(
            kit.get(&sam, &format!("{base}?view=starred")).await.status,
            StatusCode::BAD_REQUEST
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn no_view_shows_another_inbox_or_workspace(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = with_one_started(&kit).await;
        let base = format!("/api/v1/w/{workspace}/conversations");
        let news = kit.conversation_of("en-3").await;
        let sign_in = kit.conversation_of("en-4").await;
        kit.post(&sam, &format!("{base}/{news}/spam"), None).await;
        kit.post(&sam, &format!("{base}/{sign_in}/trash"), None)
            .await;
        // A member of the workspace who is not in Sam's inbox.
        let (ana, _) = kit.member("ana", workspace);
        // A member of another workspace only.
        let (leo, _) = kit.member("leo", WorkspaceId::new());
        for (view, seen) in [("sent", 2), ("archive", 1), ("spam", 1), ("trash", 1)] {
            let uri = format!("{base}?view={view}");
            assert_eq!(subjects(&kit, &sam, &uri).await.len(), seen, "{view}");
            assert!(subjects(&kit, &ana, &uri).await.is_empty(), "{view}");
            assert_eq!(
                kit.get(&leo, &uri).await.status,
                StatusCode::NOT_FOUND,
                "{view}"
            );
        }
    }
}
