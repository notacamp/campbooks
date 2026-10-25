//! Sending through an IMAP mailbox's SMTP submission server
//! (campbooks-cloud#313).
//!
//! The envelope sender and the `From` header are the connection's own
//! address, and only once that address is verified (a login proves the
//! username, not an address typed beside it); a document from any other is
//! not sent. The recipients are the document's To, Cc and Bcc, and the
//! message on the wire carries no `Bcc`. Before connecting, the mailbox
//! login's, the workspace's and (on a host that is not a big provider's)
//! the SMTP host's budgets are taken, and the login's one session slot.
//!
//! SMTP has no idempotency key, so the attempt is committed once the server
//! accepted `DATA`, before the first byte of the message: a failure before
//! then, or the server's answer to the message, is definite; a connection
//! lost once the message started may have sent it, and the send is never
//! made again (`unknown_outcome`): nothing at an SMTP server can prove it
//! either way.
//!
//! Once accepted, the sent message is indexed under the key the IMAP reader
//! gives its copy and announced, and its copy is left for the stream's next
//! run to append to Sent (`imap::sent`), unless the SMTP host keeps one
//! itself.

use std::{collections::BTreeSet, str::FromStr as _, time::Duration};

use campbooks_kernel::{JobContext, JobError, db::BeginFor as _, tenancy::WorkspaceId};
use chrono::{DateTime, Utc};
use secrecy::SecretString;
use sqlx::PgPool;

use super::{
    MAX_MESSAGE_BYTES, SendMessage, Sent, SentCopy, Target, clear_unclear, finish_failed,
    read_blob, record_attempt, unsent,
};
use crate::{
    contract::OutgoingMessage,
    domain::{
        credentials::{self, TokenError},
        health::{self, LOGIN_REFUSALS},
        imap_servers,
        model::{ConnectionId, SendId, StreamId},
        schedule::{self, Rate},
    },
    features::imap_idle::lease,
    gmail::compose,
    imap::{
        self,
        folders::Role,
        read::message_key,
        smtp::{Failed, Submission},
        state::{self, Cursor},
    },
    mail::{Fetched, Receiver},
};

/// How long a send waits for its mailbox's one SMTP session to be free.
const SESSION_WAIT: Duration = Duration::from_secs(10);

/// How long a send the server did not take now waits before trying again.
const RETRY_WAIT: Duration = Duration::from_mins(1);

/// Seconds a login's SMTP session slot takes to come back if a process died
/// holding it.
const SLOT_SECONDS: f64 = 900.0;

/// What one IMAP attempt works with.
pub(super) struct ImapSend<'a> {
    pub workspace: WorkspaceId,
    pub send: SendId,
    pub stream: StreamId,
    pub connection: ConnectionId,
    pub document: &'a OutgoingMessage,
    pub give_up: bool,
    pub stands: bool,
    pub now: DateTime<Utc>,
}

/// One budget a send takes from: its key, its rate and what it costs.
type Budget = (String, Rate, u32);

/// How a submission ended.
enum Submitted {
    /// The server took the message.
    Sent,
    /// It did not, or may have (`Failed::Unclear`).
    Failed(Failed),
    /// Another runner claimed the attempt first: nothing was sent.
    Claimed(JobError),
}

impl SendMessage {
    /// An attempt through the IMAP mailbox's SMTP server.
    #[allow(
        clippy::too_many_lines,
        reason = "one attempt read top to bottom: account, checks, budgets, submission, outcome"
    )]
    pub(super) async fn attempt_imap(
        &self,
        ctx: &mut JobContext,
        mut target: Target,
        attempt: ImapSend<'_>,
    ) -> Result<(), JobError> {
        let ImapSend {
            workspace,
            send,
            stream,
            connection,
            document,
            give_up,
            stands,
            now,
        } = attempt;
        // An earlier attempt's message may have reached the server: it is
        // never sent again, and no answer of an SMTP server can say.
        if target.unclear {
            tracing::warn!(%workspace, %send, "an earlier attempt may have been sent; not sending again");
            return finish_failed(ctx, workspace, send, &target, "unavailable").await;
        }
        if give_up || !stands {
            return unsent(ctx, workspace, send, &target, stands).await;
        }
        let Some(cipher) = self.deps.cipher.clone() else {
            return finish_failed(ctx, workspace, send, &target, "not_connected").await;
        };
        let mut tx = ctx.pool().begin_for(workspace).await?;
        let account = imap_servers::smtp_server(&mut tx, connection).await?;
        let mailbox_address: Option<(Option<String>, bool)> = sqlx::query_as(
            "SELECT c.address, c.address_verified
               FROM connectors.streams st
               JOIN connectors.connections c ON c.id = st.connection_id
              WHERE st.id = $1",
        )
        .bind(stream)
        .fetch_optional(&mut *tx)
        .await?;
        let opened = credentials::password(&mut tx, &cipher, connection).await;
        tx.commit().await?;
        let password = match opened {
            Ok(password) => password,
            Err(TokenError::NeedsReauth(reason)) => {
                return self
                    .reauth(ctx, workspace, send, &target, connection, reason)
                    .await;
            }
            Err(TokenError::Provider(_)) => {
                return Err(JobError::retry(
                    "the mailbox's password could not be opened",
                ));
            }
            Err(TokenError::Database(err)) => return Err(err.into()),
        };
        let (Some((username, server)), Some((Some(address), verified))) =
            (account, mailbox_address)
        else {
            return finish_failed(ctx, workspace, send, &target, "not_connected").await;
        };
        let config = &self.deps.imap.config;

        // From the mailbox's verified address only, to recipients that are
        // addresses.
        if !verified {
            tracing::warn!(
                %workspace,
                %send,
                code = "address_not_verified",
                "the mailbox's address is not verified; not sent"
            );
            return finish_failed(ctx, workspace, send, &target, "rejected").await;
        }
        let address = address.trim();
        if !document.from.address.trim().eq_ignore_ascii_case(address) {
            tracing::warn!(%workspace, %send, "a send from another address than the mailbox's is not sent");
            return finish_failed(ctx, workspace, send, &target, "rejected").await;
        }
        // As stored, its case kept: only the comparison ignores it.
        let Some(from) = mailbox(address) else {
            return finish_failed(ctx, workspace, send, &target, "rejected").await;
        };
        let Some(to) = envelope(document) else {
            tracing::info!(%workspace, %send, "a recipient is not an address; not sent");
            return finish_failed(ctx, workspace, send, &target, "rejected").await;
        };
        if to.is_empty()
            || to.len() > usize::try_from(config.send_recipients_per_message).unwrap_or(0)
        {
            tracing::info!(%workspace, %send, recipients = to.len(), "too many recipients, or none; not sent");
            return finish_failed(ctx, workspace, send, &target, "rejected").await;
        }

        // The message on the wire (no Bcc) and the Sent copy (Bcc kept).
        let max = usize::try_from(config.send_max_message_bytes)
            .unwrap_or(MAX_MESSAGE_BYTES)
            .min(MAX_MESSAGE_BYTES);
        let Some((wire, copy)) = messages(ctx, workspace, document, max).await? else {
            return finish_failed(ctx, workspace, send, &target, "too_large").await;
        };

        // The budgets, then the login's one session.
        let signer = &ctx.state().signer;
        let login = lease::keyed(
            signer,
            b"connectors.smtp.login",
            &format!(
                "{}\u{0}{}",
                username.trim().to_lowercase(),
                server.host.to_ascii_lowercase()
            ),
        );
        let host = lease::keyed(
            signer,
            b"connectors.smtp.host",
            &server.host.to_ascii_lowercase(),
        );
        let recipients = u32::try_from(to.len()).unwrap_or(u32::MAX);
        let keys = Keys {
            login: &login,
            host: &host,
            workspace,
            provider: imap::is_smtp_provider_host(&server.host),
        };
        let spend = budgets(config, &keys, recipients);
        let session = [(
            format!("smtp:session:{login}"),
            Rate {
                per_second: 1.0 / SLOT_SECONDS,
                burst_seconds: SLOT_SECONDS,
            },
            1,
        )];
        if let Err(wait) = take_all(ctx.pool(), &spend).await? {
            tracing::info!(%workspace, %send, "waiting for the mailbox's sending budget");
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

        let submitted = Box::pin(self.submit(
            ctx,
            &mut target,
            (workspace, send),
            (&server, &username, &password),
            (&from, &to, &wire),
        ))
        .await;
        give_back(ctx.pool(), &session).await;
        drop(password);

        match submitted {
            Submitted::Sent => {
                tracing::info!(%workspace, %send, recipients = to.len(), "the SMTP server took the message");
                let sent = self.sent(&target, document, copy, &server.host, now);
                self.finish_sent(ctx, workspace, send, &target, stream, sent, now)
                    .await
            }
            Submitted::Claimed(err) => {
                give_back(ctx.pool(), &spend).await;
                Err(err)
            }
            Submitted::Failed(Failed::Unclear) => {
                // `target.unclear`: the send ends `unknown_outcome`.
                tracing::warn!(%workspace, %send, "the connection was lost after the message went out; not sending again");
                finish_failed(ctx, workspace, send, &target, "unavailable").await
            }
            Submitted::Failed(failed) => {
                // A refusal (a recipient refused, say) keeps what it cost:
                // trying addresses at a server is never free.
                if !matches!(failed, Failed::Refused(_)) {
                    give_back(ctx.pool(), &spend).await;
                }
                self.not_taken(ctx, &target, (workspace, send, stream, connection), failed)
                    .await
            }
        }
    }

    /// Open a submission, offer the message, claim the attempt once the
    /// server accepted `DATA`, then send the message.
    async fn submit(
        &self,
        ctx: &JobContext,
        target: &mut Target,
        (workspace, send): (WorkspaceId, SendId),
        (server, username, password): (&imap::Server, &str, &SecretString),
        (from, to, wire): (&lettre::Address, &[lettre::Address], &[u8]),
    ) -> Submitted {
        let total = Duration::from_secs(u64::from(self.deps.imap.config.send_timeout_seconds));
        let opening = Submission::open(&self.deps.imap, server, username, password, total);
        let mut submission = match Box::pin(opening).await {
            Ok(submission) => submission,
            Err(failed) => return Submitted::Failed(failed),
        };
        if let Err(failed) = submission
            .offer(from, to, wire.len(), wire.is_ascii())
            .await
        {
            submission.quit().await;
            return Submitted::Failed(failed);
        }
        // From here until the server answers, the message may be out. The
        // claim is timed now, so a second runner waits a whole submission
        // from here before it settles anything.
        let claimed = self.deps.clock.now();
        if let Err(err) =
            record_attempt(ctx.pool(), workspace, send, target.attempted_at, claimed).await
        {
            // In the middle of `DATA`: no `QUIT` (it would be read as part of
            // the message) and no end of data. The connection is dropped,
            // and a server takes nothing from a message that never ended.
            submission.abandon();
            return Submitted::Claimed(err);
        }
        target.unclear = true;
        target.attempted_at = Some(claimed);
        match submission.message(wire).await {
            Ok(()) => {
                submission.quit().await;
                Submitted::Sent
            }
            Err(Failed::Unclear) => Submitted::Failed(Failed::Unclear),
            Err(failed) => {
                submission.quit().await;
                // A definite answer: this attempt did not send it.
                if let Err(err) = clear_unclear(ctx.pool(), workspace, send, Some(claimed)).await {
                    return Submitted::Claimed(err);
                }
                target.unclear = false;
                Submitted::Failed(failed)
            }
        }
    }

    /// The server did not take the message (nothing was sent): try again
    /// later, or fail the send for good.
    async fn not_taken(
        &self,
        ctx: &mut JobContext,
        target: &Target,
        (workspace, send, stream, connection): (WorkspaceId, SendId, StreamId, ConnectionId),
        failed: Failed,
    ) -> Result<(), JobError> {
        match failed {
            Failed::Retry(code) => {
                tracing::info!(%workspace, %send, code, "the SMTP server did not take the message now");
                Err(JobError::retry_after(
                    "the SMTP server did not take the message; trying again later",
                    RETRY_WAIT,
                ))
            }
            Failed::Refused(code) => {
                tracing::info!(%workspace, %send, code, "the SMTP server refused the message");
                finish_failed(ctx, workspace, send, target, "rejected").await
            }
            Failed::TooLarge => {
                tracing::info!(%workspace, %send, "the message is larger than the SMTP server takes");
                finish_failed(ctx, workspace, send, target, "too_large").await
            }
            Failed::Unsafe(code) => {
                tracing::warn!(%workspace, %send, code, "the SMTP server cannot be used safely; not sent");
                finish_failed(ctx, workspace, send, target, "unavailable").await
            }
            Failed::LoginRefused => {
                // Counted with the stream's runs' and IDLE's refusals.
                let mut tx = ctx.pool().begin_for(workspace).await?;
                let refusals = health::count_login_refusal(&mut tx, connection, stream).await?;
                tx.commit().await?;
                if refusals >= LOGIN_REFUSALS {
                    return self
                        .reauth(
                            ctx,
                            workspace,
                            send,
                            target,
                            connection,
                            "imap_login_refused",
                        )
                        .await;
                }
                Err(JobError::retry_after(
                    "the SMTP server refused the sign-in; trying again later",
                    RETRY_WAIT,
                ))
            }
            // Handled by the caller.
            Failed::Unclear => finish_failed(ctx, workspace, send, target, "unavailable").await,
        }
    }

    /// The connection needs the member (`reason`): say so, and fail the
    /// send `needs_reauth`.
    async fn reauth(
        &self,
        ctx: &mut JobContext,
        workspace: WorkspaceId,
        send: SendId,
        target: &Target,
        connection: ConnectionId,
        reason: &'static str,
    ) -> Result<(), JobError> {
        let state = ctx.state().clone();
        if let Some(granted_by) = target.granted_by {
            health::needs_reauth(
                ctx.tx().await?,
                &state,
                workspace,
                connection,
                granted_by,
                reason,
                self.deps.clock.now(),
            )
            .await?;
        }
        finish_failed(ctx, workspace, send, target, "needs_reauth").await
    }

    /// What the job records of a message the SMTP server took: indexed
    /// under the key the reader gives its copy, as a read copy in Sent the
    /// mailbox sent, and its copy to append (or the server's own).
    fn sent(
        &self,
        target: &Target,
        document: &OutgoingMessage,
        copy: Vec<u8>,
        smtp_host: &str,
        now: DateTime<Utc>,
    ) -> Sent {
        let cursor: Cursor = target
            .cursor
            .clone()
            .and_then(|c| serde_json::from_value(c).ok())
            .unwrap_or_default();
        let sent_copy = if self.deps.imap.keeps_sent_copy(smtp_host) {
            SentCopy::Provider
        } else {
            SentCopy::Pending
        };
        let Some(key) = message_key(&clean(&document.rfc_message_id)) else {
            // No key the reader could find it by: the read brings the copy.
            return Sent {
                id: None,
                thread_key: None,
                fetched: None,
                copy: None,
            };
        };
        let tokens = state::tokens([(Role::Sent, "", true, false)], true);
        let mut flags = state::flags(&tokens, &cursor);
        flags.provider_stream = state::provider_stream(None, &tokens, &cursor);
        Sent {
            id: Some(key.clone()),
            thread_key: None,
            fetched: Some(Fetched {
                id: key,
                thread_key: None,
                label_ids: tokens,
                internal_date: now,
                raw: copy,
                flags,
                receiver: Receiver::Imap,
            }),
            copy: Some(sent_copy),
        }
    }
}

/// The envelope recipients of `document`: its To, Cc and Bcc, each an
/// address (`None` when one is not), once each whatever its case.
fn envelope(document: &OutgoingMessage) -> Option<Vec<lettre::Address>> {
    let mut seen = BTreeSet::new();
    let mut to = Vec::new();
    for recipient in document.to.iter().chain(&document.cc).chain(&document.bcc) {
        let address = recipient.address.trim();
        let parsed = mailbox(address)?;
        if seen.insert(address.to_lowercase()) {
            to.push(parsed);
        }
    }
    Some(to)
}

/// `address` as an SMTP mailbox: an address `lettre` accepts, with no
/// control character or white space anywhere (not even in a quoted local
/// part), so it can never end a command line or start another.
fn mailbox(address: &str) -> Option<lettre::Address> {
    if address.is_empty() || address.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    lettre::Address::from_str(address).ok()
}

/// The message as sent (without `Bcc`) and as kept in Sent (with it), both
/// at most `max` bytes; `None` when larger.
async fn messages(
    ctx: &JobContext,
    workspace: WorkspaceId,
    document: &OutgoingMessage,
    max: usize,
) -> Result<Option<(Vec<u8>, Vec<u8>)>, JobError> {
    // What the blobs say they hold, before any is read: base64 grows
    // attachments by a third.
    let stated = document
        .attachments
        .iter()
        .map(|a| usize::try_from(a.blob.size).unwrap_or(usize::MAX))
        .fold(
            document.text.len() + document.html.as_ref().map_or(0, String::len),
            usize::saturating_add,
        );
    if stated.saturating_mul(4) / 3 > max {
        return Ok(None);
    }
    let mut attachments = Vec::with_capacity(document.attachments.len());
    let mut total = document.text.len() + document.html.as_ref().map_or(0, String::len);
    for attachment in &document.attachments {
        let Some(bytes) = read_blob(ctx, workspace, &attachment.blob, max).await? else {
            return Ok(None);
        };
        total = total.saturating_add(bytes.len());
        if total.saturating_mul(4) / 3 > max {
            return Ok(None);
        }
        attachments.push(bytes);
    }
    let wire = compose::rfc5322(
        &OutgoingMessage {
            bcc: Vec::new(),
            ..document.clone()
        },
        attachments.clone(),
    );
    let copy = compose::rfc5322(document, attachments);
    Ok((wire.len() <= max && copy.len() <= MAX_MESSAGE_BYTES).then_some((wire, copy)))
}

/// What a send's budgets are keyed by.
struct Keys<'a> {
    /// The keyed digest of the mailbox login.
    login: &'a str,
    /// The keyed digest of the SMTP host.
    host: &'a str,
    workspace: WorkspaceId,
    /// The host is a big provider's ([`imap::SMTP_PROVIDER_HOSTS`]).
    provider: bool,
}

/// The budgets one send of `recipients` recipients takes: the login's
/// messages an hour and recipients a day, the workspace's messages an hour,
/// and on a host that is not a big provider's (those keep their own limits
/// per account) the host's messages an hour, of which one workspace takes
/// at most half.
fn budgets(config: &crate::config::ImapConfig, keys: &Keys<'_>, recipients: u32) -> Vec<Budget> {
    let mut budgets = vec![
        (
            format!("smtp:login:{}", keys.login),
            Rate::per_hour(config.send_per_hour),
            1,
        ),
        (
            format!("smtp:recipients:{}", keys.login),
            Rate::per_day(config.send_recipients_per_day),
            recipients,
        ),
        (
            format!("smtp:workspace:{}", keys.workspace),
            Rate::per_hour(config.send_workspace_per_hour),
            1,
        ),
    ];
    if !keys.provider {
        budgets.push((
            format!("smtp:host:{}:{}", keys.host, keys.workspace),
            Rate::per_hour((config.send_host_per_hour / 2).max(1)),
            1,
        ));
        budgets.push((
            format!("smtp:host:{}", keys.host),
            Rate::per_hour(config.send_host_per_hour),
            1,
        ));
    }
    budgets
}

/// Take `cost` of each of `budgets`, in order; none when one is short (those
/// taken are given back): how long until there is enough.
async fn take_all(pool: &PgPool, budgets: &[Budget]) -> Result<Result<(), Duration>, JobError> {
    for (n, (key, rate, cost)) in budgets.iter().enumerate() {
        if let Err(wait) = schedule::take(pool, key, *cost, *rate).await? {
            give_back(pool, &budgets[..n]).await;
            return Ok(Err(wait));
        }
    }
    Ok(Ok(()))
}

/// Give back what `budgets` cost (nothing was sent with it); best effort.
async fn give_back(pool: &PgPool, budgets: &[Budget]) {
    for (key, rate, cost) in budgets {
        if let Err(err) = schedule::settle(pool, key, *cost, 0, *rate).await {
            tracing::warn!(
                error = %campbooks_kernel::redact::error(&err),
                "could not give back a sending budget"
            );
        }
    }
}

/// A header-like value with every control character turned into a space,
/// as the MIME builder writes it.
fn clean(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use campbooks_kernel::blobs::BlobRef;

    use super::*;
    use crate::{
        contract::{Address, OutgoingAttachment},
        domain::model::InboxId,
    };

    fn document() -> OutgoingMessage {
        let person = |name: Option<&str>, address: &str| Address {
            name: name.map(ToOwned::to_owned),
            address: address.to_owned(),
        };
        OutgoingMessage {
            version: 1,
            inbox: InboxId::new(),
            rfc_message_id: "s1@bakery.example".into(),
            from: person(Some("Ana"), "ana@bakery.example"),
            to: vec![person(Some("Bea"), "bea@example.com")],
            cc: vec![person(None, "BEA@example.com")],
            bcc: vec![person(None, "archive@example.org")],
            subject: "Re: Order".into(),
            date: Utc::now(),
            in_reply_to: None,
            references: vec![],
            thread_key: None,
            text: "Done.".into(),
            html: None,
            attachments: vec![OutgoingAttachment {
                filename: "a.pdf".into(),
                content_type: "application/pdf".into(),
                blob: BlobRef {
                    hash: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
                        .parse()
                        .unwrap(),
                    size: 9,
                    content_type: "application/pdf".into(),
                },
            }],
        }
    }

    #[test]
    fn the_envelope_holds_every_recipient_once() {
        let to = envelope(&document()).unwrap();
        let named: Vec<String> = to.iter().map(ToString::to_string).collect();
        assert_eq!(named, ["bea@example.com", "archive@example.org"]);
    }

    #[test]
    fn a_recipient_that_is_not_an_address_refuses_the_envelope() {
        for bad in [
            "bea@example.com\r\nRCPT TO:<evil@example.org>",
            "bea example.com",
            "not an address",
            "\"x\r\n RCPT TO:<evil@example.org>\"@example.com",
            "",
        ] {
            let mut document = document();
            document.to[0].address = bad.to_owned();
            assert!(envelope(&document).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn a_big_provider_keeps_its_own_limits_and_a_small_host_is_shared() {
        let config = crate::config::ImapConfig::default();
        let workspace = WorkspaceId::new();
        let keys = |provider| Keys {
            login: "l1",
            host: "h1",
            workspace,
            provider,
        };
        let named = |budgets: Vec<Budget>| -> Vec<(String, u32)> {
            budgets
                .into_iter()
                .map(|(key, _, cost)| (key, cost))
                .collect()
        };
        let small = named(budgets(&config, &keys(false), 3));
        assert_eq!(
            small,
            [
                ("smtp:login:l1".to_owned(), 1),
                ("smtp:recipients:l1".to_owned(), 3),
                (format!("smtp:workspace:{workspace}"), 1),
                (format!("smtp:host:h1:{workspace}"), 1),
                ("smtp:host:h1".to_owned(), 1),
            ]
        );
        // One workspace takes at most half of a small host's hour.
        let share = budgets(&config, &keys(false), 1)[3].1;
        let host = budgets(&config, &keys(false), 1)[4].1;
        assert!((share.per_second * 2.0 - host.per_second).abs() < 1e-9);
        // A big provider's host has no instance-wide cap.
        let provider = named(budgets(&config, &keys(true), 3));
        assert_eq!(provider.len(), 3);
        assert!(
            provider
                .iter()
                .all(|(key, _)| !key.starts_with("smtp:host:"))
        );
    }

    #[test]
    fn the_key_is_the_readers() {
        assert_eq!(
            message_key(&clean("s1@bakery.example")),
            message_key("<s1@bakery.example>")
        );
        assert_eq!(
            message_key(&clean("s1@bakery\u{1}.example")),
            message_key("<s1@bakery .example>")
        );
    }
}
