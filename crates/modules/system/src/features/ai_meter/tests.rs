use std::sync::Arc;

use campbooks_ai::{
    Role,
    usage::{Admission, Meter as _, MeteredCall, Outcome},
};
use campbooks_kernel::{
    clock::FixedClock,
    db::BeginFor as _,
    tenancy::WorkspaceId,
    testing::{self, app_role_pool},
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use sqlx::PgPool;

use super::{Admitted, AiUsageMeter};

fn morning() -> DateTime<Utc> {
    "2026-10-16T09:00:00Z".parse().unwrap()
}

fn day(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn meter(cap: Option<u32>) -> (AiUsageMeter, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::new(morning()));
    (AiUsageMeter::new(cap).with_clock(clock.clone()), clock)
}

fn call(workspace: WorkspaceId, role: Role) -> MeteredCall<'static> {
    MeteredCall {
        workspace,
        role,
        feature: "conversations.triage",
    }
}

/// `workspace`'s ledger, as its own transaction sees it: (day, role,
/// feature, calls, failed, prompt tokens, completion tokens).
async fn ledger(
    pool: &PgPool,
    workspace: WorkspaceId,
) -> Vec<(NaiveDate, String, String, i32, i32, i64, i64)> {
    let mut tx = pool.begin_for(workspace).await.unwrap();
    let rows = sqlx::query_as(
        "SELECT day, role, feature, calls, failed, prompt_tokens, completion_tokens
           FROM system.ai_usage ORDER BY day, role, feature",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    rows
}

/// `workspace`'s day rows: (day, calls, refused, capped).
async fn days(pool: &PgPool, workspace: WorkspaceId) -> Vec<(NaiveDate, i32, i32, bool)> {
    let mut tx = pool.begin_for(workspace).await.unwrap();
    let rows = sqlx::query_as(
        "SELECT day, calls, refused, capped_at IS NOT NULL FROM system.ai_usage_days ORDER BY day",
    )
    .fetch_all(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    rows
}

#[sqlx::test(migrations = "../../../migrations")]
async fn calls_are_counted_per_workspace_role_feature_and_day(db: PgPool) {
    let pool = app_role_pool(&db).await;
    let (meter, _) = meter(None);
    let workspace = WorkspaceId::new();
    for _ in 0..3 {
        let admitted = meter
            .admit(&pool, call(workspace, Role::Decide))
            .await
            .unwrap();
        assert_eq!(
            admitted,
            Admission::Admitted {
                day: day("2026-10-16")
            }
        );
    }
    meter
        .admit(&pool, call(workspace, Role::Embed))
        .await
        .unwrap();
    meter
        .settle(
            &pool,
            call(workspace, Role::Decide),
            day("2026-10-16"),
            Outcome {
                failed: false,
                prompt_tokens: 120,
                completion_tokens: 0,
            },
        )
        .await
        .unwrap();
    meter
        .settle(
            &pool,
            call(workspace, Role::Embed),
            day("2026-10-16"),
            Outcome {
                failed: true,
                ..Outcome::default()
            },
        )
        .await
        .unwrap();
    let triage = || "conversations.triage".to_owned();
    assert_eq!(
        ledger(&pool, workspace).await,
        [
            (day("2026-10-16"), "decide".into(), triage(), 3, 0, 120, 0),
            (day("2026-10-16"), "embed".into(), triage(), 1, 1, 0, 0),
        ]
    );
    assert_eq!(
        days(&pool, workspace).await,
        [(day("2026-10-16"), 4, 0, false)]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_cap_refuses_and_says_so_once(db: PgPool) {
    let pool = app_role_pool(&db).await;
    let (meter, _) = meter(Some(2));
    let workspace = WorkspaceId::new();
    let mut seen = Vec::new();
    for _ in 0..4 {
        seen.push(
            meter
                .admit_call(&pool, call(workspace, Role::Decide))
                .await
                .unwrap(),
        );
    }
    let d = day("2026-10-16");
    assert_eq!(
        seen,
        [
            Admitted::Yes { day: d, calls: 1 },
            Admitted::Yes { day: d, calls: 2 },
            Admitted::No {
                cap: 2,
                first: true
            },
            Admitted::No {
                cap: 2,
                first: false
            },
        ]
    );
    assert_eq!(days(&pool, workspace).await, [(d, 2, 2, true)]);
    assert_eq!(
        meter
            .admit(&pool, call(workspace, Role::Complete))
            .await
            .unwrap(),
        Admission::Refused { cap: 2 },
        "the cap is across roles"
    );
    // Only admitted calls are in the ledger.
    assert_eq!(ledger(&pool, workspace).await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_next_utc_day_starts_again(db: PgPool) {
    let pool = app_role_pool(&db).await;
    let (meter, clock) = meter(Some(1));
    let workspace = WorkspaceId::new();
    let decide = || call(workspace, Role::Decide);
    assert!(matches!(
        meter.admit(&pool, decide()).await.unwrap(),
        Admission::Admitted { .. }
    ));
    clock.set("2026-10-16T23:59:59Z".parse().unwrap());
    assert!(matches!(
        meter.admit(&pool, decide()).await.unwrap(),
        Admission::Refused { cap: 1 }
    ));
    clock.advance(TimeDelta::seconds(1));
    assert_eq!(
        meter.admit(&pool, decide()).await.unwrap(),
        Admission::Admitted {
            day: day("2026-10-17")
        }
    );
    assert_eq!(
        days(&pool, workspace).await,
        [
            (day("2026-10-16"), 1, 1, true),
            (day("2026-10-17"), 1, 0, false)
        ]
    );
    assert_eq!(
        AiUsageMeter::resets_at(day("2026-10-16")),
        "2026-10-17T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn workspaces_have_their_own_counters_and_see_only_theirs(db: PgPool) {
    let pool = app_role_pool(&db).await;
    let (meter, _) = meter(Some(1));
    let (a, b) = (WorkspaceId::new(), WorkspaceId::new());
    for workspace in [a, b] {
        assert!(matches!(
            meter
                .admit(&pool, call(workspace, Role::Decide))
                .await
                .unwrap(),
            Admission::Admitted { .. }
        ));
    }
    assert!(matches!(
        meter.admit(&pool, call(a, Role::Decide)).await.unwrap(),
        Admission::Refused { .. }
    ));
    assert_eq!(days(&pool, a).await, [(day("2026-10-16"), 1, 1, true)]);
    assert_eq!(days(&pool, b).await, [(day("2026-10-16"), 1, 0, false)]);
    // A third workspace sees neither.
    let mut tx = pool.begin_for(WorkspaceId::new()).await.unwrap();
    let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM system.ai_usage_days")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(seen, 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn without_a_cap_every_call_is_admitted_and_counted(db: PgPool) {
    let pool = app_role_pool(&db).await;
    for cap in [None, Some(0)] {
        let (meter, _) = meter(cap);
        assert_eq!(meter.cap(), None);
        let workspace = WorkspaceId::new();
        for _ in 0..25 {
            assert!(matches!(
                meter
                    .admit(&pool, call(workspace, Role::Decide))
                    .await
                    .unwrap(),
                Admission::Admitted { .. }
            ));
        }
        assert_eq!(
            days(&pool, workspace).await,
            [(day("2026-10-16"), 25, 0, false)]
        );
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn concurrent_calls_never_pass_the_cap(db: PgPool) {
    let pool = app_role_pool(&db).await;
    let (meter, _) = meter(Some(5));
    let workspace = WorkspaceId::new();
    let mut calls = tokio::task::JoinSet::new();
    for _ in 0..20 {
        let (meter, pool) = (meter.clone(), pool.clone());
        calls.spawn(async move {
            meter
                .admit(&pool, call(workspace, Role::Decide))
                .await
                .unwrap()
        });
    }
    let admitted = calls
        .join_all()
        .await
        .into_iter()
        .filter(|a| matches!(a, Admission::Admitted { .. }))
        .count();
    assert_eq!(admitted, 5);
    assert_eq!(
        days(&pool, workspace).await,
        [(day("2026-10-16"), 5, 15, true)]
    );
}

#[tokio::test]
async fn an_unreachable_ledger_is_an_error_not_an_admission() {
    let (meter, _) = meter(Some(10));
    let result = meter
        .admit(
            &testing::unreachable_pool(),
            call(WorkspaceId::new(), Role::Decide),
        )
        .await;
    assert!(result.is_err());
}
