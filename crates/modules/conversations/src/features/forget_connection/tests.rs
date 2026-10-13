//! A removed connection's mail is deleted or kept read-only; a departed
//! member keeps nothing.

use axum::http::StatusCode;
use campbooks_kernel::{
    DomainEvent,
    db::BeginFor as _,
    tenancy::{UserId, WorkspaceId},
};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    events::{ConnectionRemoved, MailChoice, MemberLeft},
    model::{ConnectionId, InboxId, InboxRole},
    testkit::{Kit, contract},
};

/// Sam's inbox (Sam manages it, Bea reads it) with the `en` fixture and a
/// draft of Sam's.
struct Setup {
    workspace: WorkspaceId,
    inbox: InboxId,
    sam: String,
    bea: String,
    bea_id: UserId,
    conversation: String,
    draft: String,
}

async fn setup(kit: &Kit) -> Setup {
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    let (bea, bea_id) = kit.member("bea", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam_id, InboxRole::Manager), (bea_id, InboxRole::Reader)],
        )
        .await;
    kit.ingest(workspace, inbox, "en").await;
    let conversation: String = sqlx::query_scalar(
        "SELECT conversation_id::text FROM conversations.messages
          WHERE provider_message_id = 'en-1' AND inbox_id = $1",
    )
    .bind(inbox)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let draft = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/drafts"),
            Some(
                json!({"kind": "new", "inbox_id": inbox, "to": [{"address": "leo@example.com"}],
                        "subject": "Hello", "body": "Hi Leo"}),
            ),
        )
        .await;
    assert_eq!(draft.status, StatusCode::CREATED, "{:?}", draft.json);
    Setup {
        workspace,
        inbox,
        sam,
        bea,
        bea_id,
        conversation,
        draft: draft.json["id"].as_str().unwrap().to_owned(),
    }
}

async fn publish<E: DomainEvent>(kit: &Kit, workspace: WorkspaceId, event: &E) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    kit.state.events.publish(&mut tx, event).await.unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

fn removed(workspace: WorkspaceId, inbox: InboxId, mail: MailChoice) -> ConnectionRemoved {
    let event = ConnectionRemoved {
        workspace: workspace.to_string(),
        connection: ConnectionId::new(),
        inbox_ids: vec![inbox],
        mail,
    };
    let (schemas, index) = contract("connectors.connection_removed.v1.schema.json");
    schemas
        .validate(&serde_json::to_value(&event).unwrap(), index)
        .unwrap();
    event
}

async fn count(kit: &Kit, sql: &'static str, inbox: InboxId) -> i64 {
    sqlx::query_scalar(sql)
        .bind(inbox)
        .fetch_one(&kit.db)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn deleting_a_connections_mail_leaves_nothing_behind(db: PgPool) {
    let kit = Kit::new(db).await;
    let s = setup(&kit).await;
    let other = setup(&kit).await; // another workspace's mail stays
    publish(
        &kit,
        s.workspace,
        &removed(s.workspace, s.inbox, MailChoice::Delete),
    )
    .await;

    for table in [
        "SELECT count(*) FROM conversations.inboxes WHERE id = $1",
        "SELECT count(*) FROM conversations.inbox_members WHERE inbox_id = $1",
        "SELECT count(*) FROM conversations.conversations WHERE inbox_id = $1",
        "SELECT count(*) FROM conversations.messages WHERE inbox_id = $1",
        "SELECT count(*) FROM conversations.drafts WHERE inbox_id = $1",
        "SELECT count(*) FROM conversations.labels WHERE inbox_id = $1",
    ] {
        assert_eq!(count(&kit, table, s.inbox).await, 0, "{table}");
    }
    let leftovers: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM conversations.decisions WHERE workspace_id = $1)
              + (SELECT count(*) FROM conversations.attachments WHERE workspace_id = $1)
              + (SELECT count(*) FROM kernel.blob_refs
                  WHERE workspace_id = $1::text AND owner_type LIKE 'conversations.%')",
    )
    .bind(s.workspace.as_uuid())
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(leftovers, 0);
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM conversations.removed_inboxes WHERE inbox_id = $1",
            s.inbox
        )
        .await,
        1
    );
    let gone = kit.events("conversations.inbox_removed").await;
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0]["inbox"], s.inbox.to_string());

    let ws = s.workspace;
    for token in [&s.sam, &s.bea] {
        let path = format!("/api/v1/w/{ws}/conversations/{}", s.conversation);
        assert_eq!(kit.get(token, &path).await.status, StatusCode::NOT_FOUND);
        let list = kit.get(token, &format!("/api/v1/w/{ws}/inboxes")).await;
        assert_eq!(list.json["inboxes"], json!([]));
    }
    let draft = kit
        .get(&s.sam, &format!("/api/v1/w/{ws}/drafts/{}", s.draft))
        .await;
    assert_eq!(draft.status, StatusCode::NOT_FOUND);

    // A batch still on its way, and a replayed grant, bring nothing back.
    kit.ingest(ws, s.inbox, "en").await;
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM conversations.messages WHERE inbox_id = $1",
            s.inbox
        )
        .await,
        0
    );
    // A replay of the removal is harmless.
    publish(&kit, ws, &removed(ws, s.inbox, MailChoice::Delete)).await;

    let kept = kit
        .get(
            &other.sam,
            &format!(
                "/api/v1/w/{}/conversations/{}",
                other.workspace, other.conversation
            ),
        )
        .await;
    assert_eq!(kept.status, StatusCode::OK);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn kept_mail_is_read_only_history(db: PgPool) {
    let kit = Kit::new(db).await;
    let s = setup(&kit).await;
    let ws = s.workspace;
    publish(&kit, ws, &removed(ws, s.inbox, MailChoice::Keep)).await;
    assert_eq!(kit.events("conversations.inbox_changed").await.len(), 1);

    // Members keep reading.
    let path = format!("/api/v1/w/{ws}/conversations/{}", s.conversation);
    assert_eq!(kit.get(&s.bea, &path).await.status, StatusCode::OK);
    let list = kit.get(&s.sam, &format!("/api/v1/w/{ws}/inboxes")).await;
    assert_eq!(list.json["inboxes"][0]["read_only"], true);
    // Nothing is written or sent.
    let archived = kit.post(&s.sam, &format!("{path}/archive"), None).await;
    assert_eq!(archived.status, StatusCode::FORBIDDEN);
    assert_eq!(archived.json["code"], "inbox_read_only");
    let draft = kit
        .post(
            &s.sam,
            &format!("/api/v1/w/{ws}/drafts"),
            Some(json!({"kind": "new", "inbox_id": s.inbox})),
        )
        .await;
    assert_eq!(draft.status, StatusCode::FORBIDDEN);
    let sent = kit
        .post(
            &s.sam,
            &format!("/api/v1/w/{ws}/drafts/{}/send", s.draft),
            Some(json!({"version": 1})),
        )
        .await;
    assert_eq!(sent.status, StatusCode::FORBIDDEN);
    // Nothing syncs into it.
    let before = count(
        &kit,
        "SELECT count(*) FROM conversations.messages WHERE inbox_id = $1",
        s.inbox,
    )
    .await;
    kit.ingest(ws, s.inbox, "pt-PT").await;
    let after = count(
        &kit,
        "SELECT count(*) FROM conversations.messages WHERE inbox_id = $1",
        s.inbox,
    )
    .await;
    assert_eq!(before, after);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_departed_member_keeps_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let s = setup(&kit).await;
    let ws = s.workspace;
    let beas_draft = kit
        .post(
            &s.bea,
            &format!("/api/v1/w/{ws}/drafts"),
            Some(json!({"kind": "new", "inbox_id": s.inbox})),
        )
        .await;
    // A reader cannot start one; give Bea the sender role first.
    assert_eq!(beas_draft.status, StatusCode::FORBIDDEN);
    sqlx::query("UPDATE conversations.inbox_members SET role = 'sender' WHERE user_id = $1")
        .bind(s.bea_id.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    let beas_draft = kit
        .post(
            &s.bea,
            &format!("/api/v1/w/{ws}/drafts"),
            Some(json!({"kind": "new", "inbox_id": s.inbox})),
        )
        .await;
    assert_eq!(beas_draft.status, StatusCode::CREATED);

    publish(
        &kit,
        ws,
        &MemberLeft {
            workspace: ws.to_string(),
            user: s.bea_id,
        },
    )
    .await;
    assert_eq!(kit.events("conversations.inbox_changed").await.len(), 1);
    let drafts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM conversations.drafts WHERE author = $1")
            .bind(s.bea_id.as_uuid())
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(drafts, 0);
    // Back in the workspace (the test membership still admits her), Bea sees
    // none of it: memberships are not restored by coming back.
    let path = format!("/api/v1/w/{ws}/conversations/{}", s.conversation);
    assert_eq!(kit.get(&s.bea, &path).await.status, StatusCode::NOT_FOUND);
    let list = kit.get(&s.bea, &format!("/api/v1/w/{ws}/inboxes")).await;
    assert_eq!(list.json["inboxes"], json!([]));
    // Sam is unaffected.
    assert_eq!(kit.get(&s.sam, &path).await.status, StatusCode::OK);
}
