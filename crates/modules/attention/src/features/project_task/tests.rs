//! Tasks as items on their assignee's Home.

use campbooks_kernel::tenancy::WorkspaceId;
use chrono::{NaiveDate, TimeDelta, Utc};
use sqlx::PgPool;

use crate::{
    domain::Reason,
    events::{TaskState, TaskVerb},
    testkit::{Kit, Need, Task},
};

const INBOX: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22";
const CONVERSATION: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11";
const TASK: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9d01";

/// The day the list runs on (UTC, the default offset): a task is on Home
/// only around it (campbooks-cloud#358).
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
async fn a_task_is_the_assignees_item_and_nobody_elses(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (joao, _) = kit.member("joao", workspace);
    let mut task = Task::open(TASK, 1, ana_id);
    task.verb = TaskVerb::Pay;
    task.due_on = Some(today());
    kit.task(workspace, &task).await;

    let mine = items(&kit, &ana, workspace).await;
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0]["reason"], "pay");
    assert_eq!(mine[0]["trust"], "sure");
    assert_eq!(mine[0]["due_on"], today().to_string());
    assert_eq!(mine[0]["band"], "today");
    assert_eq!(mine[0]["band_reason"]["rule"], "due_today");
    assert_eq!(mine[0]["record"]["type"], "tasks.task");
    assert_eq!(mine[0]["record"]["id"], TASK);
    assert_eq!(mine[0]["cause"]["id"], TASK);
    assert!(items(&kit, &joao, workspace).await.is_empty());
    assert_eq!(kit.events("attention.item_changed").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn handing_it_over_moves_the_item(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (joao, joao_id) = kit.member("joao", workspace);
    let mut task = Task::open(TASK, 1, ana_id);
    task.due_on = Some(today());
    kit.task(workspace, &task).await;
    assert_eq!(items(&kit, &ana, workspace).await.len(), 1);

    task.revision = 2;
    task.assignee = joao_id;
    kit.task(workspace, &task).await;
    assert!(items(&kit, &ana, workspace).await.is_empty());
    assert_eq!(items(&kit, &joao, workspace).await.len(), 1);
    // Both Homes are told: the one it came to and the one it left.
    assert_eq!(kit.events("attention.item_changed").await.len(), 3);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn done_clears_it_whatever_order_the_events_arrive_in(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let orders: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    for (n, order) in orders.iter().enumerate() {
        let id = format!("0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9e{n:02}");
        let mut created = Task::open(&id, 1, ana_id);
        created.due_on = Some(today());
        let mut edited = created.clone();
        edited.revision = 2;
        edited.due_on = Some(today() + TimeDelta::days(1));
        let mut done = edited.clone();
        done.revision = 3;
        done.state = TaskState::Done;
        let events = [created, edited, done];
        for index in order {
            kit.task(workspace, &events[*index]).await;
        }
    }
    assert!(items(&kit, &ana, workspace).await.is_empty());

    // A clear that outruns the hand-off before it still lands the item with
    // the member who closed it, not the one who handed it over.
    let (_, joao_id) = kit.member("joao", workspace);
    let id = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9eaa";
    let created = Task::open(id, 1, ana_id);
    let handed = Task::open(id, 2, joao_id);
    let mut done = handed.clone();
    done.revision = 3;
    done.state = TaskState::Done;
    for event in [&created, &done, &handed] {
        kit.task(workspace, event).await;
    }
    let (container,): (String,) =
        sqlx::query_as("SELECT container_id FROM attention.items WHERE record_id = $1")
            .bind(id)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(container, joao_id.to_string());

    // Without the clear, the newest open revision wins in any order.
    for (n, order) in [[0usize, 1], [1, 0]].iter().enumerate() {
        let id = format!("0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9f{n:02}");
        let mut first = Task::open(&id, 1, ana_id);
        first.due_on = Some(today());
        let mut second = first.clone();
        second.revision = 2;
        second.due_on = Some(today() + TimeDelta::days(1));
        let events = [first, second];
        for index in order {
            kit.task(workspace, &events[*index]).await;
        }
    }
    let open = items(&kit, &ana, workspace).await;
    assert_eq!(open.len(), 2);
    let tomorrow = (today() + TimeDelta::days(1)).to_string();
    assert!(open.iter().all(|item| item["due_on"] == tomorrow.as_str()));
}

#[sqlx::test(migrations = "../../../migrations")]
async fn later_hides_it_until_its_time_and_it_comes_back_by_itself(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let mut task = Task::open(TASK, 1, ana_id);
    task.later_until = Some(Utc::now() + TimeDelta::days(2));
    kit.task(workspace, &task).await;
    assert!(items(&kit, &ana, workspace).await.is_empty());

    // Monday morning comes: no event, no job. Undated, it is back on Home
    // all the same: its Later is over.
    sqlx::query("UPDATE attention.items SET hidden_until = now() - interval '1 minute'")
        .execute(&kit.db)
        .await
        .unwrap();
    assert_eq!(items(&kit, &ana, workspace).await.len(), 1);
}

/// campbooks-cloud#358: Home shows the tasks for today (overdue, due today
/// or tomorrow, planned for today or before, back from Later) and says how
/// many others are in Tasks.
#[sqlx::test(migrations = "../../../migrations")]
async fn only_todays_tasks_are_on_home(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let day = |offset: i64| Some(today() + TimeDelta::days(offset));
    let task = |n: u32, due_on: Option<NaiveDate>, plan_on: Option<NaiveDate>| {
        let mut task = Task::open(
            &format!("0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9a{n:02}"),
            1,
            ana_id,
        );
        task.due_on = due_on;
        task.plan_on = plan_on;
        task
    };
    let shown = [
        task(1, day(-3), None),
        task(2, day(0), None),
        task(3, day(1), None),
        task(4, None, day(0)),
        task(5, None, day(-1)),
        // Planned for tomorrow, due today: due today.
        task(6, day(0), day(1)),
    ];
    let elsewhere = [
        task(11, day(2), None),
        task(12, None, None),
        task(13, None, day(1)),
        task(14, day(5), day(3)),
    ];
    for each in shown.iter().chain(&elsewhere) {
        kit.task(workspace, each).await;
    }
    let stored: (String, Option<NaiveDate>) =
        sqlx::query_as("SELECT plan, plan_on FROM attention.items WHERE record_id = $1")
            .bind(&shown[3].id)
            .fetch_one(&kit.db)
            .await
            .unwrap();
    assert_eq!(stored, ("date".to_owned(), Some(today())));

    let list = kit
        .get(&ana, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    let listed: Vec<(String, String, String)> = list.json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["record"]["id"].as_str().unwrap().to_owned(),
                i["band"].as_str().unwrap().to_owned(),
                i["band_reason"]["rule"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let row = |task: &Task, band: &str, rule: &str| (task.id.clone(), band.into(), rule.into());
    assert_eq!(
        listed,
        [
            row(&shown[0], "now", "overdue"),
            // Today: by date, a planned day counting when it has no due date.
            row(&shown[4], "today", "planned_today"),
            row(&shown[1], "today", "due_today"),
            row(&shown[3], "today", "planned_today"),
            row(&shown[5], "today", "due_today"),
            row(&shown[2], "today", "due_tomorrow"),
        ]
    );
    assert_eq!(list.json["tasks_elsewhere"], 4);

    // A task projected before plans were kept stays as it was shown, until
    // it next changes.
    sqlx::query("UPDATE attention.items SET plan = NULL, plan_on = NULL WHERE record_id = $1")
        .bind(&elsewhere[1].id)
        .execute(&kit.db)
        .await
        .unwrap();
    let list = kit
        .get(&ana, &format!("/api/v1/w/{workspace}/attention"))
        .await;
    assert_eq!(list.json["items"].as_array().unwrap().len(), 7);
    assert_eq!(list.json["tasks_elsewhere"], 3);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_task_replaces_its_conversations_do_item(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    kit.need(
        workspace,
        &Need {
            conversation: CONVERSATION.into(),
            inbox: INBOX.into(),
            message: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c33".into(),
            revision: 1,
            reason: Reason::Do,
            due_on: None,
            sent_at: Utc::now() - TimeDelta::hours(1),
            latest: None,
            lapses_at: None,
            warning: false,
            from_person: None,
        },
    )
    .await;
    assert_eq!(items(&kit, &ana, workspace).await.len(), 1);

    let mut task = Task::open(TASK, 1, ana_id);
    task.source = Some(CONVERSATION.into());
    task.due_on = Some(today());
    kit.task(workspace, &task).await;
    let listed = items(&kit, &ana, workspace).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["record"]["type"], "tasks.task");

    // Not needed: the task's line is gone and the conversation's own does not
    // come back, because it is the one the task came from.
    task.revision = 2;
    task.state = TaskState::NotNeeded;
    kit.task(workspace, &task).await;
    assert!(items(&kit, &ana, workspace).await.is_empty());
}

fn need_of(reason: Reason, revision: i64, sent_at: chrono::DateTime<Utc>) -> Need {
    Need {
        conversation: CONVERSATION.into(),
        inbox: INBOX.into(),
        message: "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c33".into(),
        revision,
        reason,
        due_on: None,
        sent_at,
        latest: None,
        lapses_at: None,
        warning: false,
        from_person: None,
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn paying_the_task_finishes_the_conversations_pay_item_too(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    kit.need(
        workspace,
        &need_of(Reason::Pay, 1, Utc::now() - TimeDelta::hours(1)),
    )
    .await;
    let mut task = Task::open(TASK, 1, ana_id);
    task.verb = TaskVerb::Pay;
    task.source = Some(CONVERSATION.into());
    task.due_on = Some(today());
    kit.task(workspace, &task).await;
    assert_eq!(items(&kit, &ana, workspace).await.len(), 1);

    task.revision = 2;
    task.state = TaskState::Done;
    kit.task(workspace, &task).await;
    assert!(items(&kit, &ana, workspace).await.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_new_bill_in_the_same_thread_shows_again(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    kit.need(
        workspace,
        &need_of(Reason::Pay, 1, Utc::now() - TimeDelta::hours(1)),
    )
    .await;
    let mut task = Task::open(TASK, 1, ana_id);
    task.verb = TaskVerb::Pay;
    task.source = Some(CONVERSATION.into());
    kit.task(workspace, &task).await;
    task.revision = 2;
    task.state = TaskState::Done;
    kit.task(workspace, &task).await;
    assert!(items(&kit, &ana, workspace).await.is_empty());

    // A newer message in the thread opens a newer Pay item.
    kit.need(
        workspace,
        &need_of(Reason::Pay, 2, Utc::now() + TimeDelta::hours(1)),
    )
    .await;
    let listed = items(&kit, &ana, workspace).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["record"]["type"], "conversations.conversation");
    assert_eq!(listed[0]["reason"], "pay");
}
