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
