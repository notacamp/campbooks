//! Outbound changes: a member's change recorded from
//! `conversations.changes_requested`, made at the provider by the stream's
//! next run, and kept as the record of what became of it
//! (`connectors.outbound_changes`). Ids, label ids and reason codes only.

use std::collections::{HashMap, HashSet};

use campbooks_kernel::{
    Result,
    binding::{Binding, Fields},
    db::BeginFor as _,
    tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    marks::{gmail_place, with_mark, zoho_mark},
    model::StreamId,
};
use crate::{contract::ProviderStream, events::RequestedChange};

/// Gmail's category labels: a message is in one of them.
pub const CATEGORIES: [&str; 5] = [
    "CATEGORY_PERSONAL",
    "CATEGORY_UPDATES",
    "CATEGORY_PROMOTIONS",
    "CATEGORY_SOCIAL",
    "CATEGORY_FORUMS",
];

/// Gmail's system labels, never written except through their own intents
/// (`UNREAD`, `STARRED`, `INBOX`, `TRASH`, `SPAM`, the categories) and never
/// through a label change.
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
/// flag intents and the categories, a stream mark's token, or a user label.
pub fn writable(label: &str) -> bool {
    matches!(label, "UNREAD" | "STARRED" | "INBOX" | "TRASH" | "SPAM")
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
        RequestedChange::Trash => "trash",
        RequestedChange::Untrash { .. } => "untrash",
        RequestedChange::Spam => "spam",
        RequestedChange::Unspam { .. } => "unspam",
        RequestedChange::AddLabel { .. } => "add_label",
        RequestedChange::RemoveLabel { .. } => "remove_label",
        RequestedChange::MoveToStream { .. } => "move_to_stream",
        RequestedChange::Unsupported => "unsupported",
    }
}

/// The stream `stream` names, if it is one.
fn stream_of(stream: &str) -> Option<ProviderStream> {
    ProviderStream::parse(stream)
}

/// `change` in Gmail labels. `known` holds the stream's *user* label ids
/// (from `labels.list`) other than stream marks: a provider key Gmail no
/// longer has, a system label or a stream mark is skipped. A move puts the
/// message in its stream's place ([`gmail_place`]) and takes every other
/// category and stream mark off.
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
        // The trash and spam exclude each other. Trash is made with
        // `messages.trash` and taken back with `messages.untrash`, which puts
        // back the labels the message had (`gmail/write.rs`). Spam leaves the
        // inbox; "not spam" puts mail the inbox received back in it (as
        // Gmail's own button does), never the member's own.
        RequestedChange::Trash => Mapped::Labels {
            add: vec!["TRASH".to_owned()],
            remove: vec!["SPAM".to_owned()],
        },
        RequestedChange::Untrash { .. } => remove("TRASH"),
        RequestedChange::Spam => Mapped::Labels {
            add: vec!["SPAM".to_owned()],
            remove: vec!["INBOX".to_owned(), "TRASH".to_owned()],
        },
        RequestedChange::Unspam { inbound } => out_of("SPAM", *inbound),
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
            let Some(stream) = stream_of(stream) else {
                return Mapped::Skip("unsupported");
            };
            let (category, mark) = gmail_place(stream);
            let others = CATEGORIES
                .iter()
                .filter(|&&c| c != category)
                .map(|&c| c.to_owned())
                .collect();
            let (add, remove) = with_mark(vec![category.to_owned()], others, mark);
            Mapped::Labels { add, remove }
        }
        RequestedChange::Unsupported => Mapped::Skip("unsupported"),
    }
}

/// `change` in a Zoho message's state tokens (`zoho/state.rs`): read,
/// flag, inbox, trash and spam, a Zoho label (`label:<id>`) and a stream
/// mark. `labels` holds the ids of the stream's Zoho labels other than
/// stream marks, or is `None` when the grant has no labels scope: label
/// changes and moves are then skipped (`scope_missing`). Zoho has no
/// streams of its own, so a move puts its stream's mark on
/// ([`zoho_mark`], none for Primary) and every other mark off. Out of the
/// trash or spam means back where the message was.
pub fn map_zoho(change: &RequestedChange, labels: Option<&HashSet<String>>) -> Mapped {
    let set = |token: &str, on: bool| Mapped::Labels {
        add: if on {
            vec![token.to_owned()]
        } else {
            Vec::new()
        },
        remove: if on {
            Vec::new()
        } else {
            vec![token.to_owned()]
        },
    };
    match change {
        RequestedChange::MarkRead => set("UNREAD", false),
        RequestedChange::MarkUnread => set("UNREAD", true),
        RequestedChange::Star => set("STARRED", true),
        RequestedChange::Unstar => set("STARRED", false),
        RequestedChange::Archive => set("INBOX", false),
        RequestedChange::Unarchive => set("INBOX", true),
        // Into the trash (the push remembers the folder it was in) or spam;
        // out of either, mail the inbox received may go to the Inbox folder
        // (`INBOX`) where Zoho cannot say better (`zoho/write.rs`).
        RequestedChange::Trash => Mapped::Labels {
            add: vec!["TRASH".to_owned()],
            remove: vec!["SPAM".to_owned()],
        },
        RequestedChange::Spam => Mapped::Labels {
            add: vec!["SPAM".to_owned()],
            remove: vec!["TRASH".to_owned()],
        },
        RequestedChange::Untrash { inbound } => out_of("TRASH", *inbound),
        RequestedChange::Unspam { inbound } => out_of("SPAM", *inbound),
        RequestedChange::AddLabel { provider_key, .. }
        | RequestedChange::RemoveLabel { provider_key, .. } => {
            let Some(labels) = labels else {
                return Mapped::Skip("scope_missing");
            };
            match provider_key.as_deref().filter(|key| labels.contains(*key)) {
                None => Mapped::Skip("label_not_at_provider"),
                Some(key) => set(
                    &crate::zoho::state::label_token(key),
                    matches!(change, RequestedChange::AddLabel { .. }),
                ),
            }
        }
        RequestedChange::MoveToStream { stream } => {
            let Some(stream) = stream_of(stream) else {
                return Mapped::Skip("unsupported");
            };
            if labels.is_none() {
                return Mapped::Skip("scope_missing");
            }
            let (add, remove) = with_mark(Vec::new(), Vec::new(), zoho_mark(stream));
            Mapped::Labels { add, remove }
        }
        RequestedChange::Unsupported => Mapped::Skip("unsupported"),
    }
}

/// Out of the trash or spam (`bin`), and into the inbox too for mail the
/// inbox received.
fn out_of(bin: &str, inbound: bool) -> Mapped {
    Mapped::Labels {
        add: if inbound {
            vec!["INBOX".to_owned()]
        } else {
            Vec::new()
        },
        remove: vec![bin.to_owned()],
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
    /// Where each of those messages was before Campbooks put it in the
    /// trash (Zoho's location tokens), while that still holds, so untrash
    /// puts it back there.
    pub restore: HashMap<String, Vec<String>>,
}

impl Backlog {
    /// The newest pending change taken (a run continues at once when newer
    /// ones arrived meanwhile).
    pub fn last(&self) -> Option<Uuid> {
        self.pending.last().map(|p| p.id)
    }
}

/// The encoding of an outbound change's binding (campbooks-cloud#227).
const OUTBOUND_CHANGE: &str = "campbooks/connectors.outbound-change/v1";

/// An outbound change as recorded: what its binding covers.
#[derive(sqlx::FromRow)]
struct Recorded {
    id: Uuid,
    stream_id: StreamId,
    request_id: Uuid,
    provider_message_id: String,
    kind: String,
    add_labels: Vec<String>,
    remove_labels: Vec<String>,
    mac: Option<Vec<u8>>,
}

impl Recorded {
    /// The row of `workspace`, field by field: the row, its stream, its
    /// request, the message, the kind and the labels it adds and removes.
    fn fields(&self, workspace: WorkspaceId) -> Fields {
        let mut fields = Fields::new(OUTBOUND_CHANGE);
        fields
            .uuid(workspace.as_uuid())
            .uuid(&self.id)
            .uuid(self.stream_id.as_uuid())
            .uuid(&self.request_id)
            .str(&self.provider_message_id)
            .str(&self.kind)
            .count(self.add_labels.len());
        for label in &self.add_labels {
            fields.str(label);
        }
        fields.count(self.remove_labels.len());
        for label in &self.remove_labels {
            fields.str(label);
        }
        fields
    }
}

/// The outbound changes `ids`, as recorded.
async fn recorded(conn: &mut PgConnection, ids: &[Uuid]) -> Result<Vec<Recorded>> {
    Ok(sqlx::query_as(
        "SELECT id, stream_id, request_id, provider_message_id, kind, add_labels, remove_labels,
                mac
           FROM connectors.outbound_changes WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(conn)
    .await?)
}

/// Re-tag, with the current key, every pending outbound change of
/// `workspace` (the transaction's) whose MAC verifies under the previous key
/// only (a rotation of the instance secret key, campbooks-cloud#262): the
/// MAC was just verified, so what was bound stays bound and nothing new is
/// trusted. Each update is a compare-and-set on the old MAC. Returns how
/// many were re-tagged, and how many changed while this ran (left for the
/// next run). Changes no longer pending are never verified again, so they
/// are left as they are.
pub async fn retag(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
) -> Result<(u64, u64)> {
    let rows: Vec<Recorded> = sqlx::query_as(
        "SELECT id, stream_id, request_id, provider_message_id, kind, add_labels, remove_labels,
                mac
           FROM connectors.outbound_changes
          WHERE state = 'pending' AND mac IS NOT NULL ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await?;
    let (mut retagged, mut left) = (0, 0);
    for row in rows {
        let Some(mac) = binding.retag(&row.fields(workspace), row.mac.as_deref()) else {
            continue;
        };
        let done = sqlx::query(
            "UPDATE connectors.outbound_changes SET mac = $2 WHERE id = $1 AND mac = $3",
        )
        .bind(row.id)
        .bind(mac)
        .bind(row.mac.as_deref())
        .execute(&mut *conn)
        .await?;
        if done.rows_affected() == 0 {
            left += 1;
        } else {
            retagged += 1;
        }
    }
    Ok((retagged, left))
}

/// Bind the outbound changes `ids` of `workspace`, just recorded in `conn`
/// (campbooks-cloud#227): a MAC over each row as written, under a key the
/// database never holds, which a run verifies before it makes the change
/// ([`backlog`]). Without a key the rows stay unbound, and are not made.
pub async fn bind(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
    ids: &[Uuid],
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    for row in recorded(conn, ids).await? {
        let mac = binding.tag(&row.fields(workspace));
        sqlx::query("UPDATE connectors.outbound_changes SET mac = $2 WHERE id = $1")
            .bind(row.id)
            .bind(mac)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// The changes of `pending` whose binding verifies. The others (planted or
/// edited by SQL, or recorded by an older version) are never made: they end
/// `skipped` (`unverified`) and their messages are re-read from the
/// provider, so Campbooks shows the provider's state.
async fn verified(
    conn: &mut PgConnection,
    binding: &Binding,
    workspace: WorkspaceId,
    stream: StreamId,
    pending: Vec<Pending>,
    now: DateTime<Utc>,
) -> Result<Vec<Pending>> {
    if pending.is_empty() {
        return Ok(pending);
    }
    let ids: Vec<Uuid> = pending.iter().map(|p| p.id).collect();
    let bound: HashSet<Uuid> = recorded(conn, &ids)
        .await?
        .into_iter()
        .filter(|row| row.stream_id == stream)
        .filter(|row| binding.verify(&row.fields(workspace), row.mac.as_deref()))
        .map(|row| row.id)
        .collect();
    let (kept, refused): (Vec<Pending>, Vec<Pending>) =
        pending.into_iter().partition(|p| bound.contains(&p.id));
    if refused.is_empty() {
        return Ok(kept);
    }
    let ids: Vec<Uuid> = refused.iter().map(|p| p.id).collect();
    let messages: Vec<String> = refused
        .iter()
        .map(|p| p.provider_message_id.clone())
        .collect();
    sqlx::query(
        "UPDATE connectors.outbound_changes
            SET state = 'skipped', reason = 'unverified', done_at = $2
          WHERE id = ANY($1) AND state = 'pending'",
    )
    .bind(&ids)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE connectors.messages SET reemit = true
          WHERE stream_id = $1 AND provider_message_id = ANY($2)",
    )
    .bind(stream)
    .bind(&messages)
    .execute(&mut *conn)
    .await?;
    tracing::warn!(
        %workspace,
        %stream,
        refused = refused.len(),
        "outbound changes not bound to the request they were recorded for; they are not made"
    );
    Ok(kept)
}

/// The pending changes of `stream` (at most [`PUSH_LIMIT`]) with what the
/// push needs to know about their messages. Only changes whose binding
/// verifies (`binding`, campbooks-cloud#227) are taken; the others end
/// `skipped` (`unverified`) and are never made.
pub async fn backlog(
    pool: &PgPool,
    binding: &Binding,
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
    let pending = verified(&mut tx, binding, workspace, stream, pending, now).await?;
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
        r#"SELECT provider_message_id, label_ids FROM connectors.messages
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
    let restore = restore_of(&mut tx, stream, &ids).await?;
    tx.commit().await?;
    Ok(Backlog {
        pending,
        labels,
        own,
        restore,
    })
}

/// Where each of `ids` (messages of `stream`) was before Campbooks put it in
/// the trash at Zoho, while that still holds.
async fn restore_of(
    conn: &mut PgConnection,
    stream: StreamId,
    ids: &[String],
) -> Result<HashMap<String, Vec<String>>> {
    // Where each message was before Campbooks put it in the trash (Zoho):
    // only when that is the latest trash or spam change made to it, and
    // only while no sync has seen it leave the trash since (a sync clears
    // the record then, `forget_left_trash`).
    Ok(sqlx::query!(
        r#"SELECT o.provider_message_id, o.restore_to AS "restore_to!"
             FROM connectors.outbound_changes o
            WHERE o.stream_id = $1 AND o.provider_message_id = ANY($2)
              AND o.kind = 'trash' AND o.state = 'applied' AND o.restore_to IS NOT NULL
              AND NOT EXISTS (
                  SELECT 1 FROM connectors.outbound_changes l
                   WHERE l.stream_id = o.stream_id
                     AND l.provider_message_id = o.provider_message_id
                     AND l.kind IN ('trash', 'untrash', 'spam', 'unspam')
                     AND l.state = 'applied' AND l.id > o.id)"#,
        stream as StreamId,
        ids,
    )
    .fetch_all(conn)
    .await?
    .into_iter()
    .map(|row| (row.provider_message_id, row.restore_to))
    .collect())
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
    /// Changes that put a message in the trash, with where it was before
    /// (Zoho's location tokens).
    pub restore: Vec<(Uuid, Vec<String>)>,
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
            r#"UPDATE connectors.messages SET label_ids = $3
                WHERE stream_id = $1 AND provider_message_id = $2"#,
            stream as StreamId,
            message,
            labels,
        )
        .execute(&mut *conn)
        .await?;
    }
    for (id, tokens) in &outcome.restore {
        sqlx::query!(
            "UPDATE connectors.outbound_changes SET restore_to = $2 WHERE id = $1",
            id,
            tokens,
        )
        .execute(&mut *conn)
        .await?;
    }
    if !outcome.reemit.is_empty() {
        sqlx::query!(
            r#"UPDATE connectors.messages SET reemit = true
                WHERE stream_id = $1 AND provider_message_id = ANY($2)"#,
            stream as StreamId,
            &outcome.reemit,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Forget where messages of `stream` were before Campbooks put them in the
/// trash once the index no longer has them there: a sync saw them leave it
/// (moved at the provider, or untrashed), so that place is no longer where
/// to put them back.
pub async fn forget_left_trash(conn: &mut PgConnection, stream: StreamId) -> Result<()> {
    sqlx::query!(
        r#"UPDATE connectors.outbound_changes o SET restore_to = NULL
             FROM connectors.messages m
            WHERE o.stream_id = $1 AND o.restore_to IS NOT NULL
              AND m.stream_id = o.stream_id AND m.provider_message_id = o.provider_message_id
              AND NOT ('TRASH' = ANY (m.label_ids))"#,
        stream as StreamId,
    )
    .execute(conn)
    .await?;
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
            r#"UPDATE connectors.messages SET reemit = true
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
            (RequestedChange::Trash, Some((vec!["TRASH"], vec!["SPAM"]))),
            (
                RequestedChange::Untrash { inbound: true },
                Some((vec![], vec!["TRASH"])),
            ),
            (
                RequestedChange::Spam,
                Some((vec!["SPAM"], vec!["INBOX", "TRASH"])),
            ),
            (
                RequestedChange::Unspam { inbound: true },
                Some((vec!["INBOX"], vec!["SPAM"])),
            ),
            (
                RequestedChange::Unspam { inbound: false },
                Some((vec![], vec!["SPAM"])),
            ),
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
        // The other four categories and every stream mark.
        assert_eq!(remove.len(), 8);
        assert!(!remove.contains(&"CATEGORY_PROMOTIONS".to_owned()));
        assert!(remove.contains(&"mark:receipts".to_owned()));
        assert!(
            matches!(moved("primary"), Mapped::Labels { add, .. } if add == ["CATEGORY_PERSONAL"])
        );
        assert!(
            matches!(moved("updates"), Mapped::Labels { add, .. } if add == ["CATEGORY_UPDATES"])
        );
        // Receipts: Gmail's Updates tab and the Receipts mark.
        let Mapped::Labels { add, remove } = moved("receipts") else {
            panic!("receipts has a place at Gmail");
        };
        assert_eq!(add, ["CATEGORY_UPDATES", "mark:receipts"]);
        assert!(remove.contains(&"CATEGORY_PROMOTIONS".to_owned()));
        assert!(!remove.contains(&"mark:receipts".to_owned()));
        assert!(remove.contains(&"mark:updates".to_owned()));
        assert_eq!(moved("someday"), Mapped::Skip("unsupported"));
        assert_eq!(
            map(&RequestedChange::Unsupported, &known),
            Mapped::Skip("unsupported")
        );
    }

    #[test]
    fn intents_map_to_zoho_state() {
        let lab = uuid::Uuid::nil();
        let known: HashSet<String> = ["123".to_owned()].into();
        let map_zoho = |change: &RequestedChange| super::map_zoho(change, Some(&known));
        assert_eq!(
            map_zoho(&RequestedChange::Archive),
            Mapped::Labels {
                add: vec![],
                remove: labels(&["INBOX"])
            }
        );
        assert_eq!(
            map_zoho(&RequestedChange::Star),
            Mapped::Labels {
                add: labels(&["STARRED"]),
                remove: vec![]
            }
        );
        assert_eq!(
            map_zoho(&RequestedChange::MarkRead),
            Mapped::Labels {
                add: vec![],
                remove: labels(&["UNREAD"])
            }
        );
        assert_eq!(
            map_zoho(&RequestedChange::Trash),
            Mapped::Labels {
                add: labels(&["TRASH"]),
                remove: labels(&["SPAM"])
            }
        );
        assert_eq!(
            map_zoho(&RequestedChange::Spam),
            Mapped::Labels {
                add: labels(&["SPAM"]),
                remove: labels(&["TRASH"])
            }
        );
        assert_eq!(
            map_zoho(&RequestedChange::Untrash { inbound: true }),
            Mapped::Labels {
                add: labels(&["INBOX"]),
                remove: labels(&["TRASH"])
            }
        );
        assert_eq!(
            map_zoho(&RequestedChange::Unspam { inbound: false }),
            Mapped::Labels {
                add: vec![],
                remove: labels(&["SPAM"])
            }
        );
        assert_eq!(
            map_zoho(&RequestedChange::AddLabel {
                label: lab,
                provider_key: Some("123".into())
            }),
            Mapped::Labels {
                add: labels(&["label:123"]),
                remove: vec![]
            }
        );
        assert_eq!(
            map_zoho(&RequestedChange::RemoveLabel {
                label: lab,
                provider_key: Some("123".into())
            }),
            Mapped::Labels {
                add: vec![],
                remove: labels(&["label:123"])
            }
        );
        assert_eq!(
            map_zoho(&RequestedChange::AddLabel {
                label: lab,
                provider_key: Some("404".into())
            }),
            Mapped::Skip("label_not_at_provider")
        );
    }

    #[test]
    fn zoho_moves_go_through_stream_marks() {
        let lab = uuid::Uuid::nil();
        let known: HashSet<String> = ["123".to_owned()].into();
        let map_zoho = |change: &RequestedChange| super::map_zoho(change, Some(&known));
        let moved = |stream: &str| {
            map_zoho(&RequestedChange::MoveToStream {
                stream: stream.into(),
            })
        };
        assert_eq!(
            moved("newsletters"),
            Mapped::Labels {
                add: labels(&["mark:newsletters"]),
                remove: labels(&["mark:primary", "mark:updates", "mark:receipts"])
            }
        );
        // Primary is the mark-free inbox.
        assert_eq!(
            moved("primary"),
            Mapped::Labels {
                add: vec![],
                remove: labels(&[
                    "mark:primary",
                    "mark:updates",
                    "mark:receipts",
                    "mark:newsletters"
                ])
            }
        );
        // A grant without the labels scope.
        for change in [
            RequestedChange::MoveToStream {
                stream: "updates".into(),
            },
            RequestedChange::AddLabel {
                label: lab,
                provider_key: Some("123".into()),
            },
        ] {
            assert_eq!(
                super::map_zoho(&change, None),
                Mapped::Skip("scope_missing")
            );
        }
        assert_eq!(
            super::map_zoho(&RequestedChange::Archive, None),
            Mapped::Labels {
                add: vec![],
                remove: labels(&["INBOX"])
            }
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
        for label in ["SENT", "DRAFT", "CHAT", "IMPORTANT", "CATEGORY_NEW"] {
            assert!(!writable(label), "{label}");
        }
        for label in [
            "UNREAD",
            "STARRED",
            "INBOX",
            "TRASH",
            "SPAM",
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
