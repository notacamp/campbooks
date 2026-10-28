//! A reply of the member marks the open Do tasks of its conversation, and
//! nothing else.

use campbooks_kernel::tenancy::{UserId, WorkspaceId};
use campbooks_kernel::testing;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, types::Uuid};

use crate::{
    inbound::MemberReplied,
    testkit::{CONVERSATION, INBOX, Kit},
};

const OTHER_CONVERSATION: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c12";

fn reply(
    workspace: WorkspaceId,
    owner: UserId,
    conversation: &str,
    sent_at: DateTime<Utc>,
) -> MemberReplied {
    MemberReplied {
        workspace: workspace.to_string(),
        inbox: INBOX.to_owned(),
        conversation: conversation.to_owned(),
        message: Uuid::now_v7(),
        sent_at,
        owner,
    }
}

/// A task of `token`'s from `CONVERSATION`, as Scout makes them.
async fn from_email(kit: &Kit, token: &str, workspace: WorkspaceId, body: Value) -> String {
    let mut body = body;
    body["source"] = json!({"type": "conversations.conversation", "id": CONVERSATION});
    kit.add(token, workspace, body).await["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn fetch(kit: &Kit, token: &str, workspace: WorkspaceId, id: &str) -> Value {
    kit.get(token, &format!("/api/v1/w/{workspace}/tasks/{id}"))
        .await
        .json
}

fn later() -> DateTime<Utc> {
    Utc::now() + TimeDelta::hours(1)
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_marks_the_open_do_task_of_its_conversation_and_only_it(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    let task = from_email(&kit, &ana, workspace, json!({"title": "Send the lease"})).await;
    let pay = from_email(
        &kit,
        &ana,
        workspace,
        json!({"title": "Pay the deposit", "verb": "pay"}),
    )
    .await;
    let done = from_email(&kit, &ana, workspace, json!({"title": "Already done"})).await;
    kit.post(
        &ana,
        &format!("/api/v1/w/{workspace}/tasks/{done}/done"),
        Some(json!({"revision": 1})),
    )
    .await;
    let elsewhere = kit
        .add(&ana, workspace, json!({"title": "Not from the email"}))
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let sent = later();
    let sent_reply = reply(workspace, ana_id, CONVERSATION, sent);
    kit.publish(workspace, &sent_reply).await;

    let marked = fetch(&kit, &ana, workspace, &task).await;
    assert_eq!(marked["task"]["asks_done"], true);
    assert_eq!(
        marked["task"]["replied"]["message"],
        sent_reply.message.to_string()
    );
    let at: DateTime<Utc> = marked["task"]["replied"]["at"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(at, sent);
    assert_eq!(marked["task"]["revision"], 2);
    let last = marked["history"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(last["what"], "replied");
    // Facts only: the event says it, and tracks it, with no text.
    let events = kit.events("tasks.task_changed").await;
    let latest = events.iter().rfind(|event| event["task"] == task).unwrap();
    assert_eq!(latest["asks_done"], true);
    assert_eq!(latest["change"], "replied");
    let tracked = testing::tracked_events(&kit.db, "tasks.task_reply_noticed").await;
    assert_eq!(tracked.len(), 1);

    // A bill is paid, not replied to; a closed task asks nothing; a task of
    // another source is left alone.
    for untouched in [&pay, &done, &elsewhere] {
        let read = fetch(&kit, &ana, workspace, untouched).await;
        assert_eq!(read["task"]["asks_done"], false, "{untouched}");
        assert_eq!(read["task"]["replied"], Value::Null, "{untouched}");
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_in_another_conversation_or_of_another_member_marks_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    let (_, joao_id) = kit.member("joao", workspace);
    kit.let_see(INBOX, ana_id);
    let task = from_email(&kit, &ana, workspace, json!({"title": "Send the lease"})).await;

    kit.publish(
        workspace,
        &reply(workspace, ana_id, OTHER_CONVERSATION, later()),
    )
    .await;
    kit.publish(workspace, &reply(workspace, joao_id, CONVERSATION, later()))
        .await;
    let read = fetch(&kit, &ana, workspace, &task).await;
    assert_eq!(read["task"]["asks_done"], false);
    assert_eq!(read["task"]["revision"], 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reply_older_than_the_task_marks_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    let task = from_email(&kit, &ana, workspace, json!({"title": "Send the lease"})).await;

    // Mail the member sent before Scout made the task (an old sync).
    let before = Utc::now() - TimeDelta::hours(1);
    kit.publish(workspace, &reply(workspace, ana_id, CONVERSATION, before))
        .await;
    let read = fetch(&kit, &ana, workspace, &task).await;
    assert_eq!(read["task"]["asks_done"], false);
    assert_eq!(read["task"]["replied"], Value::Null);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_repeated_delivery_changes_nothing_and_only_a_newer_reply_moves_it(db: PgPool) {
    let kit = Kit::new(db).await.tracking();
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    let task = from_email(&kit, &ana, workspace, json!({"title": "Send the lease"})).await;

    let newer = reply(
        workspace,
        ana_id,
        CONVERSATION,
        later() + TimeDelta::hours(1),
    );
    kit.publish(workspace, &newer).await;
    kit.publish(workspace, &newer).await;
    // An older reply arriving after: the newest is kept.
    kit.publish(workspace, &reply(workspace, ana_id, CONVERSATION, later()))
        .await;
    let read = fetch(&kit, &ana, workspace, &task).await;
    assert_eq!(read["task"]["revision"], 2);
    assert_eq!(
        read["task"]["replied"]["message"],
        newer.message.to_string()
    );
    assert_eq!(
        testing::tracked_events(&kit.db, "tasks.task_reply_noticed")
            .await
            .len(),
        1
    );

    // A still newer one moves it forward.
    let newest = reply(
        workspace,
        ana_id,
        CONVERSATION,
        later() + TimeDelta::hours(2),
    );
    kit.publish(workspace, &newest).await;
    let read = fetch(&kit, &ana, workspace, &task).await;
    assert_eq!(read["task"]["revision"], 3);
    assert_eq!(
        read["task"]["replied"]["message"],
        newest.message.to_string()
    );
}
