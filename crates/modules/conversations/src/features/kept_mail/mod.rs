//! The kernel's `KeptMail` port: before an owner or admin removes a member
//! and keeps their mail, which of the inboxes that member's connections fed
//! would be left with nobody to read them (`keep_needs_a_reader`). A kept
//! inbox nobody can read could be neither read nor deleted.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    db::BeginFor as _,
    tenancy::{KeptMail, KeptMailFuture, KeptMailPort, UserId, WorkspaceId},
};
use sqlx::PgPool;

use crate::{events::INBOX_RECORD, model::InboxId};

/// The port, for the server to register.
pub fn kept_mail() -> KeptMailPort {
    Arc::new(InboxReaders)
}

#[derive(Debug, Clone, Copy)]
struct InboxReaders;

impl KeptMail for InboxReaders {
    fn unread_if_kept<'a>(
        &'a self,
        db: &'a PgPool,
        workspace: WorkspaceId,
        user: UserId,
    ) -> KeptMailFuture<'a> {
        Box::pin(async move {
            let mut tx = db.begin_for(workspace).await?;
            let unread = sqlx::query_scalar!(
                r#"SELECT i.id AS "id: InboxId" FROM conversations.inboxes i
                    WHERE i.owner = $1 AND i.detached_at IS NULL
                      AND NOT EXISTS (SELECT 1 FROM conversations.inbox_members m
                                       WHERE m.inbox_id = i.id AND m.user_id <> $1)
                    ORDER BY i.id"#,
                user.as_uuid(),
            )
            .fetch_all(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(unread
                .iter()
                .map(|inbox| RecordRef::new(INBOX_RECORD, inbox))
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::tenancy::{UserId, WorkspaceId};
    use sqlx::PgPool;

    use crate::{model::InboxRole, testkit::Kit};

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_inbox_only_its_owner_reads_would_be_left_unread(db: PgPool) {
        let kit = Kit::new(db).await;
        let workspace = WorkspaceId::new();
        let (sam, bea) = (UserId::new(), UserId::new());
        let alone = kit
            .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
            .await;
        kit.inbox(
            workspace,
            "team@example.com",
            &[(sam, InboxRole::Manager), (bea, InboxRole::Reader)],
        )
        .await;
        let unread = super::kept_mail()
            .unread_if_kept(&kit.state.db, workspace, sam)
            .await
            .unwrap();
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].id, alone.to_string());
        assert!(
            super::kept_mail()
                .unread_if_kept(&kit.state.db, workspace, bea)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
