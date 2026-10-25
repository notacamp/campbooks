//! Invitations awaiting an answer as Decide items on their member's Home
//! (campbooks-cloud#360).

use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{NaiveDate, TimeDelta, Utc};
use sqlx::PgPool;

use crate::testkit::{Invitation, Kit};

const EVENT: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9a01";

/// The day the list runs on (UTC, the default offset).
fn today() -> NaiveDate {
    Utc::now().date_naive()
}

async fn items(kit: &Kit, token: &str, workspace: WorkspaceId) -> Vec<serde_json::Value> {
    kit.get(token, &format!("/api/v1/w/{workspace}/attention"))
        .await
        .json["items"]
        .as_array()
        .unwrap()
        .clone()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_invitation_is_a_decide_item_for_its_member_alone(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (joao, _) = kit.member("joao", workspace);
    kit.invitation(
        workspace,
        &Invitation::open(EVENT, 1, ana_id, today() + TimeDelta::days(1)),
    )
    .await;

    let mine = items(&kit, &ana, workspace).await;
    assert_eq!(mine.len(), 1, "{mine:?}");
    let item = &mine[0];
    assert_eq!(item["reason"], "decide");
    assert_eq!(item["trust"], "sure");
    assert_eq!(item["record"]["type"], "schedule.event");
    assert_eq!(item["record"]["id"], EVENT);
    assert_eq!(item["cause"]["id"], EVENT);
    assert_eq!(item["container"]["type"], "schedule.member");
    assert_eq!(item["band"], "today");
    assert_eq!(item["band_reason"]["rule"], "event_tomorrow");
    let counts = kit
        .get(&ana, &format!("/api/v1/w/{workspace}/attention"))
        .await
        .json["counts"]
        .clone();
    let decide = counts
        .as_array()
        .unwrap()
        .iter()
        .find(|count| count["reason"] == "decide")
        .unwrap()
        .clone();
    assert_eq!(decide["count"], 1);
    assert!(items(&kit, &joao, workspace).await.is_empty());
    assert_eq!(kit.events("attention.item_changed").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn its_band_follows_the_day_of_the_event(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    for (n, days) in [0, 1, 5, 12].into_iter().enumerate() {
        let id = format!("0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9b{n:02}");
        kit.invitation(
            workspace,
            &Invitation::open(&id, 1, ana_id, today() + TimeDelta::days(days)),
        )
        .await;
    }
    let listed = items(&kit, &ana, workspace).await;
    let bands: Vec<(String, String)> = listed
        .iter()
        .map(|item| {
            (
                item["band"].as_str().unwrap().to_owned(),
                item["band_reason"]["rule"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        bands,
        [
            ("today".into(), "event_today".into()),
            ("today".into(), "event_tomorrow".into()),
            ("week".into(), "event_on".into()),
            ("whenever".into(), "event_on".into()),
        ]
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_answer_a_cancel_or_the_start_takes_it_off_home(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let open = Invitation::open(EVENT, 1, ana_id, today() + TimeDelta::days(3));
    kit.invitation(workspace, &open).await;
    assert_eq!(items(&kit, &ana, workspace).await.len(), 1);

    kit.invitation(workspace, &open.closed(2, "answered")).await;
    assert!(items(&kit, &ana, workspace).await.is_empty());
    let (state, because): (String, Option<String>) =
        sqlx::query_as("SELECT state, cleared_because FROM attention.items WHERE record_id = $1")
            .bind(EVENT)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(
        (state.as_str(), because.as_deref()),
        ("cleared", Some("answered"))
    );

    // The answer taken back: the same item is open again.
    let mut again = open.clone();
    again.revision = 3;
    kit.invitation(workspace, &again).await;
    assert_eq!(items(&kit, &ana, workspace).await.len(), 1);

    kit.invitation(workspace, &open.closed(4, "cancelled"))
        .await;
    assert!(items(&kit, &ana, workspace).await.is_empty());

    // Once its event starts it leaves the list, before any close arrives.
    let other = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9a02";
    let mut started = Invitation::open(other, 1, ana_id, today());
    started.lapses_at = Utc::now() - TimeDelta::minutes(1);
    kit.invitation(workspace, &started).await;
    assert!(items(&kit, &ana, workspace).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn stale_and_out_of_order_revisions_are_ignored(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let open = Invitation::open(EVENT, 1, ana_id, today() + TimeDelta::days(2));
    // The answer arrives before the invitation it answers.
    kit.invitation(workspace, &open.closed(2, "answered")).await;
    kit.invitation(workspace, &open).await;
    assert!(items(&kit, &ana, workspace).await.is_empty());
    // A tombstone nobody saw is not announced.
    assert!(kit.events("attention.item_changed").await.is_empty());

    // A newer open wins; an older one replayed changes nothing.
    let mut moved = open.clone();
    moved.revision = 3;
    moved.on = Some(today() + TimeDelta::days(4));
    kit.invitation(workspace, &moved).await;
    kit.invitation(workspace, &open).await;
    let listed = items(&kit, &ana, workspace).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0]["due_on"],
        (today() + TimeDelta::days(4)).to_string()
    );
}
