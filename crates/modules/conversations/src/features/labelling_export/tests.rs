//! The labelling export over `fixtures/needs.json`.

use campbooks_kernel::{
    Error,
    tenancy::{Access, Role, UserId, WorkspaceId},
};
use sqlx::PgPool;

use super::{LabellingSet, OWNER, export_inboxes, export_set, manifest, manifest_for};
use crate::{model::InboxRole, testkit::Kit};

/// Every fixture is recent enough.
const ALL: u32 = 100_000;

/// `user`, a member of `workspace`, as an export is bounded by.
fn member(workspace: WorkspaceId, user: UserId) -> Access {
    Access {
        workspace,
        user: Some(user),
        role: Some(Role::Member),
    }
}

#[sqlx::test(migrations = "../../../migrations")]
async fn lists_money_mail_only_unlabelled_and_in_the_eval_format(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "needs").await;

    let items = export_set(
        &kit.state,
        &member(workspace, sam),
        None,
        ALL,
        LabellingSet::Money,
    )
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

    // In another workspace Sam reads no inbox: refused, nothing written.
    let other = export_set(
        &kit.state,
        &member(WorkspaceId::new(), sam),
        None,
        ALL,
        LabellingSet::Money,
    )
    .await;
    assert!(matches!(other, Err(Error::NotFound)), "{other:?}");
    // Old mail is left out.
    let recent = export_set(
        &kit.state,
        &member(workspace, sam),
        None,
        0,
        LabellingSet::Money,
    )
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

    let items = export_set(
        &kit.state,
        &member(workspace, sam),
        None,
        ALL,
        LabellingSet::Actions,
    )
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

/// Sam's and Leo's private inboxes in one workspace, both full of mail.
async fn two_private_inboxes(
    kit: &Kit,
) -> (
    WorkspaceId,
    (UserId, crate::model::InboxId),
    (UserId, crate::model::InboxId),
) {
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let (_, leo) = kit.member("leo", workspace);
    let sams = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    let leos = kit
        .inbox(workspace, "leo@example.com", &[(leo, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, sams, "needs").await;
    kit.ingest(workspace, leos, "needs").await;
    (workspace, (sam, sams), (leo, leos))
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_member_never_exports_a_teammate_s_private_inbox(db: PgPool) {
    let kit = Kit::with_deps(db, crate::testkit::sorting_deps()).await;
    let (workspace, (sam, _), (leo, _)) = two_private_inboxes(&kit).await;
    for set in [LabellingSet::Money, LabellingSet::Actions] {
        let sams = export_set(&kit.state, &member(workspace, sam), None, ALL, set)
            .await
            .unwrap();
        assert!(!sams.is_empty(), "{set:?}");
        // Every item went to Sam's inbox: none of Leo's.
        assert!(
            sams.iter().all(|i| {
                i.item["message"]["recipient"]["addresses"]
                    == serde_json::json!(["sam@example.com"])
            }),
            "{set:?}"
        );
        let leos = export_set(&kit.state, &member(workspace, leo), None, ALL, set)
            .await
            .unwrap();
        assert!(!leos.is_empty(), "{set:?}");
        assert!(
            leos.iter().all(|i| {
                i.item["message"]["recipient"]["addresses"]
                    == serde_json::json!(["leo@example.com"])
            }),
            "{set:?}"
        );
    }
    // A member who reads no inbox is refused: nothing to bound an export by.
    let (_, nobody) = kit.member("nobody", workspace);
    let none = export_set(
        &kit.state,
        &member(workspace, nobody),
        None,
        ALL,
        LabellingSet::Money,
    )
    .await;
    assert!(matches!(none, Err(Error::NotFound)), "{none:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_inbox_the_member_cannot_read_is_refused(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, (sam, sams), (leo, leos)) = two_private_inboxes(&kit).await;
    // A shared inbox Sam reads.
    let team = kit
        .inbox(
            workspace,
            "team@example.com",
            &[(leo, InboxRole::Manager), (sam, InboxRole::Reader)],
        )
        .await;
    kit.ingest(workspace, team, "needs").await;
    let access = member(workspace, sam);
    let export = |inbox| export_set(&kit.state, &access, inbox, ALL, LabellingSet::Money);
    let addresses = |items: &[super::LabellingItem]| {
        let mut found: Vec<String> = items
            .iter()
            .map(|i| {
                i.item["message"]["recipient"]["addresses"][0]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect();
        found.sort();
        found
    };
    // Everything Sam reads: their own inbox and the shared one.
    assert_eq!(
        addresses(&export(None).await.unwrap()),
        ["sam@example.com", "team@example.com"]
    );
    // Narrowed to one of them.
    assert_eq!(
        addresses(&export(Some(sams)).await.unwrap()),
        ["sam@example.com"]
    );
    assert_eq!(
        addresses(&export(Some(team)).await.unwrap()),
        ["team@example.com"]
    );
    // Leo's private inbox: refused.
    let refused = export(Some(leos)).await;
    assert!(matches!(refused, Err(Error::NotFound)), "{refused:?}");
}

#[sqlx::test(migrations = "../../../migrations")]
async fn an_export_says_which_inboxes_it_reads(db: PgPool) {
    let kit = Kit::new(db).await;
    let (workspace, (sam, sams), (_, leos)) = two_private_inboxes(&kit).await;
    let access = member(workspace, sam);
    assert_eq!(
        export_inboxes(&kit.state, &access, None).await.unwrap(),
        vec![sams]
    );
    assert_eq!(
        export_inboxes(&kit.state, &access, Some(sams))
            .await
            .unwrap(),
        vec![sams]
    );
    let refused = export_inboxes(&kit.state, &access, Some(leos)).await;
    assert!(matches!(refused, Err(Error::NotFound)), "{refused:?}");
}
