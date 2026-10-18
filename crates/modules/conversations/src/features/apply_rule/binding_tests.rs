//! Rule runs and the changes they record, bound (campbooks-cloud#242): a
//! run or an application planted or changed with SQL as the runtime role
//! changes nothing at the provider, and a bad row never fails ingest or the
//! job; what members ask for still works.

use axum::http::StatusCode;
use campbooks_ai::decide::taught::Bin;
use campbooks_kernel::{db::BeginFor as _, jobs, tenancy::WorkspaceId};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use crate::{
    domain::{rule_runs::RunBatch, rules::Before},
    features::{
        apply_rule::ApplyRules,
        rules::tests::{GITHUB, kit, sams_notifications},
    },
    model::{InboxId, RuleRunId, RunKind},
    testkit::{Kit, notifications},
};

/// How many changes were asked of the provider.
async fn requested(kit: &Kit) -> usize {
    kit.events("conversations.changes_requested").await.len()
}

/// `(in_spam, in_inbox)` of message `provider_message_id`.
async fn spam(kit: &Kit, provider_message_id: &str) -> (bool, bool) {
    sqlx::query_as(
        "SELECT in_spam, in_inbox FROM conversations.messages WHERE provider_message_id = $1",
    )
    .bind(provider_message_id)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

async fn conversation(kit: &Kit, provider_message_id: &str) -> Uuid {
    kit.conversation_of(provider_message_id)
        .await
        .parse()
        .unwrap()
}

/// The states of `rule`'s runs, oldest first.
async fn runs(kit: &Kit, rule: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT state FROM conversations.rule_runs WHERE rule_id = $1 ORDER BY started_at, id",
    )
    .bind(rule)
    .fetch_all(&kit.db)
    .await
    .unwrap()
}

/// Sam's rule reporting what GitHub sends as spam, applied to the mail
/// already here when `apply`.
async fn spam_github(
    kit: &Kit,
    sam: &str,
    workspace: WorkspaceId,
    inbox: InboxId,
    apply: bool,
) -> Uuid {
    let created = kit
        .post(
            sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": [{"field": "sender_address", "value": GITHUB}],
                "actions": [{"action": "spam"}],
                "apply_to_existing": apply,
            })),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.json);
    created.json["rule"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

/// Run `sql`, binding `ids`, as the runtime role in `workspace`: what an
/// attacker with SQL inside the workspace can do.
async fn as_runtime_role(kit: &Kit, workspace: WorkspaceId, sql: &str, ids: &[Uuid]) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()));
    for id in ids {
        query = query.bind(*id);
    }
    query.execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

/// Plant, as the runtime role, a running `kind` run of `rule` asked by
/// `signed.0` with MAC `signed.1`, and the job for its batch.
async fn plant_run(
    kit: &Kit,
    workspace: WorkspaceId,
    rule: Uuid,
    kind: RunKind,
    signed: (Option<Uuid>, Option<Vec<u8>>),
) -> RuleRunId {
    let run = RuleRunId::new();
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(
        "INSERT INTO conversations.rule_runs
             (id, workspace_id, rule_id, kind, state, counted, requested_by, mac)
         VALUES ($1, $2, $3, $4, 'running', true, $5, $6)",
    )
    .bind(run.as_uuid())
    .bind(workspace.as_uuid())
    .bind(rule)
    .bind(kind.as_str())
    .bind(signed.0)
    .bind(signed.1)
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
    run
}

/// Plant (or overwrite), as the runtime role, `rule`'s application to
/// `conversation` saying `before`, with `mac`.
async fn plant_application(
    kit: &Kit,
    workspace: WorkspaceId,
    (rule, conversation): (Uuid, Uuid),
    before: Value,
    mac: Option<Vec<u8>>,
) {
    let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
    sqlx::query(
        "INSERT INTO conversations.rule_applications
             (rule_id, conversation_id, workspace_id, before, mac)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (rule_id, conversation_id) DO UPDATE
            SET before = EXCLUDED.before, mac = EXCLUDED.mac, reverted_at = NULL",
    )
    .bind(rule)
    .bind(conversation)
    .bind(workspace.as_uuid())
    .bind(before)
    .bind(mac)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

/// A tag of the right shape that no key made.
fn made_up() -> Vec<u8> {
    let mut tag = vec![1_u8];
    tag.extend([7_u8; 32]);
    tag
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_planted_undo_takes_nothing_out_of_spam(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, sam_id) = sams_notifications(&kit, 2).await;
    // Sam's real, signed rule; it has changed nothing yet.
    let rule = spam_github(&kit, &sam, workspace, inbox, false).await;
    // A phishing mail the provider put in spam.
    sqlx::query(
        "UPDATE conversations.messages SET in_spam = true, in_inbox = false
          WHERE provider_message_id = 'gh-0'",
    )
    .execute(&kit.db)
    .await
    .unwrap();
    let phish = conversation(&kit, "gh-0").await;
    let asked = requested(&kit).await;
    // SQL says the rule put it there (no MAC, then a made-up one), and plants
    // the undo: a revert run (unsigned, or claiming Sam asked) and its job.
    let forged = serde_json::to_value(Before {
        binned: Some(Bin::Spam),
        binned_messages: vec!["gh-0".into()],
        ..Before::default()
    })
    .unwrap();
    for (mac, run) in [
        (None, (None, None)),
        (Some(made_up()), (Some(*sam_id.as_uuid()), Some(made_up()))),
    ] {
        plant_application(&kit, workspace, (rule, phish), forged.clone(), mac).await;
        plant_run(&kit, workspace, rule, RunKind::Revert, run).await;
        // `run_jobs` fails the test if a job failed.
        kit.run_jobs().await;
        assert_eq!(spam(&kit, "gh-0").await, (true, false), "taken out of spam");
    }
    assert_eq!(runs(&kit, rule).await, ["cancelled", "cancelled"]);
    assert_eq!(requested(&kit).await, asked, "a change was requested");

    // Sam removes the rule and asks for its changes back: his run is signed,
    // the forged application still is not, so it stays in spam.
    let removed = kit
        .delete(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}?restore=true"),
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json);
    kit.run_jobs().await;
    assert_eq!(spam(&kit, "gh-0").await, (true, false), "taken out of spam");
    assert_eq!(requested(&kit).await, asked, "a change was requested");
    let got = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    assert_eq!(got.json["run"]["kind"], "revert");
    assert_eq!(got.json["run"]["state"], "done");
    assert_eq!(got.json["run"]["changed"], 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_planted_apply_run_of_a_signed_rule_changes_nothing(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, sam_id) = sams_notifications(&kit, 3).await;
    // Sam taught "report GitHub as spam" for new mail only.
    let rule = spam_github(&kit, &sam, workspace, inbox, false).await;
    // And, for the mail already here, "mark Marta's mail read": its run is
    // signed, and its MAC is there to copy.
    let marta = kit
        .post(
            &sam,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(json!({
                "conditions": [{"field": "sender_address", "value": "marta@example.com"}],
                "actions": [{"action": "mark_read"}],
            })),
        )
        .await;
    assert_eq!(marta.status, StatusCode::CREATED, "{:?}", marta.json);
    kit.run_jobs().await;
    let (by, mac): (Option<Uuid>, Option<Vec<u8>>) =
        sqlx::query_as("SELECT requested_by, mac FROM conversations.rule_runs WHERE rule_id = $1")
            .bind(
                marta.json["rule"]["id"]
                    .as_str()
                    .unwrap()
                    .parse::<Uuid>()
                    .unwrap(),
            )
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(by, Some(*sam_id.as_uuid()));
    assert!(mac.is_some());
    let asked = requested(&kit).await;
    // Planted runs of Sam's spam rule over the mail already here: unsigned,
    // with a made-up MAC, and with the other run's MAC copied.
    for signed in [
        (None, None),
        (Some(*sam_id.as_uuid()), Some(made_up())),
        (by, mac),
    ] {
        plant_run(&kit, workspace, rule, RunKind::Apply, signed).await;
    }
    kit.run_jobs().await;
    for id in ["gh-0", "gh-1", "gh-2"] {
        assert_eq!(spam(&kit, id).await, (false, true), "{id} was reported");
    }
    assert_eq!(requested(&kit).await, asked, "a change was requested");
    assert_eq!(
        runs(&kit, rule).await,
        ["cancelled", "cancelled", "cancelled"]
    );
    // The rule itself still acts on new mail, as Sam taught it.
    kit.ingest_value(workspace, inbox, &notifications("late", 1, GITHUB, None))
        .await;
    assert_eq!(spam(&kit, "late-0").await, (true, false));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_run_stops_when_its_rule_is_signed_again(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let rule = spam_github(&kit, &sam, workspace, inbox, true).await;
    // Sam pauses and resumes it before its run's first batch: the rule is
    // signed again, and the run started for it before stops.
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    for paused in [true, false] {
        let toggled = kit.patch(&sam, &one, json!({"paused": paused})).await;
        assert_eq!(toggled.status, StatusCode::OK, "{:?}", toggled.json);
    }
    kit.run_jobs().await;
    assert_eq!(runs(&kit, rule).await, ["cancelled"]);
    assert_eq!(spam(&kit, "gh-0").await, (false, true));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_application_changed_with_sql_is_left_while_the_rest_go_back(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 3).await;
    // Sam's rule, applied to the mail already here: all three reported.
    let rule = spam_github(&kit, &sam, workspace, inbox, true).await;
    kit.run_jobs().await;
    for id in ["gh-0", "gh-1", "gh-2"] {
        assert_eq!(spam(&kit, id).await, (true, false), "{id}");
    }
    // Every run and every change it recorded is signed.
    let unsigned: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM conversations.rule_applications WHERE mac IS NULL)
              + (SELECT count(*) FROM conversations.rule_runs WHERE mac IS NULL)",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(unsigned, 0);
    let (first, second, third) = (
        conversation(&kit, "gh-0").await,
        conversation(&kit, "gh-1").await,
        conversation(&kit, "gh-2").await,
    );
    // SQL changes gh-1's (now it also says the rule marked it read), and
    // copies gh-0's, MAC and all, onto gh-2's.
    as_runtime_role(
        &kit,
        workspace,
        "UPDATE conversations.rule_applications SET before = before || '{\"read\": true}'
          WHERE rule_id = $1 AND conversation_id = $2",
        &[rule, second],
    )
    .await;
    as_runtime_role(
        &kit,
        workspace,
        "UPDATE conversations.rule_applications a SET before = s.before, mac = s.mac
           FROM conversations.rule_applications s
          WHERE a.rule_id = $1 AND a.conversation_id = $3
            AND s.rule_id = $1 AND s.conversation_id = $2",
        &[rule, first, third],
    )
    .await;
    // Sam's undo: what still verifies goes back; the rest is left, and the
    // run goes on (`run_jobs` fails the test if a job failed).
    let removed = kit
        .delete(
            &sam,
            &format!("/api/v1/w/{workspace}/rules/{rule}?restore=true"),
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json);
    kit.run_jobs().await;
    assert_eq!(spam(&kit, "gh-0").await, (false, true), "gh-0 goes back");
    assert_eq!(spam(&kit, "gh-1").await, (true, false), "gh-1 was edited");
    assert_eq!(spam(&kit, "gh-2").await, (true, false), "gh-2 has a copy");
    let got = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/rules/{rule}"))
        .await;
    assert_eq!(got.json["run"]["state"], "done");
    assert_eq!(got.json["run"]["done"], 3);
    assert_eq!(got.json["run"]["changed"], 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_unsigned_application_never_fails_ingest_and_is_not_undone(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, _) = sams_notifications(&kit, 2).await;
    let rule = spam_github(&kit, &sam, workspace, inbox, false).await;
    // Two applications with no MAC, as before applications were bound: one
    // that does not even decode, one that does.
    let (first, second) = (
        conversation(&kit, "gh-0").await,
        conversation(&kit, "gh-1").await,
    );
    plant_application(
        &kit,
        workspace,
        (rule, first),
        json!({"labels": "not a list", "binned": 7}),
        None,
    )
    .await;
    plant_application(
        &kit,
        workspace,
        (rule, second),
        json!({"archived": true}),
        None,
    )
    .await;
    // Undo is refused before anything changes: the rule stays.
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    let refused = kit.delete(&sam, &format!("{one}?restore=true")).await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{:?}", refused.json);
    assert_eq!(refused.json["code"], "undo_unavailable");
    assert_eq!(kit.get(&sam, &one).await.json["state"], "active");
    // A follow-up arrives in gh-0's thread: ingest goes on, the rule acts,
    // and its application now holds only what it did, signed.
    let mut follow_up = notifications("gh-0-reply", 1, GITHUB, None);
    follow_up["changes"][0]["message"]["thread_key"] = json!("thr-gh-0");
    kit.ingest_value(workspace, inbox, &follow_up).await;
    assert_eq!(spam(&kit, "gh-0-reply-0").await, (true, false));
    let (signed, labels): (bool, Option<Value>) = sqlx::query_as(
        "SELECT mac IS NOT NULL, before -> 'labels' FROM conversations.rule_applications
          WHERE rule_id = $1 AND conversation_id = $2",
    )
    .bind(rule)
    .bind(first)
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert!(signed);
    assert_eq!(labels, None);
    // Removed keeping its changes: still possible.
    let removed = kit.delete(&sam, &one).await;
    assert_eq!(removed.status, StatusCode::OK, "{:?}", removed.json);
    assert_eq!(removed.json["state"], "removed");
}
