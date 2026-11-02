use axum::http::StatusCode;
use campbooks_kernel::{operations::InvokeError, tenancy::WorkspaceId};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    events::MemberLeft,
    model::{InboxId, InboxRole},
    testkit::Kit,
};

fn uri(workspace: WorkspaceId, inbox: InboxId) -> String {
    format!("/api/v1/w/{workspace}/inboxes/{inbox}/signature")
}

async fn set(kit: &Kit, token: &str, workspace: WorkspaceId, inbox: InboxId, text: &str) -> Value {
    let done = kit
        .put(token, &uri(workspace, inbox), json!({"text": text}))
        .await;
    assert_eq!(done.status, StatusCode::OK, "{:?}", done.json);
    done.json
}

async fn read(kit: &Kit, token: &str, workspace: WorkspaceId, inbox: InboxId) -> String {
    let got = kit.get(token, &uri(workspace, inbox)).await;
    assert_eq!(got.status, StatusCode::OK, "{:?}", got.json);
    got.json["text"].as_str().unwrap().to_owned()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_signature_is_set_read_and_cleared(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    assert_eq!(read(&kit, &sam, workspace, inbox).await, "");
    set(&kit, &sam, workspace, inbox, "Sam Rivers · Rivers Lettings").await;
    assert_eq!(
        read(&kit, &sam, workspace, inbox).await,
        "Sam Rivers · Rivers Lettings"
    );
    // Changed, then cleared: one row at most, none when empty.
    set(
        &kit,
        &sam,
        workspace,
        inbox,
        "**Sam Rivers**\r\n\u{202e}Lettings",
    )
    .await;
    assert_eq!(
        read(&kit, &sam, workspace, inbox).await,
        "**Sam Rivers**\nLettings"
    );
    set(&kit, &sam, workspace, inbox, "  ").await;
    assert_eq!(read(&kit, &sam, workspace, inbox).await, "");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.signatures")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn in_a_shared_inbox_each_member_signs_alone(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.add_to_inbox(inbox, ana_id, InboxRole::Sender).await;
    set(&kit, &sam, workspace, inbox, "Sam Rivers").await;
    set(&kit, &ana, workspace, inbox, "Ana Costa").await;
    assert_eq!(read(&kit, &sam, workspace, inbox).await, "Sam Rivers");
    assert_eq!(read(&kit, &ana, workspace, inbox).await, "Ana Costa");
    // Ana changing hers leaves Sam's as it was.
    set(&kit, &ana, workspace, inbox, "Ana Costa, Office").await;
    assert_eq!(read(&kit, &sam, workspace, inbox).await, "Sam Rivers");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_limit_the_roles_and_the_members_only_rule(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    // 2000 characters fit, one more does not.
    set(&kit, &sam, workspace, inbox, &"a".repeat(2000)).await;
    let long = kit
        .put(
            &sam,
            &uri(workspace, inbox),
            json!({"text": "é".repeat(2001)}),
        )
        .await;
    assert_eq!(long.status, StatusCode::BAD_REQUEST, "{:?}", long.json);
    assert_eq!(long.json["code"], "signature_too_long");
    assert_eq!(read(&kit, &sam, workspace, inbox).await, "a".repeat(2000));

    // A reader may not keep one; someone outside the inbox does not see it.
    let (rui, rui_id) = kit.member("rui", workspace);
    let outside = kit.get(&rui, &uri(workspace, inbox)).await;
    assert_eq!(outside.status, StatusCode::NOT_FOUND, "{:?}", outside.json);
    kit.add_to_inbox(inbox, rui_id, InboxRole::Reader).await;
    for response in [
        kit.get(&rui, &uri(workspace, inbox)).await,
        kit.put(&rui, &uri(workspace, inbox), json!({"text": "x"}))
            .await,
    ] {
        assert_eq!(
            response.status,
            StatusCode::FORBIDDEN,
            "{:?}",
            response.json
        );
        assert_eq!(response.json["code"], "inbox_read_only");
    }

    // A client token acting for nobody sees no inbox: 404, as everywhere.
    let client = kit.client(workspace, "conversations:read conversations:send");
    for response in [
        kit.get(&client, &uri(workspace, inbox)).await,
        kit.put(&client, &uri(workspace, inbox), json!({"text": "x"}))
            .await,
    ] {
        assert_eq!(
            response.status,
            StatusCode::NOT_FOUND,
            "{:?}",
            response.json
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_assistant_cannot_read_or_write_a_signature(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    set(&kit, &sam, workspace, inbox, "Sam Rivers").await;
    for call in [
        ("conversations.get_signature", json!({"inbox_id": inbox})),
        (
            "conversations.set_signature",
            json!({"inbox_id": inbox, "text": "Hijacked"}),
        ),
    ] {
        for confirmed in [false, true] {
            let result = kit
                .as_assistant("sam", workspace, call.clone(), confirmed)
                .await;
            assert!(
                matches!(result, Err(InvokeError::NotAllowed { .. })),
                "{result:?}"
            );
        }
    }
    assert_eq!(read(&kit, &sam, workspace, inbox).await, "Sam Rivers");
}

/// A new draft for Sam from `inbox`, as it reads.
async fn new_draft(kit: &Kit, token: &str, workspace: WorkspaceId, inbox: InboxId) -> Value {
    let created = kit
        .post(
            token,
            &format!("/api/v1/w/{workspace}/drafts"),
            Some(json!({"kind": "new", "inbox_id": inbox, "subject": "Lease",
                "to": [{"address": "leo@example.com"}], "body": "See you Thursday."})),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    created.json
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_draft_keeps_the_signature_it_was_made_with(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    // Before there is one, a draft carries none.
    let unsigned = new_draft(&kit, &sam, workspace, inbox).await;
    assert_eq!(unsigned["signature"], "");
    set(&kit, &sam, workspace, inbox, "Sam Rivers").await;
    let draft = new_draft(&kit, &sam, workspace, inbox).await;
    assert_eq!(draft["signature"], "Sam Rivers");
    let path = format!(
        "/api/v1/w/{workspace}/drafts/{}",
        draft["id"].as_str().unwrap()
    );

    // Edited meanwhile: the draft started keeps what it showed.
    set(&kit, &sam, workspace, inbox, "Sam R., Director").await;
    assert_eq!(kit.get(&sam, &path).await.json["signature"], "Sam Rivers");

    // A save that does not name it leaves it; false leaves it off (a change);
    // false again changes nothing; true takes the current one.
    let shown = kit.get(&sam, &path).await.json;
    let same = kit
        .patch(
            &sam,
            &path,
            json!({"version": shown["version"], "content_token": shown["content_token"],
                "body": "Thursday it is."}),
        )
        .await;
    assert_eq!(same.json["signature"], "Sam Rivers", "{:?}", same.json);
    let off = kit
        .patch(
            &sam,
            &path,
            json!({"version": same.json["version"], "content_token": same.json["content_token"],
                "signature": false}),
        )
        .await;
    assert_eq!(off.status, StatusCode::OK, "{:?}", off.json);
    assert_eq!(off.json["signature"], "");
    assert_eq!(
        off.json["version"],
        same.json["version"].as_u64().unwrap() + 1
    );
    let back = kit
        .patch(
            &sam,
            &path,
            json!({"version": off.json["version"], "content_token": off.json["content_token"],
                "signature": true}),
        )
        .await;
    assert_eq!(
        back.json["signature"], "Sam R., Director",
        "{:?}",
        back.json
    );
    // The token follows the signature the draft shows: the old one is stale.
    assert_ne!(back.json["content_token"], off.json["content_token"]);
    let stale = kit
        .patch(
            &sam,
            &path,
            json!({"version": back.json["version"], "content_token": off.json["content_token"],
                "signature": false}),
        )
        .await;
    assert_eq!(stale.status, StatusCode::CONFLICT, "{:?}", stale.json);
}

/// The outgoing message document of the released draft `id`.
async fn sent_document(kit: &Kit, workspace: WorkspaceId, id: &str) -> Value {
    let id: sqlx::types::Uuid = id.parse().unwrap();
    let blob: Value =
        sqlx::query_scalar("SELECT send_document FROM conversations.drafts WHERE id = $1")
            .bind(id)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    let blob: campbooks_kernel::blobs::BlobRef = serde_json::from_value(blob).unwrap();
    serde_json::from_str::<Value>(&kit.read(workspace, &blob).await).unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn sending_carries_the_signature_the_draft_showed(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    set(
        &kit,
        &sam,
        workspace,
        inbox,
        "**Sam Rivers**\nRivers Lettings",
    )
    .await;
    let signed = new_draft(&kit, &sam, workspace, inbox).await;
    let left_off = new_draft(&kit, &sam, workspace, inbox).await;
    // Edited in Settings after the drafts were started.
    set(&kit, &sam, workspace, inbox, "Someone Else").await;
    let off_path = format!(
        "/api/v1/w/{workspace}/drafts/{}",
        left_off["id"].as_str().unwrap()
    );
    let off = kit
        .patch(
            &sam,
            &off_path,
            json!({"version": left_off["version"], "content_token": left_off["content_token"],
                "signature": false}),
        )
        .await;
    assert_eq!(off.status, StatusCode::OK);
    for (draft, shown) in [(&signed, signed.clone()), (&left_off, off.json.clone())] {
        let path = format!(
            "/api/v1/w/{workspace}/drafts/{}",
            draft["id"].as_str().unwrap()
        );
        let sent = kit
            .post(
                &sam,
                &format!("{path}/send"),
                Some(json!({"version": shown["version"], "content_token": shown["content_token"]})),
            )
            .await;
        assert_eq!(sent.status, StatusCode::ACCEPTED, "{:?}", sent.json);
    }
    sqlx::query(
        "UPDATE kernel.jobs SET run_at = now() WHERE kind = 'conversations.release_send' AND state = 'pending'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    kit.run_jobs().await;

    let with = sent_document(&kit, workspace, signed["id"].as_str().unwrap()).await;
    assert_eq!(
        with["text"],
        "See you Thursday.\n\n-- \n**Sam Rivers**\nRivers Lettings"
    );
    let html = with["html"].as_str().unwrap();
    assert!(
        html.contains(
            "<div class=\"signature\"><p><strong>Sam Rivers</strong><br>Rivers Lettings</p></div>"
        ),
        "{html}"
    );
    assert!(!html.contains("Someone Else"));
    let without = sent_document(&kit, workspace, left_off["id"].as_str().unwrap()).await;
    assert_eq!(without["text"], "See you Thursday.");
    assert!(!without["html"].as_str().unwrap().contains("signature"));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_departing_member_s_signatures_go(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, inbox, sam) = kit.sams_inbox().await;
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.add_to_inbox(inbox, ana_id, InboxRole::Sender).await;
    set(&kit, &sam, workspace, inbox, "Sam Rivers").await;
    set(&kit, &ana, workspace, inbox, "Ana Costa").await;
    let left = MemberLeft {
        workspace: workspace.to_string(),
        user: ana_id,
    };
    let mut tx = kit.state.db.begin().await.unwrap();
    campbooks_kernel::db::set_workspace(&mut tx, &workspace.to_string())
        .await
        .unwrap();
    kit.state.events.publish(&mut tx, &left).await.unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    let rows: Vec<String> = sqlx::query_scalar("SELECT text FROM conversations.signatures")
        .fetch_all(&kit.db)
        .await
        .unwrap();
    assert_eq!(rows, ["Sam Rivers"]);
}
