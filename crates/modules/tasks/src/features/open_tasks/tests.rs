//! The open tasks about a conversation and its sender.

use campbooks_kernel::tenancy::WorkspaceId;
use serde_json::json;
use sqlx::PgPool;

use crate::testkit::{CONVERSATION, INBOX, Kit};

const OTHER_INBOX: &str = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c99";

async fn ask(
    kit: &Kit,
    workspace: WorkspaceId,
    inbox: &str,
    conversation: &str,
    sender: &str,
) -> Vec<(String, Option<String>)> {
    let port = crate::open_tasks();
    let mut tx = kit.begin(workspace).await;
    let found = port
        .open_tasks(&mut tx, workspace, inbox, conversation, sender)
        .await
        .unwrap();
    found
        .into_iter()
        .map(|task| (task.title, task.due_on.map(|day| day.to_string())))
        .collect()
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_conversation_or_the_same_sender_in_the_same_inbox(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    let source = json!({"type": "conversations.conversation", "id": CONVERSATION});
    kit.add(
        &ana,
        workspace,
        json!({
            "title": "Pay the EDP bill",
            "due_on": "2026-10-30",
            "source": source,
            "source_sender": "billing@edp.example",
        }),
    )
    .await;
    kit.add(&ana, workspace, json!({"title": "Unrelated"}))
        .await;

    // The same conversation.
    let by_conversation = ask(&kit, workspace, INBOX, CONVERSATION, "other@example.com").await;
    assert_eq!(
        by_conversation,
        [("Pay the EDP bill".to_owned(), Some("2026-10-30".to_owned()))]
    );
    // A new conversation from the same sender, in the same inbox.
    let by_sender = ask(
        &kit,
        workspace,
        INBOX,
        "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c55",
        "Billing@EDP.example",
    )
    .await;
    assert_eq!(by_sender.len(), 1);
    // The same sender in another inbox: nothing.
    let elsewhere = ask(
        &kit,
        workspace,
        OTHER_INBOX,
        "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c56",
        "billing@edp.example",
    )
    .await;
    assert!(elsewhere.is_empty());
    // A closed task is not open.
    sqlx::query("UPDATE tasks.tasks SET state = 'done' WHERE title = 'Pay the EDP bill'")
        .execute(&kit.db)
        .await
        .unwrap();
    assert!(
        ask(&kit, workspace, INBOX, CONVERSATION, "billing@edp.example")
            .await
            .is_empty()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn at_most_ten_newest_first(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (ana, ana_id) = kit.member("ana", workspace);
    kit.let_see(INBOX, ana_id);
    let source = json!({"type": "conversations.conversation", "id": CONVERSATION});
    for n in 0..12 {
        kit.add(
            &ana,
            workspace,
            json!({"title": format!("Task {n}"), "source": source}),
        )
        .await;
    }
    let found = ask(&kit, workspace, INBOX, CONVERSATION, "").await;
    assert_eq!(found.len(), 10);
    assert_eq!(found[0].0, "Task 11");
}
