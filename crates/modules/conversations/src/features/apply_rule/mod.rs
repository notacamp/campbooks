//! `conversations.apply_rule` — one batch of a taught rule's run over
//! existing mail (campbooks-cloud#125).
//!
//! An apply run looks at the inbox's conversations in id order, a batch at a
//! time, and carries out what the rule wins on each one it matches; a revert
//! run puts back what the rule changed. Each batch is one transaction that
//! records its position and progress, publishes the changes and the run's
//! `rule_changed`, and enqueues the next batch, so a run survives restarts
//! and never changes a conversation twice.

#[cfg(test)]
mod tests;

use campbooks_kernel::{JobContext, JobError, JobHandler, tenancy::WorkspaceId};

use crate::{
    domain::{
        needs::NeedSettings,
        rule_runs::{self, BATCH, COUNT_WINDOW, RunBatch, SCAN},
        rules::{self, Applied, Context},
    },
    model::{ConversationId, RuleId, RuleRunId, RuleState, RunKind, RunState},
};

/// Works through runs of taught rules.
pub struct ApplyRules {
    needs: NeedSettings,
}

impl ApplyRules {
    /// A handler raising needs as `needs` says.
    pub fn new(needs: NeedSettings) -> Self {
        Self { needs }
    }
}

impl JobHandler for ApplyRules {
    type Payload = RunBatch;
    const NAME: &'static str = "conversations.apply_rule";
    const QUEUE: &'static str = "rules";

    async fn run(&self, ctx: &mut JobContext, batch: RunBatch) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("apply_rule enqueued outside a workspace"))?;
        let ws = workspace.to_string();
        let events = ctx.state().events.clone();
        let tx = ctx.tx().await?;
        let run = sqlx::query!(
            r#"SELECT rule_id AS "rule: RuleId", kind, state, counted,
                      cursor AS "cursor: ConversationId"
                 FROM conversations.rule_runs WHERE id = $1 FOR UPDATE"#,
            batch.run as RuleRunId,
        )
        .fetch_optional(&mut **tx)
        .await?;
        // A newer run replaced this one, its inbox is gone, or this batch was
        // done already.
        let Some(run) = run else { return Ok(()) };
        if run.state != RunState::Running.as_str()
            || run.cursor != batch.after
            || run.counted == batch.counting
        {
            return Ok(());
        }
        let kind: RunKind = run.kind.parse()?;
        let Some(rule) = rules::find_any(tx, run.rule).await? else {
            return end(tx, batch.run, RunState::Cancelled).await;
        };
        // A rule applies only while active and its author may change the
        // inbox; putting mail back goes on whatever the rule's state.
        if kind == RunKind::Apply && rule.effective_state() != RuleState::Active {
            return end(tx, batch.run, RunState::Cancelled).await;
        }
        let context = Context {
            events: &events,
            workspace: &ws,
            workspace_id: *workspace.as_uuid(),
            inbox: rule.inbox,
            needs: &self.needs,
        };
        let next = if batch.counting {
            count_batch(tx, &rule, &batch).await?
        } else {
            let mut tally = Tally::default();
            let last = match kind {
                RunKind::Apply => apply_batch(tx, &context, &rule, &batch, &mut tally).await?,
                RunKind::Revert => revert_batch(tx, &context, &rule, &batch, &mut tally).await?,
            };
            progress(tx, batch.run, &tally, last).await?;
            last.map(|after| RunBatch {
                run: batch.run,
                after: Some(after),
                counting: false,
            })
        };
        match next {
            Some(next) => {
                campbooks_kernel::jobs::enqueue_to::<Self>(tx, &next, next.options(&ws)).await?;
            }
            None => end(tx, batch.run, RunState::Done).await?,
        }
        let view = rule_runs::latest(tx, rule.id).await?;
        let state = if kind == RunKind::Revert {
            RuleState::Removed
        } else {
            rule.state
        };
        rule_runs::announce(tx, &events, &ws, rule.inbox, rule.id, state, view.as_ref()).await?;
        Ok(())
    }
}

/// Count the conversations `rule` matches in the next window. Returns the
/// next batch: more counting, or the first applying batch once counted.
async fn count_batch(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    rule: &rules::Stored,
    batch: &RunBatch,
) -> Result<Option<RunBatch>, JobError> {
    let engine = rule.engine();
    let window = rules::scan(tx, rule.inbox, &rule.conditions, batch.after, COUNT_WINDOW).await?;
    let matched = window
        .candidates
        .iter()
        .filter(|c| engine.matches(&c.facts()))
        .count();
    let counted = window.next.is_none();
    sqlx::query!(
        r#"UPDATE conversations.rule_runs
              SET total = total + $2, counted = $3, cursor = CASE WHEN $3 THEN NULL ELSE $4::uuid END
            WHERE id = $1"#,
        batch.run as RuleRunId,
        i32::try_from(matched).unwrap_or(i32::MAX),
        counted,
        window.next.map(|c| *c.as_uuid()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(Some(RunBatch {
        run: batch.run,
        after: window.next,
        counting: !counted,
    }))
}

/// Apply `rule` to the next [`BATCH`] conversations it matches after the
/// batch's position. Returns the position to go on from, if any is left.
async fn apply_batch(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    context: &Context<'_>,
    rule: &rules::Stored,
    batch: &RunBatch,
    tally: &mut Tally,
) -> Result<Option<ConversationId>, JobError> {
    let acting = rules::acting(tx, rule.inbox).await?;
    let engine = rule.engine();
    // One window of the inbox at most, and at most [`BATCH`] changes.
    let window = rules::scan(tx, rule.inbox, &rule.conditions, batch.after, SCAN).await?;
    for candidate in &window.candidates {
        if !engine.matches(&candidate.facts()) {
            continue;
        }
        tally.done += 1;
        let applied =
            rules::apply(tx, context, rule.id, &acting, candidate, Some(batch.run)).await?;
        tally.count(applied);
        if tally.done >= BATCH {
            return Ok(Some(candidate.id));
        }
    }
    Ok(window.next)
}

/// Put back the next [`BATCH`] conversations `rule` changed.
async fn revert_batch(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    context: &Context<'_>,
    rule: &rules::Stored,
    batch: &RunBatch,
    tally: &mut Tally,
) -> Result<Option<ConversationId>, JobError> {
    let ids = applications(tx, rule.id, batch.after).await?;
    for id in &ids {
        tally.done += 1;
        if rules::revert(tx, context, rule, *id).await? {
            tally.changed += 1;
        }
    }
    Ok(ids
        .last()
        .copied()
        .filter(|_| ids.len() == usize::try_from(BATCH).unwrap_or(0)))
}

/// What one batch did.
#[derive(Debug, Default)]
struct Tally {
    done: i64,
    changed: i64,
    suspicious: i64,
    member: i64,
}

impl Tally {
    fn count(&mut self, applied: Applied) {
        match applied {
            Applied::Changed => self.changed += 1,
            Applied::Suspicious => self.suspicious += 1,
            Applied::Member => self.member += 1,
            Applied::Unchanged => {}
        }
    }
}

async fn progress(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    run: RuleRunId,
    tally: &Tally,
    cursor: Option<ConversationId>,
) -> Result<(), JobError> {
    let n = |v: i64| i32::try_from(v).unwrap_or(i32::MAX);
    sqlx::query!(
        r#"UPDATE conversations.rule_runs
              SET done = done + $2, changed = changed + $3,
                  skipped_suspicious = skipped_suspicious + $4,
                  skipped_member = skipped_member + $5, cursor = $6
            WHERE id = $1"#,
        run as RuleRunId,
        n(tally.done),
        n(tally.changed),
        n(tally.suspicious),
        n(tally.member),
        cursor.map(|c| *c.as_uuid()),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn end(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    run: RuleRunId,
    state: RunState,
) -> Result<(), JobError> {
    sqlx::query!(
        r#"UPDATE conversations.rule_runs SET state = $2, finished_at = now() WHERE id = $1"#,
        run as RuleRunId,
        state.as_str(),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The next conversations whose change by `rule` can be put back.
async fn applications(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    rule: RuleId,
    after: Option<ConversationId>,
) -> Result<Vec<ConversationId>, JobError> {
    Ok(sqlx::query_scalar!(
        r#"SELECT conversation_id AS "id: ConversationId" FROM conversations.rule_applications
            WHERE rule_id = $1 AND reverted_at IS NULL AND excepted_at IS NULL
              AND ($2::uuid IS NULL OR conversation_id > $2)
            ORDER BY conversation_id LIMIT $3"#,
        rule as RuleId,
        after.map(|a| *a.as_uuid()),
        BATCH,
    )
    .fetch_all(&mut **tx)
    .await?)
}
