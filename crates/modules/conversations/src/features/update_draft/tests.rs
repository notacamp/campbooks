//! Moving a draft to another inbox (campbooks-cloud#394).

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    model::{InboxId, InboxRole},
    testkit::Kit,
};

/// Sam's world: `personal` (manager, with mail), `office` (sender), `reading`
/// (reader), `others` (Sam is not in it) and an inbox of another workspace.
struct World {
    kit: Kit,
    workspace: WorkspaceId,
    sam: String,
    personal: InboxId,
    office: InboxId,
    reading: InboxId,
    others: InboxId,
    elsewhere: InboxId,
}

async fn world(db: PgPool) -> World {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let personal = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    let office = kit
        .inbox(
            workspace,
            "office@example.com",
            &[(sam_id, InboxRole::Sender)],
        )
        .await;
    let reading = kit
        .inbox(
            workspace,
            "reading@example.com",
            &[(sam_id, InboxRole::Reader)],
        )
        .await;
    let others = kit.inbox(workspace, "ana@example.com", &[]).await;
    let away = WorkspaceId::new();
    let elsewhere = kit.inbox(away, "away@example.com", &[]).await;
    kit.ingest(workspace, personal, "en").await;
    World {
        kit,
        workspace,
        sam,
        personal,
        office,
        reading,
        others,
        elsewhere,
    }
}

impl World {
    fn drafts(&self) -> String {
        format!("/api/v1/w/{}/drafts", self.workspace)
    }

    /// A reply to a received message, from `personal`.
    async fn reply(&self) -> (String, Value) {
        let message = self.kit.message_of("en-1").await;
        let made = self
            .kit
            .post(
                &self.sam,
                &self.drafts(),
                Some(json!({"kind": "reply", "message_id": message, "body": "Thursday works."})),
            )
            .await;
        assert_eq!(made.status, StatusCode::CREATED, "{:?}", made.json);
        let uri = format!("{}/{}", self.drafts(), made.json["id"].as_str().unwrap());
        let shown = self.kit.get(&self.sam, &uri).await.json;
        (uri, shown)
    }

    async fn sign(&self, inbox: InboxId, text: &str) {
        let done = self
            .kit
            .put(
                &self.sam,
                &format!("/api/v1/w/{}/inboxes/{inbox}/signature", self.workspace),
                json!({"text": text}),
            )
            .await;
        assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    }

    async fn move_to(
        &self,
        uri: &str,
        shown: &Value,
        inbox: InboxId,
    ) -> campbooks_kernel::testing::TestResponse {
        self.kit
            .patch(
                &self.sam,
                uri,
                json!({"version": shown["version"], "content_token": shown["content_token"],
                    "inbox_id": inbox}),
            )
            .await
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_moved_leaves_from_the_other_mailbox_with_its_reply_headers(db: PgPool) {
    let w = world(db).await;
    let (uri, shown) = w.reply().await;
    assert_eq!(shown["from"]["address"], "sam@example.com");
    let id: sqlx::types::Uuid = uri.rsplit('/').next().unwrap().parse().unwrap();
    let before: (Option<String>, Vec<String>, Option<String>) = sqlx::query_as(
        "SELECT in_reply_to, reference_ids, thread_key FROM conversations.drafts WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&w.kit.db)
    .await
    .unwrap();
    assert!(
        before.2.is_some(),
        "a reply starts with its provider thread"
    );

    let moved = w.move_to(&uri, &shown, w.office).await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    assert_eq!(moved.json["from"]["address"], "office@example.com");
    assert_eq!(moved.json["inbox_id"], w.office.to_string());
    assert_eq!(moved.json["body"], "Thursday works.");
    let after: (Option<String>, Vec<String>, Option<String>) = sqlx::query_as(
        "SELECT in_reply_to, reference_ids, thread_key FROM conversations.drafts WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&w.kit.db)
    .await
    .unwrap();
    assert_eq!((after.0, after.1), (before.0.clone(), before.1.clone()));
    assert_eq!(after.2, None, "the first mailbox's thread key is dropped");

    // Sent: from office@, with the original headers.
    let sent = w
        .kit
        .post(
            &w.sam,
            &format!("{uri}/send"),
            Some(json!({"version": moved.json["version"], "content_token": moved.json["content_token"]})),
        )
        .await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    sqlx::query(
        "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'conversations.release_send' AND state = 'pending'",
    )
    .execute(&w.kit.db)
    .await
    .unwrap();
    w.kit.run_jobs().await;
    let blob: Value =
        sqlx::query_scalar("SELECT send_document FROM conversations.drafts WHERE id = $1")
            .bind(id)
            .fetch_one(&w.kit.db)
            .await
            .unwrap();
    let blob: campbooks_kernel::blobs::BlobRef = serde_json::from_value(blob).unwrap();
    let document: Value = serde_json::from_str(&w.kit.read(w.workspace, &blob).await).unwrap();
    assert_eq!(document["from"]["address"], "office@example.com");
    assert_eq!(document["inbox"], w.office.to_string());
    assert_eq!(document["in_reply_to"].as_str(), before.0.as_deref());
    assert_eq!(document["references"], json!(before.1), "{document}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_an_inbox_the_author_may_send_from_takes_a_draft(db: PgPool) {
    let w = world(db).await;
    let (uri, shown) = w.reply().await;

    let reader = w.move_to(&uri, &shown, w.reading).await;
    assert_eq!(reader.status, StatusCode::FORBIDDEN, "{:?}", reader.json);
    assert_eq!(reader.json["code"], "inbox_read_only");
    for inbox in [w.others, w.elsewhere] {
        let refused = w.move_to(&uri, &shown, inbox).await;
        assert_eq!(refused.status, StatusCode::NOT_FOUND, "{:?}", refused.json);
    }
    // Nothing changed: same inbox, same version.
    let read = w.kit.get(&w.sam, &uri).await.json;
    assert_eq!(read["inbox_id"], w.personal.to_string());
    assert_eq!(read["version"], shown["version"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_token_from_before_a_move_is_refused(db: PgPool) {
    let w = world(db).await;
    let (uri, shown) = w.reply().await;
    let moved = w.move_to(&uri, &shown, w.office).await;
    assert_eq!(moved.status, StatusCode::OK);
    // Send naming what was on screen before the move: refused.
    let stale = w
        .kit
        .post(
            &w.sam,
            &format!("{uri}/send"),
            Some(
                json!({"version": moved.json["version"], "content_token": shown["content_token"]}),
            ),
        )
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT, "{:?}", stale.json);
    assert_eq!(stale.json["code"], "content_changed");
    // The same for a save.
    let save = w
        .kit
        .patch(
            &w.sam,
            &uri,
            json!({"version": moved.json["version"], "content_token": shown["content_token"],
                "body": "x"}),
        )
        .await;
    assert_eq!(save.json["code"], "content_changed", "{:?}", save.json);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_signature_follows_the_author_to_the_new_inbox_and_off_stays_off(db: PgPool) {
    let w = world(db).await;
    w.sign(w.personal, "Sam").await;
    w.sign(w.office, "Sam Rivers, Office").await;

    // Signed: switches to the one for the new inbox, and back.
    let (uri, shown) = w.reply().await;
    assert_eq!(shown["signature"], "Sam");
    let moved = w.move_to(&uri, &shown, w.office).await;
    assert_eq!(moved.json["signature"], "Sam Rivers, Office");
    let back = w.move_to(&uri, &moved.json, w.personal).await;
    assert_eq!(back.json["signature"], "Sam");

    // Left off: stays off.
    let off = w
        .kit
        .patch(
            &w.sam,
            &uri,
            json!({"version": back.json["version"], "content_token": back.json["content_token"],
                "signature": false}),
        )
        .await;
    let moved = w.move_to(&uri, &off.json, w.office).await;
    assert_eq!(moved.json["signature"], "");

    // None there: empty.
    let (other_uri, other) = w.reply().await;
    let bare = w
        .kit
        .patch(
            &w.sam,
            &other_uri,
            json!({"version": other["version"], "content_token": other["content_token"],
                "inbox_id": w.office}),
        )
        .await;
    assert_eq!(bare.json["signature"], "Sam Rivers, Office");
    sqlx::query("DELETE FROM conversations.signatures WHERE inbox_id = $1")
        .bind(w.personal)
        .execute(&w.kit.db)
        .await
        .unwrap();
    let (third_uri, third) = w.reply().await;
    assert_eq!(third["signature"], "");
    let still = w.move_to(&third_uri, &third, w.office).await;
    assert_eq!(still.json["signature"], "", "no signature to switch to");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_assistant_may_choose_from_and_it_sends_nothing(db: PgPool) {
    let w = world(db).await;
    let (uri, shown) = w.reply().await;
    let id = uri.rsplit('/').next().unwrap();
    let result = w
        .kit
        .as_assistant(
            "sam",
            w.workspace,
            (
                "conversations.update_draft",
                json!({"draft_id": id, "version": shown["version"],
                    "content_token": shown["content_token"], "inbox_id": w.office}),
            ),
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        result.body["inbox_id"],
        w.office.to_string(),
        "{}",
        result.body
    );
    assert_eq!(result.body["state"], "draft");
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.jobs WHERE kind = 'conversations.release_send'",
    )
    .fetch_one(&w.kit.db)
    .await
    .unwrap();
    assert_eq!(queued, 0);
    assert!(
        w.kit
            .events("conversations.send_requested")
            .await
            .is_empty()
    );
}

/// Where inboxes send from in these tests: the ones named carry at most the
/// bytes given (as Microsoft Graph does), the others what any mailbox does.
#[derive(Debug)]
struct Carries(std::collections::HashMap<sqlx::types::Uuid, u64>);

impl campbooks_kernel::outbound::OutboundDestinations for Carries {
    fn destination<'a>(
        &'a self,
        _conn: &'a mut sqlx::PgConnection,
        inbox: sqlx::types::Uuid,
    ) -> campbooks_kernel::outbound::DestinationFuture<'a> {
        let max = self.0.get(&inbox).copied();
        Box::pin(async move {
            Ok(Some(campbooks_kernel::outbound::Resolved {
                destination: campbooks_kernel::outbound::Destination {
                    connection: sqlx::types::Uuid::nil(),
                    stream: sqlx::types::Uuid::nil(),
                    mailbox: "inbox".into(),
                },
                as_at_deploy: true,
                max_attachment_bytes: max,
            }))
        })
    }
}

/// Attach `size` bytes to the draft at `uri` as `shown` showed it.
async fn attach(w: &World, uri: &str, shown: &Value, name: &str, size: usize) -> Value {
    let up = w
        .kit
        .upload(
            &w.sam,
            &format!(
                "{uri}/attachments?version={}&filename={name}",
                shown["version"]
            ),
            "application/octet-stream",
            vec![7; size],
            &[(
                "x-content-token",
                shown["content_token"].as_str().unwrap_or_default(),
            )],
        )
        .await;
    assert_eq!(up.status, StatusCode::CREATED, "{:?}", up.json);
    up.json
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_draft_with_more_than_the_new_mailbox_carries_does_not_move(db: PgPool) {
    let mut w = world(db).await;
    // office@ is a mailbox that carries about 2.1 MB; the others carry 25 MiB.
    let carries = std::collections::HashMap::from([(*w.office.as_uuid(), 2_100_000_u64)]);
    w.kit = w
        .kit
        .with_destinations(std::sync::Arc::new(Carries(carries)));
    let (uri, shown) = w.reply().await;
    // 20 MB of attachments, fine where it is.
    let mut now = shown;
    for n in 0..4 {
        now = attach(&w, &uri, &now, &format!("part{n}.bin"), 5_000_000).await;
    }
    assert_eq!(now["attachments"].as_array().unwrap().len(), 4);

    let refused = w.move_to(&uri, &now, w.office).await;
    assert_eq!(
        refused.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{:?}",
        refused.json
    );
    assert_eq!(refused.json["code"], "attachments_too_large_for_mailbox");
    assert!(
        refused.json["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("2.1 MB"),
        "{:?}",
        refused.json
    );
    // Unchanged: still in its inbox, still at its version, still all four files.
    let read = w.kit.get(&w.sam, &uri).await.json;
    assert_eq!(read["inbox_id"], w.personal.to_string());
    assert_eq!(read["version"], now["version"]);
    assert_eq!(read["attachments"].as_array().unwrap().len(), 4);

    // Dropping them in the same save lets it move; so does a mailbox that
    // carries the 20 MB (another inbox of Sam's).
    let keep_none = w
        .kit
        .patch(
            &w.sam,
            &uri,
            json!({"version": now["version"], "content_token": now["content_token"],
                "inbox_id": w.office, "attachments": []}),
        )
        .await;
    assert_eq!(keep_none.status, StatusCode::OK, "{:?}", keep_none.json);
    assert_eq!(keep_none.json["inbox_id"], w.office.to_string());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_small_draft_moves_to_a_mailbox_that_carries_less(db: PgPool) {
    let mut w = world(db).await;
    let carries = std::collections::HashMap::from([(*w.office.as_uuid(), 2_100_000_u64)]);
    w.kit = w
        .kit
        .with_destinations(std::sync::Arc::new(Carries(carries)));
    let (uri, shown) = w.reply().await;
    let now = attach(&w, &uri, &shown, "plans.bin", 1_000_000).await;
    let moved = w.move_to(&uri, &now, w.office).await;
    assert_eq!(moved.status, StatusCode::OK, "{:?}", moved.json);
    assert_eq!(moved.json["attachments"].as_array().unwrap().len(), 1);
}
