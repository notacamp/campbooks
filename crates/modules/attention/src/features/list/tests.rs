//! What needs the caller, and Done today.

use axum::http::StatusCode;
use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{NaiveDate, TimeDelta, Utc};
use sqlx::PgPool;

use crate::{
    domain::{ItemId, Reason},
    testkit::{Kit, Need},
};

const INBOX: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22";

fn need(n: u32, reason: Reason, due_in: Option<i64>) -> Need {
    Need {
        conversation: format!("0192f0c4-7d4e-7b39-a7f1-{n:012}"),
        inbox: INBOX.into(),
        message: format!("0192f0c4-7d4e-7b39-a7f2-{n:012}"),
        revision: 1,
        reason,
        due_on: due_in.map(|days| Utc::now().date_naive() + TimeDelta::days(days)),
        sent_at: Utc::now() - TimeDelta::hours(i64::from(n)),
        latest: None,
    }
}

fn reasons(list: &serde_json::Value) -> Vec<(String, Option<NaiveDate>)> {
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["reason"].as_str().unwrap().to_owned(),
                i["due_on"].as_str().map(|d| d.parse().unwrap()),
            )
        })
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn items_come_by_reason_then_overdue_then_due_date(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    kit.let_see(INBOX, sam_id);
    for need in [
        need(1, Reason::Pay, None),
        need(2, Reason::Pay, Some(5)),
        need(3, Reason::Pay, Some(-2)),
        need(4, Reason::Reply, None),
        need(5, Reason::Review, None),
        // More than a week overdue: left out.
        need(6, Reason::Pay, Some(-8)),
    ] {
        kit.need(workspace, &need).await;
    }
    let list = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.json);
    let today = Utc::now().date_naive();
    assert_eq!(
        reasons(&list.json),
        [
            ("review".into(), None),
            ("reply".into(), None),
            ("pay".into(), Some(today - TimeDelta::days(2))),
            ("pay".into(), Some(today + TimeDelta::days(5))),
            ("pay".into(), None),
        ]
    );
    assert_eq!(list.json["items"][2]["overdue"], true);
    let counts = &list.json["counts"];
    assert_eq!(counts[4]["reason"], "pay");
    assert_eq!(counts[4]["count"], 3);
    assert_eq!(counts[2]["count"], 0);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn someone_who_cannot_see_the_inbox_sees_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (eve, _) = kit.member("eve", workspace);
    kit.need(workspace, &need(1, Reason::Reply, None)).await;
    let list = kit
        .get(&eve, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    assert!(list.json["items"].as_array().unwrap().is_empty());
    let client = kit.client(workspace, "attention:read");
    let all = kit
        .get(&client, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    assert_eq!(all.json["items"].as_array().unwrap().len(), 1);
    let anonymous = kit
        .get("", &format!("/api/v1/w/{workspace}/attention"))
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn done_today_lists_what_left_and_what_was_put_off(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    kit.let_see(INBOX, sam_id);
    let replied = need(1, Reason::Reply, None);
    let later = need(2, Reason::Pay, None);
    let gone = need(3, Reason::Do, None);
    let never = need(4, Reason::Pay, None);
    for need in [&replied, &later, &gone, &never] {
        kit.need(workspace, need).await;
    }
    let mut cleared = need(1, Reason::Reply, None);
    cleared.revision = 2;
    kit.cleared(workspace, &cleared, "done", Some(sam_id)).await;
    let mut superseded = need(3, Reason::Do, None);
    superseded.revision = 2;
    kit.cleared(workspace, &superseded, "superseded", None)
        .await;
    // A taught rule saying "never" is not something the member did today.
    let mut by_rule = need(4, Reason::Pay, None);
    by_rule.revision = 2;
    kit.cleared(workspace, &by_rule, "rule", None).await;
    let open = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    let item: ItemId = open.json["items"][0]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let until = Utc::now() + TimeDelta::days(3);
    kit.post(
        &sam,
        &format!("/api/v1/w/{workspace}/attention/{item}/snooze"),
        Some(serde_json::json!({"until": until})),
    )
    .await;

    let today = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/attention?state=cleared"),
        )
        .await;
    let items = today.json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(items[0]["reason"], "pay");
    assert!(items[0]["snoozed_until"].is_string());
    assert_eq!(items[1]["cleared_because"], "done");
    assert_eq!(items[1]["cleared_by"], sam_id.to_string());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn today_is_the_callers_today(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    kit.let_see(INBOX, sam_id);
    // Due yesterday for someone 14 hours ahead of UTC is still due "the day
    // after" in UTC: what is overdue follows the caller's clock.
    let ahead = (Utc::now() + TimeDelta::hours(14)).date_naive();
    let mut due = need(1, Reason::Pay, None);
    due.due_on = Some(ahead - TimeDelta::days(1));
    kit.need(workspace, &due).await;
    let uri = format!("/api/v1/w/{workspace}/attention?utc_offset=840");
    let list = kit.get(&sam, &uri).await;
    assert_eq!(list.json["items"][0]["overdue"], true);
    let bad = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/attention?utc_offset=900"),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.json["code"], "invalid_offset");
}
