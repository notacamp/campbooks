//! The rules kept from before rules were bound are signed once, only as the
//! migration found them (campbooks-cloud#242).

use axum::http::StatusCode;
use campbooks_kernel::{
    db::BeginFor as _,
    jobs::{self, EnqueueOptions},
};
use serde_json::json;
use sqlx::{PgPool, types::Uuid};

use super::{DispatchRuleResigns, ResignRules};
use crate::{
    features::rules::tests::{GITHUB, kit, sams_notifications},
    testkit::{Kit, notifications},
};

/// The migration's snapshot statement, from the repository, run again as
/// the migration's login: the rules as they are now are the rules "at the
/// deploy".
async fn snapshot(db: &PgPool) {
    let migration = std::fs::read_to_string(format!(
        "{}/../../../migrations/20261022000000_conversations__rule_bindings.sql",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let start = migration
        .find("INSERT INTO conversations.rule_resigns")
        .unwrap();
    let end = start + migration[start..].find(';').unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(migration[start..=end].to_owned()))
        .execute(db)
        .await
        .unwrap();
}

/// What the scheduler does every minute, then every job it causes.
async fn tick(kit: &Kit) {
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(&mut tx, &DispatchRuleResigns, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

/// Sam's rule doing `actions` to what `sender` sends, created through the
/// API, then stripped of its MAC: a rule stored before rules were bound.
async fn kept_from_before(
    kit: &Kit,
    sam: &str,
    at: (
        campbooks_kernel::tenancy::WorkspaceId,
        crate::model::InboxId,
    ),
    sender: &str,
    actions: serde_json::Value,
) -> String {
    let (workspace, inbox) = at;
    let created = kit
        .post(
            sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": [{"field": "sender_address", "value": sender}],
                "actions": actions,
                "apply_to_existing": false,
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    sqlx::query(
        "UPDATE conversations.rules SET mac = NULL, binding_generation = 0 WHERE id = $1::uuid",
    )
    .bind(&rule)
    .execute(&kit.db)
    .await
    .unwrap();
    rule
}

async fn needs_confirmation(kit: &Kit, sam: &str, workspace: &str, rule: &str) -> bool {
    let got = kit
        .get(sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    assert_eq!(got.status, StatusCode::OK, "{:?}", got.json);
    got.json["needs_confirmation"] == true
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_rules_kept_at_the_deploy_are_signed_only_as_they_were(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, sam_id) = sams_notifications(&kit, 1).await;
    let ws = workspace.to_string();
    // Two rules from before the binding: one stays as it was, one is changed
    // with SQL after the deploy (trash instead of mark read).
    let untouched = kept_from_before(
        &kit,
        &sam,
        (workspace, inbox),
        GITHUB,
        json!([{"action": "archive"}]),
    )
    .await;
    let edited = kept_from_before(
        &kit,
        &sam,
        (workspace, inbox),
        "marta@example.com",
        json!([{"action": "mark_read"}]),
    )
    .await;
    snapshot(&kit.db).await;
    assert!(needs_confirmation(&kit, &sam, &ws, &untouched).await);
    let mut attacker = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(
        "UPDATE conversations.rules SET actions = '[{\"action\": \"trash\"}]' WHERE id = $1::uuid",
    )
    .bind(&edited)
    .execute(&mut *attacker)
    .await
    .unwrap();
    attacker.commit().await.unwrap();

    // The runtime role cannot add a rule to the snapshot, nor change one.
    let planted = Uuid::now_v7();
    let mut attacker = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(
        "INSERT INTO conversations.rules
             (id, workspace_id, inbox_id, teaching_id, conditions, actions, specificity,
              origin, state, taught_by, confirmed_at)
         VALUES ($1, $2, $3, $4, '[{\"field\": \"sender_address\", \"value\": \"x@example.com\"}]',
                 '[{\"action\": \"spam\"}]', 2, 'action', 'active', $5, now())",
    )
    .bind(planted)
    .bind(workspace.as_uuid())
    .bind(inbox.as_uuid())
    .bind(Uuid::now_v7())
    .bind(sam_id.as_uuid())
    .execute(&mut *attacker)
    .await
    .unwrap();
    attacker.commit().await.unwrap();
    for sql in [
        "INSERT INTO conversations.rule_resigns (rule_id, workspace_id, content_hash)
         VALUES ($1, $2, sha256('x'::bytea))",
        "UPDATE conversations.rule_resigns SET content_hash = sha256('x'::bytea)
          WHERE workspace_id = $2 OR rule_id = $1",
        "DELETE FROM conversations.rule_resigns WHERE workspace_id = $2 OR rule_id = $1",
    ] {
        let mut attacker = kit.state.db.begin_for(workspace).await.unwrap();
        let refused = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(planted)
            .bind(workspace.as_uuid())
            .execute(&mut *attacker)
            .await;
        assert!(
            refused.is_err(),
            "the runtime role wrote the snapshot: {sql}"
        );
    }
    // Nor a job planted for it: its payload names nothing, and the job takes
    // only what the snapshot holds.
    let mut attacker = kit.state.db.begin_for(workspace).await.unwrap();
    jobs::enqueue(
        &mut attacker,
        &ResignRules,
        EnqueueOptions::for_workspace(ws.clone()),
    )
    .await
    .unwrap();
    attacker.commit().await.unwrap();
    kit.run_jobs().await;
    tick(&kit).await;

    assert!(!needs_confirmation(&kit, &sam, &ws, &untouched).await);
    assert!(needs_confirmation(&kit, &sam, &ws, &edited).await);
    assert!(needs_confirmation(&kit, &sam, &ws, &planted.to_string()).await);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.rule_resigns")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(left, 0, "each snapshotted rule is offered once");

    // The signed rule acts on new mail; the edited and planted ones do not.
    let before = kit.events("conversations.changes_requested").await.len();
    kit.ingest_value(workspace, inbox, &notifications("late", 1, GITHUB, None))
        .await;
    kit.ingest_value(
        workspace,
        inbox,
        &json!({"changes": [
            {"op": "upsert", "message": {
                "provider_message_id": "marta-2", "thread_key": "thr-marta-2",
                "from": {"name": "Marta Ferreira", "address": "marta@example.com"},
                "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
                "subject": "Friday", "sent_at": "2026-10-03T09:00:00Z",
                "direction": "inbound", "text": "See you Friday.",
                "mailbox": {"in_inbox": true}, "read": false, "starred": false
            }},
            {"op": "upsert", "message": {
                "provider_message_id": "x-1", "thread_key": "thr-x-1",
                "from": {"name": "X", "address": "x@example.com"},
                "to": [{"name": "Sam Rivers", "address": "sam@example.com"}],
                "subject": "Hello", "sent_at": "2026-10-03T10:00:00Z",
                "direction": "inbound", "text": "Hello.",
                "mailbox": {"in_inbox": true}, "read": false, "starred": false
            }}
        ]}),
    )
    .await;
    let flags = |id: &'static str| {
        let db = kit.db.clone();
        async move {
            sqlx::query_as::<_, (bool, bool, bool, bool)>(
                "SELECT in_trash, in_spam, in_inbox, read FROM conversations.messages
                  WHERE provider_message_id = $1",
            )
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    assert_eq!(
        flags("late-0").await,
        (false, false, false, false),
        "archived"
    );
    assert_eq!(flags("marta-2").await, (false, false, true, false));
    assert_eq!(flags("x-1").await, (false, false, true, false));
    let kinds: Vec<String> = kit
        .events("conversations.changes_requested")
        .await
        .iter()
        .skip(before)
        .filter_map(|e| {
            e["items"][0]["change"]["kind"]
                .as_str()
                .map(ToOwned::to_owned)
        })
        .collect();
    assert_eq!(kinds, vec!["archive".to_owned()]);

    // Ticking again changes nothing: the snapshot is spent.
    tick(&kit).await;
    assert!(needs_confirmation(&kit, &sam, &ws, &edited).await);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_key_the_snapshot_waits(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 1).await;
    let rule = kept_from_before(
        &kit,
        &sam,
        (workspace, inbox),
        GITHUB,
        json!([{"action": "archive"}]),
    )
    .await;
    snapshot(&kit.db).await;
    let keyless = kit.keyless();
    tick(&keyless).await;
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM conversations.rule_resigns")
        .fetch_one(&keyless.db)
        .await
        .unwrap();
    assert_eq!(left, 1, "nothing is taken without a key");
    let unsigned: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT mac FROM conversations.rules WHERE id = $1::uuid")
            .bind(&rule)
            .fetch_one(&keyless.db)
            .await
            .unwrap();
    assert!(unsigned.is_none());
}
