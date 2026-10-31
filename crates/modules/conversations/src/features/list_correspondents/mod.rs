//! `GET /api/v1/w/{workspace_id}/correspondents` — the people the inboxes
//! the caller can read exchange mail with, for suggesting them in the
//! composer's To, Cc and Bcc as the member types (campbooks-cloud#304).
//! Both directions: who writes to those inboxes (the sender of inbound mail)
//! and who they wrote to (To and Cc of sent mail), and who was copied on
//! the mail they received (its Cc). Addresses, display names
//! and counts only: never a subject or any text. Bounded (at most ten, no
//! cursor, `q` required), and it reads only the newest mail ([`WINDOW_DAYS`],
//! at most [`MAX_SCANNED`] received and as many sent messages, through
//! `messages_inbound_recent_idx` and `messages_outbound_idx`), so its cost
//! doesn't grow with an inbox's history.
//!
//! Someone the caller wrote to is always suggested: that was the member's
//! own choice. Someone who only writes in is suggested only when it is safe
//! to write to them, as `list_senders` with `reachable` judges it
//! ([`crate::domain::reachable`]): facts only (ADR 0018), never what the mail
//! says. Someone only copied on received mail is suggested when they are not
//! a no-reply address, none of their own mail is flagged, and the newest
//! message that copied them came from someone who may be written to.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    authz::Action,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        access::visible_inboxes,
        reachable::{self, flagged_sender_sql, may_write_to},
        resources,
        search_words::like_words,
    },
    events::INBOX_RECORD,
    scopes::ConversationsRead,
};

/// Most people in one answer.
pub const MAX_LIMIT: u32 = 10;
/// How far back it reads, in days.
pub const WINDOW_DAYS: i32 = 365;
/// Most messages it reads in each direction, newest first.
pub const MAX_SCANNED: i64 = 5_000;
/// Shortest and longest `q`, in characters.
const Q_CHARS: std::ops::RangeInclusive<usize> = 2..=100;
/// Most words of `q` matched.
const MAX_WORDS: usize = 5;

/// Whom to look for.
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CorrespondentsQuery {
    /// Words (2 to 100 characters) every listed person's address or display
    /// name contains, ignoring case and accents: what is typed in To, Cc or
    /// Bcc.
    pub q: String,
    /// How many, 1 to 10 (default 10).
    pub limit: Option<u32>,
}

/// Someone the caller's inboxes exchange mail with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CorrespondentView {
    /// Their address, lower-cased.
    #[schema(example = "marta@ferreira-studio.example")]
    pub address: String,
    /// The newest display name seen for them, sent or received, when there
    /// was one.
    #[schema(example = "Marta Ferreira")]
    pub name: Option<String>,
    /// Messages the caller's inboxes sent with them in To or Cc, among the
    /// newest (the last year, at most 5,000 sent messages).
    pub wrote: u32,
    /// Messages from them in the caller's inboxes, among the newest (the
    /// last year, at most 5,000 received messages).
    pub received: u32,
}

/// People, written to most first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CorrespondentList {
    /// Written to most first, then written by most, then copied most, then by address.
    pub correspondents: Vec<CorrespondentView>,
}

/// One person as the query reads them.
#[derive(sqlx::FromRow)]
struct Row {
    address: String,
    name: Option<String>,
    wrote: i32,
    received: i32,
    /// The authentication results of their newest received mail.
    newest_results: Option<String>,
    /// Who sent the newest received message that copied them, and its
    /// authentication results.
    copier: Option<String>,
    copier_results: Option<String>,
}

/// The newest received and sent mail of the inboxes `$1` within `$4` days,
/// at most `$5` of each; every person in it (the sender and the Cc of
/// received mail, the To and Cc of sent mail) whose address or names hold
/// each word of `$2`, without the inboxes' own addresses; ranked, then
/// those not written to are checked best first, only until `$3` pass
/// ([`flagged_sender_sql`]); the facts of their newest mail are read in
/// Rust ([`may_write_to`]).
const CORRESPONDENTS: &str = concat!(
    r"
    WITH own AS (
        SELECT i.address FROM conversations.inboxes i WHERE i.id = ANY($1)
    ),
    received AS (
        SELECT m.id, m.sender_address AS address,
               m.sender->>'name' AS name,
               m.headers->>'authentication_results' AS results,
               m.recipients,
               m.sent_at
          FROM conversations.messages m
         WHERE m.inbox_id = ANY($1) AND m.direction = 'inbound'
           AND NOT m.in_spam AND NOT m.in_trash
           AND m.sent_at > now() - make_interval(days => $4)
         ORDER BY m.sent_at DESC
         LIMIT $5
    ),
    sent AS (
        SELECT m.id, m.recipients, m.sent_at
          FROM conversations.messages m
         WHERE m.inbox_id = ANY($1) AND m.direction = 'outbound'
           AND m.sent_at > now() - make_interval(days => $4)
         ORDER BY m.sent_at DESC
         LIMIT $5
    ),
    seen AS (
        SELECT lower(trim(r.address)) AS address, nullif(trim(r.name), '') AS name,
               r.sent_at, 'received' AS kind, r.id AS message_id, r.results,
               NULL::text AS copier, NULL::text AS copier_results
          FROM received r
        UNION ALL
        SELECT lower(trim(p->>'address')), nullif(trim(p->>'name'), ''),
               s.sent_at, 'sent', s.id, NULL, NULL, NULL
          FROM sent s
         CROSS JOIN LATERAL jsonb_array_elements(
               coalesce(s.recipients->'to', '[]'::jsonb)
               || coalesce(s.recipients->'cc', '[]'::jsonb)) AS p
        UNION ALL
        SELECT lower(trim(p->>'address')), nullif(trim(p->>'name'), ''),
               r.sent_at, 'copied', r.id, NULL, lower(trim(r.address)), r.results
          FROM received r
         CROSS JOIN LATERAL jsonb_array_elements(
               coalesce(r.recipients->'cc', '[]'::jsonb)) AS p
    ),
    people AS (
        SELECT address,
               (array_agg(name ORDER BY sent_at DESC) FILTER (WHERE name IS NOT NULL))[1]
                 AS name,
               (count(DISTINCT message_id) FILTER (WHERE kind = 'sent'))::int AS wrote,
               (count(*) FILTER (WHERE kind = 'received'))::int AS received,
               (count(DISTINCT message_id) FILTER (WHERE kind = 'copied'))::int AS copied,
               (array_agg(results ORDER BY sent_at DESC) FILTER (WHERE kind = 'received'))[1]
                 AS newest_results,
               (array_agg(copier ORDER BY sent_at DESC) FILTER (WHERE kind = 'copied'))[1]
                 AS copier,
               (array_agg(copier_results ORDER BY sent_at DESC) FILTER (WHERE kind = 'copied'))[1]
                 AS copier_results
          FROM seen
         WHERE address <> '' AND address NOT IN (SELECT o.address FROM own o)
         GROUP BY address
        HAVING NOT EXISTS (
                 SELECT 1 FROM unnest($2::text[]) AS w(word)
                  WHERE public.unaccent(lower(address || ' ' || coalesce(string_agg(name, ' '), '')))
                        NOT LIKE '%' || public.unaccent(lower(w.word)) || '%' ESCAPE '\')
    ),
    -- Ranked first (`OFFSET 0` keeps the check below out of the sort), so
    -- the flag check runs person by person, best first, only until `$3`
    -- pass.
    ranked AS (
        SELECT address, name, wrote, received, copied, newest_results, copier, copier_results
          FROM people
         ORDER BY wrote DESC, received DESC, copied DESC, address
        OFFSET 0
    )
    SELECT address, name, wrote, received, newest_results, copier, copier_results
      FROM ranked k
     -- Written to: the member's own choice. Otherwise none of their own
     -- mail was reported, is under review or was found suspicious; and when
     -- they only were copied, nor any of the newest sender who copied them.
     WHERE k.wrote > 0
        OR (NOT ",
    flagged_sender_sql!(),
    r"
            AND (k.received > 0
                 OR (k.copier IS NOT NULL AND NOT ",
    flagged_sender_sql!("k.copier"),
    r")))
     ORDER BY wrote DESC, received DESC, copied DESC, address
     LIMIT $3
    "
);

/// Suggest the people you exchange mail with, as you type an address.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/correspondents",
    operation_id = "conversations.list_correspondents",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().records(["conversations.inbox"]).access("conversations.inbox", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), CorrespondentsQuery),
    responses(
        (status = OK, description = "The people.", body = CorrespondentList),
        (status = BAD_REQUEST, description = "No `q` (`invalid_query`), or a `q` or `limit` out of range (`invalid_correspondents_query`).",
            body = Problem, content_type = "application/problem+json"),
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
    extract::Query(query): extract::Query<CorrespondentsQuery>,
) -> Result<Json<CorrespondentList>> {
    resources::authorize_type(&state, &workspace.access, Action::Read, INBOX_RECORD)?;
    let limit = query.limit.unwrap_or(MAX_LIMIT);
    if !Q_CHARS.contains(&query.q.chars().count()) || !(1..=MAX_LIMIT).contains(&limit) {
        return Err(invalid());
    }
    let words = like_words(&query.q, MAX_WORDS);
    if words.is_empty() {
        return Err(invalid());
    }
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    let rows: Vec<Row> = sqlx::query_as(CORRESPONDENTS)
        .bind(&inboxes)
        .bind(&words)
        .bind(i64::from(limit) + reachable::SLACK)
        .bind(WINDOW_DAYS)
        .bind(MAX_SCANNED)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    let correspondents = rows
        .into_iter()
        .filter(|row| {
            row.wrote > 0
                || if row.received > 0 {
                    may_write_to(&row.address, row.newest_results.as_deref())
                } else {
                    // Only copied: not a no-reply address, and the newest
                    // message that copied them came from someone who may be
                    // written to.
                    may_write_to(&row.address, None)
                        && row.copier.as_deref().is_some_and(|copier| {
                            may_write_to(copier, row.copier_results.as_deref())
                        })
                }
        })
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .map(|row| CorrespondentView {
            address: row.address,
            name: row.name,
            wrote: u32::try_from(row.wrote).unwrap_or_default(),
            received: u32::try_from(row.received).unwrap_or_default(),
        })
        .collect();
    Ok(Json(CorrespondentList { correspondents }))
}

fn invalid() -> Error {
    Error::BadRequest(
        crate::MESSAGES
            .reason("invalid_correspondents_query")
            .arg("max", MAX_LIMIT),
    )
}

#[cfg(test)]
mod tests;
