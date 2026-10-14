//! Runs of a taught rule over existing mail: applying it, or putting back
//! what it changed. A run is a row with a position; the job
//! `conversations.apply_rule` works through it a batch at a time and
//! announces its progress with `conversations.rule_changed`.

use campbooks_kernel::{EventBus, Result, jobs::EnqueueOptions};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::ToSchema;

use super::rules::Stored;
use crate::{
    events::{RuleChanged, RunFacts},
    model::{InboxId, RuleId, RuleRunId, RuleState, RunKind, RunState},
};

/// Most conversations changed per batch (one transaction each).
pub const BATCH: i64 = 100;
/// Most conversations an applying batch looks at.
pub const SCAN: i64 = 500;
/// Most conversations a counting batch looks at.
pub const COUNT_WINDOW: i64 = 5_000;

/// A run, as clients show its progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RunView {
    /// The run.
    pub id: RuleRunId,
    /// Applying, or putting back.
    pub kind: RunKind,
    /// Where it is.
    pub state: RunState,
    /// Conversations it matches (or will put back).
    pub total: u32,
    /// Whether `total` is final; while false the run is still counting.
    pub counted: bool,
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

/// The newest run of each of `rules` that has had one.
pub async fn latest_of(
    conn: &mut PgConnection,
    rules: &[RuleId],
) -> Result<std::collections::HashMap<RuleId, RunView>> {
    let rows = sqlx::query!(
        r#"SELECT DISTINCT ON (rule_id) rule_id AS "rule: RuleId", id AS "id: RuleRunId", kind,
                  state, total, counted, done, changed, skipped_suspicious, skipped_member,
                  started_at, finished_at
             FROM conversations.rule_runs WHERE rule_id = ANY($1)
            ORDER BY rule_id, started_at DESC, id DESC"#,
        rules as &[RuleId],
    )
    .fetch_all(conn)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok((
                r.rule,
                RunView {
                    id: r.id,
                    kind: r.kind.parse()?,
                    state: r.state.parse()?,
                    total: count(r.total),
                    counted: r.counted,
                    done: count(r.done),
                    changed: count(r.changed),
                    skipped_suspicious: count(r.skipped_suspicious),
                    skipped_member: count(r.skipped_member),
                    started_at: r.started_at,
                    finished_at: r.finished_at,
                },
            ))
        })
        .collect()
}

/// The newest run of `rule`, if it has had one.
pub async fn latest(conn: &mut PgConnection, rule: RuleId) -> Result<Option<RunView>> {
    let row = sqlx::query!(
        r#"SELECT id AS "id: RuleRunId", kind, state, total, counted, done, changed, skipped_suspicious,
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
            counted: r.counted,
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
    // An apply run counts in its own first batches, never here; a revert's
    // total is the applications to put back.
    let total = match kind {
        RunKind::Apply => None,
        RunKind::Revert => Some(
            sqlx::query_scalar!(
                r#"SELECT count(*)::int AS "n!" FROM conversations.rule_applications
                    WHERE rule_id = $1 AND reverted_at IS NULL AND excepted_at IS NULL"#,
                rule.id as RuleId,
            )
            .fetch_one(&mut **tx)
            .await?,
        ),
    };
    let id = RuleRunId::new();
    sqlx::query!(
        r#"INSERT INTO conversations.rule_runs
               (id, workspace_id, rule_id, kind, state, total, counted)
           SELECT $1, workspace_id, id, $3, 'running', coalesce($4, 0), $4 IS NOT NULL
             FROM conversations.rules WHERE id = $2"#,
        id as RuleRunId,
        rule.id as RuleId,
        kind.as_str(),
        total,
    )
    .execute(&mut **tx)
    .await?;
    let batch = RunBatch {
        run: id,
        after: None,
        counting: kind == RunKind::Apply,
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

/// One batch of a run: the conversations after `after`, counted or changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunBatch {
    /// The run.
    pub run: RuleRunId,
    /// The last conversation of the previous batch; `None` for the first.
    pub after: Option<crate::model::ConversationId>,
    /// Counting the conversations the rule matches, before changing them.
    #[serde(default)]
    pub counting: bool,
}

impl RunBatch {
    /// One pending job per batch, behind interactive work.
    pub fn options(&self, workspace: &str) -> EnqueueOptions {
        let after = self.after.map_or_else(|| "^".to_owned(), |a| a.to_string());
        let phase = if self.counting { "count" } else { "apply" };
        EnqueueOptions::for_workspace(workspace.to_owned())
            .unique(format!(
                "conversations.apply_rule:{}:{phase}:{after}",
                self.run
            ))
            .priority(-5)
    }
}
