//! Exceptions (a member changed back what a rule did) and the rule shown on
//! a conversation.

use campbooks_kernel::{Result, tenancy::UserId};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use super::{Before, EXCEPTION_DAYS, RuleAction, RuleCondition};
use crate::{
    domain::conversation::ConversationSummary,
    model::{ConversationId, LabelId, RuleId},
};

/// What a member changed back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Undone {
    /// Moved it to another stream.
    Stream,
    /// Put it back in the inbox.
    Archive,
    /// Marked it unread.
    Read,
    /// Took a label off.
    Label(LabelId),
}

/// A rule a member just changed back on one conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleException {
    /// The rule.
    pub rule_id: RuleId,
    /// Its exceptions in the last 30 days, this one included.
    pub recent_exceptions: u32,
}

/// Make conversation `id` an exception of the rule whose change `undone`
/// takes back, if a rule made it. Returns that rule.
pub async fn except(
    conn: &mut PgConnection,
    id: ConversationId,
    undone: Undone,
    by: Option<UserId>,
) -> Result<Option<RuleException>> {
    let rows = sqlx::query!(
        r#"SELECT rule_id AS "rule: RuleId", before FROM conversations.rule_applications
            WHERE conversation_id = $1 AND reverted_at IS NULL AND excepted_at IS NULL
            ORDER BY applied_at DESC"#,
        id as ConversationId,
    )
    .fetch_all(&mut *conn)
    .await?;
    let found = rows.into_iter().find(|row| {
        let before: Before = serde_json::from_value(row.before.clone()).unwrap_or_default();
        match undone {
            Undone::Stream => before.stream.is_some(),
            Undone::Archive => before.archived,
            Undone::Read => before.read,
            Undone::Label(label) => before.labels.contains(&label),
        }
    });
    let Some(found) = found else { return Ok(None) };
    sqlx::query!(
        r#"UPDATE conversations.rule_applications SET excepted_at = now(), excepted_by = $3
            WHERE rule_id = $1 AND conversation_id = $2"#,
        found.rule as RuleId,
        id as ConversationId,
        by.map(|u| *u.as_uuid()),
    )
    .execute(&mut *conn)
    .await?;
    let recent = recent_exceptions(conn, found.rule).await?;
    Ok(Some(RuleException {
        rule_id: found.rule,
        recent_exceptions: recent,
    }))
}

/// How many exceptions each of `rules` had in the last [`EXCEPTION_DAYS`]
/// (those with none are left out).
pub async fn recent_exceptions_of(
    conn: &mut PgConnection,
    rules: &[RuleId],
) -> Result<std::collections::HashMap<RuleId, u32>> {
    let rows = sqlx::query!(
        r#"SELECT rule_id AS "rule: RuleId", count(*)::int AS "n!"
             FROM conversations.rule_applications
            WHERE rule_id = ANY($1) AND excepted_at > now() - make_interval(days => $2)
            GROUP BY rule_id"#,
        rules as &[RuleId],
        i32::try_from(EXCEPTION_DAYS).unwrap_or(30),
    )
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.rule, u32::try_from(r.n).unwrap_or_default()))
        .collect())
}

/// How many exceptions `rule` had in the last [`EXCEPTION_DAYS`].
pub async fn recent_exceptions(conn: &mut PgConnection, rule: RuleId) -> Result<u32> {
    let n = sqlx::query_scalar!(
        r#"SELECT count(*)::int AS "n!" FROM conversations.rule_applications
            WHERE rule_id = $1 AND excepted_at > now() - make_interval(days => $2)"#,
        rule as RuleId,
        i32::try_from(EXCEPTION_DAYS).unwrap_or(30),
    )
    .fetch_one(conn)
    .await?;
    Ok(u32::try_from(n).unwrap_or_default())
}

/// The rule that placed or changed a conversation, as rows show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RuleMark {
    /// The rule.
    pub id: RuleId,
    /// What it does.
    pub actions: Vec<RuleAction>,
    /// Its conditions, for "like … others from …".
    pub conditions: Vec<RuleCondition>,
    /// How many conversations it has changed.
    pub applied_count: u32,
}

/// Fill in each summary's [`RuleMark`]: the newest rule that changed it and
/// that no member changed back.
pub async fn attach(conn: &mut PgConnection, summaries: &mut [ConversationSummary]) -> Result<()> {
    let ids: Vec<ConversationId> = summaries.iter().map(|s| s.id).collect();
    if ids.is_empty() {
        return Ok(());
    }
    let rows = sqlx::query!(
        r#"
        SELECT DISTINCT ON (a.conversation_id)
               a.conversation_id AS "conversation: ConversationId", r.id AS "rule: RuleId",
               r.actions, r.conditions, r.applied_count
          FROM conversations.rule_applications a
          JOIN conversations.rules r ON r.id = a.rule_id
         WHERE a.conversation_id = ANY($1) AND a.reverted_at IS NULL AND a.excepted_at IS NULL
           AND r.state <> 'removed'
         ORDER BY a.conversation_id, a.applied_at DESC
        "#,
        &ids as &[ConversationId],
    )
    .fetch_all(conn)
    .await?;
    for row in rows {
        let Some(summary) = summaries.iter_mut().find(|s| s.id == row.conversation) else {
            continue;
        };
        summary.rule = Some(RuleMark {
            id: row.rule,
            actions: serde_json::from_value(row.actions).unwrap_or_default(),
            conditions: serde_json::from_value(row.conditions).unwrap_or_default(),
            applied_count: u32::try_from(row.applied_count).unwrap_or_default(),
        });
    }
    Ok(())
}
