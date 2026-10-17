//! The labelling export over `fixtures/needs.json`.

use campbooks_kernel::tenancy::WorkspaceId;
use sqlx::PgPool;

use super::{OWNER, export_for_labelling, manifest};
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
