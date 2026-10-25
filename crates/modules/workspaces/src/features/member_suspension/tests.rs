use campbooks_kernel::{DomainEvent, jobs::Worker, tenancy::WorkspaceId};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::record;
use crate::{
    domain::suspension::Change,
    events::{AccountSuspended, AccountUnsuspended},
    testkit::Kit,
};

/// Publish an account's event as identity does: instance-wide, from a
/// transaction that set no workspace.
async fn publish<E: DomainEvent>(kit: &Kit, event: &E) {
    let mut tx = kit.state.db.begin().await.unwrap();
    kit.state.events.publish(&mut tx, event).await.unwrap();
    tx.commit().await.unwrap();
    let stats = Worker::new(kit.state.clone()).run_until_idle().await;
    assert_eq!(stats.failed, 0, "{stats:?}");
}

/// The `name` events of `workspace`, as (user, at).
async fn announced(kit: &Kit, workspace: &str, name: &str) -> Vec<(String, DateTime<Utc>)> {
    let payloads: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT payload FROM kernel.events WHERE workspace_id = $1 AND name = $2 ORDER BY id",
    )
    .bind(workspace)
    .bind(name)
    .fetch_all(&kit.db)
    .await
    .unwrap();
    payloads
        .iter()
        .map(|p| {
            (
                p["user"].as_str().unwrap().to_owned(),
                p["at"].as_str().unwrap().parse().unwrap(),
            )
        })
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_suspension_and_its_lift_reach_every_workspace_of_the_account(db: PgPool) {
    let kit = Kit::new(db).await;
    let account = Uuid::now_v7();
    let ana = kit.user(&account.to_string());
    let one = kit.create_workspace(&ana, "One").await;
    let two = kit.create_workspace(&ana, "Two").await;
    // Bea, in one of them, is not suspended.
    let bea = kit.user("bea");
    let theirs = kit.create_workspace(&bea, "Bea's").await;
    kit.join(&ana, one, "bea", "member").await;
    let ana_id = kit.user_id(&account.to_string()).await.to_string();

    let suspended_at = Utc::now();
    publish(
        &kit,
        &AccountSuspended {
            account,
            at: Some(suspended_at),
            seq: Some(1),
        },
    )
    .await;
    for workspace in [one, two] {
        assert_eq!(
            announced(&kit, &workspace.to_string(), "workspaces.member_suspended").await,
            [(ana_id.clone(), suspended_at)]
        );
    }
    assert!(
        announced(&kit, &theirs.to_string(), "workspaces.member_suspended")
            .await
            .is_empty()
    );

    let lifted_at = Utc::now();
    publish(
        &kit,
        &AccountUnsuspended {
            account,
            at: lifted_at,
            seq: Some(2),
        },
    )
    .await;
    for workspace in [one, two] {
        assert_eq!(
            announced(
                &kit,
                &workspace.to_string(),
                "workspaces.member_unsuspended"
            )
            .await,
            [(ana_id.clone(), lifted_at)]
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_older_event_without_at_uses_when_it_occurred(db: PgPool) {
    let kit = Kit::new(db).await;
    let account = Uuid::now_v7();
    let ana = kit.user(&account.to_string());
    let one = kit.create_workspace(&ana, "One").await;
    publish(
        &kit,
        &AccountSuspended {
            account,
            at: None,
            seq: None,
        },
    )
    .await;
    let occurred: DateTime<Utc> = sqlx::query_scalar(
        "SELECT occurred_at FROM kernel.events WHERE name = 'identity.account_suspended'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    let got = announced(&kit, &one.to_string(), "workspaces.member_suspended").await;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].1, occurred);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_account_that_never_signed_in_announces_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let ana = kit.user("ana");
    kit.create_workspace(&ana, "One").await;
    publish(
        &kit,
        &AccountSuspended {
            account: Uuid::now_v7(),
            at: Some(Utc::now()),
            seq: Some(1),
        },
    )
    .await;
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM kernel.events WHERE name LIKE 'workspaces.member_%suspended'",
    )
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!(n, 0);
}

// --- Ordering by the account's counter (campbooks-cloud#197) ----------------

/// The suspension changes announced in `workspace`, oldest first, as
/// (`suspended`, `seq`).
async fn changes(kit: &Kit, workspace: WorkspaceId) -> Vec<(bool, Option<i64>)> {
    let rows: Vec<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT name, payload FROM kernel.events
          WHERE workspace_id = $1 AND name LIKE 'workspaces.member_%suspended' ORDER BY id",
    )
    .bind(workspace.to_string())
    .fetch_all(&kit.db)
    .await
    .unwrap();
    rows.into_iter()
        .map(|(name, payload)| {
            (
                name == "workspaces.member_suspended",
                payload["seq"].as_i64(),
            )
        })
        .collect()
}

fn suspended(account: Uuid, seq: i64, at: DateTime<Utc>) -> AccountSuspended {
    AccountSuspended {
        account,
        at: Some(at),
        seq: Some(seq),
    }
}

fn lifted(account: Uuid, seq: i64, at: DateTime<Utc>) -> AccountUnsuspended {
    AccountUnsuspended {
        account,
        at,
        seq: Some(seq),
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_counter_orders_changes_whatever_the_clock_said(db: PgPool) {
    let kit = Kit::new(db).await;
    let account = Uuid::now_v7();
    let ana = kit.user(&account.to_string());
    let one = kit.create_workspace(&ana, "One").await;
    let now = Utc::now();

    // The lift counts later but its clock stepped back: it is announced.
    publish(&kit, &suspended(account, 1, now)).await;
    publish(&kit, &lifted(account, 2, now - Duration::minutes(5))).await;
    // The suspension it lifts, delivered again late: older, not announced.
    // An event from before the counter existed is older still.
    publish(&kit, &suspended(account, 1, now)).await;
    publish(
        &kit,
        &AccountSuspended {
            account,
            at: Some(now + Duration::hours(1)),
            seq: None,
        },
    )
    .await;
    // The lift again (a replay): announced again, which consumers ignore.
    publish(&kit, &lifted(account, 2, now - Duration::minutes(5))).await;
    assert_eq!(
        changes(&kit, one).await,
        [(true, Some(1)), (false, Some(2)), (false, Some(2))]
    );
    let (seq, suspended_now): (Option<i64>, bool) = sqlx::query_as(
        "SELECT suspension_seq, suspended FROM workspaces.users WHERE idp_subject = $1",
    )
    .bind(account.to_string())
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert_eq!((seq, suspended_now), (Some(2), false));
}

// --- Joining or leaving while a change is announced (campbooks-cloud#197) ---

/// Wait until another connection of this test's database waits for a lock.
async fn until_one_waits_for_a_lock(db: &PgPool) {
    for _ in 0..500 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
              WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(db)
        .await
        .unwrap();
        if waiting > 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("nothing waited for the user's lock");
}

/// The fan-out of `change` for `account`, as the job runs it, but holding
/// the user's lock until `go` finishes: `go` runs while the lock is held,
/// then the fan-out publishes and commits.
async fn fan_out_around<F: Future<Output = R>, R>(
    kit: &Kit,
    account: Uuid,
    change: Change,
    go: F,
) -> (Vec<WorkspaceId>, R) {
    let mut tx = kit.state.db.begin().await.unwrap();
    let (user, workspaces, fresh) = record(&mut tx, &account.to_string(), change)
        .await
        .unwrap()
        .unwrap();
    let fan_out = async {
        until_one_waits_for_a_lock(&kit.db).await;
        for workspace in &workspaces {
            super::publish(&kit.state, *workspace, user, change, fresh)
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();
    };
    let ((), result) = tokio::join!(fan_out, go);
    (workspaces, result)
}

const ACCEPT: &str = "/api/v1/workspaces/invitations/accept";

#[sqlx::test(migrations = "../../../migrations")]
async fn joining_while_being_suspended_is_paused_in_the_workspace_joined_too(db: PgPool) {
    let kit = Kit::new(db).await;
    let account = Uuid::now_v7();
    let ana = kit.user(&account.to_string());
    let one = kit.create_workspace(&ana, "One").await;
    let bea = kit.user("bea");
    let theirs = kit.create_workspace(&bea, "Bea's").await;
    let token = kit
        .invite(&bea, theirs, &format!("{account}@example.com"), "member")
        .await;

    // Ana accepts while her suspension is being announced: she waits for
    // it, which never listed Bea's workspace, and is then told on joining.
    let change = Change {
        suspended: true,
        seq: Some(1),
        at: Utc::now(),
    };
    let (listed, accepted) = fan_out_around(
        &kit,
        account,
        change,
        kit.post(&ana, ACCEPT, serde_json::json!({ "token": token })),
    )
    .await;
    assert_eq!(accepted.status.as_u16(), 200, "{:?}", accepted.json);
    assert_eq!(listed, [one]);
    assert_eq!(changes(&kit, one).await, [(true, Some(1))]);
    assert_eq!(changes(&kit, theirs).await, [(true, Some(1))]);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn leaving_while_a_lift_is_announced_hears_the_lift_first(db: PgPool) {
    let kit = Kit::new(db).await;
    let account = Uuid::now_v7();
    let bea = kit.user("bea");
    let theirs = kit.create_workspace(&bea, "Bea's").await;
    kit.join(&bea, theirs, &account.to_string(), "member").await;
    let ana_id = kit.user_id(&account.to_string()).await;
    let now = Utc::now();
    publish(&kit, &suspended(account, 1, now)).await;

    // Bea removes Ana while the lift is being announced: the removal waits,
    // so the lift reaches the workspace before Ana leaves it.
    let change = Change {
        suspended: false,
        seq: Some(2),
        at: now + Duration::seconds(5),
    };
    let (listed, removed) = fan_out_around(
        &kit,
        account,
        change,
        kit.delete(&bea, &format!("/api/v1/w/{theirs}/members/{ana_id}")),
    )
    .await;
    assert_eq!(removed.status.as_u16(), 204, "{:?}", removed.json);
    assert_eq!(listed, [theirs]);
    let events = kit.events_of(&theirs.to_string()).await;
    let lift = events
        .iter()
        .position(|e| e == "workspaces.member_unsuspended")
        .unwrap();
    let left = events
        .iter()
        .position(|e| e == "workspaces.member_left")
        .unwrap();
    assert!(lift < left, "{events:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn coming_back_after_a_lift_that_missed_the_workspace_hears_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let account = Uuid::now_v7();
    let ana = kit.user(&account.to_string());
    let bea = kit.user("bea");
    let theirs = kit.create_workspace(&bea, "Bea's").await;
    kit.join(&bea, theirs, &account.to_string(), "member").await;
    let ana_id = kit.user_id(&account.to_string()).await;
    let now = Utc::now();
    publish(&kit, &suspended(account, 1, now)).await;
    // Removed while suspended; the lift no longer reaches the workspace.
    let removed = kit
        .delete(&bea, &format!("/api/v1/w/{theirs}/members/{ana_id}"))
        .await;
    assert_eq!(removed.status.as_u16(), 204, "{:?}", removed.json);
    publish(&kit, &lifted(account, 2, now + Duration::seconds(5))).await;
    assert_eq!(changes(&kit, theirs).await, [(true, Some(1))]);

    // Invited back: joining tells the workspace of the lift.
    let token = kit
        .invite(&bea, theirs, &format!("{account}@example.com"), "member")
        .await;
    let accepted = kit
        .post(&ana, ACCEPT, serde_json::json!({ "token": token }))
        .await;
    assert_eq!(accepted.status.as_u16(), 200, "{:?}", accepted.json);
    assert_eq!(
        changes(&kit, theirs).await,
        [(true, Some(1)), (false, Some(2))]
    );
    // A workspace created now holds nothing a lift would undo: not told.
    let new = kit.create_workspace(&ana, "New").await;
    assert!(changes(&kit, new).await.is_empty());
}
