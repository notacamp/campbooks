//! A suspended author's rules pause, and act again once the suspension is
//! lifted (campbooks-cloud#147).

use campbooks_kernel::{
    DomainEvent,
    tenancy::{UserId, WorkspaceId},
};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

use crate::{
    events::{MemberLeft, MemberSuspended, MemberUnsuspended},
    features::rules::tests::{GITHUB, kit, move_github, sams_notifications, streams},
    model::{InboxId, InboxRole},
    testkit::{Kit, notifications},
};

/// Publish a workspaces event in `workspace`, as the workspaces module does,
/// and let every handler run.
async fn deliver<E: DomainEvent>(kit: &Kit, workspace: WorkspaceId, event: &E) {
    let mut tx = kit.state.db.begin().await.unwrap();
    campbooks_kernel::db::set_workspace(&mut tx, &workspace.to_string())
        .await
        .unwrap();
    kit.state.events.publish(&mut tx, event).await.unwrap();
    tx.commit().await.unwrap();
    kit.run_jobs().await;
}

/// A suspension published before the account's counter existed (no `seq`).
async fn suspend(kit: &Kit, workspace: WorkspaceId, user: UserId, at: DateTime<Utc>) {
    suspend_counted(kit, workspace, user, None, at).await;
}

/// A lift published before the account's counter existed (no `seq`).
async fn lift(kit: &Kit, workspace: WorkspaceId, user: UserId, at: DateTime<Utc>) {
    lift_counted(kit, workspace, user, None, at).await;
}

async fn suspend_counted(
    kit: &Kit,
    workspace: WorkspaceId,
    user: UserId,
    seq: Option<i64>,
    at: DateTime<Utc>,
) {
    let event = MemberSuspended {
        workspace: workspace.to_string(),
        user,
        at,
        seq,
    };
    deliver(kit, workspace, &event).await;
}

async fn lift_counted(
    kit: &Kit,
    workspace: WorkspaceId,
    user: UserId,
    seq: Option<i64>,
    at: DateTime<Utc>,
) {
    let event = MemberUnsuspended {
        workspace: workspace.to_string(),
        user,
        at,
        seq,
    };
    deliver(kit, workspace, &event).await;
}

/// Sam's inbox with Leo, a sender there, and the rule Leo taught (its run
/// over existing mail done).
async fn leos_rule(kit: &Kit) -> (WorkspaceId, InboxId, String, UserId, String) {
    let (workspace, inbox, sam, _) = sams_notifications(kit, 2).await;
    let (leo, leo_id) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    let created = kit
        .post(
            &leo,
            &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
            Some(move_github()),
        )
        .await;
    let rule = created.json["rule"]["id"].as_str().unwrap().to_owned();
    kit.run_jobs().await;
    (workspace, inbox, sam, leo_id, rule)
}

async fn stream_of(kit: &Kit, message: &str) -> String {
    streams(kit)
        .await
        .into_iter()
        .find(|(id, ..)| id == message)
        .unwrap()
        .1
}

/// The states `rule_changed` announced for `rule`, oldest first.
async fn announced(kit: &Kit, rule: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT payload ->> 'state' FROM kernel.events
          WHERE name = 'conversations.rule_changed' AND payload ->> 'rule' = $1 ORDER BY id",
    )
    .bind(rule)
    .fetch_all(&kit.db)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_suspended_authors_rule_pauses_until_the_suspension_is_lifted(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, leo, rule) = leos_rule(&kit).await;
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    let before = announced(&kit, &rule).await.len();

    suspend(&kit, workspace, leo, Utc::now()).await;
    let got = kit.get(&sam, &one).await;
    assert_eq!(got.json["state"], "paused");
    // Readers learn it acts again on its own, never why its author is away.
    assert_eq!(got.json["paused_reason"], "author_unavailable");
    assert_eq!(announced(&kit, &rule).await[before..], ["paused"]);
    // New mail is left alone.
    kit.ingest_value(workspace, inbox, &notifications("later", 1, GITHUB, None))
        .await;
    assert_eq!(stream_of(&kit, "later-0").await, "primary");

    lift(&kit, workspace, leo, Utc::now()).await;
    let got = kit.get(&sam, &one).await;
    assert_eq!(got.json["state"], "active");
    assert!(got.json.get("paused_reason").is_none(), "{:?}", got.json);
    assert_eq!(announced(&kit, &rule).await[before..], ["paused", "active"]);
    // It acts again, with nothing redone by hand.
    kit.ingest_value(workspace, inbox, &notifications("again", 1, GITHUB, None))
        .await;
    assert_eq!(stream_of(&kit, "again-0").await, "updates");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn late_and_repeated_deliveries_change_nothing(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, _, sam, leo, rule) = leos_rule(&kit).await;
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    let before = announced(&kit, &rule).await.len();
    let suspended_at = Utc::now();
    let lifted_at = suspended_at + Duration::seconds(5);

    // The lift arrives first (recorded, so announced), then the suspension
    // it lifts (older: nothing).
    lift(&kit, workspace, leo, lifted_at).await;
    suspend(&kit, workspace, leo, suspended_at).await;
    assert_eq!(kit.get(&sam, &one).await.json["state"], "active");
    assert_eq!(announced(&kit, &rule).await[before..], ["active"]);

    // A newer suspension, delivered twice: paused, announced once.
    let again = lifted_at + Duration::seconds(5);
    suspend(&kit, workspace, leo, again).await;
    suspend(&kit, workspace, leo, again).await;
    assert_eq!(kit.get(&sam, &one).await.json["state"], "paused");
    assert_eq!(announced(&kit, &rule).await[before..], ["active", "paused"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_run_stops_at_its_next_batch_when_its_author_is_suspended(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, _, _) = sams_notifications(&kit, 5).await;
    let (leo, leo_id) = kit.member("leo", workspace);
    kit.add_to_inbox(inbox, leo_id, InboxRole::Sender).await;
    kit.post(
        &leo,
        &format!("/api/v1/w/{workspace}/inboxes/{inbox}/rules"),
        Some(move_github()),
    )
    .await;
    // Leo is suspended before the run's next batch.
    sqlx::query(
        "INSERT INTO conversations.member_suspensions (workspace_id, user_id, suspended, as_of)
         VALUES ($1, $2, true, now())",
    )
    .bind(workspace.as_uuid())
    .bind(leo_id.as_uuid())
    .execute(&kit.db)
    .await
    .unwrap();
    kit.run_jobs().await;
    assert!(streams(&kit).await.iter().all(|(_, s, _)| s == "primary"));
    let state: String = sqlx::query_scalar("SELECT state FROM conversations.rule_runs")
        .fetch_one(&kit.db)
        .await
        .unwrap();
    assert_eq!(state, "cancelled");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_reason_that_would_still_hold_is_given(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, _, sam, leo, rule) = leos_rule(&kit).await;
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    suspend(&kit, workspace, leo, Utc::now()).await;
    // A manager pauses it too: lifting the suspension would not resume it.
    let paused = kit
        .patch(&sam, &one, serde_json::json!({"paused": true}))
        .await;
    assert_eq!(paused.json["paused_reason"], "paused_by_member");
    // Leo leaves the inbox: only a manager brings it back.
    sqlx::query("DELETE FROM conversations.inbox_members WHERE user_id = $1")
        .bind(leo.as_uuid())
        .execute(&kit.db)
        .await
        .unwrap();
    assert_eq!(
        kit.get(&sam, &one).await.json["paused_reason"],
        "author_cannot_write"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn leaving_ends_a_suspension_so_coming_back_resumes_the_rules(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, leo, rule) = leos_rule(&kit).await;
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    let suspended_at = Utc::now();
    suspend(&kit, workspace, leo, suspended_at).await;
    // Leo is removed while suspended; the lift will never reach this
    // workspace (he is no member of it any more).
    let left = MemberLeft {
        workspace: workspace.to_string(),
        user: leo,
    };
    deliver(&kit, workspace, &left).await;
    assert_eq!(
        kit.get(&sam, &one).await.json["paused_reason"],
        "author_cannot_write"
    );
    // A late copy of the suspension changes nothing.
    suspend(&kit, workspace, leo, suspended_at).await;
    // He signs in again and is invited back, a sender again: his rule acts.
    kit.add_to_inbox(inbox, leo, InboxRole::Sender).await;
    let got = kit.get(&sam, &one).await;
    assert_eq!(got.json["state"], "active", "{:?}", got.json);
}

// --- Ordered by the account's counter (campbooks-cloud#197) -----------------

#[sqlx::test(migrations = "../../../migrations")]
async fn a_lift_counted_later_wins_even_when_its_clock_is_earlier(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, _, sam, leo, rule) = leos_rule(&kit).await;
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    let before = announced(&kit, &rule).await.len();
    let now = Utc::now();

    suspend_counted(&kit, workspace, leo, Some(1), now).await;
    // The clock stepped back between the suspension and its lift.
    let stepped_back = now - Duration::minutes(5);
    lift_counted(&kit, workspace, leo, Some(2), stepped_back).await;
    assert_eq!(kit.get(&sam, &one).await.json["state"], "active");
    assert_eq!(announced(&kit, &rule).await[before..], ["paused", "active"]);

    // Replays change nothing and announce nothing.
    suspend_counted(&kit, workspace, leo, Some(1), now).await;
    lift_counted(&kit, workspace, leo, Some(2), stepped_back).await;
    assert_eq!(kit.get(&sam, &one).await.json["state"], "active");
    assert_eq!(announced(&kit, &rule).await[before..], ["paused", "active"]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_change_from_before_the_counter_never_overrides_a_counted_one(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, _, sam, leo, rule) = leos_rule(&kit).await;
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    let now = Utc::now();

    // Recorded before the counter existed, then a counted lift: it wins
    // whatever its clock says.
    suspend(&kit, workspace, leo, now).await;
    lift_counted(&kit, workspace, leo, Some(2), now - Duration::hours(1)).await;
    assert_eq!(kit.get(&sam, &one).await.json["state"], "active");
    // An uncounted suspension delivered late, however new its clock: older.
    suspend(&kit, workspace, leo, now + Duration::hours(1)).await;
    assert_eq!(kit.get(&sam, &one).await.json["state"], "active");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn leaving_keeps_a_counted_suspension_until_joining_again_tells_the_truth(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, inbox, sam, leo, rule) = leos_rule(&kit).await;
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    let now = Utc::now();
    suspend_counted(&kit, workspace, leo, Some(1), now).await;
    let left = MemberLeft {
        workspace: workspace.to_string(),
        user: leo,
    };
    deliver(&kit, workspace, &left).await;
    // Back in the inbox while still suspended (joining again announced the
    // same suspension): his rule stays paused, never acting for him.
    kit.add_to_inbox(inbox, leo, InboxRole::Sender).await;
    suspend_counted(&kit, workspace, leo, Some(1), now).await;
    let got = kit.get(&sam, &one).await;
    assert_eq!(got.json["state"], "paused", "{:?}", got.json);
    assert_eq!(got.json["paused_reason"], "author_unavailable");
    // Lifted (joining after the lift announces it): it acts again.
    lift_counted(&kit, workspace, leo, Some(2), now + Duration::seconds(5)).await;
    assert_eq!(kit.get(&sam, &one).await.json["state"], "active");
}

// --- Keeping a proposal whose author is away (campbooks-cloud#197) ----------

#[sqlx::test(migrations = "../../../migrations")]
async fn a_proposal_whose_author_is_away_says_it_stays_paused_once_kept(db: PgPool) {
    let kit = kit(db).await;
    let (workspace, _, sam, leo, rule) = leos_rule(&kit).await;
    let one = format!("/api/v1/w/{workspace}/rules/{rule}");
    // Leo's rule waits for a yes.
    sqlx::query(
        "UPDATE conversations.rules SET state = 'proposed', confirmed_at = NULL WHERE id = $1::uuid",
    )
    .bind(&rule)
    .execute(&kit.db)
    .await
    .unwrap();
    let got = kit.get(&sam, &one).await;
    assert_eq!(got.json["state"], "proposed");
    assert!(got.json.get("paused_once_kept").is_none(), "{:?}", got.json);

    suspend_counted(&kit, workspace, leo, Some(1), Utc::now()).await;
    let got = kit.get(&sam, &one).await;
    assert_eq!(got.json["state"], "proposed");
    assert!(got.json.get("paused_reason").is_none(), "{:?}", got.json);
    assert_eq!(got.json["paused_once_kept"], "author_unavailable");

    // Sam keeps it anyway: it is Leo's still, and paused until he is back.
    let kept = kit
        .post(
            &sam,
            &format!("{one}/confirm"),
            Some(serde_json::json!({ "apply_to_existing": false })),
        )
        .await;
    assert_eq!(kept.status.as_u16(), 200, "{:?}", kept.json);
    assert_eq!(kept.json["state"], "paused");
    assert_eq!(kept.json["paused_reason"], "author_unavailable");
    assert_eq!(kept.json["taught_by"], leo.to_string());
    assert!(
        kept.json.get("paused_once_kept").is_none(),
        "{:?}",
        kept.json
    );
}
