//! Runs of a taught rule over existing mail: applying it, or putting back
//! what it changed. A run is a row with a position; the job
//! `conversations.apply_rule` works through it a batch at a time and
//! announces its progress with `conversations.rule_changed`.
//!
//! A run row is no proof that a member asked for it: a transaction working
//! in the workspace can insert one, and the job that works through it
//! (campbooks-cloud#242). So a run carries a MAC under the kernel's
//! [`Binding`] with its own context label, signed only in the member's
//! request that starts it ([`start`]), right after that request signed or
//! verified the rule. It covers the workspace, the run, the rule, the rule's
//! binding generation then, the kind (apply or put back), the rule's inbox
//! (what the run covers: an apply run the inbox's mail its rule matches at
//! that generation, a revert run what the rule recorded there, each
//! application verified on its own) and the requesting member. The job acts
//! only on a run that verifies ([`bound`]), at the rule's generation now:
//! a rule signed again since (changed, paused and resumed, adopted) stops
//! its older runs.

use campbooks_kernel::{
    AppState, EventBus, Result,
    binding::{Binding, Fields},
    jobs::EnqueueOptions,
    tenancy::{Access, UserId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction, types::Uuid};
use utoipa::ToSchema;

use super::rules::{self, Stored};
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

/// Start a `kind` run of `rule` (cancelling one still running) for the
/// member of `access`, sign it, enqueue its first batch and announce it.
/// Returns it.
///
/// Only inside that member's authenticated request, right after it signed
/// or verified the rule (campbooks-cloud#242).
pub async fn start(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    access: &Access,
    rule: &Stored,
    kind: RunKind,
) -> Result<RunView> {
    let (events, binding) = (&state.events, &state.binding);
    let ws = access.workspace.to_string();
    let workspace = ws.as_str();
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
    sign(tx, binding, access, id, rule.id, kind).await?;
    let batch = RunBatch {
        run: id,
        after: None,
        counting: kind == RunKind::Apply,
        newest: None,
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
        // As readers see it: paused while its author may not act.
        rule.effective_state(),
        run.as_ref(),
    )
    .await?;
    run.ok_or(campbooks_kernel::Error::NotFound)
}

/// A run of a taught rule, as a member asked for it.
const RUN: &str = "campbooks/conversations.rule-run/v1";

/// The fields a run's MAC covers: `rule` as it is now (its generation and
/// inbox) and who asked for it.
fn run_fields(
    workspace: &Uuid,
    run: RuleRunId,
    rule: &Stored,
    kind: RunKind,
    by: Option<&Uuid>,
) -> Fields {
    let mut fields = Fields::new(RUN);
    fields
        .uuid(workspace)
        .uuid(run.as_uuid())
        .uuid(rule.id.as_uuid())
        .int(rule.generation)
        .str(kind.as_str())
        .uuid(rule.inbox.as_uuid())
        .opt_uuid(by);
    fields
}

/// Sign run `run` of rule `rule` for the member of `access`, from the rule
/// as stored now in this request. An apply run is signed only while its rule
/// is bound (it was just signed, or verified under its lock): an unbound
/// rule's run stays unsigned and the job stops it. A revert run is signed
/// whatever its rule's binding: the member asked to put back what it did,
/// and each application it puts back is verified on its own. Without a key
/// (no instance secret) the run stays unsigned, and stops.
async fn sign(
    conn: &mut PgConnection,
    binding: &Binding,
    access: &Access,
    run: RuleRunId,
    rule: RuleId,
    kind: RunKind,
) -> Result<()> {
    let Some(rule) = rules::find_any(conn, rule).await? else {
        return Ok(());
    };
    if kind == RunKind::Apply && !rule.bound(binding) {
        return Ok(());
    }
    let by = access.user.as_ref().map(UserId::as_uuid);
    let Some(mac) = binding.tag(&run_fields(
        access.workspace.as_uuid(),
        run,
        &rule,
        kind,
        by,
    )) else {
        return Ok(());
    };
    sqlx::query("UPDATE conversations.rule_runs SET requested_by = $2, mac = $3 WHERE id = $1")
        .bind(run.as_uuid())
        .bind(by.copied())
        .bind(mac)
        .execute(conn)
        .await?;
    Ok(())
}

/// Whether run `run`, a `kind` run of `rule` (as stored now) in `workspace`,
/// is bound to the member who asked for it: its MAC verifies over it, at
/// the rule's generation now. A run with no MAC (planted with SQL, or
/// started before runs were bound), another run's MAC, or a run of a rule
/// signed again since, is not.
pub async fn bound(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: &Uuid,
    run: RuleRunId,
    rule: &Stored,
    kind: RunKind,
) -> Result<bool> {
    let row: Option<(Option<Uuid>, Option<Vec<u8>>)> =
        sqlx::query_as("SELECT requested_by, mac FROM conversations.rule_runs WHERE id = $1")
            .bind(run.as_uuid())
            .fetch_optional(conn)
            .await?;
    let Some((by, mac)) = row else {
        return Ok(false);
    };
    let fields = run_fields(workspace, run, rule, kind, by.as_ref());
    Ok(binding.verify(&fields, mac.as_deref()))
}

/// The MAC of run `run` (a `kind` run of `rule` as stored now, asked for by
/// `by`) under the current key, when its stored `mac` verifies under the
/// previous key only (campbooks-cloud#191); `None` otherwise.
pub fn retag(
    binding: &Binding,
    workspace: &Uuid,
    run: RuleRunId,
    rule: &Stored,
    kind: RunKind,
    by: Option<&Uuid>,
    mac: Option<&[u8]>,
) -> Option<Vec<u8>> {
    binding.retag(&run_fields(workspace, run, rule, kind, by), mac)
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
    /// The newest unread conversation the rule matches that "Worth a look"
    /// may be raised for, found while counting: an apply run raises that one
    /// only (campbooks-cloud#282).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newest: Option<Newest>,
}

/// The conversation an apply run raises "Worth a look" for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Newest {
    /// The conversation.
    pub id: crate::model::ConversationId,
    /// When its newest inbound message was sent.
    pub sent_at: DateTime<Utc>,
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

/// Lock `rule`'s running runs, before anything touches the rule row.
///
/// Every path that locks a rule's runs and its row takes the runs first: a
/// batch holds its run row and then counts on the rule row, so taking them
/// the other way round (remove, restore, replace) would deadlock against it.
pub async fn lock_running(conn: &mut sqlx::PgConnection, rule: RuleId) -> Result<()> {
    sqlx::query!(
        r#"SELECT id FROM conversations.rule_runs
            WHERE rule_id = $1 AND state = 'running' ORDER BY id FOR UPDATE"#,
        rule as RuleId,
    )
    .fetch_all(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::rules::{RuleAction, RuleCondition},
        model::{RuleOrigin, TeachingId},
    };

    fn rule() -> Stored {
        Stored {
            id: RuleId::new(),
            inbox: InboxId::new(),
            teaching: TeachingId::new(),
            conditions: vec![RuleCondition::SenderAddress("news@example.com".into())],
            actions: vec![RuleAction::Trash],
            origin: RuleOrigin::Action,
            origin_text: None,
            state: RuleState::Active,
            taught_by: Some(UserId::new()),
            from_conversation: None,
            created_at: Utc::now(),
            confirmed_at: Some(Utc::now()),
            applied_count: 0,
            last_applied_at: None,
            author_may_act: true,
            author_suspended: false,
            inbox_detached: false,
            workspace: Uuid::now_v7(),
            generation: 3,
            mac: None,
        }
    }

    #[test]
    fn a_run_is_bound_only_as_it_was_started() {
        let binding = campbooks_kernel::testing::binding();
        let rule = rule();
        let workspace = Uuid::now_v7();
        let run = RuleRunId::new();
        let sam = Uuid::now_v7();
        let mac = binding
            .tag(&run_fields(
                &workspace,
                run,
                &rule,
                RunKind::Apply,
                Some(&sam),
            ))
            .unwrap();
        let holds = |fields: &Fields| binding.verify(fields, Some(mac.as_slice()));
        assert!(holds(&run_fields(
            &workspace,
            run,
            &rule,
            RunKind::Apply,
            Some(&sam)
        )));
        let mut resigned = rule.clone();
        resigned.generation += 1;
        let mut moved = rule.clone();
        moved.inbox = InboxId::new();
        let mut other_rule = rule.clone();
        other_rule.id = RuleId::new();
        let leo = Uuid::now_v7();
        let elsewhere = Uuid::now_v7();
        for (what, fields) in [
            (
                "another workspace",
                run_fields(&elsewhere, run, &rule, RunKind::Apply, Some(&sam)),
            ),
            (
                "another run",
                run_fields(
                    &workspace,
                    RuleRunId::new(),
                    &rule,
                    RunKind::Apply,
                    Some(&sam),
                ),
            ),
            (
                "another rule",
                run_fields(&workspace, run, &other_rule, RunKind::Apply, Some(&sam)),
            ),
            (
                "signed again since",
                run_fields(&workspace, run, &resigned, RunKind::Apply, Some(&sam)),
            ),
            (
                "another inbox",
                run_fields(&workspace, run, &moved, RunKind::Apply, Some(&sam)),
            ),
            (
                "a revert",
                run_fields(&workspace, run, &rule, RunKind::Revert, Some(&sam)),
            ),
            (
                "another member",
                run_fields(&workspace, run, &rule, RunKind::Apply, Some(&leo)),
            ),
            (
                "no member",
                run_fields(&workspace, run, &rule, RunKind::Apply, None),
            ),
        ] {
            assert!(!holds(&fields), "{what} still verifies");
        }
        let as_started = run_fields(&workspace, run, &rule, RunKind::Apply, Some(&sam));
        assert!(!Binding::disabled().verify(&as_started, Some(mac.as_slice())));
        assert!(!binding.verify(&as_started, None));
    }
}
