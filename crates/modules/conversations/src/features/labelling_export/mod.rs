//! `campbooks-server labelling export`: a workspace's recent money mail as
//! an eval set its owner labels on their own machine (campbooks-cloud#167).
//!
//! The money question's operating point is set from mail the owner labels,
//! never from emails an agent wrote (ADR 0018). This lists, for one
//! workspace, the newest inbound message of every conversation sorted to
//! Receipts or that ever had a Pay need, sent within the last `days`, as
//! items in the format of `crates/ai/evals/decide/` (what the decide model
//! reads: envelope, list and automation headers, attachment names and types,
//! the first part of the text body), with no labels. The operator runs it on
//! the server with shell access, as `campbooks-server admin` (the runtime
//! login, in the workspace: row-level security applies), and copies the
//! folder to their own machine; it never leaves the instance any other way
//! and is never committed. `cargo xtask eval decide --set-dir <folder>`
//! then reads the labels the owner filled in `labels.csv`.
//!
//! Nothing here decides anything: it reads stored messages only.

use campbooks_kernel::{AppState, Error, Result, db::BeginFor as _, tenancy::WorkspaceId};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};

use crate::{
    domain::{
        blobs::read_text,
        decide::{ENVELOPE_BODY_BYTES, MessageParts, envelope},
        redact,
    },
    ingest::Headers,
    model::Address,
};

/// The most messages one export lists, newest first.
pub const MAX_ITEMS: i64 = 500;

/// One message to label.
#[derive(Debug, Clone)]
pub struct LabellingItem {
    /// `owner-<n>`, newest first.
    pub id: String,
    /// When it was sent.
    pub sent_at: DateTime<Utc>,
    /// Who sent it (name and address), for the labelling sheet.
    pub from: String,
    /// Its subject, for the labelling sheet.
    pub subject: String,
    /// Its attachments' names, for the labelling sheet.
    pub attachments: Vec<String>,
    /// The item, in the eval set's format, with empty labels.
    pub item: Value,
}

#[derive(sqlx::FromRow)]
struct Row {
    sent_at: DateTime<Utc>,
    sender: Value,
    recipients: Value,
    subject: String,
    headers: Value,
    bodies: Value,
    inbox_address: String,
    inbox_name: Option<String>,
    inbox_members: i64,
    filenames: Vec<String>,
    content_types: Vec<String>,
}

/// `messages.recipients`.
#[derive(serde::Deserialize)]
struct Recipients {
    #[serde(default)]
    to: Vec<Address>,
    #[serde(default)]
    cc: Vec<Address>,
    #[serde(default)]
    reply_to: Vec<Address>,
}

/// `messages.bodies`.
#[derive(serde::Deserialize)]
struct Bodies {
    #[serde(default)]
    text: Option<campbooks_kernel::blobs::BlobRef>,
}

/// The newest inbound message of each of `workspace`'s conversations in
/// Receipts, or that had a Pay need, sent within `days`, newest first (at
/// most [`MAX_ITEMS`]).
pub async fn export_for_labelling(
    state: &AppState,
    workspace: WorkspaceId,
    days: u32,
) -> Result<Vec<LabellingItem>> {
    let since = Utc::now() - TimeDelta::days(i64::from(days));
    let mut tx = state.db.begin_for(workspace).await?;
    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT * FROM (
            SELECT DISTINCT ON (m.conversation_id)
                   m.id, m.sent_at, m.sender, m.recipients, m.subject, m.headers, m.bodies,
                   i.address AS inbox_address, i.display_name AS inbox_name,
                   (SELECT count(*) FROM conversations.inbox_members im
                     WHERE im.inbox_id = i.id) AS inbox_members,
                   ARRAY(SELECT a.filename FROM conversations.attachments a
                          WHERE a.message_id = m.id ORDER BY a.position) AS filenames,
                   ARRAY(SELECT a.content_type FROM conversations.attachments a
                          WHERE a.message_id = m.id ORDER BY a.position) AS content_types
              FROM conversations.messages m
              JOIN conversations.conversations c ON c.id = m.conversation_id
              JOIN conversations.inboxes i ON i.id = m.inbox_id
             WHERE m.direction = 'inbound' AND NOT m.in_spam AND NOT m.in_trash
               AND m.sent_at >= $1
               AND (c.stream = 'receipts'
                    OR EXISTS (SELECT 1 FROM conversations.needs n
                                WHERE n.conversation_id = c.id AND n.reason = 'pay'))
             ORDER BY m.conversation_id, m.sent_at DESC, m.id DESC
        ) newest
        ORDER BY sent_at DESC, id DESC
        LIMIT $2
        "#,
    )
    .bind(since)
    .bind(MAX_ITEMS)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let ws = workspace.to_string();
    let mut items = Vec::with_capacity(rows.len());
    for (n, row) in rows.into_iter().enumerate() {
        let decode = |e: serde_json::Error| Error::internal(redact::json(&e));
        let from: Address = serde_json::from_value(row.sender).map_err(decode)?;
        let recipients: Recipients = serde_json::from_value(row.recipients).map_err(decode)?;
        let headers: Headers = serde_json::from_value(row.headers).map_err(decode)?;
        let bodies: Bodies = serde_json::from_value(row.bodies).map_err(decode)?;
        let body = match &bodies.text {
            // A body that cannot be read is left out, not the message.
            Some(text) => read_text(&state.blobs, &ws, text, ENVELOPE_BODY_BYTES)
                .await
                .map(|(body, _)| body)
                .unwrap_or_default(),
            None => String::new(),
        };
        let attachments: Vec<(String, String)> = row
            .filenames
            .iter()
            .cloned()
            .zip(row.content_types)
            .collect();
        let env = envelope(MessageParts {
            from: &from,
            to: &recipients.to,
            cc: &recipients.cc,
            reply_to: &recipients.reply_to,
            subject: &row.subject,
            headers: &headers,
            attachments: &attachments,
            body: &body,
        });
        let id = format!("owner-{:03}", n + 1);
        let party = |p: &campbooks_ai::decide::signals::email::Party| json!({"name": p.name, "address": p.address});
        let item = json!({
            "id": id,
            "lang": OWNER,
            // Alternate dev and test: thresholds are fitted on one half and
            // reported on the other.
            "split": if n % 2 == 0 { "dev" } else { "test" },
            "workspace": OWNER,
            "hard": [],
            "message": {
                "date": row.sent_at.to_rfc3339(),
                "from": party(&env.from),
                "to": env.to.iter().map(party).collect::<Vec<_>>(),
                "cc": env.cc.iter().map(party).collect::<Vec<_>>(),
                "subject": env.subject,
                "headers": env.headers,
                "attachments": env.attachments.iter()
                    .map(|a| json!({"name": a.name, "content_type": a.content_type}))
                    .collect::<Vec<_>>(),
                "body": env.body,
                "recipient": {
                    "name": row.inbox_name.clone().unwrap_or_default(),
                    "addresses": [row.inbox_address],
                    "shared": row.inbox_members > 1,
                },
            },
            "labels": {},
            "rationale": "",
        });
        items.push(LabellingItem {
            id,
            sent_at: row.sent_at,
            from: match &from.name {
                Some(name) if !name.is_empty() => format!("{name} <{}>", from.address),
                _ => from.address.clone(),
            },
            subject: row.subject,
            attachments: row.filenames,
            item,
        });
    }
    Ok(items)
}

/// The locale and workspace name every exported item carries: the owner's
/// own mail, in whatever language it is.
pub const OWNER: &str = "owner";

/// The `manifest.json` of an export: the owner's set, private (real mail:
/// the reserved-domain checks do not apply), asking only `money`.
pub fn manifest() -> Value {
    json!({
        "name": "owner-money",
        "version": 1,
        "purpose": "conversations.triage",
        "private": true,
        "locales": [OWNER],
        "workspaces": {OWNER: "The owner's own mail (never committed)."},
        "questions": [
            {
                "id": "money",
                "type": "choice",
                "labels": ["owed", "paid", "neither"],
                "target_precision": 0.9
            }
        ]
    })
}

#[cfg(test)]
mod tests;
