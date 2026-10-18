//! A release acts only on a draft still bound to its author's request
//! (campbooks-cloud#227). SQL as the runtime role in the workspace can plant
//! or edit a scheduled draft and plant a `release_send` job for it, but the
//! binding does not verify: nothing is written, the draft is left as it is,
//! and nothing is sent.

use axum::http::StatusCode;
use campbooks_kernel::{db::BeginFor as _, jobs::EnqueueOptions, tenancy::WorkspaceId};
use serde_json::json;
use sqlx::{PgPool, types::Uuid};

use super::{ReleaseSend, ReleaseSendPayload};
use crate::{
    model::{DraftId, InboxId, InboxRole},
    testkit::Kit,
};

/// A new draft from `inbox` to Leo, saying `body`, sent by `token`: scheduled,
/// its undo window still open.
async fn scheduled(
    kit: &Kit,
    workspace: WorkspaceId,
    inbox: InboxId,
    token: &str,
    body: &str,
) -> DraftId {
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let draft = kit
        .post(
            token,
            &uri,
            Some(json!({"kind": "new", "inbox_id": inbox, "subject": "Notes",
                "to": [{"name": "Leo", "address": "leo@example.com"}], "body": body})),
        )
        .await;
    assert_eq!(draft.status, StatusCode::CREATED, "{:?}", draft.json);
    let id: DraftId = draft.json["id"].as_str().unwrap().parse().unwrap();
    let uri = format!("{uri}/{id}");
    let shown = kit.get(token, &uri).await.json;
    let sent = kit
        .post(
            token,
            &format!("{uri}/send"),
            Some(json!({"version": shown["version"], "content_token": shown["content_token"]})),
        )
        .await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    id
}

/// Run `sql` (binding `draft`, then `value` when there is one) as the
/// runtime role in `workspace`: what an attacker with SQL in the workspace
/// can do.
async fn as_runtime(
    kit: &Kit,
    workspace: WorkspaceId,
    sql: &'static str,
    draft: DraftId,
    value: Option<Uuid>,
) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    let mut query = sqlx::query(sql).bind(draft);
    if let Some(value) = value {
        query = query.bind(value);
    }
    query.execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

/// Set `draft`'s stored request binding to `mac`, as the runtime role.
async fn set_mac(kit: &Kit, workspace: WorkspaceId, draft: DraftId, mac: Option<&[u8]>) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query("UPDATE conversations.drafts SET request_mac = $2 WHERE id = $1")
        .bind(draft)
        .bind(mac)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

/// The binding `draft` was scheduled with.
async fn mac_of(kit: &Kit, draft: DraftId) -> Option<Vec<u8>> {
    sqlx::query_scalar("SELECT request_mac FROM conversations.drafts WHERE id = $1")
        .bind(draft)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

/// Plant a `conversations.release_send` job for `draft` at `generation`,
/// due now, as the runtime role in `workspace`, and run the jobs.
async fn plant_release(kit: &Kit, workspace: WorkspaceId, draft: DraftId, generation: i32) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    campbooks_kernel::jobs::enqueue_to::<ReleaseSend>(
        &mut tx,
        &ReleaseSendPayload { draft, generation },
        EnqueueOptions::for_workspace(workspace.to_string()),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

/// Whether `draft` was released (`send_requested` names it).
async fn released(kit: &Kit, draft: DraftId) -> bool {
    let draft = draft.to_string();
    kit.events("conversations.send_requested")
        .await
        .iter()
        .any(|event| event["draft"] == draft.as_str())
}

/// `draft` as stored: its state, whether a send was minted, whether a
/// document was recorded.
async fn stored(kit: &Kit, draft: DraftId) -> (String, bool, bool) {
    sqlx::query_as(
        "SELECT state, send_id IS NOT NULL, send_document IS NOT NULL
           FROM conversations.drafts WHERE id = $1",
    )
    .bind(draft)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

/// Blobs stored in all.
async fn blobs(kit: &Kit) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM kernel.blobs")
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

/// Make every pending release due now and run the jobs.
async fn window_ends(kit: &Kit) {
    sqlx::query(
        "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'conversations.release_send' AND state = 'pending'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    kit.run_jobs().await;
}

/// Still scheduled, nothing minted or written: left as it was.
fn untouched() -> (String, bool, bool) {
    ("scheduled".to_owned(), false, false)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_planted_draft_is_never_released(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let real = scheduled(&kit, workspace, inbox, &sam, "Notes attached").await;
    let real_mac = mac_of(&kit, real).await.unwrap();
    let before = blobs(&kit).await;

    // A scheduled draft of the attacker's own, from the same inbox, with a
    // member who may send from it as its author and a matching generation.
    let forged = DraftId::new();
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(
        "INSERT INTO conversations.drafts
             (id, workspace_id, inbox_id, author, kind, to_recipients, subject, body, state,
              send_generation, send_at)
         SELECT $1, workspace_id, inbox_id, author, 'new',
                '[{\"address\": \"someone@example.net\"}]', 'Invoice', 'Pay to this account',
                'scheduled', 1, now()
           FROM conversations.drafts WHERE id = $2",
    )
    .bind(forged)
    .bind(real)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // No binding, a made-up one, and the real draft's: none verifies.
    let made_up = [1_u8; 33];
    for mac in [None, Some(&made_up[..]), Some(&real_mac[..])] {
        set_mac(&kit, workspace, forged, mac).await;
        plant_release(&kit, workspace, forged, 1).await;
        assert!(!released(&kit, forged).await, "{mac:?}");
        assert_eq!(stored(&kit, forged).await, untouched());
    }
    assert_eq!(blobs(&kit).await, before, "no document was written");

    // The real one leaves when its window ends; the planted one never does.
    window_ends(&kit).await;
    assert!(released(&kit, real).await);
    assert!(!released(&kit, forged).await);
    assert_eq!(stored(&kit, forged).await, untouched());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_binding_holds_only_for_its_draft_generation_inbox_and_author(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let one = scheduled(&kit, workspace, inbox, &sam, "One").await;
    let two = scheduled(&kit, workspace, inbox, &sam, "Two").await;
    let (mac_one, mac_two) = (mac_of(&kit, one).await, mac_of(&kit, two).await);
    assert!(mac_one.is_some() && mac_one != mac_two);
    let sam_id: Uuid = sqlx::query_scalar("SELECT author FROM conversations.drafts WHERE id = $1")
        .bind(one)
        .fetch_one(&kit.db)
        .await
        .unwrap();
    // Another inbox Sam may send from, and another member who may send
    // from this one.
    let other_inbox = kit
        .inbox(
            workspace,
            "sam.other@example.com",
            &[(
                campbooks_kernel::tenancy::UserId::from(sam_id),
                InboxRole::Manager,
            )],
        )
        .await;
    let (_, leo) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo, InboxRole::Manager).await;

    // The other draft's binding.
    set_mac(&kit, workspace, one, mac_two.as_deref()).await;
    plant_release(&kit, workspace, one, 1).await;
    assert!(!released(&kit, one).await);
    set_mac(&kit, workspace, one, mac_one.as_deref()).await;

    // Another generation (its binding was for the first), another inbox,
    // another author: each changed, released, and put back.
    let from = "UPDATE conversations.drafts SET inbox_id = $2 WHERE id = $1";
    let by = "UPDATE conversations.drafts SET author = $2 WHERE id = $1";
    for (tamper, changed, restore, original, generation) in [
        (
            "UPDATE conversations.drafts SET send_generation = 2 WHERE id = $1",
            None,
            "UPDATE conversations.drafts SET send_generation = 1 WHERE id = $1",
            None,
            2,
        ),
        (
            from,
            Some(*other_inbox.as_uuid()),
            from,
            Some(*inbox.as_uuid()),
            1,
        ),
        (by, Some(*leo.as_uuid()), by, Some(sam_id), 1),
    ] {
        as_runtime(&kit, workspace, tamper, one, changed).await;
        plant_release(&kit, workspace, one, generation).await;
        assert!(!released(&kit, one).await, "{tamper}");
        as_runtime(&kit, workspace, restore, one, original).await;
    }
    assert_eq!(stored(&kit, one).await, untouched());

    // Put back as Sam asked, both leave when their window ends.
    window_ends(&kit).await;
    assert!(released(&kit, one).await);
    assert!(released(&kit, two).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn editing_what_goes_out_breaks_the_binding(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let draft = scheduled(&kit, workspace, inbox, &sam, "Notes attached").await;
    let attachment = json!([{
        "id": Uuid::now_v7(), "filename": "invoice.pdf", "content_type": "application/pdf",
        "size": 10,
        "blob": {"hash": "ab".repeat(32), "size": 10, "content_type": "application/pdf"},
    }])
    .to_string();
    let edits: [(&'static str, Option<String>); 7] = [
        (
            "UPDATE conversations.drafts
                SET to_recipients = '[{\"address\": \"someone@example.net\"}]' WHERE id = $1",
            None,
        ),
        (
            "UPDATE conversations.drafts
                SET bcc_recipients = '[{\"address\": \"someone@example.net\"}]' WHERE id = $1",
            None,
        ),
        (
            "UPDATE conversations.drafts SET to_recipients = '[{\"name\": \"Accounts\", \"address\": \"leo@example.com\"}]' WHERE id = $1",
            None,
        ),
        (
            "UPDATE conversations.drafts SET body = 'Pay to this account' WHERE id = $1",
            None,
        ),
        (
            "UPDATE conversations.drafts SET subject = 'Invoice' WHERE id = $1",
            None,
        ),
        (
            "UPDATE conversations.drafts SET in_reply_to = 'x@example.net' WHERE id = $1",
            None,
        ),
        (
            "UPDATE conversations.drafts SET attachments = $2::jsonb WHERE id = $1",
            Some(attachment),
        ),
    ];
    let original: (String, String, String, String, Option<String>, String) = sqlx::query_as(
        "SELECT to_recipients::text, bcc_recipients::text, body, subject, in_reply_to,
                attachments::text
           FROM conversations.drafts WHERE id = $1",
    )
    .bind(draft)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    for (edit, value) in edits {
        let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
        let mut query = sqlx::query(edit).bind(draft);
        if let Some(value) = value {
            query = query.bind(value);
        }
        query.execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        plant_release(&kit, workspace, draft, 1).await;
        assert!(!released(&kit, draft).await, "{edit}");
        assert_eq!(stored(&kit, draft).await, untouched(), "{edit}");
        // Put back as signed.
        let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
        sqlx::query(
            "UPDATE conversations.drafts
                SET to_recipients = $2::jsonb, bcc_recipients = $3::jsonb, body = $4,
                    subject = $5, in_reply_to = $6, attachments = $7::jsonb
              WHERE id = $1",
        )
        .bind(draft)
        .bind(&original.0)
        .bind(&original.1)
        .bind(&original.2)
        .bind(&original.3)
        .bind(&original.4)
        .bind(&original.5)
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    // As Sam signed it, it leaves.
    window_ends(&kit).await;
    assert!(released(&kit, draft).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_binding_is_spent_once_released_or_cancelled(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let draft = scheduled(&kit, workspace, inbox, &sam, "Notes attached").await;
    let mac = mac_of(&kit, draft).await;
    assert!(mac.is_some());

    // Cancelled: the request no longer stands, and a release planted
    // with it put back finds a draft that is not scheduled.
    let uri = format!("/api/v1/w/{workspace}/drafts/{draft}");
    let undone = kit.post(&sam, &format!("{uri}/cancel_send"), None).await;
    assert_eq!(undone.status, StatusCode::OK, "{:?}", undone.json);
    assert_eq!(mac_of(&kit, draft).await, None);

    // Sent again: a new binding, for the new generation, spent at release
    // and replaced by the stored send's.
    let shown = kit.get(&sam, &uri).await.json;
    let resent = kit
        .post(
            &sam,
            &format!("{uri}/send"),
            Some(json!({"version": shown["version"], "content_token": shown["content_token"]})),
        )
        .await;
    assert_eq!(resent.status, StatusCode::ACCEPTED, "{:?}", resent.json);
    let again = mac_of(&kit, draft).await;
    assert!(again.is_some() && again != mac);
    window_ends(&kit).await;
    assert!(released(&kit, draft).await);
    let (request_mac, send_mac): (Option<Vec<u8>>, Option<Vec<u8>>) =
        sqlx::query_as("SELECT request_mac, send_mac FROM conversations.drafts WHERE id = $1")
            .bind(draft)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(request_mac, None);
    assert_eq!(
        send_mac.as_ref().map(Vec::len),
        Some(campbooks_kernel::binding::TAG_LEN)
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_key_nothing_is_scheduled_or_released(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    // Scheduled while the key was there; released where there is none.
    let draft = scheduled(&kit, workspace, inbox, &sam, "Notes attached").await;
    let kit = kit.keyless();
    window_ends(&kit).await;
    assert!(!released(&kit, draft).await);
    assert_eq!(stored(&kit, draft).await, untouched());

    // And no send is scheduled at all.
    let uri = format!("/api/v1/w/{workspace}/drafts");
    let other = kit
        .post(
            &sam,
            &uri,
            Some(json!({"kind": "new", "inbox_id": inbox, "subject": "Notes",
                "to": [{"address": "leo@example.com"}], "body": "Hi"})),
        )
        .await;
    assert_eq!(other.status, StatusCode::CREATED, "{:?}", other.json);
    let uri = format!("{uri}/{}", other.json["id"].as_str().unwrap());
    let refused = kit
        .post(&sam, &format!("{uri}/send"), Some(json!({"version": 1})))
        .await;
    assert_eq!(refused.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(kit.get(&sam, &uri).await.json["state"], "draft");
}
