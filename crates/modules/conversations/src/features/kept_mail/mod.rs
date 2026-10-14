//! The kernel's `KeptMail` port: before an owner or admin removes a member
//! and keeps their mail, which of the inboxes that member's connections fed
//! would be left with nobody to read them (`keep_needs_a_reader`). A kept
//! inbox nobody can read could be neither read nor deleted.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    tenancy::{KeptMail, KeptMailFuture, KeptMailPort, UserId, WorkspaceId},
};
use sqlx::PgConnection;

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
        conn: &'a mut PgConnection,
        _workspace: WorkspaceId,
        user: UserId,
        others: &'a [UserId],
    ) -> KeptMailFuture<'a> {
        Box::pin(async move {
            // The inboxes that would be kept: the member's own, and inboxes
            // from before owners were recorded (no owner) that they read;
            // their memberships held until the departure commits (taking a
            // reader out waits for it).
            let candidates = sqlx::query_scalar!(
                r#"SELECT m.inbox_id AS "id: InboxId"
                     FROM conversations.inbox_members m
                     JOIN conversations.inboxes i ON i.id = m.inbox_id
                    WHERE i.detached_at IS NULL
                      AND (i.owner = $1
                           OR (i.owner IS NULL AND EXISTS (
                                 SELECT 1 FROM conversations.inbox_members me
                                  WHERE me.inbox_id = i.id AND me.user_id = $1)))
                      FOR SHARE OF m"#,
                user.as_uuid(),
            )
            .fetch_all(&mut *conn)
            .await?;
            let owned = sqlx::query_scalar!(
                r#"SELECT id AS "id: InboxId" FROM conversations.inboxes
                    WHERE owner = $1 AND detached_at IS NULL
                      FOR SHARE"#,
                user.as_uuid(),
            )
            .fetch_all(&mut *conn)
            .await?;
            let mut inboxes: Vec<InboxId> = candidates.into_iter().chain(owned).collect();
            inboxes.sort_unstable();
            inboxes.dedup();
            // Read by nobody who stays.
            let read = sqlx::query_scalar!(
                r#"SELECT DISTINCT inbox_id AS "id: InboxId" FROM conversations.inbox_members
                    WHERE inbox_id = ANY($1) AND user_id = ANY($2)"#,
                &inboxes as &[InboxId],
                others as &[UserId],
            )
            .fetch_all(&mut *conn)
            .await?;
            Ok(inboxes
                .iter()
                .filter(|inbox| !read.contains(inbox))
                .map(|inbox| RecordRef::new(INBOX_RECORD, inbox))
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{
        db::BeginFor as _,
        tenancy::{UserId, WorkspaceId},
    };
    use sqlx::PgPool;

    use crate::{model::InboxRole, testkit::Kit};

    /// What the port answers for `user` leaving, `others` staying.
    async fn unread(
        kit: &Kit,
        workspace: WorkspaceId,
        user: UserId,
        others: &[UserId],
    ) -> Vec<String> {
        let mut tx = kit.state.db.begin_for(workspace).await.unwrap();
        let unread = super::kept_mail()
            .unread_if_kept(&mut tx, workspace, user, others)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        unread.into_iter().map(|inbox| inbox.id).collect()
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_inbox_only_its_owner_reads_would_be_left_unread(db: PgPool) {
        let kit = Kit::new(db).await;
        let workspace = WorkspaceId::new();
        let (sam, bea) = (UserId::new(), UserId::new());
        let alone = kit
            .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Manager)])
            .await;
        let shared = kit
            .inbox(
                workspace,
                "team@example.com",
                &[(sam, InboxRole::Manager), (bea, InboxRole::Reader)],
            )
            .await;
        assert_eq!(
            unread(&kit, workspace, sam, &[bea]).await,
            [alone.to_string()]
        );
        assert!(unread(&kit, workspace, bea, &[sam]).await.is_empty());
        // A reader who is leaving too (gone from the workspace, their inbox
        // membership not yet removed) reads nothing.
        let mut both = vec![alone.to_string(), shared.to_string()];
        both.sort();
        assert_eq!(unread(&kit, workspace, sam, &[]).await, both);
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn an_inbox_from_before_owners_counts_for_its_readers(db: PgPool) {
        let kit = Kit::new(db).await;
        let workspace = WorkspaceId::new();
        let (sam, bea) = (UserId::new(), UserId::new());
        let legacy = kit
            .inbox(workspace, "old@example.com", &[(sam, InboxRole::Manager)])
            .await;
        sqlx::query("UPDATE conversations.inboxes SET owner = NULL WHERE id = $1")
            .bind(legacy)
            .execute(&kit.db)
            .await
            .unwrap();
        assert_eq!(
            unread(&kit, workspace, sam, &[bea]).await,
            [legacy.to_string()]
        );
    }
}
