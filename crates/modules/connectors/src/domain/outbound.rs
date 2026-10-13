//! Outbound changes: a member's change recorded from
//! `conversations.changes_requested`, made at the provider by the stream's
//! next run, and kept as the record of what became of it
//! (`connectors.outbound_changes`). Ids, label ids and reason codes only.

use std::collections::{HashMap, HashSet};

use campbooks_kernel::{Result, db::BeginFor as _, tenancy::WorkspaceId};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::model::StreamId;
use crate::events::RequestedChange;

/// Gmail's category labels: a message is in one of them.
pub const CATEGORIES: [&str; 5] = [
    "CATEGORY_PERSONAL",
    "CATEGORY_UPDATES",
    "CATEGORY_PROMOTIONS",
    "CATEGORY_SOCIAL",
    "CATEGORY_FORUMS",
];

/// Gmail's system labels, never written except through their own intents
/// (`UNREAD`, `STARRED`, `INBOX`, the categories) and never through a label
/// change.
pub const SYSTEM_LABELS: [&str; 9] = [
    "INBOX",
    "UNREAD",
    "STARRED",
    "IMPORTANT",
    "SENT",
    "DRAFT",
    "CHAT",
    "SPAM",
    "TRASH",
];

/// Whether `label` is a Gmail system label (or category).
pub fn is_system(label: &str) -> bool {
    label.starts_with("CATEGORY_") || SYSTEM_LABELS.contains(&label)
}

/// Whether a change may put on or take off `label`: the labels of the
/// flag intents and the categories, or a user label.
pub fn writable(label: &str) -> bool {
    matches!(label, "UNREAD" | "STARRED" | "INBOX")
        || CATEGORIES.contains(&label)
        || !is_system(label)
}

/// How long a write window may stay open (the profile call after the write
/// failed and no later run closed it) before it is taken to cover nothing.
pub const OPEN_WINDOW: chrono::TimeDelta = chrono::TimeDelta::minutes(10);

/// Most pending changes one run takes.
pub const PUSH_LIMIT: i64 = 500;

/// Runs that may meet provider trouble before a change is given up.
pub const MAX_ATTEMPTS: i32 = 8;

/// How a change ended (or that it has not yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Waiting for a run.
    Pending,
    /// Made at the provider.
    Applied,
    /// Overtaken: by a later change of a member, or at the provider.
    Superseded,
    /// Nothing to do at the provider.
    Skipped,
    /// The provider refused it, or it met trouble too often.
    Failed,
}

impl State {
    /// The database name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Superseded => "superseded",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }
}

/// What an intent means at Gmail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mapped {
    /// Put `add` on and take `remove` off.
    Labels {
        /// Label ids to add.
        add: Vec<String>,
        /// Label ids to remove.
        remove: Vec<String>,
    },
    /// Nothing Gmail can do: recorded as skipped with this reason.
    Skip(&'static str),
}

/// The stored kind of `change`.
pub const fn kind(change: &RequestedChange) -> &'static str {
    match change {
        RequestedChange::MarkRead => "mark_read",
        RequestedChange::MarkUnread => "mark_unread",
        RequestedChange::Star => "star",
        RequestedChange::Unstar => "unstar",
        RequestedChange::Archive => "archive",
        RequestedChange::Unarchive => "unarchive",
        RequestedChange::AddLabel { .. } => "add_label",
        RequestedChange::RemoveLabel { .. } => "remove_label",
        RequestedChange::MoveToStream { .. } => "move_to_stream",
        RequestedChange::Unsupported => "unsupported",
    }
}

/// `change` in Gmail labels. `known` holds the stream's *user* label ids
/// (from `labels.list`): a provider key Gmail no longer has, or a system
/// label, is skipped.
pub fn map(change: &RequestedChange, known: &HashSet<String>) -> Mapped {
    let add = |label: &str| Mapped::Labels {
        add: vec![label.to_owned()],
        remove: Vec::new(),
    };
    let remove = |label: &str| Mapped::Labels {
        add: Vec::new(),
        remove: vec![label.to_owned()],
    };
    match change {
        RequestedChange::MarkRead => remove("UNREAD"),
        RequestedChange::MarkUnread => add("UNREAD"),
        RequestedChange::Star => add("STARRED"),
        RequestedChange::Unstar => remove("STARRED"),
        RequestedChange::Archive => remove("INBOX"),
        RequestedChange::Unarchive => add("INBOX"),
        RequestedChange::AddLabel { provider_key, .. }
        | RequestedChange::RemoveLabel { provider_key, .. } => {
            let key = provider_key
                .as_deref()
                .filter(|key| known.contains(*key) && !is_system(key));
            match key {
                None => Mapped::Skip("label_not_at_provider"),
                Some(key) if matches!(change, RequestedChange::AddLabel { .. }) => add(key),
                Some(key) => remove(key),
            }
        }
        RequestedChange::MoveToStream { stream } => {
            let category = match stream.as_str() {
                "primary" => "CATEGORY_PERSONAL",
                "updates" => "CATEGORY_UPDATES",
                "newsletters" => "CATEGORY_PROMOTIONS",
                "receipts" => return Mapped::Skip("no_provider_equivalent"),
                _ => return Mapped::Skip("unsupported"),
            };
            Mapped::Labels {
                add: vec![category.to_owned()],
                remove: CATEGORIES
                    .iter()
                    .filter(|&&c| c != category)
                    .map(|&c| c.to_owned())
                    .collect(),
            }
        }
        RequestedChange::Unsupported => Mapped::Skip("unsupported"),
    }
}

/// Labels to add and labels to remove.
pub type LabelChange = (Vec<String>, Vec<String>);

/// A pending change, as a run takes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// The row.
    pub id: Uuid,
    /// The message.
    pub provider_message_id: String,
    /// Labels to add.
    pub add: Vec<String>,
    /// Labels to remove.
    pub remove: Vec<String>,
    /// Gmail's history id when it was recorded.
    pub stamp: Option<i64>,
    /// Runs that met trouble with it.
    pub attempts: i32,
    /// When it was recorded.
    pub created_at: DateTime<Utc>,
}

/// One of the module's own earlier writes: the labels it changed on a
/// message and the history window its run wrote in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnWrite {
    /// The labels it added or removed.
    pub labels: Vec<String>,
    /// Gmail's history id before the run wrote.
    pub from: i64,
    /// Gmail's history id after; `None` when it could not be read (the
    /// window is then open-ended).
    pub to: Option<i64>,
}

impl OwnWrite {
    /// Whether the history record `id` about `label` can be this write.
    pub fn covers(&self, label: &str, id: i64) -> bool {
        id > self.from
            && self.to.is_none_or(|to| id <= to)
            && self.labels.iter().any(|l| l == label)
    }
}

/// What a stream's run has to push.
#[derive(Debug, Clone, Default)]
pub struct Backlog {
    /// Pending changes, oldest first.
    pub pending: Vec<Pending>,
    /// The index's labels of their messages (absent: not carried).
    pub labels: HashMap<String, Vec<String>>,
    /// Own earlier writes to those messages.
    pub own: HashMap<String, Vec<OwnWrite>>,
}

impl Backlog {
    /// The newest pending change taken (a run continues at once when newer
    /// ones arrived meanwhile).
    pub fn last(&self) -> Option<Uuid> {
        self.pending.last().map(|p| p.id)
    }
}

/// The pending changes of `stream` (at most [`PUSH_LIMIT`]) with what the
/// push needs to know about their messages.
pub async fn backlog(
    pool: &PgPool,
    workspace: WorkspaceId,
    stream: StreamId,
    now: DateTime<Utc>,
) -> Result<Backlog> {
    let mut tx = pool.begin_for(workspace).await?;
    let pending: Vec<Pending> = sqlx::query!(
        r#"SELECT id, provider_message_id, add_labels, remove_labels, stamp_history_id,
                  attempts, created_at
             FROM connectors.outbound_changes
            WHERE stream_id = $1 AND state = 'pending'
            ORDER BY id LIMIT $2"#,
        stream as StreamId,
        PUSH_LIMIT,
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|row| Pending {
        id: row.id,
        provider_message_id: row.provider_message_id,
        add: row.add_labels,
        remove: row.remove_labels,
        stamp: row.stamp_history_id,
        attempts: row.attempts,
        created_at: row.created_at,
    })
    .collect();
    if pending.is_empty() {
        tx.commit().await?;
        return Ok(Backlog::default());
    }
    let ids: Vec<String> = pending
        .iter()
        .map(|p| p.provider_message_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let labels = sqlx::query!(
        r#"SELECT provider_message_id, label_ids FROM connectors.gmail_messages
            WHERE stream_id = $1 AND provider_message_id = ANY($2)"#,
        stream as StreamId,
        &ids,
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|row| (row.provider_message_id, row.label_ids))
    .collect();
    let mut own: HashMap<String, Vec<OwnWrite>> = HashMap::new();
    for row in sqlx::query!(
        r#"SELECT provider_message_id, add_labels, remove_labels,
                  applied_from AS "from!", applied_to, done_at
             FROM connectors.outbound_changes
            WHERE stream_id = $1 AND provider_message_id = ANY($2)
              AND state = 'applied' AND applied_from IS NOT NULL"#,
        stream as StreamId,
        &ids,
    )
    .fetch_all(&mut *tx)
    .await?
    {
        own.entry(row.provider_message_id)
            .or_default()
            .push(OwnWrite {
                labels: row
                    .add_labels
                    .into_iter()
                    .chain(row.remove_labels)
                    .collect(),
                from: row.from,
                // A window left open longer than [`OPEN_WINDOW`] covers
                // nothing: better a write of ours taken for Gmail's (Gmail
                // wins, the member sees Gmail's state) than real Gmail
                // changes hidden for days.
                to: row.applied_to.or_else(|| {
                    row.done_at
                        .is_some_and(|done| now - done > OPEN_WINDOW)
                        .then_some(row.from)
                }),
            });
    }
    tx.commit().await?;
    Ok(Backlog {
        pending,
        labels,
        own,
    })
}

/// How one change ended in a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct End {
    /// The row.
    pub id: Uuid,
    /// Its state now (`Pending` again after trouble).
    pub state: State,
    /// Why.
    pub reason: Option<&'static str>,
}

/// What a run's push did, to record in one transaction.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    /// Each change's end.
    pub ends: Vec<End>,
    /// Messages written, with the labels the index holds now.
    pub index: Vec<(String, Vec<String>)>,
    /// Messages whose provider state must be emitted again.
    pub reemit: Vec<String>,
    /// The history window the run wrote in.
    pub window: Option<(i64, Option<i64>)>,
}

/// Record `outcome` for `stream`.
pub async fn record_outcome(
    conn: &mut PgConnection,
    stream: StreamId,
    outcome: &Outcome,
    now: DateTime<Utc>,
) -> Result<()> {
    let (from, to) = outcome.window.map_or((None, None), |(f, t)| (Some(f), t));
    for end in &outcome.ends {
        if end.state == State::Pending {
            sqlx::query!(
                "UPDATE connectors.outbound_changes SET attempts = attempts + 1 WHERE id = $1",
                end.id
            )
            .execute(&mut *conn)
            .await?;
            continue;
        }
        let applied = end.state == State::Applied;
        sqlx::query!(
            r#"UPDATE connectors.outbound_changes
                  SET state = $2, reason = $3, done_at = $4,
                      applied_from = CASE WHEN $5 THEN $6::bigint END,
                      applied_to = CASE WHEN $5 THEN $7::bigint END
                WHERE id = $1"#,
            end.id,
            end.state.as_str(),
            end.reason,
            now,
            applied,
            from,
            to,
        )
        .execute(&mut *conn)
        .await?;
    }
    for (message, labels) in &outcome.index {
        sqlx::query!(
            r#"UPDATE connectors.gmail_messages SET label_ids = $3
                WHERE stream_id = $1 AND provider_message_id = $2"#,
            stream as StreamId,
            message,
            labels,
        )
        .execute(&mut *conn)
        .await?;
    }
    if !outcome.reemit.is_empty() {
        sqlx::query!(
            r#"UPDATE connectors.gmail_messages SET reemit = true
                WHERE stream_id = $1 AND provider_message_id = ANY($2)"#,
            stream as StreamId,
            &outcome.reemit,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Give up (`failed`, `expired`) the pending changes of `stream` recorded
/// before `before`, and flag their messages to be re-emitted with Gmail's
/// state: a change that waited that long (a long pause, a re-authorisation)
/// is never made late. Runs before a run takes its backlog. Returns how many
/// expired.
pub async fn expire(
    pool: &PgPool,
    workspace: WorkspaceId,
    stream: StreamId,
    before: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<usize> {
    let mut tx = pool.begin_for(workspace).await?;
    let expired = sqlx::query_scalar!(
        r#"UPDATE connectors.outbound_changes
              SET state = 'failed', reason = 'expired', done_at = $3
            WHERE stream_id = $1 AND state = 'pending' AND created_at < $2
        RETURNING provider_message_id"#,
        stream as StreamId,
        before,
        now,
    )
    .fetch_all(&mut *tx)
    .await?;
    if !expired.is_empty() {
        sqlx::query!(
            r#"UPDATE connectors.gmail_messages SET reemit = true
                WHERE stream_id = $1 AND provider_message_id = ANY($2)"#,
            stream as StreamId,
            &expired,
        )
        .execute(&mut *tx)
        .await?;
        tracing::info!(%stream, expired = expired.len(), "stale outbound changes given up");
    }
    tx.commit().await?;
    Ok(expired.len())
}

/// Release the finished changes of `stream` older than `before`, and close
/// the write windows left open (the profile call after a write failed) at
/// `history_id`, Gmail's history id this run read up to, when it is past
/// their start.
pub async fn prune(
    conn: &mut PgConnection,
    stream: StreamId,
    before: DateTime<Utc>,
    history_id: Option<i64>,
) -> Result<()> {
    sqlx::query!(
        r#"DELETE FROM connectors.outbound_changes
            WHERE stream_id = $1 AND state <> 'pending' AND done_at < $2"#,
        stream as StreamId,
        before,
    )
    .execute(&mut *conn)
    .await?;
    if let Some(history_id) = history_id {
        sqlx::query!(
            r#"UPDATE connectors.outbound_changes SET applied_to = $2
                WHERE stream_id = $1 AND state = 'applied' AND applied_to IS NULL
                  AND applied_from IS NOT NULL AND applied_from < $2"#,
            stream as StreamId,
            history_id,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Whether `stream` has pending changes newer than `after` (all of them
/// when `after` is `None`).
pub async fn more_pending(
    conn: &mut PgConnection,
    stream: StreamId,
    after: Option<Uuid>,
) -> Result<bool> {
    Ok(sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM connectors.outbound_changes
                           WHERE stream_id = $1 AND state = 'pending'
                             AND ($2::uuid IS NULL OR id > $2)) AS "more!""#,
        stream as StreamId,
        after,
    )
    .fetch_one(conn)
    .await?)
}

/// The pending label changes of `ids` in `stream`, oldest first: what a
/// run's emitted state shows until they are made.
pub async fn overlay(
    pool: &PgPool,
    workspace: WorkspaceId,
    stream: StreamId,
    ids: &[String],
) -> Result<HashMap<String, Vec<LabelChange>>> {
    let mut overlay: HashMap<String, Vec<LabelChange>> = HashMap::new();
    if ids.is_empty() {
        return Ok(overlay);
    }
    let mut tx = pool.begin_for(workspace).await?;
    for row in sqlx::query!(
        r#"SELECT provider_message_id, add_labels, remove_labels
             FROM connectors.outbound_changes
            WHERE stream_id = $1 AND state = 'pending' AND provider_message_id = ANY($2)
            ORDER BY id"#,
        stream as StreamId,
        ids,
    )
    .fetch_all(&mut *tx)
    .await?
    {
        overlay
            .entry(row.provider_message_id)
            .or_default()
            .push((row.add_labels, row.remove_labels));
    }
    tx.commit().await?;
    Ok(overlay)
}

/// `labels` with `changes` applied in order (each: add, then remove).
pub fn apply_labels(labels: &[String], changes: &[LabelChange]) -> Vec<String> {
    let mut out: Vec<String> = labels.to_vec();
    for (add, remove) in changes {
        for label in add {
            if !out.contains(label) {
                out.push(label.clone());
            }
        }
        out.retain(|l| !remove.contains(l));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(list: &[&str]) -> Vec<String> {
        list.iter().map(|&s| s.to_owned()).collect()
    }

    #[test]
    fn intents_map_to_gmail_labels() {
        let known: HashSet<String> = ["Label_7".to_owned()].into();
        let lab = uuid::Uuid::nil();
        let cases = [
            (RequestedChange::MarkRead, Some((vec![], vec!["UNREAD"]))),
            (RequestedChange::MarkUnread, Some((vec!["UNREAD"], vec![]))),
            (RequestedChange::Star, Some((vec!["STARRED"], vec![]))),
            (RequestedChange::Unstar, Some((vec![], vec!["STARRED"]))),
            (RequestedChange::Archive, Some((vec![], vec!["INBOX"]))),
            (RequestedChange::Unarchive, Some((vec!["INBOX"], vec![]))),
            (
                RequestedChange::AddLabel {
                    label: lab,
                    provider_key: Some("Label_7".into()),
                },
                Some((vec!["Label_7"], vec![])),
            ),
            (
                RequestedChange::RemoveLabel {
                    label: lab,
                    provider_key: Some("Label_7".into()),
                },
                Some((vec![], vec!["Label_7"])),
            ),
        ];
        for (change, want) in cases {
            let (add, remove) = want.unwrap();
            assert_eq!(
                map(&change, &known),
                Mapped::Labels {
                    add: labels(&add),
                    remove: labels(&remove)
                },
                "{change:?}"
            );
        }
        for change in [
            RequestedChange::AddLabel {
                label: lab,
                provider_key: None,
            },
            RequestedChange::RemoveLabel {
                label: lab,
                provider_key: Some("Label_gone".into()),
            },
        ] {
            assert_eq!(map(&change, &known), Mapped::Skip("label_not_at_provider"));
        }
        let moved = |stream: &str| {
            map(
                &RequestedChange::MoveToStream {
                    stream: stream.into(),
                },
                &known,
            )
        };
        let Mapped::Labels { add, remove } = moved("newsletters") else {
            panic!("newsletters maps to promotions");
        };
        assert_eq!(add, ["CATEGORY_PROMOTIONS"]);
        assert_eq!(remove.len(), 4);
        assert!(!remove.contains(&"CATEGORY_PROMOTIONS".to_owned()));
        assert!(
            matches!(moved("primary"), Mapped::Labels { add, .. } if add == ["CATEGORY_PERSONAL"])
        );
        assert!(
            matches!(moved("updates"), Mapped::Labels { add, .. } if add == ["CATEGORY_UPDATES"])
        );
        assert_eq!(moved("receipts"), Mapped::Skip("no_provider_equivalent"));
        assert_eq!(moved("someday"), Mapped::Skip("unsupported"));
        assert_eq!(
            map(&RequestedChange::Unsupported, &known),
            Mapped::Skip("unsupported")
        );
    }

    #[test]
    fn system_labels_are_never_label_changes() {
        // Even if a cache listed them as known, a label change never
        // touches a system label or a category.
        let known: HashSet<String> = [
            "TRASH",
            "SPAM",
            "SENT",
            "DRAFT",
            "CHAT",
            "IMPORTANT",
            "UNREAD",
            "INBOX",
            "STARRED",
            "CATEGORY_SOCIAL",
            "Label_7",
        ]
        .map(str::to_owned)
        .into();
        for key in known.iter().filter(|k| *k != "Label_7") {
            for change in [
                RequestedChange::AddLabel {
                    label: uuid::Uuid::nil(),
                    provider_key: Some(key.clone()),
                },
                RequestedChange::RemoveLabel {
                    label: uuid::Uuid::nil(),
                    provider_key: Some(key.clone()),
                },
            ] {
                assert_eq!(
                    map(&change, &known),
                    Mapped::Skip("label_not_at_provider"),
                    "{key}"
                );
            }
        }
        for label in [
            "TRASH",
            "SPAM",
            "SENT",
            "DRAFT",
            "CHAT",
            "IMPORTANT",
            "CATEGORY_NEW",
        ] {
            assert!(!writable(label), "{label}");
        }
        for label in [
            "UNREAD",
            "STARRED",
            "INBOX",
            "CATEGORY_PROMOTIONS",
            "Label_7",
        ] {
            assert!(writable(label), "{label}");
        }
    }

    #[test]
    fn unknown_kinds_read_as_unsupported() {
        let change: RequestedChange =
            serde_json::from_value(serde_json::json!({"kind": "snooze", "until": "later"}))
                .unwrap();
        assert_eq!(change, RequestedChange::Unsupported);
        assert_eq!(kind(&change), "unsupported");
    }

    #[test]
    fn own_writes_cover_their_window_and_labels() {
        let own = OwnWrite {
            labels: labels(&["INBOX"]),
            from: 10,
            to: Some(12),
        };
        assert!(own.covers("INBOX", 11) && own.covers("INBOX", 12));
        assert!(!own.covers("INBOX", 10) && !own.covers("INBOX", 13));
        assert!(!own.covers("STARRED", 11));
        let open = OwnWrite { to: None, ..own };
        assert!(open.covers("INBOX", 99));
    }

    #[test]
    fn labels_apply_in_order() {
        let changes = vec![
            (labels(&["STARRED"]), labels(&["INBOX"])),
            (labels(&["INBOX"]), vec![]),
        ];
        assert_eq!(
            apply_labels(&labels(&["INBOX", "UNREAD"]), &changes),
            labels(&["UNREAD", "STARRED", "INBOX"])
        );
    }
}
