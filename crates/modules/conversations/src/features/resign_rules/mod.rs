//! Binding the taught rules that existed before rules were bound
//! (campbooks-cloud#242).
//!
//! A rule acts only while its MAC verifies (`domain::rules::binding`), and
//! only a member's request signs one. The rules stored before the binding
//! existed have none: they would stop acting at the deploy. They are signed
//! once, here, without letting any job sign what SQL can set:
//!
//! - the migration that added the binding
//!   (`…_conversations__rule_bindings.sql`) wrote each kept rule's id and a
//!   SHA-256 of its bound fields, computed in SQL, to
//!   `conversations.rule_resigns`, which the runtime role may read but never
//!   write (no `INSERT`, `UPDATE` or `DELETE` grant);
//! - `conversations.dispatch_rule_resigns`, every minute, reads which
//!   workspaces still have rows there (the maintenance scope: ids only,
//!   through the table's maintenance policy) and enqueues
//!   `conversations.resign_rules` in each;
//! - `conversations.resign_rules`, in the workspace, locks each listed rule
//!   and asks `conversations.take_rule_resign` (a `SECURITY DEFINER` function
//!   owned by `campbooks_definer`) to remove its row. The function answers
//!   true only when the rule, as stored now, still hashes to what the
//!   migration recorded; the rule is then signed as it is. A rule changed
//!   since is not signed (a WARN with ids only): it waits for a member to
//!   confirm it.
//!
//! The comparison is SQL against SQL (the same expression at the migration
//! and in the function), so it never depends on the Rust encoding matching
//! a SQL one. Nothing new can be added to the snapshot after the migration:
//! a row inserted by the runtime role is refused, and a job planted with any
//! payload only consumes rows that are already there. Without an instance
//! secret nothing is taken, so the rows wait for a key.

use std::time::Duration;

use campbooks_kernel::{
    Job, JobContext, JobError,
    jobs::{self, EnqueueOptions},
    tenancy::WorkspaceId,
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{domain::rules::binding, model::RuleId};

/// How often workspaces with rules still to sign are looked for.
pub const EVERY: Duration = Duration::from_mins(1);

/// Workspaces enqueued per tick.
const DISPATCH_BATCH: i64 = 500;

/// Rules signed per job; the next tick enqueues the rest.
const RULES_PER_JOB: i64 = 200;

/// The dispatcher tick: enqueue [`ResignRules`] in each workspace that still
/// has rules stored before rules were bound.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchRuleResigns;

impl Job for DispatchRuleResigns {
    const NAME: &'static str = "conversations.dispatch_rule_resigns";
    const QUEUE: &'static str = "maintenance";
    const MAX_ATTEMPTS: u32 = 1;

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        // Across workspaces, through the table's maintenance policy (ids
        // only): the maintenance scope, allowed for this file in
        // `architecture.toml`. Each job is enqueued in its workspace.
        let tx = ctx.maintenance_tx().await?;
        let workspaces: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT workspace_id FROM conversations.rule_resigns
              ORDER BY workspace_id LIMIT $1",
        )
        .bind(DISPATCH_BATCH)
        .fetch_all(&mut **tx)
        .await?;
        for workspace in workspaces {
            let workspace = WorkspaceId::from_uuid(workspace).to_string();
            jobs::enqueue_in(
                tx,
                &workspace,
                &ResignRules,
                EnqueueOptions::for_workspace(workspace.clone())
                    .unique("conversations.resign_rules")
                    .priority(-10),
            )
            .await?;
        }
        Ok(())
    }
}

/// Signs, in the job's workspace, the rules the migration snapshotted that
/// are still as they were.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResignRules;

impl Job for ResignRules {
    const NAME: &'static str = "conversations.resign_rules";
    const QUEUE: &'static str = "maintenance";

    async fn run(self, ctx: &mut JobContext) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("resign_rules enqueued outside a workspace"))?;
        let key = ctx.state().binding.clone();
        if !key.is_enabled() {
            // Nothing can be signed: the rows wait for a key.
            tracing::warn!(
                %workspace,
                "taught rules are not bound: there is no key to bind them with \
                 (CAMPBOOKS_SECRET_KEY is unset); they do not act"
            );
            return Ok(());
        }
        let tx = ctx.tx().await?;
        let pending: Vec<Uuid> = sqlx::query_scalar(
            "SELECT rule_id FROM conversations.rule_resigns ORDER BY rule_id LIMIT $1",
        )
        .bind(RULES_PER_JOB)
        .fetch_all(&mut **tx)
        .await?;
        for id in pending {
            let rule = RuleId::from_uuid(id);
            // A member's request holding it signs or leaves it; the next
            // tick tries again. Never waiting, so never deadlocking.
            let locked: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM conversations.rules WHERE id = $1 FOR UPDATE SKIP LOCKED",
            )
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?;
            if locked.is_none() {
                continue;
            }
            let unchanged: bool = sqlx::query_scalar("SELECT conversations.take_rule_resign($1)")
                .bind(id)
                .fetch_one(&mut **tx)
                .await?;
            if !unchanged {
                tracing::warn!(
                    %workspace,
                    %rule,
                    "a taught rule changed since rules were bound; it waits for a member to \
                     confirm it"
                );
                continue;
            }
            binding::sign(tx, &key, rule).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
