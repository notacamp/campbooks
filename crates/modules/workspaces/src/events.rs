//! The domain events this module publishes, each in the transaction of the
//! change it describes. Every event belongs to its workspace; `workspace` is
//! the workspace id as a string (the event bus keys deliveries by it).
//! Each is declared in the module's `background` (`publishes`), which lists
//! it in `openapi/events.json`, and declares the records it changed for the
//! client change feed (`changes()`): `workspaces.workspace` (id: the
//! workspace) and `workspaces.member` (id: the member's user id).

use campbooks_kernel::{
    DomainEvent, RecordRef,
    changes::Change,
    tenancy::{Role, UserId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::model::Kind;

/// The record type of a workspace in the change feed.
pub const WORKSPACE_RECORD: &str = "workspaces.workspace";
/// The record type of a membership in the change feed; its id is the
/// member's user id.
pub const MEMBER_RECORD: &str = "workspaces.member";

fn workspace_record(workspace: &str) -> RecordRef {
    RecordRef::new(WORKSPACE_RECORD, workspace)
}

fn member_record(user: UserId) -> RecordRef {
    RecordRef::new(MEMBER_RECORD, &user)
}

/// A workspace was created; its creator is its first owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorkspaceCreated {
    /// The workspace.
    pub workspace: String,
    /// Who created it (its owner).
    pub owner: UserId,
    /// Business or personal.
    pub kind: Kind,
}

impl DomainEvent for WorkspaceCreated {
    const NAME: &'static str = "workspaces.workspace_created";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        // The owner's membership comes with the `MemberJoined` published
        // alongside.
        vec![Change::upsert(workspace_record(&self.workspace))]
    }
}

/// A user became a member: by creating the workspace or accepting an
/// invitation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MemberJoined {
    /// The workspace.
    pub workspace: String,
    /// The new member.
    pub user: UserId,
    /// Their role.
    pub role: Role,
}

impl DomainEvent for MemberJoined {
    const NAME: &'static str = "workspaces.member_joined";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![Change::upsert(member_record(self.user))]
    }
}

/// A user stopped being a member: they left, or were removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MemberLeft {
    /// The workspace.
    pub workspace: String,
    /// The former member.
    pub user: UserId,
    /// Who removed them; `None` when they left.
    pub removed_by: Option<UserId>,
    /// What becomes of the mail of the mailboxes they connected: deleted
    /// (always when they leave), or kept read-only when the owner or admin
    /// removing them chose so.
    #[serde(default)]
    pub mail: MailChoice,
    /// Their identity subject (an opaque account id), so the issuer can
    /// remove the clients they registered in the workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// What becomes of a departing member's mail (campbooks-cloud#117).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MailChoice {
    /// Delete it with their connections (the default).
    #[default]
    Delete,
    /// Keep it as read-only history for the workspace.
    Keep,
}

impl DomainEvent for MemberLeft {
    const NAME: &'static str = "workspaces.member_left";
    const VERSION: u16 = 1;
    /// Rejoining waits for every module to have handled the departure.
    const SETTLES: bool = true;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![Change::delete(member_record(self.user))]
    }
}

/// A member's role changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MemberRoleChanged {
    /// The workspace.
    pub workspace: String,
    /// The member.
    pub user: UserId,
    /// Their previous role.
    pub from: Role,
    /// Their new role.
    pub to: Role,
    /// Who changed it.
    pub changed_by: UserId,
}

impl DomainEvent for MemberRoleChanged {
    const NAME: &'static str = "workspaces.member_role_changed";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![Change::upsert(member_record(self.user))]
    }
}

/// A workspace setting changed (its name or its colour). Carries which
/// settings changed, never their values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorkspaceUpdated {
    /// The workspace.
    pub workspace: String,
    /// The settings that changed: `name`, `accent`.
    pub fields: Vec<String>,
    /// Who changed them.
    pub updated_by: UserId,
}

impl DomainEvent for WorkspaceUpdated {
    const NAME: &'static str = "workspaces.workspace_updated";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![Change::upsert(workspace_record(&self.workspace))]
    }
}

/// A workspace was deleted. Every module purges its data for it; the
/// workspace's own records are purged at `purge_after`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct WorkspaceDeleting {
    /// The workspace.
    pub workspace: String,
    /// When its records are hard-deleted.
    pub purge_after: DateTime<Utc>,
    /// Who deleted it.
    pub deleted_by: UserId,
}

impl DomainEvent for WorkspaceDeleting {
    const NAME: &'static str = "workspaces.workspace_deleting";
    const VERSION: u16 = 1;

    fn workspace(&self) -> Option<&str> {
        Some(&self.workspace)
    }

    fn changes(&self) -> Vec<Change> {
        vec![Change::delete(workspace_record(&self.workspace))]
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use sqlx::PgPool;

    use crate::testkit::Kit;

    #[sqlx::test(migrations = "../../../migrations")]
    async fn every_change_is_recorded_for_the_change_feed_with_ids_only(db: PgPool) {
        let kit = Kit::new(db).await;
        let ana = kit.user("ana");
        let ws = kit.create_workspace(&ana, "Shop").await;
        let bea = kit.join(&ana, ws, "bea", "member").await;
        let (ana_id, bea_id) = (kit.user_id("ana").await, kit.user_id("bea").await);
        let member = format!("/api/v1/w/{ws}/members/{bea_id}");
        let ok = kit
            .patch(
                &ana,
                &format!("/api/v1/w/{ws}"),
                serde_json::json!({ "name": "Shop two" }),
            )
            .await;
        assert_eq!(ok.status, StatusCode::OK);
        let ok = kit
            .patch(&ana, &member, serde_json::json!({ "role": "admin" }))
            .await;
        assert_eq!(ok.status, StatusCode::OK);
        assert_eq!(
            kit.delete(&ana, &member).await.status,
            StatusCode::NO_CONTENT
        );
        drop(bea);
        assert_eq!(
            kit.delete(&ana, &format!("/api/v1/w/{ws}")).await.status,
            StatusCode::ACCEPTED
        );

        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT record_type, record_id, op FROM kernel.changes
              WHERE workspace_id = $1 ORDER BY published",
        )
        .bind(ws.to_string())
        .fetch_all(&kit.db)
        .await
        .unwrap();
        let change = |t: &str, id: &str, op: &str| (t.to_owned(), id.to_owned(), op.to_owned());
        let (ws, ana_id, bea_id) = (ws.to_string(), ana_id.to_string(), bea_id.to_string());
        assert_eq!(
            rows,
            vec![
                change("workspaces.workspace", &ws, "upsert"),
                change("workspaces.member", &ana_id, "upsert"),
                change("workspaces.member", &bea_id, "upsert"),
                change("workspaces.workspace", &ws, "upsert"),
                change("workspaces.member", &bea_id, "upsert"),
                change("workspaces.member", &bea_id, "delete"),
                change("workspaces.workspace", &ws, "delete"),
            ],
            "created, owner joined, bea joined, renamed, promoted, removed, deleted; \
             the invitation is not a feed change"
        );
    }
}
