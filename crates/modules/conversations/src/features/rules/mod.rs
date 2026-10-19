//! Taught rules: preview, create, confirm, change, remove and list them
//! (campbooks-cloud#125).
//!
//! Every producer of rules (a member's action, an onboarding answer, the
//! member's own words, corrections) goes through these operations, so a rule
//! is checked, stored and applied one way whatever taught it.

#[cfg(test)]
mod binding_tests;
#[cfg(test)]
pub(crate) mod tests;

use axum::{Json, extract::State, http::StatusCode};
use campbooks_kernel::{
    AppState, Error, Operation, Problem, Result, Risk,
    auth::{Principal, PrincipalKind, ScopeName},
    authz::Action,
    binding::Binding,
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
        resources,
        rule_runs::{self, RunView, SCAN},
        rules::{self, NewRule, RuleAction, RuleCondition, Stored, binding as rule_binding},
    },
    events::RULE_RECORD,
    features::rule_notices,
    model::{
        ConversationId, InboxId, PausedReason, RuleId, RuleNoticeId, RuleOrigin, RuleState,
        RunKind, TeachingId,
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
    /// Whether it acts. `paused` when its author can no longer change the
    /// inbox or is away for now, its inbox is kept read-only, or a member
    /// paused it.
    pub state: RuleState,
    /// Why it is paused; absent unless `state` is `paused`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_reason: Option<PausedReason>,
    /// For a proposed rule, why it would be paused once kept (the member
    /// who says yes becomes its author): say so before a member says yes.
    /// Absent for any other rule, or when it would act.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_once_kept: Option<PausedReason>,
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
    /// Whether it waits for a member to confirm it again before it acts.
    #[serde(default)]
    pub needs_confirmation: bool,
    /// What you were shown, bound to you (campbooks-cloud#242): send it
    /// back to say yes to it (`conversations.confirm_rule`) or to adopt it.
    /// Absent when the server has no instance secret.
    #[serde(default)]
    pub content_token: Option<String>,
}

async fn view(
    conn: &mut PgConnection,
    binding: &Binding,
    access: &Access,
    rule: &Stored,
) -> Result<RuleView> {
    let mut views = views(conn, binding, access, std::slice::from_ref(rule)).await?;
    views.pop().ok_or(Error::NotFound)
}

/// `stored` rules as the caller sees them, with a few queries for all of them.
async fn views(
    conn: &mut PgConnection,
    binding: &Binding,
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
            paused_reason: rule.paused_reason(),
            paused_once_kept: rule.paused_once_kept(),
            taught_by: rule.taught_by,
            from_conversation_id: rule.from_conversation,
            created_at: rule.created_at,
            confirmed_at: rule.confirmed_at,
            applied_count: u32::try_from(rule.applied_count).unwrap_or_default(),
            last_applied_at: rule.last_applied_at,
            recent_exceptions: exceptions.get(&rule.id).copied().unwrap_or_default(),
            run: runs.remove(&rule.id),
            shared: shared.contains(&rule.inbox),
            // Nobody changes a rule of an inbox kept read-only.
            may_change: !rule.inbox_detached
                && rules::may_change(
                    inbox_roles.get(&rule.inbox).copied(),
                    access.user,
                    rule.taught_by,
                ),
            // Kept, but not bound to the member who taught it
            // (campbooks-cloud#242): it acts again once confirmed.
            needs_confirmation: rule.needs_confirmation(binding),
            // What this answer shows, for a yes to sign only that.
            content_token: rule_binding::content_token(access, rule)
                .ok()
                .and_then(|token| token.issue(binding)),
        })
        .collect())
}

/// 404 unless the caller sees `inbox`; 403 `inbox_read_only` unless they may
/// write in it (its owner, a sender or a manager) and it is not kept
/// read-only: the catalogue's `write` on the inbox.
pub(crate) async fn may_teach(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    inbox: InboxId,
) -> Result<()> {
    resources::authorize(state, conn, access, Action::Write, &resources::inbox(inbox)).await
}

/// 403 `rule_needs_member` for a client token: rules are taught by members.
/// A client a member registered arrives as that member, so the principal's
/// kind decides, not whether a user is attached.
pub(crate) fn member_only(principal: &Principal) -> Result<()> {
    if principal.kind == PrincipalKind::Client {
        return Err(Error::Denied(crate::MESSAGES.reason("rule_needs_member")));
    }
    Ok(())
}

/// 403 `rule_needs_your_yes` unless the request is the member's own
/// (campbooks-cloud#242). The assistant, or an automation rule, acting for
/// a member may propose a rule, pause one or take one away, but never say
/// yes to one, adopt it, let it act again or change what a kept rule does:
/// each of those signs a rule as the member's, and the member may never
/// have seen it (mail is untrusted input to the assistant, so a message
/// could ask for it). Only the kernel says who invoked a request
/// ([`Workspace::invoked_by`]), from a request extension no client can
/// send. Checked after the access checks, before anything changes.
fn members_own_yes<S: ScopeName>(workspace: &Workspace<S>, rule: Option<RuleId>) -> Result<()> {
    let Some(caller) = workspace.invoked_by else {
        return Ok(());
    };
    tracing::warn!(
        workspace = %workspace.id,
        rule = %rule.map_or_else(|| "none".to_owned(), |id| id.to_string()),
        caller = %caller,
        "a rule was not signed: only a member's own request says yes to one"
    );
    Err(Error::Denied(crate::MESSAGES.reason("rule_needs_your_yes")))
}

/// 403 `images_need_you` when a request run in-process for the member (the
/// assistant, or an automation rule) would save, reshape or bring back a
/// rule that shows a sender's images (campbooks-cloud#242). Showing images
/// tells a sender their mail was opened, and mail is untrusted input to the
/// assistant: only the member's own "Always show images" trusts a sender.
/// A proposal filed for them would also hold back the member's own rule for
/// that sender (`images_already_trusted`) until they declined it. Checked
/// after the access checks, before anything changes.
fn members_own_images<S: ScopeName>(
    workspace: &Workspace<S>,
    rule: Option<RuleId>,
    actions: &[&[RuleAction]],
) -> Result<()> {
    let Some(caller) = workspace.invoked_by else {
        return Ok(());
    };
    if !actions.iter().any(|a| a.contains(&RuleAction::ShowImages)) {
        return Ok(());
    }
    tracing::warn!(
        workspace = %workspace.id,
        rule = %rule.map_or_else(|| "none".to_owned(), |id| id.to_string()),
        caller = %caller,
        "a rule showing a sender's images was refused: only a member's own request trusts one"
    );
    Err(Error::Denied(crate::MESSAGES.reason("images_need_you")))
}

/// Rule `id` (not removed) if the caller may read it (any role in its
/// inbox), else 404.
async fn visible_rule(
    state: &AppState,
    conn: &mut PgConnection,
    access: &Access,
    id: RuleId,
) -> Result<Stored> {
    resources::authorize(state, conn, access, Action::Read, &resources::rule(id)).await?;
    rules::find(conn, id).await?.ok_or(Error::NotFound)
}

/// Rule `id` if the caller may change it: 404 when they cannot see it, 403
/// `rule_needs_member` for a client, `inbox_read_only` when they may not
/// write in its inbox (or it is kept read-only), `rule_not_yours` when they
/// are neither its author nor a manager.
async fn changeable_rule(
    state: &AppState,
    conn: &mut PgConnection,
    workspace: (&Principal, &Access),
    id: RuleId,
) -> Result<Stored> {
    let (principal, access) = workspace;
    let rule = visible_rule(state, conn, access, id).await?;
    may_teach(state, conn, access, rule.inbox).await?;
    member_only(principal)?;
    let inbox_role = rules::role_in(conn, rule.inbox, access.user).await?;
    if !rules::may_change(inbox_role, access.user, rule.taught_by) {
        return Err(Error::Denied(crate::MESSAGES.reason("rule_not_yours")));
    }
    Ok(rule)
}

/// 409 `content_changed` unless `token` is the one a response gave the
/// caller with `rule` as it is now, under its lock (campbooks-cloud#242): a
/// yes signs only the conditions, actions, inbox, state and author the
/// member was shown. Checked after every access check, before anything
/// changes; without an instance secret nothing can be signed (500).
fn shown_to_caller(
    state: &AppState,
    access: &Access,
    rule: &Stored,
    token: Option<&str>,
) -> Result<()> {
    if !state.binding.is_enabled() {
        tracing::warn!(
            workspace = %access.workspace,
            rule = %rule.id,
            "a taught rule was refused: there is no key to bind it with \
             (CAMPBOOKS_SECRET_KEY is unset)"
        );
        return Err(Error::internal("rules are not bound: no instance secret"));
    }
    if rule_binding::content_token(access, rule)?.holds(&state.binding, token) {
        return Ok(());
    }
    tracing::warn!(
        workspace = %access.workspace,
        rule = %rule.id,
        why = if token.is_some() { "mismatched" } else { "missing" },
        "a rule was not kept: it is not what the member was shown"
    );
    Err(Error::Conflict(crate::MESSAGES.reason("content_changed")))
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
    extensions(("x-campbooks" = json!(Operation::body_query().assistant().records(["conversations.rule"]).access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
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
    may_teach(&state, &mut tx, access, path.inbox_id).await?;
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
    // "Worth a look" is raised for the newest unread match only: its
    // candidate, and whether another change counted it already.
    let mut look: Option<(rules::Candidate, bool)> = None;
    loop {
        let window = rules::scan(conn, inbox, conditions, after, SCAN).await?;
        looked += window.examined;
        for candidate in &window.candidates {
            let facts = candidate.facts();
            if !new.matches(&facts) {
                continue;
            }
            out.matching += 1;
            // A rule for the trash or spam is about such mail: it acts on it.
            if candidate.suspicious && rules::hides(actions) && !rules::bins(actions) {
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
            let changes = would_change(candidate, &after_outcome, NEW);
            if changes {
                out.would_change += 1;
                // Suspicious mail is counted, never shown as an example.
                if !candidate.suspicious {
                    found.push(candidate.clone());
                }
            }
            if after_outcome.highlight.as_deref() == Some(NEW)
                && candidate.unread
                && look
                    .as_ref()
                    .is_none_or(|(c, _)| candidate.sent_at > c.sent_at)
            {
                look = Some((candidate.clone(), changes));
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
    // The newest unread one raises it; the rest do not change for it.
    if let Some((candidate, false)) = look {
        out.would_change += 1;
        found.push(candidate);
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
fn winners(outcome: &campbooks_ai::decide::taught::Outcome) -> [Option<&str>; 9] {
    [
        outcome.stream.as_ref().map(|a| a.rule.as_str()),
        outcome.archive.as_deref(),
        outcome.read.as_deref(),
        outcome.needs_reply.as_ref().map(|a| a.rule.as_str()),
        outcome.asks_action.as_ref().map(|a| a.rule.as_str()),
        outcome.surface.as_ref().map(|a| a.rule.as_str()),
        outcome.bin.as_ref().map(|a| a.rule.as_str()),
        outcome.paid.as_deref(),
        outcome.highlight.as_deref(),
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
        // The scan leaves out mail already in the trash or spam.
        || outcome.bin.as_ref().is_some_and(|a| won(&a.rule))
        || outcome.paid.as_deref().is_some_and(won)
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
    /// The suggestion it answers ("Scout noticed"), which the member saw as
    /// a smart rule and confirmed: the rule acts at once (origin
    /// `suggestion`), and the suggestion is accepted.
    #[serde(default)]
    pub notice_id: Option<RuleNoticeId>,
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
        .records(["conversations.rule", "conversations.conversation"]).access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
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
        (status = FORBIDDEN, description = "Missing scope, you may only read this inbox, a suggestion saved by the assistant rather than by you (`rule_needs_your_yes`), or a rule showing a sender's images saved other than in your own request (`images_need_you`).",
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
    let mut tx = state.db.begin_for(access.workspace).await?;
    may_teach(&state, &mut tx, access, path.inbox_id).await?;
    member_only(&workspace.principal)?;
    members_own_images(&workspace, None, &[body.actions.as_slice()])?;
    // A suggestion saved as a smart rule acts at once: the member's own yes.
    if body.notice_id.is_some() {
        members_own_yes(&workspace, None)?;
    }
    let teacher = if workspace.on_own_request() {
        Teacher::Member
    } else {
        Teacher::OnTheirBehalf
    };
    let created = create_in(&mut tx, &state, access, path.inbox_id, &body, teacher).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// Who is creating a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Teacher {
    /// The member, in their own request: it starts as they ask.
    Member,
    /// The assistant or an automation rule acting for the member
    /// (campbooks-cloud#242): it is only ever proposed, and acts once the
    /// member says yes to it.
    OnTheirBehalf,
}

/// The state a new rule starts in, and whether it acts without asking.
async fn starting_state(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    access: &Access,
    (inbox, teacher): (InboxId, Teacher),
    body: &CreateRule,
    origin: RuleOrigin,
) -> Result<(RuleState, bool)> {
    if teacher == Teacher::OnTheirBehalf {
        return Ok((RuleState::Proposed, false));
    }
    let from_evidence = body.notice_id.is_none()
        && matches!(origin, RuleOrigin::Correction | RuleOrigin::Suggestion);
    Ok(if from_evidence {
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
    })
}

/// Create `body` in `inbox` for `access` in `tx` (the caller checked they
/// may teach there), taught by `teacher`.
pub async fn create_in(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    access: &Access,
    inbox: InboxId,
    body: &CreateRule,
    teacher: Teacher,
) -> Result<CreatedRule> {
    rules::labels_of_inbox(tx, inbox, &body.conditions, &body.actions).await?;
    rules::sender_seen(tx, inbox, &body.conditions, &body.actions).await?;
    rules::not_trusted_yet(tx, inbox, &body.conditions, &body.actions, None).await?;
    if let Some(notice) = body.notice_id {
        rule_notices::check_open(tx, notice, inbox).await?;
    }
    // A suggestion the member confirmed as a smart rule is theirs: it acts.
    let origin = if body.notice_id.is_some() {
        RuleOrigin::Suggestion
    } else {
        body.origin
    };
    let (rule_state, acted_without_asking) =
        starting_state(tx, state, access, (inbox, teacher), body, origin).await?;
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
            origin,
            origin_text: body.origin_text.as_deref(),
            state: rule_state,
            taught_by: access.user,
            from_conversation: body.from_conversation_id,
        },
    )
    .await?;
    // The member's own request, the row it just wrote: bound to them
    // (campbooks-cloud#242). A proposal is signed when someone says yes.
    rule_binding::sign(tx, &state.binding, id).await?;
    let rule = rules::find(tx, id).await?.ok_or(Error::NotFound)?;
    if let Some(notice) = body.notice_id {
        rule_notices::accept(tx, notice, id).await?;
    }
    // Showing images changes no mail: nothing to apply to what is here.
    if rule_state == RuleState::Active
        && body.apply_to_existing
        && !rules::only_shows_images(&body.actions)
    {
        rule_runs::start(tx, state, access, &rule, RunKind::Apply).await?;
    } else {
        rule_runs::announce(
            tx,
            &state.events,
            &ws,
            inbox,
            id,
            rule.effective_state(),
            None,
        )
        .await?;
    }
    Ok(CreatedRule {
        rule: view(tx, &state.binding, access, &rule).await?,
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
    rule_runs::lock_running(tx, rule).await?;
    // Locked by `rules::replaced`: still bound as it is, it stays bound once
    // removed, so Undo can bring it back acting.
    let bound = rules::find_any(tx, rule)
        .await?
        .is_some_and(|r| r.bound(&state.binding));
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET state = 'removed', removed_from = state, removed_at = now(),
                  updated_at = now()
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
    if bound {
        rule_binding::sign(tx, &state.binding, rule).await?;
    }
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
    /// The `content_token` that came with the rule on screen. Missing,
    /// another member's, or not the rule as it is now: 409
    /// `content_changed`, and nothing is kept (campbooks-cloud#242).
    #[serde(default)]
    pub content_token: Option<String>,
}

/// Say yes to a proposed rule.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/rules/{rule_id}/confirm",
    operation_id = "conversations.confirm_rule",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    // Never the assistant's (campbooks-cloud#242): only a member's own
    // request says yes to a rule.
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.remove_rule")
        .records(["conversations.rule", "conversations.conversation"]).access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("rule_id" = RuleId, Path, description = "The rule."),
    ),
    request_body = ConfirmRule,
    responses(
        (status = OK, description = "The rule, now active.", body = RuleView),
        (status = BAD_REQUEST, description = "Not a valid rule (any more).", body = Problem,
            content_type = "application/problem+json"),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, you may only read this inbox, or not your own request (`rule_needs_your_yes`): only you say yes to a rule.",
            body = Problem, content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such rule, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "It is not waiting for a yes (`rule_not_proposed`), or it is not what you were shown (`content_changed`): read it again, check it, and say yes again.",
            body = Problem, content_type = "application/problem+json"),
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
    let rule = visible_rule(&state, &mut tx, access, path.rule_id).await?;
    may_teach(&state, &mut tx, access, rule.inbox).await?;
    member_only(&workspace.principal)?;
    members_own_yes(&workspace, Some(rule.id))?;
    let user = access
        .user
        .ok_or_else(|| Error::Denied(crate::MESSAGES.reason("rule_needs_member")))?;
    // Runs, then the rule row, then the check, so it holds to the end: a
    // decline or another confirm in between finds it no longer proposed.
    rule_runs::lock_running(&mut tx, rule.id).await?;
    let locked = rules::lock_state(&mut tx, rule.id).await?;
    // What the member says yes to: the rule as it is under the lock.
    let rule = rules::find_any(&mut tx, rule.id)
        .await?
        .ok_or(Error::NotFound)?;
    match locked {
        Some(RuleState::Proposed) => {
            shown_to_caller(&state, access, &rule, body.content_token.as_deref())?;
        }
        // A kept rule that is not bound to the member who taught it
        // (campbooks-cloud#242): confirming it again binds it as it is,
        // in its state, with its author.
        Some(RuleState::Active | RuleState::Paused) if rule.needs_confirmation(&state.binding) => {
            shown_to_caller(&state, access, &rule, body.content_token.as_deref())?;
            return confirm_again(&state, tx, access, &rule, &body).await;
        }
        _ => return Err(Error::Conflict(crate::MESSAGES.reason("rule_not_proposed"))),
    }
    // As `create` does: a proposal stored before its domain became a public
    // suffix, or a free mail or shared platform's (for images), is not kept
    // (it could never act), nor a second rule trusting the same sender.
    rules::may_keep_again(&mut tx, &rule).await?;
    replace(
        &mut tx,
        &state,
        access,
        rule.inbox,
        (&rule.conditions, &rule.actions),
        Some(rule.id),
    )
    .await?;
    // The member who says yes is its author from now on, as for a yes again
    // (campbooks-cloud#242): the stored author is SQL's to choose, and was
    // never on screen, so the rule acts with the rights of the member whose
    // click signs it.
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET state = 'active', confirmed_at = now(), updated_at = now(),
                  taught_by = $2
            WHERE id = $1"#,
        rule.id as RuleId,
        user.as_uuid(),
    )
    .execute(&mut *tx)
    .await?;
    // A member's yes, in their request: the rule is bound as they kept it.
    rule_binding::sign(&mut tx, &state.binding, rule.id).await?;
    let rule = rules::find(&mut tx, rule.id)
        .await?
        .ok_or(Error::NotFound)?;
    let ws = access.workspace.to_string();
    if body.apply_to_existing && !rules::only_shows_images(&rule.actions) {
        rule_runs::start(&mut tx, &state, access, &rule, RunKind::Apply).await?;
    } else {
        rule_runs::announce(
            &mut tx,
            &state.events,
            &ws,
            rule.inbox,
            rule.id,
            rule.effective_state(),
            None,
        )
        .await?;
    }
    let view = view(&mut tx, &state.binding, access, &rule).await?;
    tx.commit().await?;
    Ok(Json(view))
}

/// Confirm again a kept rule that is not bound (campbooks-cloud#242): sign
/// it as it is (its conditions, actions and state, which the member sees in
/// the list), now taught by the member who confirms it: its stored author
/// is SQL's to choose (or none, which no author check would stop), so it
/// acts with the confirming member's rights from now on. Apply it to the
/// mail already here when asked and it is active. Anyone who may teach in
/// its inbox may, as for a proposal.
async fn confirm_again(
    state: &AppState,
    mut tx: Transaction<'static, Postgres>,
    access: &Access,
    rule: &Stored,
    body: &ConfirmRule,
) -> Result<Json<RuleView>> {
    rules::may_keep_again(&mut tx, rule).await?;
    let user = access
        .user
        .ok_or_else(|| Error::Denied(crate::MESSAGES.reason("rule_needs_member")))?;
    sqlx::query("UPDATE conversations.rules SET taught_by = $2, updated_at = now() WHERE id = $1")
        .bind(rule.id.as_uuid())
        .bind(user.as_uuid())
        .execute(&mut *tx)
        .await?;
    rule_binding::sign(&mut tx, &state.binding, rule.id).await?;
    let rule = rules::find(&mut tx, rule.id)
        .await?
        .ok_or(Error::NotFound)?;
    let ws = access.workspace.to_string();
    if body.apply_to_existing
        && rule.effective_state() == RuleState::Active
        && !rules::only_shows_images(&rule.actions)
    {
        rule_runs::start(&mut tx, state, access, &rule, RunKind::Apply).await?;
    } else {
        rule_runs::announce(
            &mut tx,
            &state.events,
            &ws,
            rule.inbox,
            rule.id,
            rule.effective_state(),
            None,
        )
        .await?;
    }
    let view = view(&mut tx, &state.binding, access, &rule).await?;
    tx.commit().await?;
    Ok(Json(view))
}

/// Turn down a proposed rule ("No"): like confirming it, anyone who may
/// teach in its inbox may, since it does not act yet. Undo with
/// `conversations.restore_rule`.
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/rules/{rule_id}/decline",
    operation_id = "conversations.decline_rule",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.restore_rule")
        .assistant()
        .records(["conversations.rule"])
        .access("conversations.inbox", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("rule_id" = RuleId, Path, description = "The rule."),
    ),
    responses(
        (status = OK, description = "The rule, turned down (removed).", body = RuleView),
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
pub async fn decline(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<RulePath>,
) -> Result<Json<RuleView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let rule = visible_rule(&state, &mut tx, access, path.rule_id).await?;
    may_teach(&state, &mut tx, access, rule.inbox).await?;
    member_only(&workspace.principal)?;
    // Checked under the rule's lock (after its runs'), as confirm does.
    rule_runs::lock_running(&mut tx, rule.id).await?;
    if rules::lock_state(&mut tx, rule.id).await? != Some(RuleState::Proposed) {
        return Err(Error::Conflict(crate::MESSAGES.reason("rule_not_proposed")));
    }
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET state = 'removed', removed_from = state, removed_at = now(), updated_at = now()
            WHERE id = $1"#,
        rule.id as RuleId,
    )
    .execute(&mut *tx)
    .await?;
    rule_runs::announce(
        &mut tx,
        &state.events,
        &access.workspace.to_string(),
        rule.inbox,
        rule.id,
        RuleState::Removed,
        None,
    )
    .await?;
    let mut declined = rule;
    declined.state = RuleState::Removed;
    let view = view(&mut tx, &state.binding, access, &declined).await?;
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
    /// With `adopt`, unless this change sets both the conditions and the
    /// actions: the `content_token` that came with the rule on screen, so
    /// you adopt only what you were shown (409 `content_changed`
    /// otherwise).
    #[serde(default)]
    pub content_token: Option<String>,
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
        .records(["conversations.rule"]).access("conversations.rule", campbooks_kernel::authz::Action::Write)))),
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
        (status = FORBIDDEN, description = "Missing scope, not your rule, adopting, resuming or reshaping a kept rule other than in your own request (`rule_needs_your_yes`), or reshaping a rule that shows a sender's images other than in your own request (`images_need_you`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such rule, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "It needs a yes again first (`rule_needs_confirmation`), or you adopt one that is not what you were shown (`content_changed`).",
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
    let rule = changeable_rule(
        &state,
        &mut tx,
        (&workspace.principal, access),
        path.rule_id,
    )
    .await?;
    // Runs, then the rule row (the order every path keeps), then the rule as
    // it is under the lock: what it keeps of it is what is checked below.
    rule_runs::lock_running(&mut tx, rule.id).await?;
    rules::lock_state(&mut tx, rule.id).await?;
    let rule = rules::find(&mut tx, rule.id)
        .await?
        .ok_or(Error::NotFound)?;
    // Adopting, letting it act again and changing what a kept rule does all
    // sign it as the member's: their own request only. Pausing it, or
    // reshaping a proposal (which still waits for a yes), signs nothing new.
    let reshaped = body.conditions.is_some() || body.actions.is_some();
    if body.adopt || body.paused == Some(false) || (reshaped && rule.state != RuleState::Proposed) {
        members_own_yes(&workspace, Some(rule.id))?;
    }
    // Nor may it reshape a rule into, or out of, trusting a sender's images.
    if reshaped {
        let new = body.actions.as_deref().unwrap_or_default();
        members_own_images(&workspace, Some(rule.id), &[rule.actions.as_slice(), new])?;
    }
    if body.paused.is_some() && rule.state == RuleState::Proposed {
        // A proposed rule acts only once someone says yes (`confirm_rule`).
        return Err(Error::Conflict(crate::MESSAGES.reason("rule_not_active")));
    }
    // A kept rule keeps fields this request does not set (its author, the
    // conditions or actions it leaves alone): it is signed again only when
    // it is bound as it is (campbooks-cloud#242). One that is not may only be
    // paused; anything else waits until a member confirms it again.
    let bound = rule.bound(&state.binding);
    let only_pauses = body.paused == Some(true) && !reshaped && !body.adopt;
    if rule.state != RuleState::Proposed && !bound && !only_pauses {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("rule_needs_confirmation"),
        ));
    }
    // Adopting keeps what the rule holds and makes it act with this
    // member's rights: only what they were shown (campbooks-cloud#242). A
    // change that sets the whole rule signs what it sent.
    let whole = body.conditions.is_some() && body.actions.is_some();
    if body.adopt && !whole {
        shown_to_caller(&state, access, &rule, body.content_token.as_deref())?;
    }
    let conditions = rules::normalise(&body.conditions.unwrap_or_else(|| rule.conditions.clone()));
    let actions = body.actions.unwrap_or_else(|| rule.actions.clone());
    rules::validate(&conditions, &actions)?;
    rules::labels_of_inbox(&mut tx, rule.inbox, &conditions, &actions).await?;
    // Pausing or resuming changes no sender: only a new shape is checked.
    if reshaped {
        rules::sender_seen(&mut tx, rule.inbox, &conditions, &actions).await?;
        rules::not_trusted_yet(&mut tx, rule.inbox, &conditions, &actions, Some(rule.id)).await?;
    }
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
    if bound {
        rule_binding::sign(&mut tx, &state.binding, rule.id).await?;
    }
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
    let view = view(&mut tx, &state.binding, access, &rule).await?;
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
        .records(["conversations.rule", "conversations.conversation"]).access("conversations.rule", campbooks_kernel::authz::Action::Delete)))),
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
        (status = CONFLICT, description = "Some of what it changed can't be put back: remove it keeping its changes.",
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
    // Removed (or, proposed, turned down) by its author or a manager: the
    // catalogue's `delete` on the rule, in a kept read-only inbox too.
    let rule = visible_rule(&state, &mut tx, access, path.rule_id).await?;
    campbooks_kernel::authz::require(
        &state,
        &mut tx,
        access,
        Action::Delete,
        &resources::rule(rule.id),
        || Error::Denied(crate::MESSAGES.reason("rule_not_yours")),
    )
    .await?;
    member_only(&workspace.principal)?;
    rule_runs::lock_running(&mut tx, rule.id).await?;
    // Removed meanwhile (another Undo): nothing left to remove.
    if matches!(
        rules::lock_state(&mut tx, rule.id).await?,
        None | Some(RuleState::Removed)
    ) {
        return Err(Error::NotFound);
    }
    // Nothing is put back in an inbox kept read-only.
    // A rule that only showed images changed no mail: nothing to put back.
    let put_back =
        query.restore && !rule.inbox_detached && !rules::only_shows_images(&rule.actions);
    // What it changed before applications were bound (campbooks-cloud#242)
    // cannot be put back: refused before anything changes.
    if put_back && rules::unsigned_held(&mut tx, rule.id).await? > 0 {
        return Err(Error::Conflict(crate::MESSAGES.reason("undo_unavailable")));
    }
    // Bound as it is (under the lock), it stays bound once removed, so Undo
    // brings it back acting (campbooks-cloud#242).
    let bound = rules::find_any(&mut tx, rule.id)
        .await?
        .is_some_and(|r| r.bound(&state.binding));
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET state = 'removed', removed_from = state, removed_at = now(), updated_at = now()
            WHERE id = $1"#,
        rule.id as RuleId,
    )
    .execute(&mut *tx)
    .await?;
    if bound {
        rule_binding::sign(&mut tx, &state.binding, rule.id).await?;
    }
    let mut removed = rule.clone();
    removed.state = RuleState::Removed;
    // Undone right after it was saved from a suggestion: offered again.
    if query.restore {
        rule_notices::reopen(&mut tx, rule.id, RESTORE_MINUTES).await?;
    }
    let ws = access.workspace.to_string();
    if put_back {
        rule_runs::start(&mut tx, &state, access, &removed, RunKind::Revert).await?;
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
    let view = view(&mut tx, &state.binding, access, &removed).await?;
    tx.commit().await?;
    Ok(Json(view))
}

// --- Restore -----------------------------------------------------------------

/// How long after a removal Undo may bring a rule back, in minutes.
pub const RESTORE_MINUTES: i32 = 15;

/// How to bring a removed rule back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct RestoreRule {
    /// Apply it again to the mail already in the inbox (after its mail was
    /// put back). Default false.
    #[serde(default)]
    pub apply_to_existing: bool,
}

/// Bring back a rule removed in the last 15 minutes, as it was (Undo).
#[utoipa::path(
    post,
    path = "/api/v1/w/{workspace_id}/rules/{rule_id}/restore",
    operation_id = "conversations.restore_rule",
    tag = "conversations",
    security(("bearer" = ["conversations:write"])),
    extensions(("x-campbooks" = json!(Operation::command(Risk::Low)
        .undo("conversations.remove_rule")
        .assistant()
        .records(["conversations.rule", "conversations.conversation"])
        .access("conversations.rule", campbooks_kernel::authz::Action::Write)))),
    params(
        ("workspace_id" = WorkspaceId, Path, description = "The workspace."),
        ("rule_id" = RuleId, Path, description = "The rule."),
    ),
    request_body = RestoreRule,
    responses(
        (status = OK, description = "The rule, back as it was.", body = RuleView),
        (status = UNAUTHORIZED, description = "No valid access token.", body = Problem,
            content_type = "application/problem+json"),
        (status = FORBIDDEN, description = "Missing scope, not your rule, bringing a kept rule back other than in your own request (`rule_needs_your_yes`), or bringing back a rule that shows a sender's images other than in your own request (`images_need_you`).", body = Problem,
            content_type = "application/problem+json"),
        (status = NOT_FOUND, description = "No such rule, or you cannot see it.",
            body = Problem, content_type = "application/problem+json"),
        (status = CONFLICT, description = "It is not removed, or was removed too long ago.",
            body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn restore(
    State(state): State<AppState>,
    workspace: Workspace<ConversationsWrite>,
    extract::Path(path): extract::Path<RulePath>,
    extract::Json(body): extract::Json<RestoreRule>,
) -> Result<Json<RuleView>> {
    let access = &workspace.access;
    let mut tx = state.db.begin_for(access.workspace).await?;
    resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &resources::rule(path.rule_id),
    )
    .await?;
    // Runs before the rule row, as every path takes them.
    rule_runs::lock_running(&mut tx, path.rule_id).await?;
    let found = sqlx::query!(
        r#"SELECT removed_from,
                  (state = 'removed' AND removed_at > now() - make_interval(mins => $2))
                      AS "restorable!"
             FROM conversations.rules WHERE id = $1 FOR UPDATE"#,
        path.rule_id as RuleId,
        RESTORE_MINUTES,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(Error::NotFound)?;
    let back: RuleState = found.removed_from.as_deref().unwrap_or("active").parse()?;
    let rule = rules::find_any(&mut tx, path.rule_id)
        .await?
        .ok_or(Error::NotFound)?;
    // Who may remove it may bring it back.
    may_teach(&state, &mut tx, access, rule.inbox).await?;
    member_only(&workspace.principal)?;
    if back != RuleState::Proposed {
        let inbox_role = rules::role_in(&mut tx, rule.inbox, access.user).await?;
        if !rules::may_change(inbox_role, access.user, rule.taught_by) {
            return Err(Error::Denied(crate::MESSAGES.reason("rule_not_yours")));
        }
        // Back as kept, it is signed and acts again: the member's own request.
        members_own_yes(&workspace, Some(rule.id))?;
    }
    members_own_images(&workspace, Some(rule.id), &[rule.actions.as_slice()])?;
    if !found.restorable {
        return Err(Error::Conflict(
            crate::MESSAGES.reason("rule_not_restorable"),
        ));
    }
    // Brought back as if taught again: never a rule `create` would refuse
    // (a domain that joined a list since, campbooks-cloud#235), nor a second
    // rule trusting a sender another rule trusts by now.
    rules::may_keep_again(&mut tx, &rule).await?;
    // The run putting its mail back stops where it is.
    sqlx::query!(
        r#"UPDATE conversations.rule_runs SET state = 'cancelled', finished_at = now()
            WHERE rule_id = $1 AND state = 'running'"#,
        rule.id as RuleId,
    )
    .execute(&mut *tx)
    .await?;
    if back == RuleState::Active {
        replace(
            &mut tx,
            &state,
            access,
            rule.inbox,
            (&rule.conditions, &rule.actions),
            Some(rule.id),
        )
        .await?;
    }
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET state = $2, removed_at = NULL, removed_from = NULL, updated_at = now()
            WHERE id = $1"#,
        rule.id as RuleId,
        back.as_str(),
    )
    .execute(&mut *tx)
    .await?;
    // Bound as it was removed (read under the lock): bound again as it comes
    // back. One that was not stays waiting for a member to confirm it, and a
    // proposal waits for its yes (campbooks-cloud#242).
    if back != RuleState::Proposed && rule.bound(&state.binding) {
        rule_binding::sign(&mut tx, &state.binding, rule.id).await?;
    }
    rule_notices::reaccept(&mut tx, rule.id).await?;
    let rule = rules::find(&mut tx, rule.id)
        .await?
        .ok_or(Error::NotFound)?;
    let ws = access.workspace.to_string();
    if back == RuleState::Active
        && body.apply_to_existing
        && !rules::only_shows_images(&rule.actions)
    {
        rule_runs::start(&mut tx, &state, access, &rule, RunKind::Apply).await?;
    } else {
        rule_runs::announce(
            &mut tx,
            &state.events,
            &ws,
            rule.inbox,
            rule.id,
            rule.effective_state(),
            None,
        )
        .await?;
    }
    let view = view(&mut tx, &state.binding, access, &rule).await?;
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
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.rule"]).access("conversations.rule", campbooks_kernel::authz::Action::Read)))),
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
    resources::authorize_type(&state, access, Action::Read, RULE_RECORD)?;
    let mut tx = state.db.begin_for(access.workspace).await?;
    let mut inboxes = visible_inboxes(&mut tx, access).await?;
    if let Some(inbox) = query.inbox {
        if !inboxes.contains(&inbox) {
            return Err(Error::NotFound);
        }
        inboxes = vec![inbox];
    }
    let stored = rules::of_inboxes(&mut tx, &inboxes).await?;
    let mut views = views(&mut tx, &state.binding, access, &stored).await?;
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
    extensions(("x-campbooks" = json!(Operation::query().assistant().records(["conversations.rule"]).access("conversations.rule", campbooks_kernel::authz::Action::Read)))),
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
    resources::authorize(
        &state,
        &mut tx,
        access,
        Action::Read,
        &resources::rule(path.rule_id),
    )
    .await?;
    let rule = rules::find_any(&mut tx, path.rule_id)
        .await?
        .ok_or(Error::NotFound)?;
    let view = view(&mut tx, &state.binding, access, &rule).await?;
    tx.commit().await?;
    Ok(Json(view))
}
