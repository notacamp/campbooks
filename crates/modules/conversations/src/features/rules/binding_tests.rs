//! Rules bound to the member who taught them (campbooks-cloud#242): a rule
//! planted or changed with SQL as the runtime role changes nothing, and asks
//! the provider for nothing; what members teach still acts.

use axum::http::StatusCode;
use campbooks_kernel::{db::BeginFor as _, jobs, tenancy::WorkspaceId};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use super::tests::{GITHUB, kit, move_github, sams_notifications};
use crate::{
    domain::rule_runs::RunBatch,
    features::apply_rule::ApplyRules,
    model::{InboxId, InboxRole, RuleRunId},
    testkit::{Kit, notifications},
};

/// Every action a rule may ask the provider for, as `changes_requested`
/// names them.
async fn requested(kit: &Kit) -> Vec<String> {
    kit.events("conversations.changes_requested")
        .await
        .iter()
        .filter_map(|e| {
            e["items"][0]["change"]["kind"]
                .as_str()
                .map(ToOwned::to_owned)
        })
        .collect()
}

/// `(in_trash, in_spam, in_inbox, read)` of message `provider_message_id`.
async fn flags(kit: &Kit, provider_message_id: &str) -> (bool, bool, bool, bool) {
    sqlx::query_as(
        "SELECT in_trash, in_spam, in_inbox, read FROM conversations.messages
          WHERE provider_message_id = $1",
    )
    .bind(provider_message_id)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

/// Run `sql` on rule `id` as the runtime role in `workspace`: what an
/// attacker with SQL inside the workspace can do.
async fn as_runtime_role(kit: &Kit, workspace: WorkspaceId, sql: &str, id: Uuid) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

/// Sam's rule archiving what GitHub sends, created through the API.
async fn archive_github(kit: &Kit, sam: &str, workspace: WorkspaceId, inbox: InboxId) -> Uuid {
    let created = kit
        .post(
            sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": [{"field": "sender_address", "value": GITHUB}],
                "actions": [{"action": "archive"}],
                "apply_to_existing": false,
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    assert_eq!(created.json["rule"]["needs_confirmation"], false);
    created.json["rule"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

/// Plant an active rule with SQL: `conditions` → `actions`, taught by
/// `author`, with `mac` (or none).
async fn plant(
    kit: &Kit,
    workspace: WorkspaceId,
    inbox: InboxId,
    author: Uuid,
    parts: (Value, Value),
    mac: Option<Vec<u8>>,
) -> Uuid {
    let id = Uuid::now_v7();
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(
        "INSERT INTO conversations.rules
             (id, workspace_id, inbox_id, teaching_id, conditions, actions, specificity,
              origin, state, taught_by, confirmed_at, mac)
         VALUES ($1, $2, $3, $4, $5, $6, 2, 'action', 'active', $7, now(), $8)",
    )
    .bind(id)
    .bind(workspace.as_uuid())
    .bind(inbox.as_uuid())
    .bind(Uuid::now_v7())
    .bind(parts.0)
    .bind(parts.1)
    .bind(author)
    .bind(mac)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    id
}

fn from_github(actions: Value) -> (Value, Value) {
    (
        json!([{"field": "sender_address", "value": GITHUB}]),
        actions,
    )
}

/// A planted rule run by a planted `apply_rule` job: its run row and batch.
async fn plant_run(kit: &Kit, workspace: WorkspaceId, rule: Uuid) {
    let run = RuleRunId::new();
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(
        "INSERT INTO conversations.rule_runs (id, workspace_id, rule_id, kind, state, counted)
         VALUES ($1, $2, $3, 'apply', 'running', true)",
    )
    .bind(run.as_uuid())
    .bind(workspace.as_uuid())
    .bind(rule)
    .execute(&mut *tx)
    .await
    .unwrap();
    let batch = RunBatch {
        run,
        after: None,
        counting: false,
    };
    jobs::enqueue_to::<ApplyRules>(&mut tx, &batch, batch.options(&workspace.to_string()))
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_rule_planted_with_sql_changes_nothing_at_the_provider(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, sam_id) = sams_notifications(&kit, 3).await;
    let before = requested(&kit).await.len();
    // "Trash everything from GitHub", taught by Sam (a manager), with no
    // MAC, a made-up one, or one that is not even a tag.
    let mut forged = vec![1_u8];
    forged.extend([7_u8; 32]);
    let mut planted = Vec::new();
    for mac in [None, Some(forged), Some(vec![0_u8; 3])] {
        planted.push(
            plant(
                &kit,
                workspace,
                inbox,
                *sam_id.as_uuid(),
                from_github(json!([{"action": "trash"}, {"action": "mark_read"}])),
                mac,
            )
            .await,
        );
    }
    // Its run, planted as well, over the mail already here.
    plant_run(&kit, workspace, planted[0]).await;
    kit.run_jobs().await;
    // New mail arrives: ingest succeeds, and the rule does nothing.
    kit.ingest_value(workspace, inbox, &notifications("late", 2, GITHUB, None))
        .await;
    for id in ["gh-0", "gh-1", "gh-2", "late-0", "late-1"] {
        assert_eq!(
            flags(&kit, id).await,
            (false, false, true, false),
            "{id} was changed"
        );
    }
    assert_eq!(
        requested(&kit).await.len(),
        before,
        "a change was requested"
    );
    let run: String = sqlx::query_scalar("SELECT state FROM conversations.rule_runs")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(run, "cancelled");
    // Listed as waiting for a member to confirm it, never as acting quietly.
    let listed = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules?inbox={inbox}"))
        .await;
    let rules = listed.json["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 3);
    assert!(rules.iter().all(|r| r["needs_confirmation"] == true));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_signed_rule_changed_with_sql_stops_acting(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, sam_id) = sams_notifications(&kit, 1).await;
    // Leo may teach in the inbox too: a rule of his would act.
    let (_, leo) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo, InboxRole::Sender).await;
    let ops = kit
        .inbox(
            workspace,
            "ops@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    let set_author = format!(
        "UPDATE conversations.rules SET taught_by = '{}' WHERE id = $1",
        leo.as_uuid()
    );
    let set_inbox = format!(
        "UPDATE conversations.rules SET inbox_id = '{}' WHERE id = $1",
        ops.as_uuid()
    );
    // Each way SQL can change a signed rule ("archive what GitHub sends").
    let edits: [(&str, &str); 8] = [
        (
            "actions",
            "UPDATE conversations.rules SET actions = '[{\"action\": \"trash\"}]' WHERE id = $1",
        ),
        (
            "conditions",
            "UPDATE conversations.rules
                SET conditions = '[{\"field\": \"sender_domain\", \"value\": \"github.com\"}]'
              WHERE id = $1",
        ),
        ("author", set_author.as_str()),
        ("inbox", set_inbox.as_str()),
        (
            "precedence",
            "UPDATE conversations.rules SET confirmed_at = now() + interval '1 day' WHERE id = $1",
        ),
        (
            "generation",
            "UPDATE conversations.rules SET binding_generation = binding_generation + 1
              WHERE id = $1",
        ),
        // Sam paused it; SQL lets it act again.
        (
            "paused",
            "UPDATE conversations.rules SET state = 'active' WHERE id = $1",
        ),
        // Sam removed it; SQL brings it back.
        (
            "removed",
            "UPDATE conversations.rules
                SET state = 'active', removed_at = NULL, removed_from = NULL WHERE id = $1",
        ),
    ];
    for (i, (what, sql)) in edits.iter().enumerate() {
        let rule = archive_github(&kit, &sam, workspace, inbox).await;
        let one = format!("/api/v1/w/{workspace}/rules/{rule}");
        match *what {
            "paused" => {
                let paused = kit.patch(&sam, &one, json!({"paused": true})).await;
                assert_eq!(paused.status, StatusCode::OK, "{:?}", paused.json);
            }
            "removed" => {
                let removed = kit.delete(&sam, &one).await;
                assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json);
            }
            _ => {}
        }
        as_runtime_role(&kit, workspace, sql, rule).await;
        let before = requested(&kit).await.len();
        let prefix = format!("edit{i}");
        let target = if *what == "inbox" { ops } else { inbox };
        kit.ingest_value(workspace, target, &notifications(&prefix, 1, GITHUB, None))
            .await;
        assert_eq!(
            flags(&kit, &format!("{prefix}-0")).await,
            (false, false, true, false),
            "{what}: the changed rule acted"
        );
        assert_eq!(requested(&kit).await.len(), before, "{what}");
        let got = kit.get(&sam, &one).await;
        assert_eq!(
            got.json["needs_confirmation"], true,
            "{what}: {:?}",
            got.json
        );
        // Out of the way of the next round's same conditions.
        let gone = kit.delete(&sam, &one).await;
        assert_eq!(gone.status, StatusCode::OK, "{what}: {:?}", gone.json);
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_mac_copied_onto_another_rule_inbox_or_workspace_does_not_verify(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, sam_id) = sams_notifications(&kit, 1).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    // Sam's rule: mark what GitHub sends read. Its MAC is read, then Sam
    // removes it, so it no longer acts.
    let created = kit
        .post(
            &sam,
            &rules,
            Some(json!({
                "conditions": [{"field": "sender_address", "value": GITHUB}],
                "actions": [{"action": "mark_read"}],
                "apply_to_existing": false,
            })),
        )
        .await;
    let real = created.json["rule"]["id"].as_str().unwrap().to_owned();
    let (mac, generation, confirmed): (Vec<u8>, i64, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as(
            "SELECT mac, binding_generation, confirmed_at FROM conversations.rules
              WHERE id = $1::uuid",
        )
        .bind(&real)
        .fetch_one(&kit.db)
        .await
        .unwrap();
    kit.delete(&sam, &format!("/api/v1/w/{workspace}/rules/{real}"))
        .await;
    // Word for word the same rule, as another rule of the inbox, with the
    // same author, generation, precedence and MAC.
    let copy = plant(
        &kit,
        workspace,
        inbox,
        *sam_id.as_uuid(),
        from_github(json!([{"action": "mark_read"}])),
        Some(mac.clone()),
    )
    .await;
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(
        "UPDATE conversations.rules SET binding_generation = $2, confirmed_at = $3 WHERE id = $1",
    )
    .bind(copy)
    .bind(generation)
    .bind(confirmed)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    // Another inbox of the workspace, and another workspace.
    let ops = kit
        .inbox(
            workspace,
            "ops@example.com",
            &[(sam_id, InboxRole::Manager)],
        )
        .await;
    plant(
        &kit,
        workspace,
        ops,
        *sam_id.as_uuid(),
        from_github(json!([{"action": "mark_read"}])),
        Some(mac.clone()),
    )
    .await;
    let elsewhere = WorkspaceId::new();
    let (eve, eve_id) = kit.member("eve", elsewhere);
    let eves = kit
        .inbox(
            elsewhere,
            "eve@example.com",
            &[(eve_id, InboxRole::Manager)],
        )
        .await;
    plant(
        &kit,
        elsewhere,
        eves,
        *eve_id.as_uuid(),
        from_github(json!([{"action": "mark_read"}])),
        Some(mac),
    )
    .await;
    let before = requested(&kit).await.len();
    kit.ingest_value(workspace, inbox, &notifications("late", 1, GITHUB, None))
        .await;
    kit.ingest_value(workspace, ops, &notifications("ops", 1, GITHUB, None))
        .await;
    kit.ingest_value(elsewhere, eves, &notifications("eve", 1, GITHUB, None))
        .await;
    for id in ["late-0", "ops-0", "eve-0"] {
        assert_eq!(flags(&kit, id).await, (false, false, true, false), "{id}");
    }
    assert_eq!(requested(&kit).await.len(), before);
    let listed = kit.get(&eve, &format!("/api/v1/w/{elsewhere}/rules")).await;
    assert_eq!(listed.json["rules"][0]["needs_confirmation"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn what_members_teach_change_and_confirm_still_acts(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 1).await;
    let rules = format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules");
    // Taught, then changed by Sam: archive and mark read.
    let created = kit.post(&sam, &rules, Some(move_github())).await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    let changed = kit
        .patch(
            &sam,
            &one,
            json!({"actions": [{"action": "archive"}, {"action": "mark_read"}]}),
        )
        .await;
    assert_eq!(changed.status, StatusCode::OK, "{:?}", changed.json);
    assert_eq!(changed.json["needs_confirmation"], false);
    // Paused and resumed: still bound.
    for paused in [true, false] {
        let toggled = kit.patch(&sam, &one, json!({"paused": paused})).await;
        assert_eq!(toggled.status, StatusCode::OK, "{:?}", toggled.json);
        assert_eq!(toggled.json["needs_confirmation"], false);
    }
    // Removed and brought back (Undo): still bound.
    let removed = kit.delete(&sam, &one).await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json);
    let restored = kit
        .post(&sam, &format!("{one}/restore"), Some(json!({})))
        .await;
    assert_eq!(restored.status, StatusCode::OK, "{:?}", restored.json);
    assert_eq!(restored.json["needs_confirmation"], false);
    kit.ingest_value(workspace, inbox, &notifications("late", 1, GITHUB, None))
        .await;
    let (_, _, in_inbox, read) = flags(&kit, "late-0").await;
    assert!(!in_inbox && read, "the rule acts on arrival");
    let kinds = requested(&kit).await;
    assert!(kinds.contains(&"archive".to_owned()), "{kinds:?}");
    assert!(kinds.contains(&"mark_read".to_owned()), "{kinds:?}");
    // Each change is recorded with its own MAC, for the connector's check.
    let unbound: i64 =
        sqlx::query_scalar("SELECT count(*) FROM conversations.change_requests WHERE mac IS NULL")
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(unbound, 0);

    // Changed with SQL: only a pause is taken, until a member confirms it.
    as_runtime_role(
        &kit,
        workspace,
        "UPDATE conversations.rules SET actions = '[{\"action\": \"spam\"}]' WHERE id = $1",
        rule.parse().unwrap(),
    )
    .await;
    let refused = kit
        .patch(&sam, &one, json!({"actions": [{"action": "archive"}]}))
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{:?}", refused.json);
    assert_eq!(refused.json["code"], "rule_needs_confirmation");
    let resumed = kit.patch(&sam, &one, json!({"adopt": true})).await;
    assert_eq!(resumed.status, StatusCode::CONFLICT, "{:?}", resumed.json);
    let paused = kit.patch(&sam, &one, json!({"paused": true})).await;
    assert_eq!(paused.status, StatusCode::OK, "{:?}", paused.json);
    assert_eq!(paused.json["needs_confirmation"], true);
    // Sam looks at it (it now reports spam) and confirms it again: bound as
    // it is, still paused until he resumes it.
    let confirmed = kit
        .post(
            &sam,
            &format!("{one}/confirm"),
            Some(json!({"apply_to_existing": false})),
        )
        .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{:?}", confirmed.json);
    assert_eq!(confirmed.json["needs_confirmation"], false);
    assert_eq!(confirmed.json["state"], "paused");
    let resumed = kit.patch(&sam, &one, json!({"paused": false})).await;
    assert_eq!(resumed.status, StatusCode::OK, "{:?}", resumed.json);
    kit.ingest_value(workspace, inbox, &notifications("after", 1, GITHUB, None))
        .await;
    let (_, in_spam, _, _) = flags(&kit, "after-0").await;
    assert!(in_spam, "the confirmed rule acts");
    // A bound rule is not confirmed twice.
    let again = kit
        .post(&sam, &format!("{one}/confirm"), Some(json!({})))
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json["code"], "rule_not_proposed");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_bad_rule_never_fails_ingest(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, sam_id) = sams_notifications(&kit, 1).await;
    // Sam's rule, and two planted: one that does not even decode, one that
    // does but is not bound.
    let created = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": [{"field": "sender_address", "value": GITHUB}],
                "actions": [{"action": "mark_read"}],
                "apply_to_existing": false,
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    plant(
        &kit,
        workspace,
        inbox,
        *sam_id.as_uuid(),
        (
            json!([{"field": "no_such_field", "value": 1}]),
            json!({"not": "a list"}),
        ),
        None,
    )
    .await;
    plant(
        &kit,
        workspace,
        inbox,
        *sam_id.as_uuid(),
        from_github(json!([{"action": "trash"}])),
        None,
    )
    .await;
    // `run_jobs` fails the test if any job failed.
    kit.ingest_value(workspace, inbox, &notifications("late", 2, GITHUB, None))
        .await;
    for id in ["late-0", "late-1"] {
        let (trashed, _, _, read) = flags(&kit, id).await;
        assert!(read, "{id}: Sam's rule still acts");
        assert!(!trashed, "{id}: the planted rule acted");
    }
}
