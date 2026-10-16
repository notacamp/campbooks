use std::sync::Arc;

use axum::http::{Method, StatusCode};
use campbooks_ai::{
    Role,
    usage::{Meter as _, MeteredCall, Outcome},
};
use campbooks_kernel::{clock::FixedClock, tenancy::WorkspaceId, testing::app_role_pool};
use serde_json::json;
use sqlx::PgPool;

use crate::{AiUsageMeter, Deps, testkit::world};

/// Two workspaces' usage on 2026-10-16 under a cap of 3: `capped` made 3
/// calls and was refused 2, `busy` made 2 with tokens and a failure. The
/// meter, at noon that day.
async fn usage(db: &PgPool) -> (AiUsageMeter, WorkspaceId, WorkspaceId) {
    let clock = Arc::new(FixedClock::new("2026-10-16T12:00:00Z".parse().unwrap()));
    let meter = AiUsageMeter::new(Some(3)).with_clock(clock);
    let pool = app_role_pool(db).await;
    let (capped, busy) = (WorkspaceId::new(), WorkspaceId::new());
    let call = |workspace, role| MeteredCall {
        workspace,
        role,
        feature: "conversations.triage",
    };
    for _ in 0..5 {
        meter
            .admit(&pool, call(capped, Role::Decide))
            .await
            .unwrap();
    }
    for role in [Role::Decide, Role::Embed] {
        meter.admit(&pool, call(busy, role)).await.unwrap();
    }
    meter
        .settle(
            &pool,
            call(busy, Role::Decide),
            "2026-10-16".parse().unwrap(),
            Outcome {
                failed: true,
                prompt_tokens: 120,
                completion_tokens: 4,
            },
        )
        .await
        .unwrap();
    (meter, capped, busy)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn only_system_readers_see_ai_usage(db: PgPool) {
    let w = world(db, Deps::default()).await;
    for sub in ["member", "ops", "fixer"] {
        let response = w
            .call(Method::GET, "/admin/v1/system/ai-usage", sub, None)
            .await;
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{sub}");
    }
    assert!(w.trail.0.lock().unwrap().is_empty());
    let response = w
        .call(Method::GET, "/admin/v1/system/ai-usage", "ada", None)
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    assert_eq!(
        w.trail.0.lock().unwrap()[0].operation,
        "system.admin_ai_usage"
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn todays_usage_lists_capped_workspaces_first(db: PgPool) {
    let (meter, capped, busy) = usage(&db).await;
    let w = world(
        db,
        Deps {
            ai_usage: meter,
            ..Deps::default()
        },
    )
    .await;
    let response = w
        .call(Method::GET, "/admin/v1/system/ai-usage", "ada", None)
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    assert_eq!(
        response.json,
        json!({
            "day": "2026-10-16",
            "resets_at": "2026-10-17T00:00:00Z",
            "cap": 3,
            "workspaces": [
                {
                    "workspace_id": capped.to_string(),
                    "calls": 3, "refused": 2, "capped": true,
                    "capped_at": "2026-10-16T12:00:00Z", "remaining": 0,
                    "uses": [{"role": "decide", "feature": "conversations.triage", "calls": 3,
                              "failed": 0, "prompt_tokens": 0, "completion_tokens": 0}]
                },
                {
                    "workspace_id": busy.to_string(),
                    "calls": 2, "refused": 0, "capped": false,
                    "capped_at": null, "remaining": 1,
                    "uses": [
                        {"role": "decide", "feature": "conversations.triage", "calls": 1,
                         "failed": 1, "prompt_tokens": 120, "completion_tokens": 4},
                        {"role": "embed", "feature": "conversations.triage", "calls": 1,
                         "failed": 0, "prompt_tokens": 0, "completion_tokens": 0}
                    ]
                }
            ]
        })
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_day_has_no_remaining_and_a_quiet_day_is_empty(db: PgPool) {
    let (meter, ..) = usage(&db).await;
    // Asked the next day about the day before.
    let tomorrow = Arc::new(FixedClock::new("2026-10-17T08:00:00Z".parse().unwrap()));
    let later = world(
        db,
        Deps {
            ai_usage: meter.with_clock(tomorrow),
            ..Deps::default()
        },
    )
    .await;
    let response = later
        .call(
            Method::GET,
            "/admin/v1/system/ai-usage?day=2026-10-16&limit=1",
            "ada",
            None,
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.json);
    let workspaces = response.json["workspaces"].as_array().unwrap();
    assert_eq!(workspaces.len(), 1, "limit");
    assert_eq!(workspaces[0]["remaining"], json!(null));
    assert_eq!(workspaces[0]["capped"], json!(true));

    let today = later
        .call(Method::GET, "/admin/v1/system/ai-usage", "ada", None)
        .await;
    assert_eq!(today.json["day"], "2026-10-17");
    assert_eq!(today.json["workspaces"], json!([]));

    let bad = later
        .call(
            Method::GET,
            "/admin/v1/system/ai-usage?day=yesterday",
            "ada",
            None,
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}
