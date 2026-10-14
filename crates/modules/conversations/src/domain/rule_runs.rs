//! Runs of a taught rule over existing mail: applying it, or putting back
//! what it changed. A run is a row with a position; the job
//! `conversations.apply_rule` works through it a batch at a time and
//! announces its progress with `conversations.rule_changed`.

use campbooks_kernel::{EventBus, Result, jobs::EnqueueOptions};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;

use super::rules::{self, Stored};
use crate::{
    events::{RuleChanged, RunFacts},
    model::{InboxId, RuleId, RuleRunId, RuleState, RunKind, RunState},
};

/// Conversations changed per batch (one transaction each).
pub const BATCH: i64 = 100;
/// Conversations looked at per batch while counting or applying.
pub const SCAN: i64 = 500;

/// A run, as clients show its progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RunView {
    /// The run.
    pub id: RuleRunId,
    /// Applying, or putting back.
    pub kind: RunKind,
    /// Where it is.
    pub state: RunState,
    /// Conversations it looks at.
    pub total: u32,
    /// Looked at so far.
    pub done: u32,
    /// Changed so far.
    pub changed: u32,
    /// Left alone because they look suspicious.
    pub skipped_suspicious: u32,
    /// Left alone because a member placed them or changed them back.
    pub skipped_member: u32,
    /// When it started.
    pub started_at: DateTime<Utc>,
    /// When it ended.
    pub finished_at: Option<DateTime<Utc>>,
}

fn count(n: i32) -> u32 {
    u32::try_from(n).unwrap_or_default()
}

/// The newest run of `rule`, if it has had one.
pub async fn latest(conn: &mut PgConnection, rule: RuleId) -> Result<Option<RunView>> {
    let row = sqlx::query!(
        r#"SELECT id AS "id: RuleRunId", kind, state, total, done, changed, skipped_suspicious,
                  skipped_member, started_at, finished_at
             FROM conversations.rule_runs WHERE rule_id = $1
            ORDER BY started_at DESC, id DESC LIMIT 1"#,
        rule as RuleId,
    )
    .fetch_optional(conn)
    .await?;
    row.map(|r| {
        Ok(RunView {
            id: r.id,
            kind: r.kind.parse()?,
            state: r.state.parse()?,
            total: count(r.total),
            done: count(r.done),
            changed: count(r.changed),
            skipped_suspicious: count(r.skipped_suspicious),
            skipped_member: count(r.skipped_member),
            started_at: r.started_at,
            finished_at: r.finished_at,
        })
    })
    .transpose()
}

/// How many conversations of `rule`'s inbox it matches now.
pub async fn matching(conn: &mut PgConnection, rule: &Stored) -> Result<u32> {
    let engine = rule.engine();
    let mut after = None;
    let mut n = 0_u32;
    loop {
        let page = rules::scan(conn, rule.inbox, &rule.conditions, after, SCAN).await?;
        n += u32::try_from(page.iter().filter(|c| engine.matches(&c.facts())).count())
            .unwrap_or_default();
        if page.len() < usize::try_from(SCAN).unwrap_or(usize::MAX) {
            return Ok(n);
        }
        after = page.last().map(|c| c.id);
    }
}

/// Start a `kind` run of `rule` (cancelling one still running), enqueue its
/// first batch and announce it. Returns it.
pub async fn start(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    workspace: &str,
    rule: &Stored,
    kind: RunKind,
) -> Result<RunView> {
    sqlx::query!(
        r#"UPDATE conversations.rule_runs SET state = 'cancelled', finished_at = now()
            WHERE rule_id = $1 AND state = 'running'"#,
        rule.id as RuleId,
    )
    .execute(&mut **tx)
    .await?;
    let total = match kind {
        RunKind::Apply => matching(tx, rule).await?,
        RunKind::Revert => u32::try_from(
            sqlx::query_scalar!(
                r#"SELECT count(*)::int AS "n!" FROM conversations.rule_applications
                    WHERE rule_id = $1 AND reverted_at IS NULL AND excepted_at IS NULL"#,
                rule.id as RuleId,
            )
            .fetch_one(&mut **tx)
            .await?,
        )
        .unwrap_or_default(),
    };
    let id = RuleRunId::new();
    sqlx::query!(
        r#"INSERT INTO conversations.rule_runs (id, workspace_id, rule_id, kind, state, total)
           SELECT $1, workspace_id, id, $3, 'running', $4 FROM conversations.rules WHERE id = $2"#,
        id as RuleRunId,
        rule.id as RuleId,
        kind.as_str(),
        i32::try_from(total).unwrap_or(i32::MAX),
    )
    .execute(&mut **tx)
    .await?;
    let batch = RunBatch {
        run: id,
        after: None,
    };
    campbooks_kernel::jobs::enqueue_to::<crate::features::apply_rule::ApplyRules>(
        tx,
        &batch,
        batch.options(workspace),
    )
    .await?;
    let run = latest(tx, rule.id).await?;
    announce(
        tx,
        events,
        workspace,
        rule.inbox,
        rule.id,
        rule.state,
        run.as_ref(),
    )
    .await?;
    run.ok_or(campbooks_kernel::Error::NotFound)
}

/// Publish `rule_changed` for `rule` in `state`, with `run`.
pub async fn announce(
    tx: &mut Transaction<'_, Postgres>,
    events: &EventBus,
    workspace: &str,
    inbox: InboxId,
    rule: RuleId,
    state: RuleState,
    run: Option<&RunView>,
) -> Result<()> {
    events
        .publish(
            tx,
            &RuleChanged {
                workspace: workspace.to_owned(),
                inbox,
                rule,
                state,
                run: run.map(|r| RunFacts {
                    id: r.id,
                    kind: r.kind,
                    state: r.state,
                    total: r.total,
                    done: r.done,
                }),
            },
        )
        .await?;
    Ok(())
}

/// One batch of a run: the conversations after `after`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunBatch {
    /// The run.
    pub run: RuleRunId,
    /// The last conversation of the previous batch; `None` for the first.
    pub after: Option<crate::model::ConversationId>,
}

impl RunBatch {
    /// One pending job per batch, behind interactive work.
    pub fn options(&self, workspace: &str) -> EnqueueOptions {
        let after = self.after.map_or_else(|| "^".to_owned(), |a| a.to_string());
        EnqueueOptions::for_workspace(workspace.to_owned())
            .unique(format!("conversations.apply_rule:{}:{after}", self.run))
            .priority(-5)
    }
}
