//! "Always show images from <sender>" (campbooks-cloud#200): whether a
//! smart rule trusts a message's sender with its remote images. Facts only
//! (ADR 0018): the sender's address and the receiving provider's
//! authentication results, never what the mail says. The images still go
//! through the sanitiser, which removes tracking pixels either way.

use campbooks_ai::decide::{
    signals::facts::{dmarc_refused, sender_authenticated, sender_checks_failed},
    taught::{self, Action, Facts},
};
use campbooks_kernel::Result;
use sqlx::PgConnection;

use super::acting;
use crate::{
    ingest::Headers,
    model::{InboxId, MessageId},
};

/// Whether an active rule of `message`'s inbox (one of `inboxes`, those the
/// member may see) shows its remote images: its sender is trusted, the
/// provider's results authenticate that sender (a spoofed From, failed
/// checks or no results never pass), and nothing about the conversation
/// looks suspicious. An outbound message, or one not in `inboxes`, never.
pub async fn images_trusted(
    conn: &mut PgConnection,
    inboxes: &[InboxId],
    message: MessageId,
) -> Result<bool> {
    let row: Option<(InboxId, String, serde_json::Value, bool)> = sqlx::query_as(
        "SELECT m.inbox_id, m.sender_address, m.headers,
                (EXISTS (SELECT 1 FROM conversations.decisions d
                          WHERE d.conversation_id = m.conversation_id
                            AND d.question = 'suspicious' AND d.value = 'true'::jsonb)
                 OR EXISTS (SELECT 1 FROM conversations.needs n
                             WHERE n.conversation_id = m.conversation_id
                               AND n.state = 'open' AND n.reason = 'review'))
           FROM conversations.messages m
          WHERE m.id = $1 AND m.inbox_id = ANY($2) AND m.direction = 'inbound'",
    )
    .bind(message)
    .bind(inboxes)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((inbox, sender, headers, suspicious)) = row else {
        return Ok(false);
    };
    let rules: Vec<taught::TaughtRule> = acting(conn, inbox)
        .await?
        .into_iter()
        .filter(|r| r.actions.contains(&Action::ShowImages))
        .collect();
    if rules.is_empty() {
        return Ok(false);
    }
    let headers: Headers = serde_json::from_value(headers).unwrap_or_default();
    let results = headers.authentication_results.as_deref();
    let facts = Facts {
        sender_address: &sender,
        suspicious,
        auth_failed: sender_checks_failed(&sender, results),
        impersonated: dmarc_refused(results),
        ..Facts::default()
    };
    Ok(taught::shows_images(&rules, &facts, sender_authenticated(&sender, results)).is_some())
}
