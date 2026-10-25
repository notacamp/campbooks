//! Sending through Microsoft Graph (campbooks-cloud#317).
//!
//! The message goes out with `POST /me/sendMail` as the RFC 5322 message
//! Gmail is sent (`Message-ID`, `In-Reply-To`, `References`, Bcc in the
//! headers, which Exchange strips on delivery), base64. Graph sends as the
//! signed-in mailbox and files the copy in Sent Items itself: Campbooks
//! writes none. The From is the connection's own address, and only once that
//! address is verified (Graph's `mail` can be set by a tenant's admin; only
//! `xms_edov` proves it); a document from any other address, or with a
//! recipient that is not an address, is not sent. A message whose base64 is
//! over Graph's 4 MB is `too_large` before any call.
//!
//! `sendMail` has no idempotency key and answers 202 with no id, so the
//! attempt is committed before the call, and an attempt that may have gone
//! out (a timeout, a 5xx) is looked up in Sent Items by its
//! `internetMessageId` first: found, it is sent; otherwise it is looked for
//! again, and it is never sent again (`unknown_outcome` once no attempt is
//! left), since Graph files the copy asynchronously and a miss proves
//! nothing. After a 202 the copy is looked up once: found, the message is
//! indexed and announced under its Graph id, so the next sync knows it;
//! not yet there, the next sync brings it.

use campbooks_kernel::{JobContext, JobError, db::BeginFor as _};
use chrono::{DateTime, Utc};

use super::{
    Attempt, SendMessage, Sent, Target, UNCLEAR_WAIT, clear_unclear, finish_failed, message_bytes,
    record_attempt, unsent, via_imap::mailbox,
};
use crate::{
    contract::OutgoingMessage,
    domain::{
        model::{ConnectionId, StreamId},
        schedule::{self, Rate},
    },
    mail::{Fetched, Receiver},
    microsoft::{
        graph::MessageState,
        graph_write::{MAX_SEND_BODY, SendAnswer, SentCopy, SentLookup},
        state::{self, Cursor},
    },
    provider::ProviderError,
};

/// Graph requests one attempt may make: a look-up, the send, the look-up
/// of its copy.
const REQUESTS_PER_SEND: u32 = 3;

/// The length of the base64 of `bytes` bytes.
const fn base64_len(bytes: usize) -> usize {
    bytes.div_ceil(3).saturating_mul(4)
}

impl SendMessage {
    /// An attempt through Microsoft Graph.
    #[allow(
        clippy::too_many_lines,
        reason = "one attempt read top to bottom: look-up, checks, claim, send, copy"
    )]
    pub(super) async fn attempt_microsoft(
        &self,
        ctx: &mut JobContext,
        mut target: Target,
        attempt: Attempt<'_>,
    ) -> Result<(), JobError> {
        let Attempt {
            workspace,
            send,
            stream,
            token,
            document,
            give_up,
            stands,
            now,
        } = attempt;
        let (Some(client), Some(connection)) = (self.deps.microsoft.clone(), target.connection)
        else {
            return finish_failed(ctx, workspace, send, &target, "not_connected").await;
        };
        self.take_microsoft_budget(ctx, connection).await?;

        // An earlier attempt may have reached Graph: look before anything
        // else, and never send it again.
        if target.unclear {
            return match client.sent_copy(token, &document.rfc_message_id).await {
                Ok(SentLookup::Found(copy)) => {
                    tracing::info!(%workspace, %send, "an earlier attempt was sent");
                    // Indexed only while the send stands: these bytes are
                    // the recorded document's.
                    let raw = if stands {
                        message_bytes(ctx, workspace, document).await?
                    } else {
                        None
                    };
                    let sent = as_sent(&target, copy, raw, now);
                    self.finish_sent(ctx, workspace, send, &target, stream, sent, now)
                        .await
                }
                Ok(SentLookup::Missing | SentLookup::Several)
                    if give_up || ctx.is_last_attempt() =>
                {
                    // `target.unclear`: the send ends `unknown_outcome`.
                    tracing::warn!(%workspace, %send, "an earlier attempt may have been sent; not sending again");
                    finish_failed(ctx, workspace, send, &target, "unavailable").await
                }
                Ok(SentLookup::Missing | SentLookup::Several) => Err(JobError::retry_after(
                    "an earlier attempt is not in Sent Items yet; looking again later",
                    UNCLEAR_WAIT,
                )),
                Err(_) => Err(JobError::retry_after(
                    "looking up an earlier attempt failed",
                    UNCLEAR_WAIT,
                )),
            };
        }
        if give_up || !stands {
            return unsent(ctx, workspace, send, &target, stands).await;
        }

        // From the mailbox's verified address only, to recipients that are
        // addresses.
        if let Some(code) = refusal(ctx, workspace, stream, document).await? {
            tracing::warn!(%workspace, %send, code, "a Microsoft send was refused before Graph; not sent");
            return finish_failed(ctx, workspace, send, &target, "rejected").await;
        }
        let Some(raw) = message_bytes(ctx, workspace, document).await? else {
            return finish_failed(ctx, workspace, send, &target, "too_large").await;
        };
        if base64_len(raw.len()) > MAX_SEND_BODY {
            tracing::info!(%workspace, %send, "the message is larger than Graph takes; not sent");
            return finish_failed(ctx, workspace, send, &target, "too_large").await;
        }

        // From here until Graph answers, the message may be out.
        record_attempt(ctx.pool(), workspace, send, target.attempted_at, now).await?;
        target.unclear = true;
        match client.send_mime(token, &raw).await {
            Ok(SendAnswer::Accepted) => {
                // Graph filed the copy in Sent Items; it may not be there yet.
                let sent = match client.sent_copy(token, &document.rfc_message_id).await {
                    Ok(SentLookup::Found(copy)) => as_sent(&target, copy, Some(raw), now),
                    Ok(SentLookup::Missing | SentLookup::Several) | Err(_) => {
                        tracing::info!(%workspace, %send, "the sent message will come with the next sync");
                        Sent {
                            id: None,
                            thread_key: None,
                            fetched: None,
                            copy: None,
                        }
                    }
                };
                self.finish_sent(ctx, workspace, send, &target, stream, sent, now)
                    .await
            }
            Ok(SendAnswer::TooLarge) => {
                // A definite answer: not sent.
                clear_unclear(ctx.pool(), workspace, send, Some(now)).await?;
                target.unclear = false;
                finish_failed(ctx, workspace, send, &target, "too_large").await
            }
            Err(ProviderError::Unavailable(_)) => {
                // Graph may have it: look it up before anything else.
                tracing::warn!(%workspace, %send, "send ended unclearly");
                Err(JobError::retry_after(
                    "the send ended unclearly; looking it up next time",
                    UNCLEAR_WAIT,
                ))
            }
            Err(err) => {
                // A definite answer (a refusal, throttling): not sent.
                clear_unclear(ctx.pool(), workspace, send, Some(now)).await?;
                target.unclear = false;
                self.refused(ctx, workspace, send, &target, &err).await
            }
        }
    }

    /// Take a send's requests from the Microsoft connection's budget, or
    /// wait.
    async fn take_microsoft_budget(
        &self,
        ctx: &JobContext,
        connection: ConnectionId,
    ) -> Result<(), JobError> {
        let rate = Rate::per_minute(self.deps.config.microsoft.adapter().requests_per_minute);
        let key = schedule::microsoft_budget(connection);
        match schedule::take(ctx.pool(), &key, REQUESTS_PER_SEND, rate).await? {
            Ok(()) => Ok(()),
            Err(wait) => Err(JobError::retry_after(
                "waiting for the provider budget",
                wait,
            )),
        }
    }
}

/// Why `document` may not go out through `stream`'s connection (a stable
/// code, for the log), or `None` when it may: the connection's address is
/// verified, the document is from it, and it has recipients, each an
/// address.
async fn refusal(
    ctx: &JobContext,
    workspace: campbooks_kernel::tenancy::WorkspaceId,
    stream: StreamId,
    document: &OutgoingMessage,
) -> Result<Option<&'static str>, JobError> {
    let mut tx = ctx.pool().begin_for(workspace).await?;
    let mailbox_address: Option<(Option<String>, bool)> = sqlx::query_as(
        "SELECT c.address, c.address_verified
           FROM connectors.streams st
           JOIN connectors.connections c ON c.id = st.connection_id
          WHERE st.id = $1",
    )
    .bind(stream)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some((Some(address), verified)) = mailbox_address else {
        return Ok(Some("no_address"));
    };
    if !verified {
        return Ok(Some("address_not_verified"));
    }
    if !document
        .from
        .address
        .trim()
        .eq_ignore_ascii_case(address.trim())
    {
        return Ok(Some("another_from"));
    }
    let recipients: Vec<&str> = document
        .to
        .iter()
        .chain(&document.cc)
        .chain(&document.bcc)
        .map(|a| a.address.trim())
        .collect();
    if recipients.is_empty() {
        return Ok(Some("no_recipient"));
    }
    if recipients.iter().any(|a| mailbox(a).is_none()) {
        return Ok(Some("recipient_not_an_address"));
    }
    Ok(None)
}

/// What the job records of a message Graph sent, whose copy in Sent Items
/// is `copy`: indexed under the copy's id as a read message in Sent Items
/// (when its bytes, `raw`, are given), so the next sync knows it.
fn as_sent(target: &Target, copy: SentCopy, raw: Option<Vec<u8>>, now: DateTime<Utc>) -> Sent {
    let cursor: Cursor = target
        .cursor
        .clone()
        .and_then(|c| serde_json::from_value(c).ok())
        .unwrap_or_default();
    let folder = copy
        .folder
        .clone()
        .or_else(|| {
            cursor
                .folder_of_role(crate::imap::folders::Role::Sent)
                .map(|f| f.id.clone())
        })
        .unwrap_or_default();
    let state_now = MessageState {
        folder: Some(folder.clone()),
        conversation: copy.conversation.clone(),
        received: Some(now),
        read: true,
        draft: false,
        flagged: false,
        categories: Vec::new(),
    };
    let mut tokens = state::tokens(&cursor, &folder, &state_now);
    // It is in Sent Items, whatever the cursor knows of that folder.
    let sent_role = crate::imap::folders::Role::Sent.token();
    if !tokens.iter().any(|t| t == sent_role) {
        tokens.push(sent_role.to_owned());
        tokens.sort();
    }
    let flags = state::flags(&tokens, &cursor);
    Sent {
        id: Some(copy.id.clone()),
        thread_key: copy.conversation.clone(),
        copy: None,
        fetched: raw.map(|raw| Fetched {
            id: copy.id,
            thread_key: copy.conversation,
            label_ids: tokens,
            internal_date: now,
            raw,
            flags,
            receiver: Receiver::Microsoft,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_s_limit_is_on_the_base64() {
        assert_eq!(base64_len(0), 0);
        assert_eq!(base64_len(1), 4);
        assert_eq!(base64_len(3), 4);
        assert_eq!(base64_len(4), 8);
        assert!(base64_len(3_000_000) <= MAX_SEND_BODY);
        assert!(base64_len(3_000_001) > MAX_SEND_BODY);
    }
}
