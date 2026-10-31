use campbooks_kernel::{
    attachments::AttachmentBlob,
    db::BeginFor as _,
    tenancy::{Access, Role, UserId, WorkspaceId},
};
use sqlx::PgPool;
use sqlx::types::Uuid;

use crate::{model::InboxRole, testkit::Kit};

/// The ids of the first attachment of `provider_message_id`:
/// (conversation, message, attachment).
async fn ids(kit: &Kit, provider_message_id: &str) -> (Uuid, Uuid, Uuid) {
    sqlx::query_as(
        "SELECT m.conversation_id, m.id, a.id
           FROM conversations.attachments a
           JOIN conversations.messages m ON m.id = a.message_id
          WHERE m.provider_message_id = $1 ORDER BY a.position LIMIT 1",
    )
    .bind(provider_message_id)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

/// The conversation and the message `provider_message_id` is.
async fn message_ids(kit: &Kit, provider_message_id: &str) -> (Uuid, Uuid) {
    sqlx::query_as(
        "SELECT conversation_id, id FROM conversations.messages WHERE provider_message_id = $1",
    )
    .bind(provider_message_id)
    .fetch_one(&kit.db)
    .await
    .unwrap()
}

fn access(workspace: WorkspaceId, user: UserId) -> Access {
    Access {
        workspace,
        user: Some(user),
        role: Some(Role::Member),
    }
}

async fn ask(
    kit: &Kit,
    access: &Access,
    (conversation, message, attachment): (Uuid, Uuid, Uuid),
) -> Option<AttachmentBlob> {
    let mut tx = kit.state.db.begin_for(access.workspace).await.unwrap();
    let blob = super::attachment_blobs()
        .blob_of(
            &kit.state,
            &mut tx,
            access,
            conversation,
            message,
            attachment,
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    blob
}

#[sqlx::test(migrations = "../../../migrations")]
async fn a_reader_of_the_inbox_is_answered_the_blob_and_the_name(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let (_, lia) = kit.member("lia", workspace);
    let inbox = kit
        .inbox(
            workspace,
            "sam@example.com",
            &[(sam, InboxRole::Manager), (lia, InboxRole::Reader)],
        )
        .await;
    kit.ingest(workspace, inbox, "en").await;
    let wanted = ids(&kit, "en-1").await;

    let answered = ask(&kit, &access(workspace, sam), wanted).await.unwrap();
    assert_eq!(answered.filename, "q4-plan.pdf");
    assert_eq!(
        kit.read(workspace, &answered.blob).await,
        "%PDF-1.7 fictional Q4 plan"
    );

    // A member who only reads the inbox is answered too: it is a read.
    assert!(ask(&kit, &access(workspace, lia), wanted).await.is_some());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_members_inbox_is_answered_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "en").await;
    let wanted = ids(&kit, "en-1").await;

    // Ana belongs to the workspace, not to Sam's inbox.
    let (_, ana) = kit.member("ana", workspace);
    assert!(ask(&kit, &access(workspace, ana), wanted).await.is_none());

    // Nor does having an inbox of their own help.
    let (_, lia) = kit.member("lia", workspace);
    kit.inbox(workspace, "lia@example.com", &[(lia, InboxRole::Manager)])
        .await;
    assert!(ask(&kit, &access(workspace, lia), wanted).await.is_none());

    // A client token acting for nobody sees no inbox.
    let nobody = Access {
        workspace,
        user: None,
        role: None,
    };
    assert!(ask(&kit, &nobody, wanted).await.is_none());
}

#[sqlx::test(migrations = "../../../migrations")]
async fn the_attachment_must_belong_to_that_message_of_that_conversation(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "en").await;
    let (conversation, message, attachment) = ids(&kit, "en-1").await;
    let (other_conversation, other_message) = message_ids(&kit, "en-3").await;
    let sam = access(workspace, sam);

    assert!(
        ask(&kit, &sam, (conversation, message, attachment))
            .await
            .is_some()
    );
    // The right attachment under a wrong message, a wrong conversation, or
    // an id that does not exist: the same answer.
    assert!(
        ask(&kit, &sam, (conversation, other_message, attachment))
            .await
            .is_none()
    );
    assert!(
        ask(&kit, &sam, (other_conversation, message, attachment))
            .await
            .is_none()
    );
    assert!(
        ask(&kit, &sam, (conversation, message, Uuid::now_v7()))
            .await
            .is_none()
    );
}

#[sqlx::test(migrations = "../../../migrations")]
async fn another_workspace_is_answered_nothing(db: PgPool) {
    let kit = Kit::new(db).await;
    let workspace = WorkspaceId::new();
    let (_, sam) = kit.member("sam", workspace);
    let inbox = kit
        .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
        .await;
    kit.ingest(workspace, inbox, "en").await;
    let wanted = ids(&kit, "en-1").await;

    // The same user id, asked from another workspace's transaction.
    let elsewhere = WorkspaceId::new();
    assert!(ask(&kit, &access(elsewhere, sam), wanted).await.is_none());
}
