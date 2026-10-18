//! Re-tagging this module's bound rows after a rotation of the instance
//! secret key (campbooks-cloud#191).
//!
//! Taught rules, their applications and runs, change requests and sends
//! carry MACs under the kernel's `Binding` (campbooks-cloud#227, #242).
//! While a previous key is set they verify under it too, but once it is
//! removed a rule would need confirming again, its changes could not be put
//! back, and a scheduled send would not go out. So the kernel enqueues this
//! job in every workspace while a previous key is set
//! (`Background::rekey`). It re-tags, with the current key, every row
//! whose MAC verifies under the previous key only (`Binding::retag`): the
//! MAC was just verified, so what was bound stays bound and nothing new is
//! trusted. A row that does not verify at all is left as it is. Each update
//! is a compare-and-set on the old MAC, so a member's request that signed
//! the row meanwhile wins, and the job is idempotent: a second run finds
//! nothing to re-tag.
//!
//! It logs what it re-tagged and what it left (rows changed while it ran,
//! re-tagged at the next tick); the rotation runbook waits until every
//! workspace says nothing is left before the previous key is removed.

use campbooks_kernel::{
    Job, JobContext, JobError,
    binding::Binding,
    tenancy::{Access, UserId, WorkspaceId},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, types::Uuid};

use crate::{
    domain::{
        binding::{ChangeRequest, SendRequest, StoredSend, content_hash},
        drafts, rule_runs,
        rules::{self, Before, applications},
    },
    events::RequestedItem,
    model::{ConversationId, DraftId, InboxId, RuleId, RuleRunId, RunKind, SendId},
};

/// What a pass re-tagged, and what it left for the next one.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Tally {
    /// Rows re-tagged with the current key.
    pub retagged: u64,
    /// Rows bound under the previous key that changed while the job ran.
    pub left: u64,
}

impl Tally {
    /// Count one compare-and-set: re-tagged, or left when the row changed.
    fn count(&mut self, rows_affected: u64) {
        if rows_affected == 0 {
            self.left += 1;
        } else {
            self.retagged += 1;
        }
    }
}

/// Re-tag, in the job's workspace, what is bound under the previous key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetagBindings;

impl Job for RetagBindings {
    const NAME: &'static str = "conversations.retag_bindings";
    const QUEUE: &'static str = "maintenance";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("retag_bindings enqueued outside a workspace"))?;
        let key = ctx.state().binding.clone();
        if !key.has_previous() {
            return Ok(());
        }
        let tx = ctx.tx().await?;
        let tally = retag(tx, &key, workspace).await?;
        if tally.left == 0 {
            tracing::info!(
                %workspace,
                retagged = tally.retagged,
                left = 0,
                "binding re-tag: nothing in this workspace is bound under the previous key any more"
            );
        } else {
            tracing::warn!(
                %workspace,
                retagged = tally.retagged,
                left = tally.left,
                "binding re-tag: rows bound under the previous key changed while they were \
                 re-tagged; the next tick tries again"
            );
        }
        Ok(())
    }
}

/// One pass over `workspace` (the transaction's), every kind of bound row.
pub async fn retag(
    conn: &mut PgConnection,
    key: &Binding,
    workspace: WorkspaceId,
) -> campbooks_kernel::Result<Tally> {
    let mut tally = Tally::default();
    retag_rules(conn, key, &mut tally).await?;
    retag_applications(conn, key, &mut tally).await?;
    retag_runs(conn, key, workspace, &mut tally).await?;
    retag_change_requests(conn, key, workspace, &mut tally).await?;
    retag_scheduled_sends(conn, key, workspace, &mut tally).await?;
    retag_stored_sends(conn, key, workspace, &mut tally).await?;
    Ok(tally)
}

/// Taught rules, at their generation (members' content tokens stay valid).
async fn retag_rules(
    conn: &mut PgConnection,
    key: &Binding,
    tally: &mut Tally,
) -> campbooks_kernel::Result<()> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM conversations.rules
          WHERE mac IS NOT NULL AND state IN ('active', 'paused') ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await?;
    for id in ids {
        let Some(rule) = rules::find_any(conn, RuleId::from_uuid(id)).await? else {
            continue;
        };
        let Some(mac) = rule.retag(key) else {
            continue;
        };
        let done = sqlx::query(
            "UPDATE conversations.rules SET mac = $3
              WHERE id = $1 AND binding_generation = $2 AND mac = $4",
        )
        .bind(id)
        .bind(rule.generation)
        .bind(mac)
        .bind(rule.mac.as_deref())
        .execute(&mut *conn)
        .await?;
        tally.count(done.rows_affected());
    }
    Ok(())
}

/// A rule's application: its rule, the rule's inbox, its conversation, run
/// and `before`, and its MAC.
type ApplicationRow = (Uuid, Uuid, Uuid, Uuid, Option<Uuid>, Value, Vec<u8>);

/// Rules' applications, kept or put back (both may be read again).
async fn retag_applications(
    conn: &mut PgConnection,
    key: &Binding,
    tally: &mut Tally,
) -> campbooks_kernel::Result<()> {
    let rows: Vec<ApplicationRow> = sqlx::query_as(
        "SELECT a.workspace_id, a.rule_id, r.inbox_id, a.conversation_id, a.run_id, a.before,
                a.mac
           FROM conversations.rule_applications a
           JOIN conversations.rules r ON r.id = a.rule_id
          WHERE a.mac IS NOT NULL
          ORDER BY a.rule_id, a.conversation_id",
    )
    .fetch_all(&mut *conn)
    .await?;
    for (workspace, rule, inbox, conversation, run, before, mac) in rows {
        let Ok(decoded) = serde_json::from_value::<Before>(before) else {
            continue;
        };
        let owner = (
            workspace,
            InboxId::from_uuid(inbox),
            RuleId::from_uuid(rule),
        );
        let conversation_id = ConversationId::from_uuid(conversation);
        let Some(new) = applications::retag(
            key,
            owner,
            conversation_id,
            run,
            (&decoded, Some(mac.as_slice())),
        ) else {
            continue;
        };
        let done = sqlx::query(
            "UPDATE conversations.rule_applications SET mac = $3
              WHERE rule_id = $1 AND conversation_id = $2 AND mac = $4",
        )
        .bind(rule)
        .bind(conversation)
        .bind(new)
        .bind(mac)
        .execute(&mut *conn)
        .await?;
        tally.count(done.rows_affected());
    }
    Ok(())
}

/// A run: its id, rule, kind, requester and MAC.
type RunRow = (Uuid, Uuid, String, Option<Uuid>, Vec<u8>);

/// Running rule runs, at their rule's generation now.
async fn retag_runs(
    conn: &mut PgConnection,
    key: &Binding,
    workspace: WorkspaceId,
    tally: &mut Tally,
) -> campbooks_kernel::Result<()> {
    let rows: Vec<RunRow> = sqlx::query_as(
        "SELECT id, rule_id, kind, requested_by, mac FROM conversations.rule_runs
          WHERE state = 'running' AND mac IS NOT NULL ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await?;
    for (id, rule, kind, by, mac) in rows {
        let Ok(kind) = kind.parse::<RunKind>() else {
            continue;
        };
        let Some(rule) = rules::find_any(conn, RuleId::from_uuid(rule)).await? else {
            continue;
        };
        let Some(new) = rule_runs::retag(
            key,
            workspace.as_uuid(),
            RuleRunId::from_uuid(id),
            &rule,
            kind,
            by.as_ref(),
            Some(mac.as_slice()),
        ) else {
            continue;
        };
        let done =
            sqlx::query("UPDATE conversations.rule_runs SET mac = $2 WHERE id = $1 AND mac = $3")
                .bind(id)
                .bind(new)
                .bind(mac)
                .execute(&mut *conn)
                .await?;
        tally.count(done.rows_affected());
    }
    Ok(())
}

/// Recorded change requests.
async fn retag_change_requests(
    conn: &mut PgConnection,
    key: &Binding,
    workspace: WorkspaceId,
    tally: &mut Tally,
) -> campbooks_kernel::Result<()> {
    let rows: Vec<(Uuid, Uuid, Uuid, Value, Vec<u8>)> = sqlx::query_as(
        "SELECT id, inbox_id, conversation_id, items, mac FROM conversations.change_requests
          WHERE mac IS NOT NULL ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await?;
    for (id, inbox, conversation, items, mac) in rows {
        let Ok(items) = serde_json::from_value::<Vec<RequestedItem>>(items) else {
            continue;
        };
        let request = ChangeRequest {
            workspace,
            request: id,
            inbox: InboxId::from_uuid(inbox),
            conversation: ConversationId::from_uuid(conversation),
            items: &items,
        };
        let Some(new) = request.retag(key, Some(mac.as_slice()))? else {
            continue;
        };
        let done = sqlx::query(
            "UPDATE conversations.change_requests SET mac = $2 WHERE id = $1 AND mac = $3",
        )
        .bind(id)
        .bind(new)
        .bind(mac)
        .execute(&mut *conn)
        .await?;
        tally.count(done.rows_affected());
    }
    Ok(())
}

/// Scheduled sends: the author's request, over the draft as stored.
async fn retag_scheduled_sends(
    conn: &mut PgConnection,
    key: &Binding,
    workspace: WorkspaceId,
    tally: &mut Tally,
) -> campbooks_kernel::Result<()> {
    let rows: Vec<(Uuid, Option<Uuid>, Vec<u8>)> = sqlx::query_as(
        "SELECT id, author, request_mac FROM conversations.drafts
          WHERE state = 'scheduled' AND request_mac IS NOT NULL ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await?;
    for (id, author, mac) in rows {
        let author = author.map(UserId::from);
        let access = Access {
            workspace,
            user: author,
            role: None,
        };
        let Some(row) = drafts::find(conn, &access, DraftId::from_uuid(id), false).await? else {
            continue;
        };
        let Ok(content) = content_hash(&row) else {
            continue;
        };
        let request = SendRequest {
            workspace,
            draft: row.id,
            generation: row.send_generation,
            author,
            inbox: row.inbox_id,
            content: &content,
        };
        let Some(new) = request.retag(key, Some(mac.as_slice())) else {
            continue;
        };
        let done = sqlx::query(
            "UPDATE conversations.drafts SET request_mac = $2
              WHERE id = $1 AND send_generation = $3 AND request_mac = $4",
        )
        .bind(id)
        .bind(new)
        .bind(row.send_generation)
        .bind(mac)
        .execute(&mut *conn)
        .await?;
        tally.count(done.rows_affected());
    }
    Ok(())
}

/// A stored send: its draft, send, inbox, author, document and MAC.
type SendRow = (Uuid, Uuid, Uuid, Option<Uuid>, Value, Vec<u8>);

/// Released sends not handed over yet.
async fn retag_stored_sends(
    conn: &mut PgConnection,
    key: &Binding,
    workspace: WorkspaceId,
    tally: &mut Tally,
) -> campbooks_kernel::Result<()> {
    let rows: Vec<SendRow> = sqlx::query_as(
        "SELECT id, send_id, inbox_id, author, send_document, send_mac FROM conversations.drafts
          WHERE send_mac IS NOT NULL AND send_id IS NOT NULL AND send_document IS NOT NULL
          ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await?;
    for (id, send, inbox, author, document, mac) in rows {
        let Ok(document) = serde_json::from_value(document) else {
            continue;
        };
        let stored = StoredSend {
            workspace,
            send: SendId::from_uuid(send),
            inbox: InboxId::from_uuid(inbox),
            author: author.map(UserId::from),
            document: &document,
        };
        let Some(new) = stored.retag(key, Some(mac.as_slice())) else {
            continue;
        };
        let done = sqlx::query(
            "UPDATE conversations.drafts SET send_mac = $2 WHERE id = $1 AND send_mac = $3",
        )
        .bind(id)
        .bind(new)
        .bind(mac)
        .execute(&mut *conn)
        .await?;
        tally.count(done.rows_affected());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
