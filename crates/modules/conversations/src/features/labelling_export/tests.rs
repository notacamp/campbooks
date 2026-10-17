//! The labelling export over `fixtures/needs.json`.

use campbooks_kernel::tenancy::WorkspaceId;
use sqlx::PgPool;

use super::{LabellingSet, OWNER, export_for_labelling, export_set, manifest, manifest_for};
use crate::{model::InboxRole, testkit::Kit};

/// Every fixture is recent enough.
const ALL: u32 = 100_000;

#[sqlx::test(migrations = "../../../migrations")]
async fn lists_money_mail_only_unlabelled_and_in_the_eval_format(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "needs").await;

    let items = export_for_labelling(&kit.state, workspace, ALL)
        .await
        .unwrap();
    // The bill sorted to Receipts; not the people's questions or phishing.
    assert_eq!(
        items.len(),
        1,
        "{:?}",
        items.iter().map(|i| &i.subject).collect::<Vec<_>>()
    );
    let bill = &items[0];
    assert_eq!(bill.id, "owner-001");
    assert_eq!(bill.subject, "Your invoice INV-2041");
    assert_eq!(bill.from, "Acme Hosting <billing@acme-hosting.example>");
    let item = &bill.item;
    assert_eq!(item["lang"], OWNER);
    assert_eq!(item["workspace"], OWNER);
    assert_eq!(item["split"], "dev");
    assert_eq!(item["labels"], serde_json::json!({}));
    assert_eq!(
        item["message"]["from"]["address"],
        "billing@acme-hosting.example"
    );
    assert_eq!(
        item["message"]["recipient"]["addresses"][0],
        "sam@example.com"
    );
    assert!(
        item["message"]["body"]
            .as_str()
            .unwrap()
            .contains("Amount due")
    );
    assert_eq!(manifest()["questions"][0]["id"], "money");
    assert_eq!(manifest()["private"], true);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_workspace_exports_nothing_of_it(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "needs").await;

    let other = export_for_labelling(&kit.state, WorkspaceId::new(), ALL)
        .await
        .unwrap();
    assert!(other.is_empty());
    // Old mail is left out.
    let recent = export_for_labelling(&kit.state, workspace, 0)
        .await
        .unwrap();
    assert!(recent.is_empty());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_actions_set_lists_mail_that_had_a_need(db: PgPool) {
    let kit = Kit::with_deps(db, crate::testkit::sorting_deps()).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "needs").await;
    let with_needs: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT conversation_id) FROM conversations.needs WHERE workspace_id = $1",
    )
    .bind(workspace.as_uuid())
    .fetch_one(&kit.db)
    .await
    .unwrap();
    assert!(with_needs > 1);
    // A later inbound "Thanks!" that raised nothing: the newest inbound
    // message of its conversation, but not the one the pass rated.
    sqlx::query(
        "INSERT INTO conversations.messages
                (id, workspace_id, inbox_id, conversation_id, provider_message_id, direction,
                 sender, sender_address, recipients, subject, sent_at, headers, in_inbox,
                 bodies, participants)
         SELECT gen_random_uuid(), m.workspace_id, m.inbox_id, m.conversation_id,
                m.provider_message_id || '-thanks', 'inbound', m.sender, m.sender_address,
                m.recipients, 'Thanks!', m.sent_at + interval '1 hour', m.headers, true,
                m.bodies, m.participants
           FROM conversations.needs n JOIN conversations.messages m ON m.id = n.message_id
          WHERE n.workspace_id = $1
          LIMIT 1",
    )
    .bind(workspace.as_uuid())
    .execute(&kit.db)
    .await
    .unwrap();

    let items = export_set(&kit.state, workspace, ALL, LabellingSet::Actions)
        .await
        .unwrap();
    assert_eq!(
        i64::try_from(items.len()).unwrap(),
        with_needs,
        "{:?}",
        items.iter().map(|i| &i.subject).collect::<Vec<_>>()
    );
    assert!(
        items
            .iter()
            .all(|i| i.item["labels"] == serde_json::json!({}))
    );
    // Each is the message that raised its conversation's need.
    let mut raised: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT ON (n.conversation_id) m.subject
           FROM conversations.needs n JOIN conversations.messages m ON m.id = n.message_id
          WHERE n.workspace_id = $1
          ORDER BY n.conversation_id, m.sent_at DESC",
    )
    .bind(workspace.as_uuid())
    .fetch_all(&kit.db)
    .await
    .unwrap();
    let mut exported: Vec<String> = items.iter().map(|i| i.subject.clone()).collect();
    raised.sort();
    exported.sort();
    assert_eq!(exported, raised);
    assert!(!exported.iter().any(|s| s == "Thanks!"));
    let actions = manifest_for(LabellingSet::Actions);
    assert_eq!(actions["name"], "owner-actions");
    assert_eq!(actions["private"], true);
    assert_eq!(actions["actions"][0], "reply");
    // The money set is as it was.
    assert_eq!(manifest_for(LabellingSet::Money), manifest());
}
