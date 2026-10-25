//! `connectors.send_address_check` — send an IMAP mailbox's address check
//! (campbooks-cloud#314): one plain-text message, from and to the
//! mailbox's address, through its own SMTP server, carrying the check's
//! code in its `Message-ID`; then the stream is due at once, and its runs
//! look for the message in the connection's Inbox (`imap::check`).
//!
//! The submission is IMAP sending's (#313): the guard's address, TLS or
//! STARTTLS required, bounded replies, a time limit per step, the sealed
//! password, the login's, workspace's and host's sending budgets (one
//! message, one recipient) and the login's one SMTP session slot. A server
//! that will not send as or to the address (a 5xx), a refused password or a
//! server that cannot be used safely ends the check `failed`; a busy one is
//! tried again, up to six attempts. A connection lost after the message
//! went out counts as sent: the search decides. The code is never logged, and no error carries it.

use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use campbooks_kernel::{
    JobContext, JobError, db::BeginFor as _, i18n::Locale, jobs::JobHandler, tenancy::WorkspaceId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    MESSAGES,
    deps::Deps,
    domain::{
        address_checks,
        credentials::{self, TokenError},
        imap_servers,
        model::{ConnectionId, StreamId},
        schedule,
    },
    features::send_message::via_imap::{give_back, mailbox, sending_budgets, take_all},
    imap::smtp::{Failed, Submission},
};

/// The job's name.
pub const SEND_ADDRESS_CHECK: &str = "connectors.send_address_check";

/// How long a check waits for its mailbox's one SMTP session to be free.
const SESSION_WAIT: Duration = Duration::from_secs(10);

/// How long a check the server did not take now waits before trying again.
const RETRY_WAIT: Duration = Duration::from_mins(1);

/// Which connection's address check to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendAddressCheckPayload {
    /// The connection (an id, never an address).
    pub connection: ConnectionId,
}

/// Sends address checks.
#[derive(Debug, Clone)]
pub struct SendAddressCheck {
    deps: Deps,
}

impl SendAddressCheck {
    /// The job handler, composed with the module's dependencies.
    pub fn new(deps: Deps) -> Self {
        Self { deps }
    }
}

/// The connection a check is sent for.
struct Mailbox {
    address: String,
    verified: bool,
    stream: Option<StreamId>,
}

impl JobHandler for SendAddressCheck {
    type Payload = SendAddressCheckPayload;
    const NAME: &'static str = SEND_ADDRESS_CHECK;
    const QUEUE: &'static str = "sync";
    const MAX_ATTEMPTS: u32 = 6;

    async fn run(
        &self,
        ctx: &mut JobContext,
        payload: SendAddressCheckPayload,
    ) -> Result<(), JobError> {
        let workspace: WorkspaceId = ctx
            .workspace()
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| JobError::fatal("an address check enqueued outside a workspace"))?;
        let connection = payload.connection;
        let mut tx = ctx.pool().begin_for(workspace).await?;
        let Some(check) = address_checks::sending(&mut tx, connection).await? else {
            return Ok(()); // Sent, ended or replaced meanwhile.
        };
        tx.commit().await?;
        match self.attempt(ctx, workspace, connection, check).await {
            Ok(()) => Ok(()),
            // Out of attempts: the check ends, rather than wait for ever.
            Err(JobError::Retry { .. }) if ctx.is_last_attempt() => {
                tracing::info!(%workspace, %connection, "an address check could not be sent");
                self.end(ctx, connection, check.nonce, "smtp_unavailable")
                    .await
            }
            Err(err) => Err(err),
        }
    }
}

impl SendAddressCheck {
    /// One attempt at sending check `check` of `connection`.
    #[allow(
        clippy::too_many_lines,
        reason = "one attempt read top to bottom: mailbox, message, budgets, submission, outcome"
    )]
    async fn attempt(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        connection: ConnectionId,
        check: address_checks::Sending,
    ) -> Result<(), JobError> {
        let now = self.deps.clock.now();
        let Some(cipher) = self
            .deps
            .cipher
            .clone()
            .filter(|_| self.deps.imap_enabled())
        else {
            return self
                .end(ctx, connection, check.nonce, "provider_not_configured")
                .await;
        };
        let mut tx = ctx.pool().begin_for(workspace).await?;
        let found = load(&mut tx, connection).await?;
        let account = imap_servers::smtp_server(&mut tx, connection).await?;
        let opened = credentials::password(&mut tx, &cipher, connection).await;
        tx.commit().await?;
        let Some(found) = found else {
            return Ok(()); // Disconnected meanwhile.
        };
        if found.verified {
            let tx = ctx.tx().await?;
            address_checks::verified(tx, connection, check.nonce, now).await?;
            return Ok(());
        }
        let password = match opened {
            Ok(password) => password,
            Err(TokenError::NeedsReauth(_)) => {
                return self.end(ctx, connection, check.nonce, "needs_reauth").await;
            }
            Err(TokenError::Provider(_)) => {
                return Err(JobError::retry(
                    "the mailbox's password could not be opened",
                ));
            }
            Err(TokenError::Database(err)) => return Err(err.into()),
        };
        let (Some((username, server)), Some(stream)) = (account, found.stream) else {
            return self
                .end(ctx, connection, check.nonce, "not_connected")
                .await;
        };
        let (Some(address), Some(domain)) = (
            mailbox(found.address.trim()),
            found
                .address
                .trim()
                .rsplit_once('@')
                .map(|(_, d)| d.to_owned()),
        ) else {
            return self
                .end(ctx, connection, check.nonce, "address_invalid")
                .await;
        };
        let code = address_checks::code(&ctx.state().signer, connection, check.nonce);
        let message = message(
            check.locale,
            AsRef::<str>::as_ref(&address),
            &address_checks::message_id(&code, &domain),
            now,
        );

        let config = &self.deps.imap.config;
        let (spend, session) = sending_budgets(
            &ctx.state().signer,
            config,
            workspace,
            (&username, &server.host),
            1,
        );
        if let Err(wait) = take_all(ctx.pool(), &spend).await? {
            return Err(JobError::retry_after(
                "waiting for the mailbox's sending budget",
                wait,
            ));
        }
        if take_all(ctx.pool(), &session).await?.is_err() {
            give_back(ctx.pool(), &spend).await;
            return Err(JobError::retry_after(
                "another send of this mailbox is in progress",
                SESSION_WAIT,
            ));
        }
        let total = Duration::from_secs(u64::from(config.send_timeout_seconds));
        let submitted = Box::pin(submit(
            &self.deps,
            (&server, &username, &password),
            &address,
            &message,
            total,
        ))
        .await;
        give_back(ctx.pool(), &session).await;
        drop(password);

        match submitted {
            Ok(()) | Err(Failed::Unclear) => {
                tracing::info!(%workspace, %connection, "an address check was sent");
                let tx = ctx.tx().await?;
                if address_checks::sent(tx, connection, check.nonce, now).await? {
                    // Its runs look for it from now on.
                    schedule::run_now(tx, workspace, stream, now).await?;
                }
                Ok(())
            }
            Err(Failed::Retry(code)) => {
                give_back(ctx.pool(), &spend).await;
                tracing::info!(%workspace, %connection, code, "the SMTP server did not take an address check now");
                Err(JobError::retry_after(
                    "the SMTP server did not take the address check; trying again later",
                    RETRY_WAIT,
                ))
            }
            Err(Failed::Refused(code)) => {
                // A refusal keeps what it cost: trying addresses is never free.
                tracing::info!(%workspace, %connection, code, "the SMTP server would not send as or to the address");
                self.end(ctx, connection, check.nonce, "address_refused")
                    .await
            }
            Err(Failed::TooLarge) => {
                self.end(ctx, connection, check.nonce, "address_refused")
                    .await
            }
            Err(Failed::LoginRefused) => {
                give_back(ctx.pool(), &spend).await;
                self.end(ctx, connection, check.nonce, "smtp_login_refused")
                    .await
            }
            Err(Failed::Unsafe(code)) => {
                give_back(ctx.pool(), &spend).await;
                tracing::warn!(%workspace, %connection, code, "the SMTP server cannot be used safely; no address check");
                self.end(ctx, connection, check.nonce, "smtp_unavailable")
                    .await
            }
        }
    }

    /// End check `nonce` of `connection`, unverified, for `reason`.
    async fn end(
        &self,
        ctx: &mut JobContext,
        connection: ConnectionId,
        nonce: Uuid,
        reason: &'static str,
    ) -> Result<(), JobError> {
        let now = self.deps.clock.now();
        let tx = ctx.tx().await?;
        address_checks::failed(tx, connection, nonce, reason, now).await?;
        Ok(())
    }
}

/// The IMAP connection `connection` of the transaction's workspace: its
/// address, whether it is verified, and its messages stream.
async fn load(
    conn: &mut sqlx::PgConnection,
    connection: ConnectionId,
) -> Result<Option<Mailbox>, JobError> {
    let row: Option<(String, bool, Option<StreamId>)> = sqlx::query_as(
        "SELECT c.address, c.address_verified,
                (SELECT s.id FROM connectors.streams s
                  WHERE s.connection_id = c.id AND s.capability = 'messages'
                  ORDER BY s.created_at, s.id LIMIT 1)
           FROM connectors.connections c
          WHERE c.id = $1 AND c.provider = 'imap'",
    )
    .bind(connection)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(address, verified, stream)| Mailbox {
        address,
        verified,
        stream,
    }))
}

/// Submit `message` from and to `address` through `server`.
async fn submit(
    deps: &Deps,
    (server, username, password): (&crate::imap::Server, &str, &secrecy::SecretString),
    address: &lettre::Address,
    message: &[u8],
    total: Duration,
) -> Result<(), Failed> {
    let mut submission = Box::pin(Submission::open(
        &deps.imap, server, username, password, total,
    ))
    .await?;
    if let Err(failed) = submission
        .offer(address, std::slice::from_ref(address), message.len(), true)
        .await
    {
        submission.quit().await;
        return Err(failed);
    }
    match submission.message(message).await {
        Ok(()) => {
            submission.quit().await;
            Ok(())
        }
        Err(Failed::Unclear) => Err(Failed::Unclear),
        Err(failed) => {
            submission.quit().await;
            Err(failed)
        }
    }
}

/// The check message: plain text in `locale`, from and to `address`, with
/// `message_id`; ASCII on the wire (the subject an RFC 2047 encoded word,
/// the body base64), and marked as sent by a machine so nothing answers it.
fn message(locale: Locale, address: &str, message_id: &str, now: DateTime<Utc>) -> Vec<u8> {
    let subject = MESSAGES.t(locale, "address_check.subject", &[]);
    let body = [
        MESSAGES.t(
            locale,
            "address_check.why",
            &[("address", address.to_owned().into())],
        ),
        MESSAGES.t(locale, "address_check.nothing_to_do", &[]),
        MESSAGES.t(locale, "address_check.not_you", &[]),
    ]
    .join("\r\n\r\n");
    let mut out = String::new();
    for (name, value) in [
        ("From", format!("<{address}>")),
        ("To", format!("<{address}>")),
        ("Subject", encoded_word(&subject)),
        ("Date", now.to_rfc2822()),
        ("Message-ID", message_id.to_owned()),
        ("MIME-Version", "1.0".to_owned()),
        ("Content-Type", "text/plain; charset=utf-8".to_owned()),
        ("Content-Transfer-Encoding", "base64".to_owned()),
        ("Auto-Submitted", "auto-generated".to_owned()),
        ("X-Auto-Response-Suppress", "All".to_owned()),
    ] {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(&value);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    let encoded = STANDARD.encode(format!("{body}\r\n").as_bytes());
    for line in encoded.as_bytes().chunks(76) {
        out.push_str(&String::from_utf8_lossy(line));
        out.push_str("\r\n");
    }
    out.into_bytes()
}

/// `text` as a header value: as it is when it is printable ASCII, else
/// RFC 2047 encoded words (UTF-8, base64) of at most 45 bytes each, folded.
/// A control character never reaches the header.
fn encoded_word(text: &str) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if text.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return text;
    }
    let mut words = Vec::new();
    let mut chunk = String::new();
    for c in text.chars() {
        if chunk.len() + c.len_utf8() > 45 {
            words.push(std::mem::take(&mut chunk));
        }
        chunk.push(c);
    }
    if !chunk.is_empty() {
        words.push(chunk);
    }
    words
        .iter()
        .map(|word| format!("=?UTF-8?B?{}?=", STANDARD.encode(word.as_bytes())))
        .collect::<Vec<_>>()
        .join("\r\n ")
}

#[cfg(test)]
mod tests;
