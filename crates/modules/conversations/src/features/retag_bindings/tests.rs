//! A rotation of the instance secret key: what was bound under the old key
//! is re-tagged with the new one while the old is the previous key, and
//! keeps working once it is removed (campbooks-cloud#191).

use axum::http::StatusCode;
use campbooks_kernel::{
    binding::Binding,
    crypto::{SECRET_KEY_LEN, SecretKey},
    db::BeginFor as _,
    jobs::{self, DispatchRekeys, EnqueueOptions},
};
use sqlx::{PgPool, types::Uuid};

use super::{Tally, retag};
use crate::{
    domain::rules,
    features::rules::tests::{GITHUB, kit, move_github, sams_notifications, streams},
    model::RuleId,
    testkit::notifications,
};

/// The kit's key (`testing::binding`).
fn old() -> SecretKey {
    SecretKey::from_bytes([0x5a; SECRET_KEY_LEN])
}

fn new() -> SecretKey {
    SecretKey::from_bytes([0x6b; SECRET_KEY_LEN])
}

/// How many of the workspace's applications verify under `binding`.
async fn bound_applications(kit: &crate::testkit::Kit, binding: &Binding) -> usize {
    let rows: Vec<(
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        Option<Uuid>,
        serde_json::Value,
        Option<Vec<u8>>,
    )> = sqlx::query_as(
        "SELECT a.workspace_id, a.rule_id, r.inbox_id, a.conversation_id, a.run_id, a.before,
                    a.mac
               FROM conversations.rule_applications a
               JOIN conversations.rules r ON r.id = a.rule_id",
    )
    .fetch_all(&kit.db)
    .await
    .unwrap();
    rows.into_iter()
        .filter(|(workspace, rule, inbox, conversation, run, before, mac)| {
            let before: rules::Before = serde_json::from_value(before.clone()).unwrap();
            rules::applications::verifies(
                binding,
                (
                    *workspace,
                    crate::model::InboxId::from_uuid(*inbox),
                    RuleId::from_uuid(*rule),
                ),
                crate::model::ConversationId::from_uuid(*conversation),
                *run,
                (&before, mac.as_deref()),
            )
        })
        .count()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_rotated_key_retags_rules_and_their_changes_which_keep_working_without_it(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 5).await;
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    let rule_id: RuleId = rule.parse().unwrap();
    kit.run_jobs().await;
    let new_only = Binding::new(&new());
    let stored = |kit: &crate::testkit::Kit| {
        let pool = kit.state.db.clone();
        async move {
            let mut tx = pool.begin_for(workspace).await.unwrap();
            let rule = rules::find_any(&mut tx, rule_id).await.unwrap().unwrap();
            tx.commit().await.unwrap();
            rule
        }
    };
    // Bound under the old key: the new key alone would not take them.
    let generation = stored(&kit).await.generation;
    assert!(!stored(&kit).await.bound(&new_only));
    assert_eq!(bound_applications(&kit, &new_only).await, 0);
    assert_eq!(bound_applications(&kit, &Binding::new(&old())).await, 5);

    // The rotation: the new key, the old as the previous one. The kernel's
    // dispatcher enqueues the re-tag in every workspace with a change feed.
    let rotated = Binding::new(&new().with_previous(&old()));
    let kit = kit.with_binding(rotated.clone());
    sqlx::query(
        "INSERT INTO kernel.change_heads (workspace_id, head) VALUES ($1, 0)
         ON CONFLICT (workspace_id) DO NOTHING",
    )
    .bind(workspace.to_string())
    .execute(&kit.db)
    .await
    .unwrap();
    let mut tx = kit.state.db.begin().await.unwrap();
    jobs::enqueue(&mut tx, &DispatchRekeys, EnqueueOptions::default())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
    let rule_now = stored(&kit).await;
    assert!(rule_now.bound(&new_only), "the rule is re-tagged");
    assert_eq!(rule_now.generation, generation, "at the same generation");
    assert_eq!(bound_applications(&kit, &new_only).await, 5);
    // Idempotent: nothing left to re-tag.
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    assert_eq!(
        retag(&mut tx, &rotated, workspace).await.unwrap(),
        Tally::default()
    );
    tx.commit().await.unwrap();

    // The previous key removed: the rule still acts on new mail, and its
    // changes are still put back.
    let kit = kit.with_binding(new_only);
    kit.ingest_value(workspace, inbox, &notifications("after", 1, GITHUB, None))
        .await;
    let placed = streams(&kit).await;
    assert_eq!(
        placed.iter().find(|(id, ..)| id == "after-0").unwrap().1,
        "updates",
        "the rule still acts"
    );
    let removed = kit
        .delete(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}?restore=true"),
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json);
    assert_eq!(removed.json["run"]["kind"], "revert");
    kit.run_jobs().await;
    for (id, stream, _) in streams(&kit).await {
        if id.starts_with("gh-") || id.starts_with("after-") {
            assert_eq!(stream, "primary", "{id} is put back");
        }
    }
}
