//! Carrying out a rule on one conversation (in a run) and putting it back.

use std::collections::BTreeSet;

use campbooks_ai::decide::{
    signals::email::triage,
    taught::{self, Outcome, Surface, TaughtRule},
};
use campbooks_kernel::{EventBus, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgConnection, Postgres, Transaction};

use super::{
    Candidate, Stored,
    effects::{self, Effects, TaughtAnswer},
};
use crate::{
    domain::{
        flags::Flag,
        needs::{self, NeedSettings},
    },
    events::{Change, RequestedChange},
    model::{ConversationId, InboxId, LabelId, RuleId, RuleRunId, Stream},
};

/// What a rule did to one conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// It changed something.
    Changed,
    /// Nothing to do: already so, or another rule wins every slot.
    Unchanged,
    /// Left alone: it looks suspicious.
    Suspicious,
    /// Left alone: a member placed it or changed back what the rule did.
    Member,
}

/// What a conversation was before a rule changed it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Before {
    /// Its stream and who put it there, when the rule moved it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream: Option<(Stream, String)>,
    /// Whether the rule archived it.
    #[serde(default)]
    pub archived: bool,
    /// Whether the rule marked it read.
    #[serde(default)]
    pub read: bool,
    /// Labels the rule put on it.
    #[serde(default)]
    pub labels: Vec<LabelId>,
    /// Questions the rule answered on its newest inbound message.
    #[serde(default)]
    pub answered: Vec<String>,
}

/// Where a rule acts: the inbox, the workspace's event key and the events.
pub struct Context<'a> {
    /// The event bus.
    pub events: &'a EventBus,
    /// The workspace, as events key it.
    pub workspace: &'a str,
    /// The workspace id.
    pub workspace_id: sqlx::types::Uuid,
    /// The inbox.
    pub inbox: InboxId,
    /// How needs are raised.
    pub needs: &'a NeedSettings,
}

/// Carry out what `rule` wins in `rules`' outcome on `candidate`, as part of
/// `run`. Records the application; a conversation the rule already changed,
/// or that a member made an exception of, is left alone.
pub async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &Context<'_>,
    rule: RuleId,
    rules: &[TaughtRule],
    candidate: &Candidate,
    run: Option<RuleRunId>,
) -> Result<Applied> {
    let existing = sqlx::query!(
        r#"SELECT excepted_at IS NOT NULL AS "excepted!", reverted_at IS NOT NULL AS "reverted!"
             FROM conversations.rule_applications WHERE rule_id = $1 AND conversation_id = $2"#,
        rule as RuleId,
        candidate.id as ConversationId,
    )
    .fetch_optional(&mut **tx)
    .await?;
    match existing {
        Some(e) if e.excepted => return Ok(Applied::Member),
        Some(e) if !e.reverted => return Ok(Applied::Unchanged),
        _ => {}
    }
    let acting = acting_on(tx, rules, candidate.id).await?;
    let outcome = taught::evaluate(&acting, &candidate.facts());
    let id = rule.to_string();
    let mine = Mine::of(&outcome, &id);
    if mine.is_empty() {
        let wanted = acting
            .iter()
            .any(|r| r.id == id && r.matches(&candidate.facts()));
        return Ok(if wanted && candidate.suspicious {
            Applied::Suspicious
        } else {
            Applied::Unchanged
        });
    }
    if mine.stream.is_some_and(|s| s != candidate.stream) && candidate.stream_source == "member" {
        return Ok(Applied::Member);
    }
    let (before, effects) = carry_out(tx, ctx, candidate, &mine).await?;
    let mut before = before;
    before.answered = answer(tx, ctx, candidate, &outcome, &id).await?;
    if before == Before::default() {
        return Ok(Applied::Unchanged);
    }
    effects::record(tx, ctx.workspace_id, rule, (candidate.id, run), &before).await?;
    effects
        .publish(
            tx,
            ctx.events,
            (ctx.workspace, ctx.inbox, candidate.id),
            true,
        )
        .await?;
    needs::refresh(tx, ctx.events, ctx.workspace, candidate.id, ctx.needs).await?;
    Ok(Applied::Changed)
}

/// Move, archive, mark read and label as `mine` says. Returns what it was
/// before and what to announce.
async fn carry_out(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &Context<'_>,
    candidate: &Candidate,
    mine: &Mine,
) -> Result<(Before, Effects)> {
    let mut before = Before::default();
    let mut effects = Effects::default();
    if let Some(stream) = mine.stream
        && effects::place(tx, candidate.id, stream).await?
    {
        before.stream = Some((candidate.stream, candidate.stream_source.clone()));
        let messages = crate::domain::conversation::provider_message_ids(tx, candidate.id).await?;
        effects.add(
            Change::Stream,
            messages,
            RequestedChange::MoveToStream { stream },
        );
    }
    for (wanted, flag) in [(mine.archive, Flag::Archive), (mine.read, Flag::Read)] {
        if !wanted {
            continue;
        }
        let messages = effects::flag_conversation(tx, candidate.id, flag).await?;
        if messages.is_empty() {
            continue;
        }
        if flag == Flag::Archive {
            before.archived = true;
        } else {
            before.read = true;
        }
        effects.add(flag.change(), messages, flag.requested());
    }
    for label in &mine.labels {
        let linked =
            effects::link_label(tx, ctx.workspace_id, ctx.inbox, candidate.id, *label).await?;
        let Some(provider_key) = linked else { continue };
        before.labels.push(*label);
        let messages = crate::domain::conversation::provider_message_ids(tx, candidate.id).await?;
        effects.add(
            Change::Labels,
            messages,
            RequestedChange::AddLabel {
                label: *label,
                provider_key,
            },
        );
    }
    Ok((before, effects))
}

struct Mine {
    stream: Option<Stream>,
    archive: bool,
    read: bool,
    labels: Vec<LabelId>,
    answers: bool,
}

impl Mine {
    fn of(outcome: &Outcome, id: &str) -> Self {
        Self {
            stream: outcome
                .stream
                .as_ref()
                .filter(|a| a.rule == id)
                .and_then(|a| a.value.parse().ok()),
            archive: outcome.archive.as_deref() == Some(id),
            read: outcome.read.as_deref() == Some(id),
            labels: outcome
                .labels
                .iter()
                .filter(|a| a.rule == id)
                .filter_map(|a| a.value.parse().ok())
                .collect(),
            answers: outcome.needs_reply.as_ref().is_some_and(|a| a.rule == id)
                || outcome.asks_action.as_ref().is_some_and(|a| a.rule == id)
                || outcome.surface.as_ref().is_some_and(|a| a.rule == id),
        }
    }

    fn is_empty(&self) -> bool {
        self.stream.is_none()
            && !self.archive
            && !self.read
            && self.labels.is_empty()
            && !self.answers
    }
}

/// The answers `rule` wins in `outcome`, as `taught` decisions to store.
pub fn answers<'a>(outcome: &Outcome, rule: &'a str) -> Vec<TaughtAnswer<'a>> {
    let flag = |a: &Option<taught::Applied<bool>>| {
        a.as_ref()
            .filter(|a| a.rule == rule)
            .map(|a| (json!(a.value), json!(["false", "true"])))
    };
    let surface = outcome
        .surface
        .as_ref()
        .filter(|a| a.rule == rule)
        .map(|a| {
            let value = match a.value {
                Surface::Always => "always",
                Surface::Never => "never",
            };
            (json!(value), json!(["always", "never"]))
        });
    [
        (triage::NEEDS_REPLY, flag(&outcome.needs_reply)),
        (triage::ASKS_ACTION, flag(&outcome.asks_action)),
        (needs::SURFACE, surface),
    ]
    .into_iter()
    .filter_map(|(question, answer)| {
        answer.map(|(value, options)| TaughtAnswer {
            question,
            value,
            options,
            rule,
        })
    })
    .collect()
}

/// Store the answers `rule` wins on `candidate`'s newest inbound message.
/// Returns the questions.
async fn answer(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &Context<'_>,
    candidate: &Candidate,
    outcome: &Outcome,
    rule: &str,
) -> Result<Vec<String>> {
    let mut answered = Vec::new();
    for answer in answers(outcome, rule) {
        effects::store_decision(
            tx,
            ctx.workspace_id,
            (candidate.message, candidate.id),
            &answer,
        )
        .await?;
        answered.push(answer.question.to_owned());
    }
    Ok(answered)
}

async fn excepted_rules(conn: &mut PgConnection, id: ConversationId) -> Result<BTreeSet<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT rule_id AS "rule: RuleId" FROM conversations.rule_applications
            WHERE conversation_id = $1 AND excepted_at IS NOT NULL"#,
        id as ConversationId,
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .map(|r| r.to_string())
    .collect())
}

/// The rules acting in `inbox` without those a member made an exception of
/// on `conversation`.
pub async fn acting_on(
    conn: &mut PgConnection,
    rules: &[TaughtRule],
    conversation: ConversationId,
) -> Result<Vec<TaughtRule>> {
    let excepted = excepted_rules(conn, conversation).await?;
    Ok(rules
        .iter()
        .filter(|r| !excepted.contains(&r.id))
        .cloned()
        .collect())
}

/// Put back what `rule` changed on conversation `id`, where the
/// conversation still holds what the rule set. Returns whether anything
/// changed.
pub async fn revert(
    tx: &mut Transaction<'_, Postgres>,
    ctx: &Context<'_>,
    rule: &Stored,
    id: ConversationId,
) -> Result<bool> {
    let row = sqlx::query!(
        r#"SELECT before FROM conversations.rule_applications
            WHERE rule_id = $1 AND conversation_id = $2
              AND reverted_at IS NULL AND excepted_at IS NULL
            FOR UPDATE"#,
        rule.id as RuleId,
        id as ConversationId,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else { return Ok(false) };
    let before: Before = serde_json::from_value(row.before).unwrap_or_default();
    let effects = undo(tx, id, &before).await?;
    if !before.answered.is_empty() {
        sqlx::query!(
            r#"DELETE FROM conversations.decisions
                WHERE conversation_id = $1 AND source = 'taught' AND question = ANY($2)
                  AND reasons @> jsonb_build_array(jsonb_build_object('evidence', $3::text))"#,
            id as ConversationId,
            &before.answered,
            rule.id.to_string(),
        )
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query!(
        r#"UPDATE conversations.rule_applications SET reverted_at = now()
            WHERE rule_id = $1 AND conversation_id = $2"#,
        rule.id as RuleId,
        id as ConversationId,
    )
    .execute(&mut **tx)
    .await?;
    effects
        .publish(tx, ctx.events, (ctx.workspace, ctx.inbox, id), true)
        .await?;
    needs::refresh(tx, ctx.events, ctx.workspace, id, ctx.needs).await?;
    Ok(true)
}

/// Put conversation `id` back as `before` says, where a rule still holds it.
async fn undo(conn: &mut PgConnection, id: ConversationId, before: &Before) -> Result<Effects> {
    let mut effects = Effects::default();
    if let Some((stream, source)) = &before.stream
        && effects::unplace(conn, id, *stream, source).await?
    {
        let messages = crate::domain::conversation::provider_message_ids(conn, id).await?;
        effects.add(
            Change::Stream,
            messages,
            RequestedChange::MoveToStream { stream: *stream },
        );
    }
    for (did, flag) in [
        (before.archived, Flag::Unarchive),
        (before.read, Flag::Unread),
    ] {
        if !did {
            continue;
        }
        let messages = effects::flag_conversation(conn, id, flag).await?;
        if !messages.is_empty() {
            effects.add(flag.change(), messages, flag.requested());
        }
    }
    for label in &before.labels {
        if let Some(provider_key) = effects::unlink_label(conn, id, *label).await? {
            let messages = crate::domain::conversation::provider_message_ids(conn, id).await?;
            effects.add(
                Change::Labels,
                messages,
                RequestedChange::RemoveLabel {
                    label: *label,
                    provider_key,
                },
            );
        }
    }
    Ok(effects)
}
