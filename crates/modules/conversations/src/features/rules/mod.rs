//! Taught rules: preview, create, confirm, change, remove and list them
//! (campbooks-cloud#125).
//!
//! Every producer of rules (a member's action, an onboarding answer, the
//! member's own words, corrections) goes through these operations, so a rule
//! is checked, stored and applied one way whatever taught it.

#[cfg(test)]
pub(crate) mod tests;

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    db::BeginFor as _,
    extract,
    tenancy::{Access, UserId, Workspace, WorkspaceId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, Postgres, Transaction};
use utoipa::{IntoParams, ToSchema};

use crate::{
    domain::{
        access::visible_inboxes,
        rule_runs::{self, RunView, SCAN},
        rules::{self, NewRule, RuleAction, RuleCondition, Stored},
    },
    model::{
        ConversationId, InboxId, InboxRole, RuleId, RuleOrigin, RuleState, RunKind, TeachingId,
    },
    scopes::{ConversationsRead, ConversationsWrite},
};

/// Longest instruction kept with a rule, in characters.
pub const MAX_ORIGIN_TEXT_CHARS: usize = 500;
/// Most sample conversations a preview shows.
pub const PREVIEW_SAMPLES: usize = 5;
/// Most conversations a preview looks at; past it the counts say "at least".
pub const PREVIEW_EXAMINE: usize = 20_000;
/// How long a preview, or all of a suggestion's previews together, may look
/// before answering with what it found (`complete: false`).
pub const PREVIEW_DEADLINE: std::time::Duration = std::time::Duration::from_secs(3);

/// A read-only transaction in `workspace` whose statements give up after
/// five seconds: previews and suggestions never write, and never run long.
pub async fn begin_read(
    state: &AppState,
    workspace: WorkspaceId,
) -> Result<Transaction<'static, Postgres>> {
    let mut tx = state.db.begin_for(workspace).await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// `…/inboxes/{inbox_id}/rules…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct InboxPath {
    /// The inbox.
    pub inbox_id: InboxId,
}

/// `…/rules/{rule_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RulePath {
    /// The rule.
    pub rule_id: RuleId,
}

/// A taught rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleView {
    /// The rule.
    pub id: RuleId,
    /// Its inbox.
    pub inbox_id: InboxId,
    /// The teaching it came with (shared by its rules in other inboxes).
    pub teaching_id: TeachingId,
    /// All must hold.
    pub conditions: Vec<RuleCondition>,
    /// What it does.
    pub actions: Vec<RuleAction>,
    /// Where it came from.
    pub origin: RuleOrigin,
    /// The member's own words, for an instruction.
    pub origin_text: Option<String>,
    /// Whether it acts. `paused` when its author can no longer change the inbox.
    pub state: RuleState,
    /// Who taught it; `None` when a client did.
    pub taught_by: Option<UserId>,
    /// The conversation it was taught from.
    pub from_conversation_id: Option<ConversationId>,
    /// When it was made.
    pub created_at: DateTime<Utc>,
    /// When a member said yes to it.
    pub confirmed_at: Option<DateTime<Utc>>,
    /// Conversations it has changed.
    pub applied_count: u32,
    /// When it last changed one.
    pub last_applied_at: Option<DateTime<Utc>>,
    /// Conversations members changed back in the last 30 days.
    pub recent_exceptions: u32,
    /// Its newest run over existing mail.
    pub run: Option<RunView>,
    /// Whether other members use its inbox: it acts for them too.
    pub shared: bool,
    /// Whether the caller may change or remove it.
    pub may_change: bool,
}

async fn view(conn: &mut PgConnection, access: &Access, rule: &Stored) -> Result<RuleView> {
    let mut views = views(conn, access, std::slice::from_ref(rule)).await?;
    views.pop().ok_or(Error::NotFound)
}

/// `stored` rules as the caller sees them, with a few queries for all of them.
async fn views(
    conn: &mut PgConnection,
    access: &Access,
    stored: &[Stored],
) -> Result<Vec<RuleView>> {
    let ids: Vec<RuleId> = stored.iter().map(|r| r.id).collect();
    let mut inboxes: Vec<InboxId> = stored.iter().map(|r| r.inbox).collect();
    inboxes.sort_unstable();
    inboxes.dedup();
    let inbox_roles = rules::roles_of(conn, &inboxes, access.user).await?;
    let shared = rules::shared_of(conn, &inboxes).await?;
    let exceptions = rules::recent_exceptions_of(conn, &ids).await?;
    let mut runs = rule_runs::latest_of(conn, &ids).await?;
    Ok(stored
        .iter()
        .map(|rule| RuleView {
            id: rule.id,
            inbox_id: rule.inbox,
            teaching_id: rule.teaching,
            conditions: rule.conditions.clone(),
            actions: rule.actions.clone(),
            origin: rule.origin,
            origin_text: rule.origin_text.clone(),
            state: rule.effective_state(),
            taught_by: rule.taught_by,
            from_conversation_id: rule.from_conversation,
            created_at: rule.created_at,
            confirmed_at: rule.confirmed_at,
            applied_count: u32::try_from(rule.applied_count).unwrap_or_default(),
            last_applied_at: rule.last_applied_at,
            recent_exceptions: exceptions.get(&rule.id).copied().unwrap_or_default(),
            run: runs.remove(&rule.id),
            shared: shared.contains(&rule.inbox),
            may_change: rules::may_change(
                inbox_roles.get(&rule.inbox).copied(),
                access.user,
                rule.taught_by,
            ),
        })
        .collect())
}

/// 404 unless the caller sees `inbox`; 403 `inbox_read_only` unless they may
/// tidy it (a sender or above, or a client token).
async fn may_teach(conn: &mut PgConnection, access: &Access, inbox: InboxId) -> Result<()> {
    if !visible_inboxes(conn, access).await?.contains(&inbox) {
        return Err(Error::NotFound);
    }
    if access.user.is_some()
        && !matches!(
            rules::role_in(conn, inbox, access.user).await?,
            Some(InboxRole::Sender | InboxRole::Manager)
        )
    {
        return Err(Error::Denied(crate::MESSAGES.reason("inbox_read_only")));
    }
    Ok(())
}

/// 403 `rule_needs_member` for a client token: rules are taught by members.
fn member_only(access: &Access) -> Result<()> {
    if access.user.is_none() {
        return Err(Error::Denied(crate::MESSAGES.reason("rule_needs_member")));
    }
    Ok(())
}

/// Rule `id` if the caller sees its inbox, else 404.
async fn visible_rule(conn: &mut PgConnection, access: &Access, id: RuleId) -> Result<Stored> {
    let rule = rules::find(conn, id).await?.ok_or(Error::NotFound)?;
    if !visible_inboxes(conn, access).await?.contains(&rule.inbox) {
        return Err(Error::NotFound);
    }
    Ok(rule)
}

/// Rule `id` if the caller may change it: 404 when they cannot see it, 403
/// `rule_not_yours` when they are not its author (a sender) or a manager.
async fn changeable_rule(conn: &mut PgConnection, access: &Access, id: RuleId) -> Result<Stored> {
    let rule = visible_rule(conn, access, id).await?;
    member_only(access)?;
    let inbox_role = rules::role_in(conn, rule.inbox, access.user).await?;
    if !rules::may_change(inbox_role, access.user, rule.taught_by) {
        return Err(Error::Denied(crate::MESSAGES.reason("rule_not_yours")));
    }
    Ok(rule)
}

// --- Preview -----------------------------------------------------------------

/// A rule to preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewRule {
    /// All must hold.
    pub conditions: Vec<RuleCondition>,
    /// What it would do.
    pub actions: Vec<RuleAction>,
}

/// One conversation a rule would change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PreviewSample {
    /// The conversation.
    pub conversation_id: ConversationId,
    /// Its newest inbound message's subject.
    pub subject: String,
    /// Its sender's address.
    pub sender: String,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
}

/// An active rule a new one would take over from on some conversations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleOverride {
    /// The rule.
    pub rule_id: RuleId,
    /// Its conditions, to say which.
    pub conditions: Vec<RuleCondition>,
    /// Its actions.
    pub actions: Vec<RuleAction>,
    /// On how many conversations.
    pub conversations: u32,
}

/// What a rule would do to the inbox's mail now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RulePreview {
    /// Conversations it matches.
    pub matching: u32,
    /// Of those, the ones it would change.
    pub would_change: u32,
    /// The newest of them.
    pub samples: Vec<PreviewSample>,
    /// Left alone because they look suspicious.
    pub skipped_suspicious: u32,
    /// Left alone because a member placed them.
    pub skipped_member: u32,
    /// Active rules it would take over from.
    pub overrides: Vec<RuleOverride>,
    /// Whether every conversation of the inbox was looked at; when false the
    /// counts are "at least" (a very large inbox).
    pub complete: bool,
}

/// Preview what a rule would do in an inbox. A read served by POST: the
/// rule's addresses and words stay out of the URL (and so out of access logs).
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/inboxes/{inbox_id}/rules/preview",
    operation_id = "conversations.preview_rule",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::body_query().assistant().records(["conversations.rule"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("inbox_id" = InboxId, Path, description = "The inbox."),
    ),
    request_body = PreviewRule,
    responses(
        (status = OK, description = "What it would do.", body = RulePreview),
        (status = BAD_REQUEST, description = "Not a valid rule.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such inbox, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn preview(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<InboxPath>,
    extract::Json(body): extract::Json<PreviewRule>,
) -> Result<Json<RulePreview>> {
    let (conditions, actions) = (rules::normalise(&body.conditions), body.actions);
    rules::validate(&conditions, &actions)?;
    let access = &workspace.access;
    let mut tx = begin_read(&state, access.workspace).await?;
    may_teach(&mut tx, access, path.inbox_id).await?;
    let deadline = std::time::Instant::now() + PREVIEW_DEADLINE;
    let preview =
        compute_preview(&mut tx, path.inbox_id, (&conditions, &actions), deadline).await?;
    tx.commit().await?;
    Ok(Json(preview))
}

/// What a rule with `parts` (its conditions and actions) would do in
/// `inbox`. Looks at one window at least, then stops at `deadline` or
/// [`PREVIEW_EXAMINE`] conversations, whichever comes first (`complete:
/// false`).
pub async fn compute_preview(
    conn: &mut PgConnection,
    inbox: InboxId,
    parts: (&[RuleCondition], &[RuleAction]),
    deadline: std::time::Instant,
) -> Result<RulePreview> {
    use campbooks_ai::decide::taught::{TaughtRule, evaluate};

    const NEW: &str = "new";
    let (conditions, actions) = parts;
    let (c, a) = rules::engine_parts(conditions, actions);
    let new = TaughtRule {
        id: NEW.to_owned(),
        conditions: c,
        actions: a,
        confirmed_at: i64::MAX,
    };
    let stored = rules::of_inboxes(conn, &[inbox]).await?;
    let acting: Vec<TaughtRule> = stored
        .iter()
        .filter(|r| r.effective_state() == RuleState::Active)
        .map(Stored::engine)
        .collect();
    let mut with_new = acting.clone();
    with_new.push(new.clone());

    let mut out = RulePreview {
        matching: 0,
        would_change: 0,
        samples: Vec::new(),
        skipped_suspicious: 0,
        skipped_member: 0,
        overrides: Vec::new(),
        complete: false,
    };
    let mut overridden: Vec<(String, u32)> = Vec::new();
    let mut found = Vec::new();
    let mut after = None;
    let mut looked = 0_usize;
    loop {
        let window = rules::scan(conn, inbox, conditions, after, SCAN).await?;
        looked += window.examined;
        for candidate in &window.candidates {
            let facts = candidate.facts();
            if !new.matches(&facts) {
                continue;
            }
            out.matching += 1;
            if candidate.suspicious && rules::hides(actions) {
                out.skipped_suspicious += 1;
                continue;
            }
            let moves = actions.iter().any(
                |a| matches!(a, RuleAction::MoveToStream { stream } if *stream != candidate.stream),
            );
            if candidate.stream_source == "member" && moves {
                out.skipped_member += 1;
                continue;
            }
            let before = evaluate(&acting, &facts);
            let after_outcome = evaluate(&with_new, &facts);
            if would_change(candidate, &after_outcome, NEW) {
                out.would_change += 1;
                // Suspicious mail is counted, never shown as an example.
                if !candidate.suspicious {
                    found.push(candidate.clone());
                }
            }
            count_overrides(&before, &after_outcome, NEW, &mut overridden);
        }
        match window.next {
            None => {
                out.complete = true;
                break;
            }
            Some(_) if looked >= PREVIEW_EXAMINE || std::time::Instant::now() >= deadline => {
                break;
            }
            next => after = next,
        }
    }
    found.sort_by_key(|c| std::cmp::Reverse(c.sent_at));
    out.samples = found.iter().take(PREVIEW_SAMPLES).map(sample).collect();
    out.overrides = overridden
        .into_iter()
        .filter_map(|(id, n)| {
            let rule = stored.iter().find(|r| r.id.to_string() == id)?;
            Some(RuleOverride {
                rule_id: rule.id,
                conditions: rule.conditions.clone(),
                actions: rule.actions.clone(),
                conversations: n,
            })
        })
        .collect();
    Ok(out)
}

fn sample(c: &rules::Candidate) -> PreviewSample {
    PreviewSample {
        conversation_id: c.id,
        subject: c.subject.clone(),
        sender: c.sender_address.clone(),
        sent_at: c.sent_at,
    }
}

/// The rule winning each slot of `outcome` (labels aside).
fn winners(outcome: &campbooks_ai::decide::taught::Outcome) -> [Option<&str>; 6] {
    [
        outcome.stream.as_ref().map(|a| a.rule.as_str()),
        outcome.archive.as_deref(),
        outcome.read.as_deref(),
        outcome.needs_reply.as_ref().map(|a| a.rule.as_str()),
        outcome.asks_action.as_ref().map(|a| a.rule.as_str()),
        outcome.surface.as_ref().map(|a| a.rule.as_str()),
    ]
}

/// Count, per rule, the slots `new` takes over from it between `before`
/// and `after`.
fn count_overrides(
    before: &campbooks_ai::decide::taught::Outcome,
    after: &campbooks_ai::decide::taught::Outcome,
    new: &str,
    overridden: &mut Vec<(String, u32)>,
) {
    for (old, winner) in winners(before).into_iter().zip(winners(after)) {
        if let (Some(old), Some(winner)) = (old, winner)
            && winner == new
            && old != new
        {
            match overridden.iter_mut().find(|(id, _)| id == old) {
                Some((_, n)) => *n += 1,
                None => overridden.push((old.to_owned(), 1)),
            }
        }
    }
}

/// Whether `outcome`'s slots won by `rule` would change `candidate`.
fn would_change(
    candidate: &rules::Candidate,
    outcome: &campbooks_ai::decide::taught::Outcome,
    rule: &str,
) -> bool {
    let won = |r: &str| r == rule;
    outcome
        .stream
        .as_ref()
        .is_some_and(|a| won(&a.rule) && a.value != candidate.stream.as_str())
        || outcome.archive.as_deref().is_some_and(won) && !candidate.archived
        || outcome.read.as_deref().is_some_and(won) && candidate.unread
        || outcome
            .labels
            .iter()
            .any(|a| won(&a.rule) && !candidate.labels.contains(&a.value))
        || outcome.needs_reply.as_ref().is_some_and(|a| won(&a.rule))
        || outcome.asks_action.as_ref().is_some_and(|a| won(&a.rule))
        || outcome.surface.as_ref().is_some_and(|a| won(&a.rule))
}

// --- Create ------------------------------------------------------------------

/// Whether a new rule should act now or wait for a yes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NewRuleState {
    /// Act now (the member said yes).
    #[default]
    Active,
    /// Wait for a member to say yes.
    Proposed,
}

/// A rule to create.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRule {
    /// All must hold.
    pub conditions: Vec<RuleCondition>,
    /// What it does.
    pub actions: Vec<RuleAction>,
    /// Where it came from (default `action`).
    #[serde(default = "default_origin")]
    pub origin: RuleOrigin,
    /// The member's own words, for an instruction (at most 500 characters).
    #[serde(default)]
    pub origin_text: Option<String>,
    /// The conversation it was taught from.
    #[serde(default)]
    pub from_conversation_id: Option<ConversationId>,
    /// Act now or wait for a yes (default active). Rules from corrections
    /// or suggestions act now only when the member lets Campbooks act
    /// without asking and the inbox is theirs alone.
    #[serde(default)]
    pub state: NewRuleState,
    /// Apply it to the mail already in the inbox too (default true).
    #[serde(default = "yes")]
    pub apply_to_existing: bool,
    /// The teaching it belongs to, when one teaching makes rules in several
    /// inboxes.
    #[serde(default)]
    pub teaching_id: Option<TeachingId>,
}

const fn yes() -> bool {
    true
}

const fn default_origin() -> RuleOrigin {
    RuleOrigin::Action
}

/// A rule just created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreatedRule {
    /// The rule (with its run when it is applying to existing mail).
    pub rule: RuleView,
    /// Campbooks made it active on its own, from evidence: tell the member,
    /// with Undo.
    pub acted_without_asking: bool,
}

/// Create a taught rule in an inbox.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/inboxes/{inbox_id}/rules",
    operation_id = "conversations.create_rule",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.remove_rule")
        .automation()
        .assistant()
        .records(["conversations.rule", "conversations.conversation"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("inbox_id" = InboxId, Path, description = "The inbox."),
    ),
    request_body = CreateRule,
    responses(
        (status = CREATED, description = "The rule.", body = CreatedRule),
        (status = BAD_REQUEST, description = "Not a valid rule.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such inbox, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<InboxPath>,
    extract::Json(mut body): extract::Json<CreateRule>,
) -> Result<(StatusCode, Json<CreatedRule>)> {
    body.conditions = rules::normalise(&body.conditions);
    rules::validate(&body.conditions, &body.actions)?;
    if body
        .origin_text
        .as_deref()
        .is_some_and(|t| t.chars().count() > MAX_ORIGIN_TEXT_CHARS)
    {
        return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_rule")));
    }
    let access = &workspace.access;
    member_only(access)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    may_teach(&mut tx, access, path.inbox_id).await?;
    let created = create_in(&mut tx, &state, access, path.inbox_id, &body).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// Create `body` in `inbox` for `access` in `tx` (the caller checked they
/// may teach there).
pub async fn create_in(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    access: &Access,
    inbox: InboxId,
    body: &CreateRule,
) -> Result<CreatedRule> {
    let from_evidence = matches!(body.origin, RuleOrigin::Correction | RuleOrigin::Suggestion);
    let (rule_state, acted_without_asking) = if from_evidence {
        // Rules from evidence act for the member only when they let
        // Campbooks act without asking, and never for others.
        let personal = !rules::is_shared(tx, inbox).await?;
        let lets_act = match access.user {
            Some(user) if personal => {
                state
                    .preferences
                    .acts_without_asking(tx, access.workspace, user)
                    .await?
            }
            _ => false,
        };
        if lets_act {
            (RuleState::Active, true)
        } else {
            (RuleState::Proposed, false)
        }
    } else {
        match body.state {
            NewRuleState::Active => (RuleState::Active, false),
            NewRuleState::Proposed => (RuleState::Proposed, false),
        }
    };
    let ws = access.workspace.to_string();
    if let Some(from) = body.from_conversation_id {
        let in_inbox = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM conversations.conversations
                               WHERE id = $1 AND inbox_id = $2) AS "e!""#,
            from as ConversationId,
            inbox as InboxId,
        )
        .fetch_one(&mut **tx)
        .await?;
        if !in_inbox {
            return Err(Error::BadRequest(crate::MESSAGES.reason("invalid_rule")));
        }
    }
    if rule_state == RuleState::Active {
        replace(
            tx,
            state,
            access,
            inbox,
            (&body.conditions, &body.actions),
            None,
        )
        .await?;
    }
    if rules::templates_missing(tx, inbox).await? {
        crate::features::fill_templates::enqueue(tx, &ws, inbox).await?;
    }
    let id = rules::insert(
        tx,
        *access.workspace.as_uuid(),
        &NewRule {
            inbox,
            teaching: body.teaching_id.unwrap_or_default(),
            conditions: &body.conditions,
            actions: &body.actions,
            origin: body.origin,
            origin_text: body.origin_text.as_deref(),
            state: rule_state,
            taught_by: access.user,
            from_conversation: body.from_conversation_id,
        },
    )
    .await?;
    let rule = rules::find(tx, id).await?.ok_or(Error::NotFound)?;
    if rule_state == RuleState::Active && body.apply_to_existing {
        rule_runs::start(tx, &state.events, &ws, &rule, RunKind::Apply).await?;
    } else {
        rule_runs::announce(tx, &state.events, &ws, inbox, id, rule_state, None).await?;
    }
    Ok(CreatedRule {
        rule: view(tx, access, &rule).await?,
        acted_without_asking,
    })
}

/// Retire the active rules (other than `this`) that a rule with `parts`
/// (normalised conditions, and actions) replaces as it becomes active: the
/// same conditions and an action of the same kind. Their mail is not put
/// back. 403 `rule_not_yours` when the caller may not change one of them.
async fn replace(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    access: &Access,
    inbox: InboxId,
    parts: (&[RuleCondition], &[RuleAction]),
    this: Option<RuleId>,
) -> Result<()> {
    let (conditions, actions) = parts;
    let replaced = rules::replaced(tx, inbox, conditions, actions, this).await?;
    if replaced.is_empty() {
        return Ok(());
    }
    let inbox_role = rules::role_in(tx, inbox, access.user).await?;
    if replaced
        .iter()
        .any(|(_, author)| !rules::may_change(inbox_role, access.user, *author))
    {
        return Err(Error::Denied(crate::MESSAGES.reason("rule_not_yours")));
    }
    for (old, _) in replaced {
        retire(tx, state, access, inbox, old).await?;
    }
    Ok(())
}

/// Mark `rule` removed, stop its running runs and announce it.
async fn retire(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    access: &Access,
    inbox: InboxId,
    rule: RuleId,
) -> Result<()> {
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET state = 'removed', removed_at = now(), updated_at = now()
            WHERE id = $1"#,
        rule as RuleId,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"UPDATE conversations.rule_runs SET state = 'cancelled', finished_at = now()
            WHERE rule_id = $1 AND state = 'running'"#,
        rule as RuleId,
    )
    .execute(&mut **tx)
    .await?;
    rule_runs::announce(
        tx,
        &state.events,
        &access.workspace.to_string(),
        inbox,
        rule,
        RuleState::Removed,
        None,
    )
    .await
}

// --- Confirm, update, remove -------------------------------------------------

/// How to confirm a proposed rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct ConfirmRule {
    /// Apply it to the mail already in the inbox too (default true).
    #[serde(default = "yes")]
    pub apply_to_existing: bool,
}

/// Say yes to a proposed rule.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/rules/{rule_id}/confirm",
    operation_id = "conversations.confirm_rule",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.remove_rule")
        .assistant()
        .records(["conversations.rule", "conversations.conversation"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("rule_id" = RuleId, Path, description = "The rule."),
    ),
    request_body = ConfirmRule,
    responses(
        (status = OK, description = "The rule, now active.", body = RuleView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or you may only read this inbox.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such rule, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "It is not waiting for a yes.", body = Problem,
            content_type = "application/problem+json"),
    )
)]
pub async fn confirm(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<RulePath>,
    extract::Json(body): extract::Json<ConfirmRule>,
) -> Result<Json<RuleView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let rule = visible_rule(&mut tx, access, path.rule_id).await?;
    member_only(access)?;
    may_teach(&mut tx, access, rule.inbox).await?;
    if rule.state != RuleState::Proposed {
        return Err(Error::Conflict(crate::MESSAGES.reason("rule_not_proposed")));
    }
    replace(
        &mut tx,
        &state,
        access,
        rule.inbox,
        (&rule.conditions, &rule.actions),
        Some(rule.id),
    )
    .await?;
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET state = 'active', confirmed_at = now(), updated_at = now(),
                  taught_by = coalesce($2, taught_by)
            WHERE id = $1"#,
        rule.id as RuleId,
        access.user.map(|u| *u.as_uuid()),
    )
    .execute(&mut *tx)
    .await?;
    let rule = rules::find(&mut tx, rule.id)
        .await?
        .ok_or(Error::NotFound)?;
    let ws = access.workspace.to_string();
    if body.apply_to_existing {
        rule_runs::start(&mut tx, &state.events, &ws, &rule, RunKind::Apply).await?;
    } else {
        rule_runs::announce(
            &mut tx,
            &state.events,
            &ws,
            rule.inbox,
            rule.id,
            rule.state,
            None,
        )
        .await?;
    }
    let view = view(&mut tx, access, &rule).await?;
    tx.commit().await?;
    Ok(Json(view))
}

/// What to change about a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateRule {
    /// New conditions.
    #[serde(default)]
    pub conditions: Option<Vec<RuleCondition>>,
    /// New actions.
    #[serde(default)]
    pub actions: Option<Vec<RuleAction>>,
    /// `true` to pause it, `false` to let it act again.
    #[serde(default)]
    pub paused: Option<bool>,
    /// A manager takes over a rule whose author can no longer change the
    /// inbox, so it acts again with their rights.
    #[serde(default)]
    pub adopt: bool,
}

/// Change a rule's conditions or actions, pause it or let it act again.
#[utoipa::path(
    patch,
    path = "/api/v1/w/{workspace_id}/rules/{rule_id}",
    operation_id = "conversations.update_rule",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["conversations.rule"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("rule_id" = RuleId, Path, description = "The rule."),
    ),
    request_body = UpdateRule,
    responses(
        (status = OK, description = "The rule afterwards.", body = RuleView),
        (status = BAD_REQUEST, description = "Not a valid rule.", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not your rule.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such rule, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<RulePath>,
    extract::Json(body): extract::Json<UpdateRule>,
) -> Result<Json<RuleView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let rule = changeable_rule(&mut tx, access, path.rule_id).await?;
    if body.paused.is_some() && rule.state == RuleState::Proposed {
        // A proposed rule acts only once someone says yes (`confirm_rule`).
        return Err(Error::Conflict(crate::MESSAGES.reason("rule_not_active")));
    }
    let conditions = rules::normalise(&body.conditions.unwrap_or_else(|| rule.conditions.clone()));
    let actions = body.actions.unwrap_or_else(|| rule.actions.clone());
    rules::validate(&conditions, &actions)?;
    let next_state = match body.paused {
        Some(true) => RuleState::Paused,
        Some(false) if rule.state == RuleState::Paused => RuleState::Active,
        _ => rule.state,
    };
    if next_state == RuleState::Active {
        replace(
            &mut tx,
            &state,
            access,
            rule.inbox,
            (&conditions, &actions),
            Some(rule.id),
        )
        .await?;
    }
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET conditions = $2, actions = $3, specificity = $4, state = $5,
                  taught_by = CASE WHEN $6 THEN $7 ELSE taught_by END, updated_at = now()
            WHERE id = $1"#,
        rule.id as RuleId,
        serde_json::json!(conditions),
        serde_json::json!(actions),
        rules::specificity(&conditions),
        next_state.as_str(),
        body.adopt,
        access.user.map(|u| *u.as_uuid()),
    )
    .execute(&mut *tx)
    .await?;
    let rule = rules::find(&mut tx, rule.id)
        .await?
        .ok_or(Error::NotFound)?;
    rule_runs::announce(
        &mut tx,
        &state.events,
        &access.workspace.to_string(),
        rule.inbox,
        rule.id,
        rule.effective_state(),
        None,
    )
    .await?;
    let view = view(&mut tx, access, &rule).await?;
    tx.commit().await?;
    Ok(Json(view))
}

/// Whether to put back what a removed rule changed.
#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RemoveQuery {
    /// `true` to put back every conversation it changed that no member has
    /// changed since.
    #[serde(default)]
    pub restore: bool,
}

/// Remove a rule, optionally putting back what it changed.
#[utoipa::path(
    delete,
    path = "/api/v1/w/{workspace_id}/rules/{rule_id}",
    operation_id = "conversations.remove_rule",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .assistant()
        .records(["conversations.rule", "conversations.conversation"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("rule_id" = RuleId, Path, description = "The rule."),
        RemoveQuery,
    ),
    responses(
        (status = OK, description = "The rule, removed; with the run putting mail back when asked.",
            body = RuleView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, or not your rule.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such rule, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn remove(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<RulePath>,
    extract::Query(query): extract::Query<RemoveQuery>,
) -> Result<Json<RuleView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    // A proposed rule can be turned down by anyone who may teach there.
    let rule = match visible_rule(&mut tx, access, path.rule_id).await? {
        r if r.state == RuleState::Proposed => {
            member_only(access)?;
            may_teach(&mut tx, access, r.inbox).await?;
            r
        }
        _ => changeable_rule(&mut tx, access, path.rule_id).await?,
    };
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET state = 'removed', removed_at = now(), updated_at = now()
            WHERE id = $1"#,
        rule.id as RuleId,
    )
    .execute(&mut *tx)
    .await?;
    let mut removed = rule.clone();
    removed.state = RuleState::Removed;
    let ws = access.workspace.to_string();
    if query.restore {
        rule_runs::start(&mut tx, &state.events, &ws, &removed, RunKind::Revert).await?;
    } else {
        sqlx::query!(
            r#"UPDATE conversations.rule_runs SET state = 'cancelled', finished_at = now()
                WHERE rule_id = $1 AND state = 'running'"#,
            rule.id as RuleId,
        )
        .execute(&mut *tx)
        .await?;
        rule_runs::announce(
            &mut tx,
            &state.events,
            &ws,
            rule.inbox,
            rule.id,
            RuleState::Removed,
            None,
        )
        .await?;
    }
    let view = view(&mut tx, access, &removed).await?;
    tx.commit().await?;
    Ok(Json(view))
}

// --- Read --------------------------------------------------------------------

/// Which rules to list.
#[derive(Debug, Clone, Copy, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListRulesQuery {
    /// Only this inbox; every inbox you can read when absent.
    pub inbox: Option<InboxId>,
}

/// Rules, proposed ones first, then newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleList {
    /// The rules.
    pub rules: Vec<RuleView>,
}

/// List the taught rules of the inboxes you can read.
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/rules",
    operation_id = "conversations.list_rules",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.rule"])))),
    params(("workspace_id" = WorkspaceId, Path, description = "The workspace."), ListRulesQuery),
    responses(
        (status = OK, description = "The rules.", body = RuleList),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such inbox, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Query(query): extract::Query<ListRulesQuery>,
) -> Result<Json<RuleList>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let mut inboxes = visible_inboxes(&mut tx, access).await?;
    if let Some(inbox) = query.inbox {
        if !inboxes.contains(&inbox) {
            return Err(Error::NotFound);
        }
        inboxes = vec![inbox];
    }
    let stored = rules::of_inboxes(&mut tx, &inboxes).await?;
    let mut views = views(&mut tx, access, &stored).await?;
    views.sort_by_key(|v| v.state != RuleState::Proposed);
    tx.commit().await?;
    Ok(Json(RuleList { rules: views }))
}

/// Read one taught rule (a removed one too, with the run putting its mail
/// back).
#[utoipa::path(
    get,
    path = "/api/v1/w/{workspace_id}/rules/{rule_id}",
    operation_id = "conversations.get_rule",
    tag = "conversations",
    security(("bearer" = ["conversations:read"])),
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.rule"])))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("rule_id" = RuleId, Path, description = "The rule."),
    ),
    responses(
        (status = OK, description = "The rule, removed or not.", body = RuleView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope.", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such rule, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsRead>,
    extract::Path(path): extract::Path<RulePath>,
) -> Result<Json<RuleView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    // A removed rule stays readable, so the run putting its mail back can be
    // followed.
    let rule = rules::find_any(&mut tx, path.rule_id)
        .await?
        .ok_or(Error::NotFound)?;
    if !visible_inboxes(&mut tx, access)
        .await?
        .contains(&rule.inbox)
    {
        return Err(Error::NotFound);
    }
    let view = view(&mut tx, access, &rule).await?;
    tx.commit().await?;
    Ok(Json(view))
}
