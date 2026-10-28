//! `member_replied` (campbooks-cloud#396): a fact for the tasks module,
//! published once for each new outbound message of a personal inbox.

use campbooks_kernel::tenancy::WorkspaceId;
use sqlx::PgPool;

use crate::{
    model::InboxRole,
    testkit::{Kit, contract},
};

#[sqlx::test(migrations = "../../../migrations")]
async fn the_members_reply_is_a_fact_for_their_tasks_once(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "en").await;

    // The fixture holds one outbound message among three received.
    let replied = kit.events("conversations.member_replied").await;
    assert_eq!(replied.len(), 1, "{replied:?}");
    let sent = kit.events("conversations.message_sent").await;
    assert_eq!(replied[0]["message"], sent[0]["message"]);
    assert_eq!(replied[0]["conversation"], sent[0]["conversation"]);
    assert_eq!(replied[0]["sent_at"], sent[0]["sent_at"]);
    assert_eq!(replied[0]["inbox"], inbox.to_string());
    assert_eq!(replied[0]["owner"], sam.to_string());
    // Ids and a time only: nothing anyone wrote, no address.
    assert!(!replied[0].to_string().contains('@'));
    let (schemas, index) = contract("conversations.member_replied.v1.schema.json");
    if let Err(error) = schemas.validate(&replied[0], index) {
        panic!("{} breaks the contract: {error}", replied[0]);
    }

    // The same batch again stores nothing new, so it says nothing new.
    kit.ingest(workspace, inbox, "en").await;
    assert_eq!(kit.events("conversations.member_replied").await.len(), 1);
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_shared_inbox_has_no_one_member_who_replied(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let (_, ana) = kit.member("ana", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "team@example.com",
            &[(sam, InboxRole::Manager), (ana, InboxRole::Reader)],
        )
        .await;
    kit.ingest(workspace, inbox, "en").await;

    assert_eq!(kit.events("conversations.message_sent").await.len(), 1);
    assert!(kit.events("conversations.member_replied").await.is_empty());
}
