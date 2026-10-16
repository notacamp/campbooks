use campbooks_kernel::{DomainEvent, jobs::Worker};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
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
    publish(&kit, &AccountSuspended { account, at: None }).await;
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
