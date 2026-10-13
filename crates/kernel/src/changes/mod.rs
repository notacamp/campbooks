//! The client change feed (ADR 0011): a per-workspace stream of *what*
//! changed, so clients refetch only what they show.
//!
//! - **Declare.** A [`DomainEvent`](crate::DomainEvent) lists the records its
//!   change touched in [`changes`](crate::DomainEvent::changes): a
//!   [`RecordRef`] and an [`ChangeOp`] each, ids only.
//! - **Record.** [`EventBus::publish`](crate::EventBus::publish) writes them to
//!   `kernel.changes` in the publisher's transaction, unnumbered, under the
//!   workspace's shared advisory lock, and enqueues a [`SequenceChanges`]
//!   for the workspace unless one is already pending (a burst collapses into
//!   one job).
//! - **Sequence.** [`SequenceChanges`] tries the workspace's exclusive lock
//!   without waiting (busy means "run again in a moment", so producers never
//!   wait for it). Holding it, no producer is in flight: it numbers all of
//!   the workspace's unnumbered changes from its head in publish order and
//!   notifies [`NOTIFY_CHANNEL`]. A cursor is handed out only once every
//!   smaller one is committed: a reader never skips a change.
//! - **Serve.** [`open`] streams a workspace's changes as Server-Sent Events
//!   from a cursor, woken by the process's [`ChangeHub`], filtered by what
//!   the caller may see (the scopes of the operations that read each record
//!   type, and the [`Visibility`] port for containers).
//!
//! The design and the alternatives are in `openspec/changes/change-feed/design.md`.

mod hub;
mod sequence;
mod stream;
mod visibility;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;

use crate::{
    events::EventId,
    jobs::{self, Job as _},
    records::RecordRef,
    tenancy::{Access, UserId},
};

pub use hub::ChangeHub;
pub use sequence::{PruneChanges, SequenceChanges};
pub use stream::{Subscription, open};
pub use visibility::{
    ContainersFuture, NoContainers, Visibility, VisibilityFuture, VisibilityPort,
};

/// The `LISTEN/NOTIFY` channel the sequencer signals on; the payload is the
/// workspace whose cursor moved.
pub const NOTIFY_CHANNEL: &str = "kernel_changes";

/// The queue [`SequenceChanges`] runs on, apart from sync and AI work.
pub const QUEUE: &str = "changes";

/// What happened to a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOp {
    /// Created or changed: refetch it.
    Upsert,
    /// Deleted (or no longer exists for the workspace).
    Delete,
}

impl ChangeOp {
    /// The wire and database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Delete => "delete",
        }
    }

    fn parse(value: &str) -> Self {
        if value == "delete" {
            Self::Delete
        } else {
            Self::Upsert
        }
    }
}

/// Who may see a change beyond the rules of its record type and container,
/// for records private to a person: their drafts, the connections they
/// granted. Decided by the kernel from the caller's
/// [`Access`](crate::tenancy::Access) alone, so it works for records that
/// are already gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Audience {
    /// Only this user (never a client token).
    User(UserId),
    /// This user, the workspace's owners and admins, and client tokens of
    /// the workspace (who see every connection over the API).
    UserAndManagers(UserId),
    /// Client tokens of the workspace only (a record a client wrote).
    Clients,
}

impl Audience {
    /// Whether the caller with `access` is in the audience.
    pub fn admits(self, access: &Access) -> bool {
        match self {
            Self::User(user) => access.user == Some(user),
            Self::UserAndManagers(user) => match access.user {
                None => true,
                Some(caller) => {
                    caller == user
                        || access
                            .role
                            .is_some_and(crate::tenancy::Role::can_manage)
                }
            },
            Self::Clients => access.user.is_none(),
        }
    }

    /// The stored kind and user.
    fn columns(self) -> (&'static str, Option<String>) {
        match self {
            Self::User(user) => ("user", Some(user.to_string())),
            Self::UserAndManagers(user) => ("user_and_managers", Some(user.to_string())),
            Self::Clients => ("clients", None),
        }
    }

    /// From the stored kind and user; `None` for an unknown or malformed
    /// pair, which the stream treats as visible to nobody.
    fn from_columns(kind: &str, user: Option<&str>) -> Option<Self> {
        let user = || user.and_then(|u| u.parse::<UserId>().ok());
        match kind {
            "user" => user().map(Self::User),
            "user_and_managers" => user().map(Self::UserAndManagers),
            "clients" => Some(Self::Clients),
            _ => None,
        }
    }
}

/// One record a domain event changed: its reference and what happened to
/// it, and, when not every member may see it, the container it lives in
/// and the people it is private to. Never record contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The changed record.
    pub record: RecordRef,
    /// What happened to it.
    pub op: ChangeOp,
    /// What it lives in when only some members may see it (an inbox, a
    /// calendar); `None` when every member may. Checked through the
    /// [`Visibility`] port.
    pub container: Option<RecordRef>,
    /// Who it is private to; `None` when everyone the other rules admit may
    /// see it.
    pub audience: Option<Audience>,
}

impl Change {
    /// `record` was created or changed.
    pub fn upsert(record: RecordRef) -> Self {
        Self {
            record,
            op: ChangeOp::Upsert,
            container: None,
            audience: None,
        }
    }

    /// `record` was deleted.
    pub fn delete(record: RecordRef) -> Self {
        Self {
            record,
            op: ChangeOp::Delete,
            container: None,
            audience: None,
        }
    }

    /// The same change, visible only to members who may see `container`.
    #[must_use]
    pub fn within(mut self, container: RecordRef) -> Self {
        self.container = Some(container);
        self
    }

    /// The same change, shown only to `audience`.
    #[must_use]
    pub fn for_audience(mut self, audience: Audience) -> Self {
        self.audience = Some(audience);
        self
    }
}

/// The advisory lock key of a workspace's feed, hashed in SQL by
/// `hashtextextended(key, 0)`.
fn lock_key(workspace: &str) -> String {
    format!("kernel.changes:{workspace}")
}

/// Record `changes` of `workspace`, declared by event `event`, inside the
/// publisher's transaction (called by
/// [`EventBus::publish`](crate::EventBus::publish)).
pub(crate) async fn record(
    tx: &mut Transaction<'_, Postgres>,
    event: EventId,
    workspace: &str,
    changes: &[Change],
) -> Result<(), sqlx::Error> {
    // Held until the publisher commits: the sequencer takes the exclusive
    // lock, so it cannot number anything while this transaction is in
    // flight, whether the job below is new or already pending or running.
    sqlx::query_scalar!(
        "SELECT pg_advisory_xact_lock_shared(hashtextextended($1, 0))",
        lock_key(workspace)
    )
    .fetch_one(&mut **tx)
    .await?;

    let record_types: Vec<&str> = changes
        .iter()
        .map(|c| c.record.record_type.as_str())
        .collect();
    let record_ids: Vec<&str> = changes.iter().map(|c| c.record.id.as_str()).collect();
    let ops: Vec<&str> = changes.iter().map(|c| c.op.as_str()).collect();
    let container_types: Vec<Option<&str>> = changes
        .iter()
        .map(|c| c.container.as_ref().map(|r| r.record_type.as_str()))
        .collect();
    let container_ids: Vec<Option<&str>> = changes
        .iter()
        .map(|c| c.container.as_ref().map(|r| r.id.as_str()))
        .collect();
    let (audiences, audience_users): (Vec<Option<&str>>, Vec<Option<String>>) = changes
        .iter()
        .map(|c| match c.audience.map(Audience::columns) {
            Some((kind, user)) => (Some(kind), user),
            None => (None, None),
        })
        .unzip();
    // Rows are inserted in declared order, so `published` keeps it.
    sqlx::query!(
        r#"
        INSERT INTO kernel.changes
            (workspace_id, event_id, record_type, record_id, op, container_type, container_id,
             audience, audience_user)
        SELECT $1, $2, c.record_type, c.record_id, c.op, c.container_type, c.container_id,
               c.audience, c.audience_user
          FROM unnest($3::text[], $4::text[], $5::text[], $6::text[], $7::text[], $8::text[],
                      $9::text[])
               WITH ORDINALITY AS c(record_type, record_id, op, container_type, container_id,
                                    audience, audience_user, n)
         ORDER BY c.n
        "#,
        workspace,
        event as EventId,
        &record_types as &[&str],
        &record_ids as &[&str],
        &ops as &[&str],
        &container_types as &[Option<&str>],
        &container_ids as &[Option<&str>],
        &audiences as &[Option<&str>],
        &audience_users as &[Option<String>],
    )
    .execute(&mut **tx)
    .await?;

    request_sequencing(tx, workspace, None).await
}

/// Enqueue [`SequenceChanges`] for `workspace` unless a committed one is
/// already pending.
///
/// Deliberately not a `unique_key`: a unique index makes a concurrent insert
/// of the same key wait for the first inserter's transaction to end, so
/// every producer of a workspace would queue behind the first one in
/// flight. Here a producer only skips a job that is committed and not yet
/// started; producers in flight at the same instant may each add one (a
/// handful, harmless: a run with nothing to number ends at once). The
/// workspace lock pair, not this check, is what keeps the numbering correct.
pub(crate) async fn request_sequencing(
    conn: &mut sqlx::PgConnection,
    workspace: &str,
    delay: Option<std::time::Duration>,
) -> Result<(), sqlx::Error> {
    let run_at = delay
        .and_then(|delay| chrono::TimeDelta::from_std(delay).ok())
        .map(|delay| chrono::Utc::now() + delay);
    let payload = serde_json::to_value(SequenceChanges {
        workspace: workspace.to_owned(),
    })
    .map_err(|e| sqlx::Error::Encode(e.into()))?;
    let created = sqlx::query_scalar!(
        r#"
        INSERT INTO kernel.jobs (queue, kind, payload, workspace_id, max_attempts, run_at)
        SELECT $1, $2, $3, $4, $5, coalesce($6, now())
         WHERE NOT EXISTS (
            SELECT 1 FROM kernel.jobs
             WHERE queue = $1 AND workspace_key = $4 AND kind = $2 AND state = 'pending'
         )
        RETURNING id
        "#,
        QUEUE,
        SequenceChanges::NAME,
        payload,
        workspace,
        jobs::attempts_to_db(SequenceChanges::MAX_ATTEMPTS),
        run_at,
    )
    .fetch_optional(&mut *conn)
    .await?;
    if created.is_some() {
        jobs::notify(conn, QUEUE).await?;
    }
    Ok(())
}

/// Delete every change of `workspace` and its cursor head, e.g. when the
/// workspace is purged. Returns how many changes were deleted.
pub async fn purge_workspace(
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
) -> Result<u64, sqlx::Error> {
    let deleted = sqlx::query!(
        "DELETE FROM kernel.changes WHERE workspace_id = $1",
        workspace
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    sqlx::query!(
        "DELETE FROM kernel.change_heads WHERE workspace_id = $1",
        workspace
    )
    .execute(&mut **tx)
    .await?;
    Ok(deleted)
}
