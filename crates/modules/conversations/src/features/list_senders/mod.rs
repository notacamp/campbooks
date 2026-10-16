//! `GET /api/v1/w/{workspace_id}/senders` — who writes to the inboxes the
//! caller can read, for picking people to always see or never be asked about
//! (the assistant's questions), and for matching a name in a sentence
//! someone typed to an address. Addresses, display names and counts only:
//! never a subject or any text. Bounded (at most ten, no cursor), so it
//! cannot page out a whole inbox's correspondents.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use campbooks_kernel::authz::Action;

use crate::{
    domain::{access::visible_inboxes, resources},
    events::INBOX_RECORD,
    model::InboxId,
    scopes::ConversationsRead,
};

/// Most senders in one answer.
pub const MAX_LIMIT: u32 = 10;
/// Shortest and longest `q`, in characters.
const Q_CHARS: std::ops::RangeInclusive<usize> = 2..=100;
/// Most words of `q` matched.
const MAX_WORDS: usize = 5;

/// Which senders.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SendersQuery {
    /// Words (2 to 100 characters) every listed sender's address, display
    /// name or domain contains, ignoring case and accents. Without it, the
    /// senders replied to most.
    pub q: Option<String>,
    /// How many, 1 to 10 (default 10).
    pub limit: Option<u32>,
}

/// Someone who writes to the caller's inboxes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SenderView {
    /// Their address, lower-cased.
    #[schema(example = "rita@silva-contas.example")]
    pub address: String,
    /// The display name on their newest message, when it had one.
    #[schema(example = "Rita Gomes")]
    pub name: Option<String>,
    /// Inbound messages from them in the caller's inboxes.
    pub received: u32,
    /// Their conversations the caller's inboxes replied to.
    pub replied: u32,
}

/// Senders, replied to most first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SenderList {
    /// Replied to most first, then written most, then by address.
    pub senders: Vec<SenderView>,
}

/// List who writes to the inboxes you can read.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/senders",
    operation_id = "conversations.list_senders",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.inbox"]).access("conversations.inbox", campbooks_kernel::authz::Action::Read)))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), SendersQuery),
    responses(
        (status = OK, description = "The senders.", body = SenderList),
        (status = BAD_REQUEST, description = "A `q` or `limit` out of range (`invalid_senders_query`).",
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
    extract::Query(query): extract::Query<SendersQuery>,
) -> Result<Json<SenderList>> {
    resources::authorize_type(&state, &workspace.access, Action::Read, INBOX_RECORD)?;
    let limit = query.limit.unwrap_or(MAX_LIMIT);
    let words = match query.q.as_deref() {
        None => None,
        Some(q) if Q_CHARS.contains(&q.chars().count()) => Some(words(q)),
        Some(_) => return Err(invalid()),
    };
    if !(1..=MAX_LIMIT).contains(&limit) || words.as_ref().is_some_and(Vec::is_empty) {
        return Err(invalid());
    }
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let inboxes = visible_inboxes(&mut tx, access).await?;
    let rows = sqlx::query!(
        r#"
        WITH inbound AS (
            SELECT m.sender_address AS address,
                   nullif(m.sender->>'name', '') AS name,
                   m.sent_at,
                   m.conversation_id,
                   EXISTS (SELECT 1 FROM conversations.messages o
                            WHERE o.conversation_id = m.conversation_id
                              AND o.direction = 'outbound' AND o.sent_at > m.sent_at)
                     AS replied
              FROM conversations.messages m
             WHERE m.inbox_id = ANY($1) AND m.direction = 'inbound'
               AND NOT m.in_spam AND NOT m.in_trash
        )
        SELECT address AS "address!",
               (array_agg(name ORDER BY sent_at DESC) FILTER (WHERE name IS NOT NULL))[1]
                 AS name,
               count(*)::int AS "received!",
               (count(DISTINCT conversation_id) FILTER (WHERE replied))::int AS "replied!"
          FROM inbound
         GROUP BY address
        HAVING $2::text[] IS NULL OR NOT EXISTS (
                 SELECT 1 FROM unnest($2::text[]) AS w(word)
                  WHERE public.unaccent(lower(address || ' ' || coalesce(string_agg(name, ' '), '')))
                        NOT LIKE '%' || public.unaccent(lower(w.word)) || '%' ESCAPE '\')
         ORDER BY 4 DESC, 3 DESC, 1
         LIMIT $3
        "#,
        &inboxes as &[InboxId],
        words.as_deref() as Option<&[String]>,
        i64::from(limit),
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let senders = rows
        .into_iter()
        .map(|row| SenderView {
            address: row.address,
            name: row.name,
            received: u32::try_from(row.received).unwrap_or_default(),
            replied: u32::try_from(row.replied).unwrap_or_default(),
        })
        .collect();
    Ok(Json(SenderList { senders }))
}

fn invalid() -> Error {
    Error::BadRequest(
        crate::MESSAGES
            .reason("invalid_senders_query")
            .arg("max", MAX_LIMIT),
    )
}

/// The words of `q`, at most [`MAX_WORDS`], with `LIKE` wildcards escaped.
fn words(q: &str) -> Vec<String> {
    q.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
        .take(MAX_WORDS)
        .map(|word| {
            word.chars().fold(String::new(), |mut out, c| {
                if matches!(c, '\\' | '%' | '_') {
                    out.push('\\');
                }
                out.push(c);
                out
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use campbooks_kernel::tenancy::WorkspaceId;
    use sqlx::PgPool;

    use super::words;
    use crate::{model::InboxRole, testkit::Kit};

    #[test]
    fn wildcards_are_escaped() {
        assert_eq!(words("50%_off a\\b"), vec!["50\\%\\_off", "a\\\\b"]);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn replied_to_most_first_without_text(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;

        let response = kit
            .get(&sam, &format!("/api/v1/w/{workspace}/senders"))
            .await;
        assert_eq!(response.status, StatusCode::OK);
        let senders = response.json["senders"].as_array().unwrap();
        assert_eq!(senders[0]["address"], "maya@northwind-studio.example");
        assert_eq!(senders[0]["name"], "Maya Okafor");
        assert_eq!(senders[0]["replied"], 1);
        assert!(senders.len() >= 3);
        for sender in senders {
            let keys: Vec<_> = sender.as_object().unwrap().keys().cloned().collect();
            assert_eq!(keys, ["address", "name", "received", "replied"]);
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn words_match_address_name_and_domain_ignoring_accents(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let uri = format!("/api/v1/w/{workspace}/senders");

        let found = kit.get(&sam, &format!("{uri}?q=Okáfor")).await;
        assert_eq!(found.json["senders"].as_array().unwrap().len(), 1);
        let domain = kit.get(&sam, &format!("{uri}?q=paperkite")).await;
        assert_eq!(
            domain.json["senders"][0]["address"],
            "hello@letters.paperkite.example"
        );
        let none = kit.get(&sam, &format!("{uri}?q=maya%20cloudvault")).await;
        assert!(none.json["senders"].as_array().unwrap().is_empty());
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn bounded(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let uri = format!("/api/v1/w/{workspace}/senders");

        for bad in ["?limit=11", "?limit=0", "?q=a", "?q=%20%20"] {
            let response = kit.get(&sam, &format!("{uri}{bad}")).await;
            assert_eq!(response.status, StatusCode::BAD_REQUEST, "{bad}");
            assert_eq!(response.json["code"], "invalid_senders_query");
        }
        let one = kit.get(&sam, &format!("{uri}?limit=1")).await;
        assert_eq!(one.json["senders"].as_array().unwrap().len(), 1);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn only_inboxes_the_caller_reads(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, _) = kit.sams_inbox().await;
        let (ana, ana_id) = kit.member("ana", workspace);
        let uri = format!("/api/v1/w/{workspace}/senders");

        let outsider = kit.get(&ana, &uri).await;
        assert_eq!(outsider.status, StatusCode::OK);
        assert!(outsider.json["senders"].as_array().unwrap().is_empty());

        kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
        let reader = kit.get(&ana, &uri).await;
        assert!(!reader.json["senders"].as_array().unwrap().is_empty());

        let stranger = kit
            .get(&ana, &format!("/api/v1/w/{}/senders", WorkspaceId::new()))
            .await;
        assert_eq!(stranger.status, StatusCode::NOT_FOUND);
        assert_eq!(kit.get("", &uri).await.status, StatusCode::UNAUTHORIZED);
    }
}
