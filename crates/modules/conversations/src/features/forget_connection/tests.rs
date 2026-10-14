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

#[sqlx::test(migrations = "../../../migrations")]
async fn a_batch_being_applied_finishes_before_its_inbox_goes(db: PgPool) {
    use campbooks_kernel::{blobs::BlobOwner, jobs::Worker};

    use crate::model::MessageId;

    let kit = Kit::new(db).await;
    let s = setup(&kit).await;
    let ws = s.workspace;
    let key = ws.to_string();
    // A batch in flight: it holds the inbox `FOR SHARE`, as `apply_changes`
    // does, and attaches a new file to one of its messages.
    let mut batch = kit.state.db.begin_for(ws).await.unwrap();
    sqlx::query("SELECT id FROM conversations.inboxes WHERE id = $1 FOR SHARE")
        .bind(s.inbox)
        .fetch_one(&mut *batch)
        .await
        .unwrap();
    let message: String = sqlx::query_scalar(
        "SELECT id::text FROM conversations.messages WHERE inbox_id = $1 LIMIT 1",
    )
    .bind(s.inbox)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let message: MessageId = message.parse().unwrap();
    let blob = kit
        .state
        .blobs
        .put(&key, b"a late part".to_vec(), "text/plain")
        .await
        .unwrap();
    kit.state
        .blobs
        .attach(
            &mut batch,
            &key,
            &blob,
            &BlobOwner::new("conversations.message", *message.as_uuid()),
        )
        .await
        .unwrap();

    // The connection is removed meanwhile: the purge waits for the batch.
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state
        .events
        .publish(&mut tx, &removed(ws, s.inbox, MailChoice::Delete))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let state = kit.state.clone();
    let purge = tokio::spawn(async move { Worker::new(state).run_until_idle().await });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(
        !purge.is_finished(),
        "the purge waits for the batch in flight"
    );
    batch.commit().await.unwrap();
    let stats = purge.await.unwrap();
    assert_eq!(stats.failed, 0, "{stats:?}");

    // Nothing the batch attached outlives the inbox.
    let refs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.blob_refs
          WHERE workspace_id = $1 AND owner_type LIKE 'conversations.%'",
    )
    .bind(&key)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(refs, 0);
}

/// The real `apply_changes` of batch `fr` caught mid-way (it holds the
/// inbox and waits on a table the test locks) while the connection is
/// removed with `mail`: the removal waits for the batch, then sees all of
/// it.
async fn a_batch_racing_its_removal(kit: &Kit, mail: MailChoice) -> Setup {
    use campbooks_kernel::jobs::Worker;

    let s = setup(kit).await;
    let ws = s.workspace;
    let batch = kit.batch(ws, s.inbox, "fr").await;
    // A new conversation can't be written until the test lets go.
    let mut gate = kit.db.begin().await.unwrap();
    sqlx::query("LOCK TABLE conversations.conversations IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *gate)
        .await
        .unwrap();
    crate::dev::submit(&kit.state, ws, &batch).await.unwrap();
    let state = kit.state.clone();
    let apply = tokio::spawn(async move { Worker::new(state).run_until_idle().await });
    // The batch is under way: it holds the inbox `FOR SHARE`.
    let holding = "SELECT count(*) FROM pg_locks l JOIN pg_stat_activity a ON a.pid = l.pid
                    WHERE l.relation = 'conversations.conversations'::regclass
                      AND NOT l.granted";
    for _ in 0..100 {
        let waiting: i64 = sqlx::query_scalar(holding)
            .fetch_one(&kit.db)
            .await
            .unwrap();
        if waiting > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state
        .events
        .publish(&mut tx, &removed(ws, s.inbox, mail))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let state = kit.state.clone();
    let remove = tokio::spawn(async move { Worker::new(state).run_until_idle().await });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(!apply.is_finished() && !remove.is_finished(), "both wait");
    gate.commit().await.unwrap();
    let (applied, removed) = (apply.await.unwrap(), remove.await.unwrap());
    assert_eq!(
        applied.failed + removed.failed,
        0,
        "{applied:?} {removed:?}"
    );
    s
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_real_batch_racing_a_delete_leaves_nothing_behind(db: PgPool) {
    let kit = Kit::new(db).await;
    let s = a_batch_racing_its_removal(&kit, MailChoice::Delete).await;
    let key = s.workspace.to_string();
    assert_eq!(
        count(
            &kit,
            "SELECT count(*) FROM conversations.messages WHERE inbox_id = $1",
            s.inbox
        )
        .await,
        0
    );
    let refs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.blob_refs
          WHERE workspace_id = $1 AND owner_type LIKE 'conversations.%'",
    )
    .bind(&key)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(refs, 0, "no file of the batch outlives its inbox");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_real_batch_racing_a_keep_lands_whole_then_nothing_more(db: PgPool) {
    let kit = Kit::new(db).await;
    let s = a_batch_racing_its_removal(&kit, MailChoice::Keep).await;
    let french = "SELECT count(*) FROM conversations.messages m
                    JOIN conversations.inboxes i ON i.id = m.inbox_id
                   WHERE m.inbox_id = $1 AND i.detached_at IS NOT NULL";
    let kept = count(&kit, french, s.inbox).await;
    assert!(kept > 0, "the batch landed before the inbox froze");
    // A batch after the freeze is dropped.
    kit.ingest(s.workspace, s.inbox, "es-ES").await;
    assert_eq!(count(&kit, french, s.inbox).await, kept);
}

/// Sam's inbox deleted with its connection while an admin removes Sam
/// keeping his mail: the deletion holds the inbox and is about to remove
/// its memberships; the departure asks which inboxes would be left unread.
/// Both take the inbox before its memberships, so the departure waits and
/// then finds nothing to keep: no deadlock.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_departure_keeping_mail_and_a_deletion_never_deadlock(db: PgPool) {
    use campbooks_kernel::jobs::Worker;

    let kit = Kit::new(db).await;
    let s = setup(&kit).await;
    let ws = s.workspace;
    let sam: UserId = sqlx::query_scalar("SELECT owner FROM conversations.inboxes WHERE id = $1")
        .bind(s.inbox)
        .fetch_one(&kit.db)
        .await
        .unwrap();
    // The deletion pauses once it holds the inbox (it lets go of files next).
    let mut gate = kit.db.begin().await.unwrap();
    sqlx::query("LOCK TABLE kernel.blob_refs IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *gate)
        .await
        .unwrap();
    let mut tx = kit.state.db.begin_for(ws).await.unwrap();
    kit.state
        .events
        .publish(&mut tx, &removed(ws, s.inbox, MailChoice::Delete))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let state = kit.state.clone();
    let deletion = tokio::spawn(async move { Worker::new(state).run_until_idle().await });
    let waiting = "SELECT count(*) FROM pg_locks
                    WHERE relation = 'kernel.blob_refs'::regclass AND NOT granted";
    for _ in 0..100 {
        let n: i64 = sqlx::query_scalar(waiting)
            .fetch_one(&kit.db)
            .await
            .unwrap();
        if n > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    // The departure, in its own transaction.
    let state = kit.state.clone();
    let departure = tokio::spawn(async move {
        let mut tx = state.db.begin_for(ws).await?;
        let unread = crate::kept_mail()
            .unread_if_kept(&mut tx, ws, sam, &[])
            .await?;
        tx.commit().await?;
        Ok::<_, campbooks_kernel::Error>(unread)
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(
        !departure.is_finished(),
        "the departure waits for the inbox"
    );
    gate.commit().await.unwrap();
    let stats = deletion.await.unwrap();
    assert_eq!(stats.failed, 0, "{stats:?}");
    let unread = departure.await.unwrap().expect("no deadlock");
    assert!(unread.is_empty(), "the inbox is gone: {unread:?}");
}
