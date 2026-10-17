use axum::http::StatusCode;
use sqlx::PgPool;

use crate::{model::InboxRole, testkit::Kit};

fn kinds(requested: &[serde_json::Value]) -> Vec<String> {
    requested
        .iter()
        .flat_map(|e| e["items"].as_array().unwrap().clone())
        .map(|i| i["change"]["kind"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn trash_takes_it_off_every_list_and_untrash_brings_it_back(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, _, sam) = kit.sams_inbox().await;
    let id = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}");
    let list = format!("/api/v1/w/{workspace}/conversations");
    let count = |v: &serde_json::Value| v["conversations"].as_array().unwrap().len();
    assert_eq!(count(&kit.get(&sam, &list).await.json), 3);

    let trashed = kit.post(&sam, &format!("{base}/trash"), None).await;
    assert_eq!(trashed.status, StatusCode::OK);
    assert_eq!(count(&kit.get(&sam, &list).await.json), 2);
    assert_eq!(
        count(&kit.get(&sam, &format!("{list}?archived=true")).await.json),
        0
    );
    // Twice is a no-op: nothing more asked of the provider.
    kit.post(&sam, &format!("{base}/trash"), None).await;
    let requested = kit.events("conversations.changes_requested").await;
    assert_eq!(kinds(&requested), ["trash", "trash"]);

    let back = kit.post(&sam, &format!("{base}/untrash"), None).await;
    assert_eq!(back.status, StatusCode::OK);
    assert_eq!(count(&kit.get(&sam, &list).await.json), 3);
    let requested = kit.events("conversations.changes_requested").await;
    assert_eq!(kinds(&requested), ["trash", "trash", "untrash", "untrash"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn readers_cannot_trash_and_strangers_see_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, _) = kit.sams_inbox().await;
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.add_to_inbox(inbox, ana_id, InboxRole::Reader).await;
    let (leo, _) = kit.member("leo", workspace);
    let id = kit.conversation_of("en-1").await;
    let base = format!("/api/v1/w/{workspace}/conversations/{id}");
    for action in ["trash", "untrash"] {
        let denied = kit.post(&ana, &format!("{base}/{action}"), None).await;
        assert_eq!(denied.status, StatusCode::FORBIDDEN, "{action}");
        assert_eq!(denied.json["code"], "inbox_read_only", "{action}");
        let hidden = kit.post(&leo, &format!("{base}/{action}"), None).await;
        assert_eq!(hidden.status, StatusCode::NOT_FOUND, "{action}");
    }
    assert!(
        kit.events("conversations.changes_requested")
            .await
            .is_empty()
    );
}
