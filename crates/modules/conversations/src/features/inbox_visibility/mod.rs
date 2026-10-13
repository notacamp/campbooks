//! The change feed's `Visibility` port for this module's containers: a
//! conversation or message change lives within its inbox, and a member sees
//! it only if they are a member of that inbox (a client token bound to the
//! workspace sees every inbox, as it does over the API). Registered by the
//! server with `AppState::with_visibility`.

use std::sync::Arc;

use campbooks_kernel::{
    RecordRef,
    changes::{Visibility, VisibilityFuture, VisibilityPort},
    db::BeginFor as _,
    tenancy::Access,
};
use sqlx::PgPool;

use crate::{events::INBOX_RECORD, model::InboxId};

/// The port, for the server to register.
pub fn visibility() -> VisibilityPort {
    Arc::new(InboxVisibility)
}

/// Inbox membership decides who sees changes inside an inbox.
#[derive(Debug, Clone, Copy, Default)]
pub struct InboxVisibility;

impl Visibility for InboxVisibility {
    fn container_visible<'a>(
        &'a self,
        db: &'a PgPool,
        access: &'a Access,
        container: &'a RecordRef,
    ) -> VisibilityFuture<'a> {
        Box::pin(async move {
            // A container of another type is not this module's to grant.
            if container.record_type != INBOX_RECORD {
                return Ok(false);
            }
            let Ok(inbox) = container.id.parse::<InboxId>() else {
                return Ok(false);
            };
            let mut tx = db.begin_for(access.workspace).await?;
            let visible = match access.user {
                Some(user) => {
                    sqlx::query_scalar!(
                        r#"SELECT EXISTS (SELECT 1 FROM conversations.inbox_members
                                           WHERE inbox_id = $1 AND user_id = $2) AS "visible!""#,
                        inbox as InboxId,
                        user.as_uuid(),
                    )
                    .fetch_one(&mut *tx)
                    .await?
                }
                None => {
                    sqlx::query_scalar!(
                        r#"SELECT EXISTS (SELECT 1 FROM conversations.inboxes
                                           WHERE id = $1) AS "visible!""#,
                        inbox as InboxId,
                    )
                    .fetch_one(&mut *tx)
                    .await?
                }
            };
            tx.commit().await?;
            Ok(visible)
        })
    }
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::{
        RecordRef,
        tenancy::{Access, UserId, WorkspaceId},
    };
    use sqlx::PgPool;

    use super::InboxVisibility;
    use crate::{
        events::INBOX_RECORD,
        model::{InboxId, InboxRole},
        testkit::Kit,
    };
    use campbooks_kernel::changes::Visibility as _;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn inbox_members_and_clients_see_changes_inside_an_inbox(db: PgPool) {
        let kit = Kit::new(db).await;
        let workspace = WorkspaceId::new();
        let sam = UserId::new();
        let inbox = kit
            .inbox(workspace, "sam@example.com", &[(sam, InboxRole::Reader)])
            .await;
        let pool = &kit.state.db;
        let access = |user| Access {
            workspace,
            user,
            role: None,
        };
        let container = RecordRef::new(INBOX_RECORD, &inbox);
        let visible = |access: Access, container: RecordRef| async move {
            InboxVisibility
                .container_visible(pool, &access, &container)
                .await
                .unwrap()
        };
        assert!(visible(access(Some(sam)), container.clone()).await);
        assert!(!visible(access(Some(UserId::new())), container.clone()).await);
        assert!(visible(access(None), container.clone()).await);
        assert!(!visible(access(None), RecordRef::new(INBOX_RECORD, &InboxId::new())).await);
        assert!(
            !visible(
                access(Some(sam)),
                RecordRef::new("schedule.calendar", &inbox)
            )
            .await
        );
    }

    #[sqlx::test(migrations = "../../../migrations")]
    async fn conversation_changes_are_declared_within_their_inbox(db: PgPool) {
        let kit = Kit::new(db.clone()).await;
        let (workspace, inbox, sam) = kit.sams_inbox().await;
        let id = kit.conversation_of("en-1").await;
        kit.post(
            &sam,
            &format!("/api/v1/w/{workspace}/conversations/{id}/archive"),
            None,
        )
        .await;
        let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
            "SELECT record_type, record_id, op, container_id FROM kernel.changes
              WHERE workspace_id = $1 AND record_id = $2 ORDER BY published",
        )
        .bind(workspace.to_string())
        .bind(&id)
        .fetch_all(&db)
        .await
        .unwrap();
        assert!(!rows.is_empty());
        for (record_type, _, op, container) in &rows {
            assert_eq!(record_type, "conversations.conversation");
            assert_eq!(op, "upsert");
            assert_eq!(container.as_deref(), Some(inbox.to_string().as_str()));
        }
    }
}
