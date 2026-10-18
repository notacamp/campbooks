//! `GET /api/v1/w/{workspace_id}/senders` — who writes to the inboxes the
//! caller can read, for picking people to always see or never be asked about
//! (the assistant's questions), and for matching a name in a sentence
//! someone typed to an address. Addresses, display names and counts only:
//! never a subject or any text. Bounded (at most ten, no cursor), so it
//! cannot page out a whole inbox's correspondents; and it reads only the
//! newest inbound mail ([`WINDOW_DAYS`], at most [`MAX_SCANNED`] messages,
//! through `messages_inbound_recent_idx`), so its cost doesn't grow with an
//! inbox's history.
//!
//! With `reachable`, only the senders it is safe to write to, such as
//! Forward's picker (campbooks-cloud#233): never a no-reply address, nor
//! a sender whose newest mail fails its sender's checks or pretends to be
//! them, nor one with mail reported (in spam), under review or found
//! suspicious and not said safe. Facts only (ADR 0018): addresses, the
//! provider's authentication results, stored answers, needs and member
//! corrections, never what the mail says.

use axum::{Json, extract::State};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result,
    db::BeginFor as _,
    extract,
    tenancy::{Workspace, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use campbooks_ai::decide::signals::facts::{Facts, dmarc_refused, sender_checks_failed};
use campbooks_kernel::authz::Action;

use crate::{
    domain::{access::visible_inboxes, resources},
    events::INBOX_RECORD,
    scopes::ConversationsRead,
};

/// Most senders in one answer.
pub const MAX_LIMIT: u32 = 10;
/// How far back it reads, in days.
pub const WINDOW_DAYS: i32 = 365;
/// Most inbound messages it reads, newest first.
pub const MAX_SCANNED: i64 = 5_000;
/// Shortest and longest `q`, in characters.
const Q_CHARS: std::ops::RangeInclusive<usize> = 2..=100;
/// Most words of `q` matched.
const MAX_WORDS: usize = 5;
/// Senders read beyond `limit` with `reachable`, so those it leaves out
/// rarely shorten the answer.
const REACHABLE_SLACK: i64 = 40;

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
    /// Only senders it is safe to write to (Forward's picker): never a
    /// no-reply address, nor a sender whose newest mail fails their checks
    /// or pretends to be them, nor one with mail reported (in spam), under
    /// review or found suspicious. Default false.
    pub reachable: Option<bool>,
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
    /// Inbound messages from them in the caller's inboxes, among the newest
    /// (the last year, at most 5,000 messages).
    pub received: u32,
    /// Of those, their conversations the caller's inboxes replied to.
    pub replied: u32,
}

/// Senders, replied to most first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SenderList {
    /// Replied to most first, then written most, then by address.
    pub senders: Vec<SenderView>,
}

/// One sender as the query reads them: address, name, received, replied,
/// and their newest mail's authentication results.
type Row = (String, Option<String>, i32, i32, Option<String>);

/// The newest inbound mail only (`messages_inbound_recent_idx`); then one
/// "was it answered" per thread, not per message, and only for the senders
/// `q` keeps. With `reachable` ($6), a sender Campbooks or the member
/// flagged is left out here (`flagged`), and the facts of their newest mail
/// are read for the checks in Rust ([`may_write_to`]).
const SENDERS: &str = r"
    WITH recent AS (
        SELECT m.sender_address AS address,
               nullif(m.sender->>'name', '') AS name,
               m.headers->>'authentication_results' AS results,
               m.sent_at,
               m.conversation_id
          FROM conversations.messages m
         WHERE m.inbox_id = ANY($1) AND m.direction = 'inbound'
           AND NOT m.in_spam AND NOT m.in_trash
           AND m.sent_at > now() - make_interval(days => $4)
         ORDER BY m.sent_at DESC
         LIMIT $5
    ),
    senders AS (
        SELECT address,
               (array_agg(name ORDER BY sent_at DESC) FILTER (WHERE name IS NOT NULL))[1]
                 AS name,
               (array_agg(results ORDER BY sent_at DESC))[1] AS newest_results,
               count(*)::int AS received
          FROM recent
         GROUP BY address
        HAVING $2::text[] IS NULL OR NOT EXISTS (
                 SELECT 1 FROM unnest($2::text[]) AS w(word)
                  WHERE public.unaccent(lower(address || ' ' || coalesce(string_agg(name, ' '), '')))
                        NOT LIKE '%' || public.unaccent(lower(w.word)) || '%' ESCAPE '\')
    ),
    -- Any of the sender's own mail in the window, spam included: reported
    -- (Report and delete moves it to spam), under review (open, or cleared
    -- because a later message replaced it), or found suspicious and not
    -- said safe by a member. Judged by the flagged message's own sender,
    -- so the real people in a hijacked thread stay.
    flagged AS (
        SELECT DISTINCT m.sender_address AS address
          FROM conversations.messages m
         WHERE $6
           AND m.inbox_id = ANY($1) AND m.direction = 'inbound'
           AND m.sent_at > now() - make_interval(days => $4)
           AND m.sender_address IN (SELECT address FROM senders)
           AND (m.in_spam
                OR EXISTS (SELECT 1 FROM conversations.needs n
                            WHERE n.message_id = m.id AND n.reason = 'review'
                              AND (n.state = 'open' OR n.cleared_because = 'superseded'))
                OR (EXISTS (SELECT 1 FROM conversations.decisions d
                             WHERE d.message_id = m.id AND d.question = 'suspicious'
                               AND d.value = 'true'::jsonb)
                    AND NOT EXISTS (SELECT 1 FROM conversations.corrections c
                                     WHERE c.message_id = m.id AND c.question = 'suspicious'
                                       AND c.to_value = 'false' AND c.withdrawn_at IS NULL)))
    ),
    threads AS (
        SELECT r.address, r.conversation_id, min(r.sent_at) AS first_at
          FROM recent r
          JOIN senders s USING (address)
         GROUP BY r.address, r.conversation_id
    ),
    replied AS (
        SELECT t.address, count(*)::int AS replied
          FROM threads t
         WHERE EXISTS (SELECT 1 FROM conversations.messages o
                        WHERE o.conversation_id = t.conversation_id
                          AND o.inbox_id = ANY($1)
                          AND o.direction = 'outbound' AND o.sent_at > t.first_at)
         GROUP BY t.address
    )
    SELECT s.address,
           s.name,
           s.received,
           coalesce(r.replied, 0),
           s.newest_results
      FROM senders s
      LEFT JOIN replied r USING (address)
     WHERE s.address NOT IN (SELECT address FROM flagged)
     ORDER BY 4 DESC, 3 DESC, 1
     LIMIT $3
    ";

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
    let reachable = query.reachable.unwrap_or(false);
    let rows: Vec<Row> = sqlx::query_as(SENDERS)
        .bind(&inboxes)
        .bind(words.as_deref())
        .bind(i64::from(limit) + if reachable { REACHABLE_SLACK } else { 0 })
        .bind(WINDOW_DAYS)
        .bind(MAX_SCANNED)
        .bind(reachable)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    let senders = rows
        .into_iter()
        .filter(|(address, .., results)| !reachable || may_write_to(address, results.as_deref()))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .map(|(address, name, received, replied, _)| SenderView {
            address,
            name,
            received: u32::try_from(received).unwrap_or_default(),
            replied: u32::try_from(replied).unwrap_or_default(),
        })
        .collect();
    Ok(Json(SenderList { senders }))
}

/// Whether `address` is safe to write to, from facts alone: not a no-reply
/// address, and its newest mail (`results`, the provider's authentication
/// results) neither fails its sender's checks nor pretends to be them.
/// Absent results say nothing.
fn may_write_to(address: &str, results: Option<&str>) -> bool {
    let no_reply = Facts {
        from: address.to_lowercase(),
        ..Facts::default()
    }
    .no_reply_sender();
    !no_reply && !sender_checks_failed(address, results) && !dmarc_refused(results)
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

    use serde_json::json;

    use super::{may_write_to, words};
    use crate::{model::InboxRole, testkit::Kit};

    #[test]
    fn wildcards_are_escaped() {
        assert_eq!(words("50%_off a\\b"), vec!["50\\%\\_off", "a\\\\b"]);
    }

    #[test]
    fn who_may_be_written_to_is_read_from_facts() {
        let pass = "mx.google.com; dkim=pass header.i=@silva-contas.example header.s=s1; \
                    dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=silva-contas.example";
        let refused = "mx.google.com; spf=pass smtp.mailfrom=bounce@spoofer.example; \
                       dmarc=fail (p=REJECT sp=REJECT dis=REJECT) header.from=silva-contas.example";
        assert!(may_write_to("rita@silva-contas.example", Some(pass)));
        // No results say nothing (Zoho, IMAP).
        assert!(may_write_to("rita@silva-contas.example", None));
        assert!(!may_write_to("rita@silva-contas.example", Some(refused)));
        for address in [
            "no-reply@accounts.example",
            "noreply@shop.example",
            "do-not-reply@bank.example",
            "mailer-daemon@example.com",
        ] {
            assert!(!may_write_to(address, Some(pass)), "{address}");
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn reachable_leaves_out_who_one_should_not_write_to(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        // The phishing sender under review, and a no-reply address.
        kit.ingest(workspace, inbox, "needs").await;
        kit.ingest_value(
            workspace,
            inbox,
            &json!({"changes": [{"op": "upsert", "message": {
                "provider_message_id": "accounts-1",
                "from": {"name": "Accounts", "address": "no-reply@accounts.example"},
                "to": [{"address": "sam@example.com"}],
                "subject": "Your sign-in code",
                "sent_at": "2026-10-14T09:00:00Z",
                "direction": "inbound",
                "text": "Your code is 000000.",
                "mailbox": {"in_inbox": true},
                "read": false, "starred": false
            }}]}),
        )
        .await;
        let uri = format!("/api/v1/w/{workspace}/senders");
        let addresses = |json: &serde_json::Value| -> Vec<String> {
            json["senders"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["address"].as_str().unwrap().to_owned())
                .collect()
        };
        let left_out = [
            "it-support@at3lier-sol.example",
            "no-reply@accounts.example",
            "no-reply@accounts.cloudvault.example",
        ];
        // Without it, as before: anyone found by name.
        for (q, address) in [("support", left_out[0]), ("accounts", left_out[1])] {
            let all = kit.get(&sam, &format!("{uri}?q={q}")).await;
            assert!(addresses(&all.json).contains(&address.to_owned()), "{q}");
        }
        // With it: never, whether typed or not.
        let top = kit.get(&sam, &format!("{uri}?reachable=true")).await;
        assert_eq!(top.status, StatusCode::OK);
        let listed = addresses(&top.json);
        assert!(!listed.is_empty());
        assert!(listed.contains(&"maya@northwind-studio.example".to_owned()));
        for q in ["support", "at3lier", "accounts", "no-reply"] {
            let found = kit.get(&sam, &format!("{uri}?q={q}&reachable=true")).await;
            let found = addresses(&found.json);
            assert!(
                left_out.iter().all(|a| !found.contains(&(*a).to_owned())),
                "{q}: {found:?}"
            );
        }
        for address in left_out {
            assert!(!listed.contains(&address.to_owned()), "{address}");
        }
    }

    /// One inbound message from `from`, in thread `thread`, with the
    /// provider's authentication `results` when given.
    fn mail(
        id: &str,
        thread: &str,
        from: &str,
        sent_at: &str,
        results: Option<&str>,
    ) -> serde_json::Value {
        let mut message = json!({
            "provider_message_id": id,
            "thread_key": thread,
            "from": {"name": "Lumen Billing", "address": from},
            "to": [{"address": "sam@example.com"}],
            "subject": format!("Invoice {id}"),
            "sent_at": sent_at,
            "direction": "inbound",
            "text": "Your invoice is attached.",
            "mailbox": {"in_inbox": true},
            "read": false, "starred": false
        });
        if let Some(results) = results {
            message["headers"] = json!({"authentication_results": results});
        }
        json!({"op": "upsert", "message": message})
    }

    /// The addresses `list_senders` answers for `q`, with `reachable` or not.
    async fn found(
        kit: &Kit,
        sam: &str,
        workspace: WorkspaceId,
        q: &str,
        reachable: bool,
    ) -> Vec<String> {
        let reachable = if reachable { "&reachable=true" } else { "" };
        let response = kit
            .get(
                sam,
                &format!("/api/v1/w/{workspace}/senders?q={q}{reachable}"),
            )
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.json);
        response.json["senders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["address"].as_str().unwrap().to_owned())
            .collect()
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_reported_sender_stays_out_whatever_else_they_send(db: PgPool) {
        let kit = Kit::new(db).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let from = "billing@lumen-pay.example";
        kit.ingest_value(
            workspace,
            inbox,
            &json!({"changes": [
                mail("lumen-1", "thr-lumen-1", from, "2026-10-15T09:00:00Z", None),
                mail("lumen-2", "thr-lumen-2", from, "2026-10-16T09:00:00Z", None),
            ]}),
        )
        .await;
        assert_eq!(found(&kit, &sam, workspace, "lumen", true).await, [from]);
        // Reported: Report and delete moves that conversation to spam.
        let reported = kit.conversation_of("lumen-1").await;
        let spam = kit
            .post(
                &sam,
                &format!("/api/v1/w/{workspace}/conversations/{reported}/spam"),
                None,
            )
            .await;
        assert_eq!(spam.status, StatusCode::OK);
        // Their other, unflagged conversation doesn't bring them back.
        assert!(found(&kit, &sam, workspace, "lumen", true).await.is_empty());
        assert_eq!(found(&kit, &sam, workspace, "lumen", false).await, [from]);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_review_replaced_by_a_later_message_still_keeps_them_out(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        kit.ingest(workspace, inbox, "needs").await;
        let phish = "it-support@at3lier-sol.example";
        assert!(
            !found(&kit, &sam, workspace, "support", true)
                .await
                .contains(&phish.to_owned())
        );
        // The flagged message, `$1`.
        let flagged = "needs-4";
        // A follow-up replaced the review: it is cleared as superseded.
        sqlx::query(
            "UPDATE conversations.needs
                SET state = 'cleared', cleared_because = 'superseded', cleared_at = now()
              WHERE message_id =
                    (SELECT id FROM conversations.messages WHERE provider_message_id = $1)",
        )
        .bind(flagged)
        .execute(&db)
        .await
        .unwrap();
        assert!(
            !found(&kit, &sam, workspace, "support", true)
                .await
                .contains(&phish.to_owned())
        );
        // Rebuilt for a later message, the review leaves no trace in the
        // needs: the stored answer that it is suspicious still does.
        sqlx::query(
            "DELETE FROM conversations.needs WHERE message_id =
                 (SELECT id FROM conversations.messages WHERE provider_message_id = $1)",
        )
        .bind(flagged)
        .execute(&db)
        .await
        .unwrap();
        assert!(
            !found(&kit, &sam, workspace, "support", true)
                .await
                .contains(&phish.to_owned())
        );
        // A member said it is safe: they may be written to again.
        sqlx::query(
            "INSERT INTO conversations.corrections
                 (id, workspace_id, conversation_id, message_id, question, from_value,
                  to_value, overridden_source)
             SELECT gen_random_uuid(), workspace_id, conversation_id, id, 'suspicious',
                    'true', 'false', 'model'
               FROM conversations.messages WHERE provider_message_id = $1",
        )
        .bind(flagged)
        .execute(&db)
        .await
        .unwrap();
        assert!(
            found(&kit, &sam, workspace, "support", true)
                .await
                .contains(&phish.to_owned())
        );
    }

    /// Only the headers speak: whatever else was concluded about `from`'s
    /// messages (a suspicious answer, a review) is set aside.
    async fn headers_only(db: &PgPool, from: &str) {
        for sql in [
            "DELETE FROM conversations.needs WHERE message_id IN
                 (SELECT id FROM conversations.messages WHERE sender_address = $1)",
            "DELETE FROM conversations.decisions WHERE question = 'suspicious'
                AND message_id IN
                 (SELECT id FROM conversations.messages WHERE sender_address = $1)",
        ] {
            sqlx::query(sql).bind(from).execute(db).await.unwrap();
        }
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn a_sender_whose_newest_mail_fails_its_checks_is_left_out(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let from = "billing@lumen-pay.example";
        let pass = "mx.google.com; dkim=pass header.i=@lumen-pay.example header.s=s1; \
                    dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=lumen-pay.example";
        let refused = "mx.google.com; spf=pass smtp.mailfrom=bounce@spoofer.example; \
                       dmarc=fail (p=REJECT sp=REJECT dis=REJECT) header.from=lumen-pay.example";
        let batch = |id: &str, sent_at: &str, results: &str| json!({"changes": [mail(id, id, from, sent_at, Some(results))]});
        kit.ingest_value(
            workspace,
            inbox,
            &batch("lumen-1", "2026-10-15T09:00:00Z", pass),
        )
        .await;
        headers_only(&db, from).await;
        assert_eq!(found(&kit, &sam, workspace, "lumen", true).await, [from]);
        // Newest, pretending to be them: left out.
        kit.ingest_value(
            workspace,
            inbox,
            &batch("lumen-2", "2026-10-16T09:00:00Z", refused),
        )
        .await;
        headers_only(&db, from).await;
        assert!(found(&kit, &sam, workspace, "lumen", true).await.is_empty());
        assert_eq!(found(&kit, &sam, workspace, "lumen", false).await, [from]);
        // Their genuine mail again, newest: back.
        kit.ingest_value(
            workspace,
            inbox,
            &batch("lumen-3", "2026-10-17T09:00:00Z", pass),
        )
        .await;
        headers_only(&db, from).await;
        assert_eq!(found(&kit, &sam, workspace, "lumen", true).await, [from]);
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
    async fn reads_only_the_newest_mail(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, _, sam) = kit.sams_inbox().await;
        let uri = format!("/api/v1/w/{workspace}/senders");
        let maya = "maya@northwind-studio.example";

        let before = kit.get(&sam, &uri).await;
        assert_eq!(before.json["senders"][0]["address"], maya);
        sqlx::query(
            "UPDATE conversations.messages SET sent_at = now() - interval '2 years'
              WHERE sender_address = $1 AND direction = 'inbound'",
        )
        .bind(maya)
        .execute(&db)
        .await
        .unwrap();

        let after = kit.get(&sam, &uri).await;
        let senders = after.json["senders"].as_array().unwrap();
        assert!(!senders.is_empty());
        assert!(
            senders.iter().all(|sender| sender["address"] != maya),
            "mail older than the window isn't read"
        );
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
