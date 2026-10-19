//! The changes a rule makes to one conversation, made the way the member's
//! operations make them, then announced together: one
//! `conversation_changed`, and a `changes_requested` per kind of change so
//! the provider makes it too.

use campbooks_kernel::{EventBus, Result, binding::Binding};
use sqlx::{PgConnection, Postgres, Transaction, types::Uuid};

use super::applications;
use crate::{
    domain::{
        conversation::{Changed, provider_message_ids, request},
        decide::STATE_VERSION,
        flags::{self, Flag},
    },
    events::{Change, Origin, RequestedChange},
    model::{ConversationId, DecisionId, InboxId, LabelId, MessageId, RuleId, RuleRunId, Stream},
};

/// What changed on one conversation, to announce at the end.
#[derive(Debug, Default)]
pub struct Effects {
    changes: Vec<Change>,
    requests: Vec<(Vec<String>, RequestedChange)>,
}

impl Effects {
    /// Record a change of `kind` that the provider should make on `messages`.
    pub fn add(&mut self, kind: Change, messages: Vec<String>, request: RequestedChange) {
        self.changes.push(kind);
        self.requests.push((messages, request));
    }

    /// The kinds of change recorded.
    pub fn kinds(&self) -> &[Change] {
        &self.changes
    }

    /// Ask the provider for every change; publish `conversation_changed`
    /// too unless `announce` is false (the caller announces it with others).
    pub async fn publish(
        self,
        tx: &mut Transaction<'_, Postgres>,
        binding: &Binding,
        events: &EventBus,
        at: (&str, InboxId, ConversationId),
        announce: bool,
    ) -> Result<()> {
        if self.changes.is_empty() {
            return Ok(());
        }
        let (workspace, inbox, conversation) = at;
        let mut changed = Changed {
            workspace: workspace.to_owned(),
            inbox,
            conversation,
            origin: Origin::Member,
            changes: self.changes,
            provider_message_ids: Vec::new(),
        };
        for (messages, change) in self.requests {
            changed.provider_message_ids = messages;
            request(binding, events, tx, &changed, &change).await?;
        }
        if announce {
            changed.provider_message_ids = provider_message_ids(tx, conversation).await?;
            changed.publish(events, tx).await?;
        }
        Ok(())
    }
}

/// Put conversation `id` in `stream` (`stream_source` `taught`), unless a
/// member placed it. Returns whether it moved.
pub async fn place(conn: &mut PgConnection, id: ConversationId, stream: Stream) -> Result<bool> {
    let moved = sqlx::query!(
        r#"UPDATE conversations.conversations
              SET stream = $2, stream_source = 'taught', updated_at = now()
            WHERE id = $1 AND stream_source <> 'member' AND stream <> $2"#,
        id as ConversationId,
        stream.as_str(),
    )
    .execute(conn)
    .await?
    .rows_affected();
    Ok(moved > 0)
}

/// Put conversation `id` back in `stream` placed by `source`, if a rule
/// still holds it in `moved_to` (the stream the rule put it in; another
/// rule's later placement is left alone). Returns whether it moved.
pub async fn unplace(
    conn: &mut PgConnection,
    id: ConversationId,
    back: (Stream, &str),
    moved_to: Stream,
) -> Result<bool> {
    let (stream, source) = back;
    let moved = sqlx::query!(
        r#"UPDATE conversations.conversations
              SET stream = $2, stream_source = $3, updated_at = now()
            WHERE id = $1 AND stream_source = 'taught' AND stream = $4"#,
        id as ConversationId,
        stream.as_str(),
        source,
        moved_to.as_str(),
    )
    .execute(conn)
    .await?
    .rows_affected();
    Ok(moved > 0)
}

/// Set `flag` (archive, read, trash or spam) on message `id` alone, as
/// [`flags::change`] does for a conversation. Returns the provider ids
/// changed.
pub async fn flag_message(
    conn: &mut PgConnection,
    id: MessageId,
    flag: Flag,
) -> Result<Vec<String>> {
    Ok(match flag {
        Flag::Trash => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET in_trash = true, in_spam = false,
                        updated_at = now()
                  WHERE id = $1 AND (NOT in_trash OR in_spam) RETURNING provider_message_id",
                id as MessageId,
            )
            .fetch_all(conn)
            .await?
        }
        Flag::Spam => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages
                    SET in_spam = true, in_inbox = false, in_trash = false, updated_at = now()
                  WHERE id = $1 AND NOT in_spam RETURNING provider_message_id",
                id as MessageId,
            )
            .fetch_all(conn)
            .await?
        }
        Flag::Archive => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET in_inbox = false, updated_at = now()
                  WHERE id = $1 AND in_inbox RETURNING provider_message_id",
                id as MessageId,
            )
            .fetch_all(conn)
            .await?
        }
        _ => {
            sqlx::query_scalar!(
                "UPDATE conversations.messages SET read = true, updated_at = now()
                  WHERE id = $1 AND NOT read RETURNING provider_message_id",
                id as MessageId,
            )
            .fetch_all(conn)
            .await?
        }
    })
}

/// Set `flag` on conversation `id` as the member's operations do.
pub async fn flag_conversation(
    conn: &mut PgConnection,
    id: ConversationId,
    flag: Flag,
) -> Result<Vec<String>> {
    flags::change(conn, id, flag).await
}

/// Put `label` (of `inbox`) on conversation `id`. Returns the label's
/// provider key (`Some(None)` when the provider has none), or `None` when it
/// already was there or is not the inbox's.
pub async fn link_label(
    conn: &mut PgConnection,
    workspace: Uuid,
    inbox: InboxId,
    id: ConversationId,
    label: LabelId,
) -> Result<Option<Option<String>>> {
    let row = sqlx::query!(
        r#"
        WITH linked AS (
            INSERT INTO conversations.conversation_labels (workspace_id, conversation_id, label_id)
            SELECT $1, $2, l.id FROM conversations.labels l WHERE l.id = $3 AND l.inbox_id = $4
            ON CONFLICT DO NOTHING
            RETURNING label_id
        )
        SELECT l.provider_key FROM linked JOIN conversations.labels l ON l.id = linked.label_id
        "#,
        workspace,
        id as ConversationId,
        label as LabelId,
        inbox as InboxId,
    )
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|r| r.provider_key))
}

/// Take `label` off conversation `id`. Returns its provider key as
/// [`link_label`] does, or `None` when it was not there.
pub async fn unlink_label(
    conn: &mut PgConnection,
    id: ConversationId,
    label: LabelId,
) -> Result<Option<Option<String>>> {
    let row = sqlx::query!(
        r#"DELETE FROM conversations.conversation_labels cl
            USING conversations.labels l
            WHERE cl.conversation_id = $1 AND cl.label_id = $2 AND l.id = cl.label_id
           RETURNING l.provider_key"#,
        id as ConversationId,
        label as LabelId,
    )
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|r| r.provider_key))
}

/// A `taught` answer to store about a message.
pub struct TaughtAnswer<'a> {
    /// The question.
    pub question: &'a str,
    /// The answer.
    pub value: serde_json::Value,
    /// The question's options, in order.
    pub options: serde_json::Value,
    /// The rule that gave it.
    pub rule: &'a str,
}

/// Store `answer` about `message` as a `taught` decision (replacing an older
/// taught one).
pub async fn store_decision(
    conn: &mut PgConnection,
    workspace: Uuid,
    at: (MessageId, ConversationId),
    answer: &TaughtAnswer<'_>,
) -> Result<()> {
    let (message, conversation) = at;
    sqlx::query!(
        r#"
        INSERT INTO conversations.decisions
            (id, workspace_id, message_id, conversation_id, question, source, strategy, value,
             options, probabilities, confidence, model, latency_ms, reasons, acted, state_version)
        VALUES ($1, $2, $3, $4, $5, 'taught', 'taught', $6, $7, NULL, 1.0, 'taught', 0,
                jsonb_build_array(jsonb_build_object('rule', 'taught.rule', 'evidence', $8::text)),
                true, $9)
        ON CONFLICT (message_id, question, source) DO UPDATE
           SET value = EXCLUDED.value, options = EXCLUDED.options, reasons = EXCLUDED.reasons
        "#,
        DecisionId::new() as DecisionId,
        workspace,
        message as MessageId,
        conversation as ConversationId,
        answer.question,
        answer.value,
        answer.options,
        answer.rule,
        STATE_VERSION,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Record that `rule` (of `inbox`, in `workspace`) changed conversation
/// `id` (in `run`, or as mail arrived), and count it unless it already held
/// it changed. A conversation it changed again keeps what it was before the
/// first time, adds what the rule did since and where it last moved it.
///
/// Only right after the rule verified (`rules::acting`, on arrival and in a
/// run): the row is signed here (campbooks-cloud#242). It is claimed and
/// locked first (a row another transaction is inserting is waited for),
/// merged in Rust with what it held only when that row verifies, and
/// written with its MAC in one statement, so what is signed is never a
/// value SQL produced.
pub async fn record(
    conn: &mut PgConnection,
    binding: &Binding,
    owner: applications::Owner,
    at: (ConversationId, Option<RuleRunId>),
    before: &super::Before,
) -> Result<()> {
    let (_, _, rule) = owner;
    let (id, run) = at;
    let held = applications::claim(conn, binding, owner, id).await?;
    let run = run.map(|r| *r.as_uuid());
    let (merged, run) = match held.bound {
        Some((old, old_run)) if held.holds => (applications::merge(old, before), run.or(old_run)),
        Some((_, old_run)) => (before.clone(), run.or(old_run)),
        None => (before.clone(), run),
    };
    applications::write(conn, binding, owner, (id, run), &merged).await?;
    // Counted once per conversation it holds changed: not again when it
    // changes one it already holds (a new message in it), again after a
    // revert put it back.
    let holds = held.holds;
    sqlx::query!(
        r#"UPDATE conversations.rules
              SET applied_count = applied_count + CASE WHEN $2 THEN 0 ELSE 1 END,
                  last_applied_at = now(), updated_at = now()
            WHERE id = $1"#,
        rule as RuleId,
        holds,
    )
    .execute(conn)
    .await?;
    Ok(())
}
