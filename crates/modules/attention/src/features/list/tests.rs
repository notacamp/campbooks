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
        lapses_at: None,
        warning: false,
        from_person: None,
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

fn bands(list: &serde_json::Value) -> Vec<String> {
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["band"].as_str().unwrap().to_owned())
        .collect()
}

/// campbooks-cloud#358: by band whatever the reason, then the soonest date
/// first, then the oldest cause first.
#[sqlx::test(migrations = "../../../migrations")]
async fn items_come_by_band_then_date_then_age(db: PgPool) {
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
        need(6, Reason::Do, Some(-8)),
        // But a bill stays until a member answers it (campbooks-cloud#275).
        need(7, Reason::Pay, Some(-9)),
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
            ("pay".into(), Some(today - TimeDelta::days(9))),
            ("pay".into(), Some(today - TimeDelta::days(2))),
            ("pay".into(), Some(today + TimeDelta::days(5))),
            // Undated, nobody known to be waiting: the oldest first.
            ("review".into(), None),
            ("reply".into(), None),
            ("pay".into(), None),
        ]
    );
    assert_eq!(
        bands(&list.json),
        ["now", "now", "week", "whenever", "whenever", "whenever"]
    );
    let items = &list.json["items"];
    assert_eq!(items[0]["overdue"], true);
    assert_eq!(
        items[0]["band_reason"],
        serde_json::json!({"rule": "overdue", "days": 9, "on": null})
    );
    assert_eq!(items[2]["band_reason"]["rule"], "due_on");
    assert_eq!(
        items[2]["band_reason"]["on"],
        (today + TimeDelta::days(5)).to_string()
    );
    assert_eq!(items[3]["band_reason"]["rule"], "no_date");
    assert_eq!(list.json["tasks_elsewhere"], 0);
    let counts = &list.json["counts"];
    assert_eq!(counts[4]["reason"], "pay");
    assert_eq!(counts[4]["count"], 4);
    assert_eq!(counts[3]["count"], 0);
    assert_eq!(counts[2]["count"], 0);
}

/// campbooks-cloud#358: the warning first, a person who has waited four
/// days today, one who wrote an hour ago this week, mail a rule keeps
/// whenever; each with why.
#[sqlx::test(migrations = "../../../migrations")]
async fn warnings_and_people_band_by_the_facts_the_need_carries(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    kit.let_see(INBOX, sam_id);
    let mut warning = need(1, Reason::Review, None);
    warning.warning = true;
    let mut waited = need(2, Reason::Reply, None);
    waited.from_person = Some(true);
    waited.sent_at = Utc::now() - TimeDelta::hours(4 * 24 + 2);
    let mut fresh = need(3, Reason::Reply, None);
    fresh.from_person = Some(true);
    let mut service = need(4, Reason::Reply, None);
    service.from_person = Some(false);
    service.sent_at = Utc::now() - TimeDelta::days(6);
    let look = need(5, Reason::WorthALook, None);
    for need in [&look, &service, &fresh, &waited, &warning] {
        kit.need(workspace, need).await;
    }
    let stored: (bool, Option<bool>) =
        sqlx::query_as("SELECT warning, from_person FROM attention.items WHERE record_id = $1")
            .bind(&warning.conversation)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(stored, (true, None), "the item keeps the facts");

    let list = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.json);
    let ids: Vec<&str> = list.json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["record"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            warning.conversation.as_str(),
            waited.conversation.as_str(),
            fresh.conversation.as_str(),
            service.conversation.as_str(),
            look.conversation.as_str(),
        ]
    );
    assert_eq!(
        bands(&list.json),
        ["now", "today", "week", "whenever", "whenever"]
    );
    let rules: Vec<&str> = list.json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["band_reason"]["rule"].as_str().unwrap())
        .collect();
    assert_eq!(
        rules,
        [
            "warning",
            "waited",
            "waiting_answer",
            "no_date",
            "kept_by_rule"
        ]
    );
    assert_eq!(list.json["items"][1]["band_reason"]["days"], 4);

    // Done today leaves no task out.
    let done = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/attention?state=cleared"),
        )
        .await;
    assert_eq!(done.json["tasks_elsewhere"], 0);
}

/// campbooks-cloud#275: a sign-in link valid for ten minutes leaves Home
/// once they are over, before its producer has cleared it, and a need
/// cleared as lapsed is not something the member did today.
#[sqlx::test(migrations = "../../../migrations")]
async fn a_lapsed_item_leaves_the_list_at_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    kit.let_see(INBOX, sam_id);
    let mut link = need(1, Reason::Do, None);
    link.lapses_at = Some(Utc::now() - TimeDelta::minutes(1));
    let mut later = need(2, Reason::Do, None);
    later.lapses_at = Some(Utc::now() + TimeDelta::minutes(9));
    let mut old = need(3, Reason::Reply, None);
    old.lapses_at = Some(Utc::now() - TimeDelta::minutes(30));
    for need in [&link, &later, &old] {
        kit.need(workspace, need).await;
    }
    let stored: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT lapses_at FROM attention.items WHERE record_id = $1")
            .bind(&link.conversation)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert!(stored.is_some(), "the item keeps its lapse time");
    let list = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    let ids: Vec<&str> = list.json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["record"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [later.conversation.as_str()]);

    // The producer clears the old one as lapsed: not in Done today.
    old.revision = 2;
    kit.cleared(workspace, &old, "lapsed", None).await;
    let done = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/attention?state=cleared"),
        )
        .await;
    assert!(
        done.json["items"].as_array().unwrap().is_empty(),
        "{:?}",
        done.json
    );
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
    // A client acting for no member has no attention list (ADR 0017).
    let client = kit.client(workspace, "attention:read");
    let refused = kit
        .get(&client, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
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
    assert_eq!(list.json["items"][0]["band"], "now");
    let bad = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/attention?utc_offset=900"),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.json["code"], "invalid_offset");
}

/// campbooks-cloud#293: a wait is not in the queue; a Nudge is, for today
/// (campbooks-cloud#358), and stays however overdue; `state=waiting` lists
/// the waits, soonest first, with no band; hearing back is in Done today and
/// going back to waiting is not.
#[sqlx::test(migrations = "../../../migrations")]
async fn waits_stay_out_of_the_queue_and_nudges_are_for_today(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (sam, sam_id) = kit.member("sam", workspace);
    kit.let_see(INBOX, sam_id);
    let today = Utc::now().date_naive();
    let waiting_soon = need(1, Reason::Waiting, Some(3));
    let waiting_later = need(2, Reason::Waiting, Some(6));
    let nudge = need(3, Reason::Nudge, Some(-10));
    for need in [
        &waiting_later,
        &waiting_soon,
        &nudge,
        &need(4, Reason::Pay, None),
        &need(5, Reason::WorthALook, None),
    ] {
        kit.need(workspace, need).await;
    }
    let list = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    assert_eq!(list.status, StatusCode::OK, "{:?}", list.json);
    assert_eq!(
        reasons(&list.json),
        [
            ("nudge".into(), Some(today - TimeDelta::days(10))),
            // Whenever: the oldest first.
            ("worth_a_look".into(), None),
            ("pay".into(), None),
        ]
    );
    assert_eq!(bands(&list.json), ["today", "whenever", "whenever"]);
    assert_eq!(
        list.json["items"][0]["band_reason"],
        serde_json::json!({
            "rule": "nudge_due",
            "days": null,
            "on": (today - TimeDelta::days(10)).to_string(),
        })
    );
    let counts: Vec<&str> = list.json["counts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["reason"].as_str().unwrap())
        .collect();
    assert_eq!(
        counts,
        [
            "review",
            "reply",
            "decide",
            "do",
            "pay",
            "nudge",
            "worth_a_look"
        ]
    );

    let waiting = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/attention?state=waiting"),
        )
        .await;
    assert_eq!(waiting.status, StatusCode::OK, "{:?}", waiting.json);
    assert_eq!(
        reasons(&waiting.json),
        [
            ("waiting".into(), Some(today + TimeDelta::days(3))),
            ("waiting".into(), Some(today + TimeDelta::days(6))),
        ]
    );
    assert_eq!(waiting.json["counts"][0]["reason"], "waiting");
    assert_eq!(waiting.json["counts"][0]["count"], 2);
    assert_eq!(waiting.json["items"][0]["band"], serde_json::Value::Null);

    // Nudged: back to waiting, out of the queue, not done today. Heard
    // back: done today.
    let mut nudged = need(3, Reason::Waiting, Some(4));
    nudged.revision = 2;
    kit.need(workspace, &nudged).await;
    let mut heard = need(1, Reason::Waiting, Some(3));
    heard.revision = 2;
    kit.cleared(workspace, &heard, "heard_back", None).await;
    let list = kit
        .get(&sam, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    assert_eq!(
        reasons(&list.json),
        [("worth_a_look".into(), None), ("pay".into(), None)]
    );
    let done = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/attention?state=cleared"),
        )
        .await;
    let items = done.json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0]["cleared_because"], "heard_back");
    let waiting = kit
        .get(
            &sam,
            &format!("/api/v1/w/{workspace}/attention?state=waiting"),
        )
        .await;
    assert_eq!(
        reasons(&waiting.json),
        [
            ("waiting".into(), Some(today + TimeDelta::days(4))),
            ("waiting".into(), Some(today + TimeDelta::days(6))),
        ]
    );
}
