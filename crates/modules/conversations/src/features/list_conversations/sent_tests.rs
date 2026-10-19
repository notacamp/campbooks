//! The Sent list (campbooks-cloud#240): conversations with a message you sent.

use axum::http::StatusCode;
use sqlx::PgPool;

use crate::{model::InboxId, testkit::Kit};

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

fn batch(messages: &[serde_json::Value]) -> serde_json::Value {
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
        &batch(&[
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
        &batch(&[message(
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
        &batch(&[
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
        &batch(&[
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
    sqlx::query("UPDATE conversations.conversations SET last_sent_at = NULL WHERE id = $1::uuid")
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
