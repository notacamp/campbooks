use axum::http::StatusCode;
use sqlx::PgPool;

use crate::{model::InboxRole, testkit::Kit};

#[sqlx::test(migrations = "../../../migrations")]
async fn report_moves_every_message_to_spam_and_not_spam_back(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}");
    let list = format!("/api/v1/w/{workspace}/conversations");
    let count = |v: &serde_json::Value| v["conversations"].as_array().unwrap().len();

    let reported = kit.post(&sam, &format!("{base}/spam"), None).await;
    assert_eq!(reported.status, StatusCode::OK);
    assert_eq!(count(&kit.get(&sam, &list).await.json), 2);
    let flags: Vec<(bool, bool)> = sqlx::query_as(
        "SELECT in_spam, in_inbox FROM conversations.messages WHERE conversation_id = $1::uuid",
    )
    .bind(&id)
    .fetch_all(&db)
    .await
    .unwrap();
    assert!(
        flags.iter().all(|&(spam, inbox)| spam && !inbox),
        "{flags:?}"
    );
    let requested = kit.events("conversations.changes_requested").await;
    assert_eq!(requested[0]["items"][0]["change"]["kind"], "spam");

    let back = kit.post(&sam, &format!("{base}/not-spam"), None).await;
    assert_eq!(back.status, StatusCode::OK);
    assert_eq!(back.json["archived"], false);
    assert_eq!(count(&kit.get(&sam, &list).await.json), 3);
    let requested = kit.events("conversations.changes_requested").await;
    assert_eq!(
        requested.last().unwrap()["items"][0]["change"]["kind"],
        "unspam"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn readers_cannot_report(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = kit.sams_inbox().await;
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
    let id = kit.conversation_of("en-1").await;
    let denied = kit
        .post(
            &ana,
            &format!("/api/v1/w/{workspace}/conversations/{id}/spam"),
            None,
        )
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert!(
        kit.events("conversations.changes_requested")
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn strangers_see_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, _) = kit.sams_inbox().await;
    let (leo, _) = kit.member("leo", workspace);
    let id = kit.conversation_of("en-1").await;
    for action in ["spam", "not-spam"] {
        let hidden = kit
            .post(
                &leo,
                &format!("/api/v1/w/{workspace}/conversations/{id}/{action}"),
                None,
            )
            .await;
        assert_eq!(hidden.status, StatusCode::NOT_FOUND, "{action}");
    }
    assert!(
        kit.events("conversations.changes_requested")
            .await
            .is_empty()
    );
}

/// `(in_inbox, in_trash, in_spam)` of each message of conversation `id`.
async fn places(db: &PgPool, id: &str) -> Vec<(bool, bool, bool)> {
    sqlx::query_as(
        "SELECT in_inbox, in_trash, in_spam FROM conversations.messages
          WHERE conversation_id = $1::uuid ORDER BY sent_at, id",
    )
    .bind(id)
    .fetch_all(db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn not_spam_puts_it_back_where_it_was(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}");
    // Archived first: not spam leaves it archived, never in the inbox.
    kit.post(&sam, &format!("{base}/archive"), None).await;
    let archived = places(&db, &id).await;
    kit.post(&sam, &format!("{base}/spam"), None).await;
    let back = kit.post(&sam, &format!("{base}/not-spam"), None).await;
    assert_eq!(back.json["archived"], true);
    assert_eq!(places(&db, &id).await, archived);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn trash_and_spam_exclude_each_other(db: PgPool) {
    let kit = Kit::new(db.clone()).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}");
    kit.post(&sam, &format!("{base}/spam"), None).await;
    kit.post(&sam, &format!("{base}/trash"), None).await;
    assert!(
        places(&db, &id)
            .await
            .iter()
            .all(|&(_, trash, spam)| trash && !spam),
        "{:?}",
        places(&db, &id).await
    );
    kit.post(&sam, &format!("{base}/spam"), None).await;
    assert!(
        places(&db, &id)
            .await
            .iter()
            .all(|&(_, trash, spam)| !trash && spam),
        "{:?}",
        places(&db, &id).await
    );
}
