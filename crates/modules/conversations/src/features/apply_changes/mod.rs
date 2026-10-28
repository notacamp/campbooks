//! `conversations.apply_changes` — apply a batch of message changes to one
//! inbox (see [`crate::ingest`]).
//!
//! In three steps, so slow work stays out of the job's transaction:
//!
//! 1. read the batch and find which messages are new (a short read);
//! 2. for each new message, store its participants document and ask the
//!    rules strategy, the facts (blob reads and writes, no transaction);
//! 3. in the job's transaction, apply every change in order: insert and
//!    thread new messages, update flags, labels and provider stream of known
//!    ones (a provider stream that changed is a move in the member's mail
//!    client, which the conversation follows), remove deleted ones, record
//!    the rules' answers, and publish the events.
//!
//! Batches of one inbox are enqueued with the same ordering key, so they
//! apply one at a time, in order.

mod threading;

#[cfg(test)]
mod mirror_tests;
#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use campbooks_ai::decide::{
    AnswerValue, Question,
    signals::{
        email::{subject_template, triage, triage_questions},
        facts::{dmarc_refused, sender_authenticated, sender_checks_failed},
    },
    strategy::{Decided, Strategy},
    taught::{Condition, Facts, Outcome, TaughtRule, evaluate},
};
use campbooks_kernel::{
    AppState, EventBus, JobContext, JobError, JobHandler,
    blobs::{BlobError, BlobOwner, BlobRef},
    db::BeginFor as _,
    jobs::EnqueueOptions,
    tenancy::WorkspaceId,
};
use serde_json::json;
use sqlx::{PgConnection, Postgres, Transaction};

use crate::{
    documents::{PARTICIPANTS_CONTENT_TYPE, ParticipantsDocument},
    domain::{
        blobs::{cut, read_bytes, snippet, words},
        conversation::{Changed, recount},
        decide::{ENVELOPE_BODY_BYTES, ENVELOPE_BUDGET, MessageParts, Source, envelope},
        decisions::Recorded,
        needs::{self, NeedSettings},
        plain_text::Quotes,
        policy::Policy,
        redact, rules,
        streams::{self, Inbound, sort_first_inbound},
        tracked,
    },
    events::{Change, MESSAGE_RECORD, MessageFacts, MessageReceived, MessageSent, Origin},
    ingest::{
        ApplyChangesPayload, BATCH_VERSION, MessageChange, MessageChangeBatch, NormalizedMessage,
        ProviderLabel,
    },
    model::{
        Address, AttachmentId, BlobPointer, Channel, ConversationId, Direction, InboxId, LabelId,
        MessageId, Stream, StreamSource,
    },
    tracking,
};

/// A message's stored provider stream when the provider has it in none of
/// the four streams (Gmail's Social and Forums tabs, several stream marks).
const NO_STREAM: &str = "none";

/// Largest batch document read, in bytes.
pub const MAX_BATCH_BYTES: usize = 16 * 1024 * 1024;

/// Characters of a message's snippet.
pub const SNIPPET_CHARS: usize = 200;

/// Longest `Content-ID` stored, in characters (the `MessageChangeBatch` v1
/// limit); a longer one is dropped, never cut (a cut id could name another
/// part).
pub const MAX_CONTENT_ID_CHARS: usize = 255;

/// Record type of an attachment, for blob references.
pub const ATTACHMENT_RECORD: &str = "conversations.attachment";
/// Record type holding a pending change batch (owner id: its inbox), so the
/// blob sweep keeps it while its job waits or retries.
pub const BATCH_RECORD: &str = "conversations.change_batch";

/// Applies change batches, sorting new conversations with the rules
/// strategy the module was composed with.
pub struct ApplyChanges {
    rules: Arc<dyn Strategy>,
    policy: Policy,
    needs: NeedSettings,
}

impl ApplyChanges {
    /// A handler sorting with `rules`, acting on their answers as `policy`
    /// allows, and raising needs as `needs` says.
    pub fn new(rules: Arc<dyn Strategy>, policy: Policy, needs: NeedSettings) -> Self {
        Self {
            rules,
            policy,
            needs,
        }
    }
}

/// The batch `payload` points at, checked: within the size limit, parsing,
/// of the supported version and for the payload's inbox.
async fn read_batch(
    state: &AppState,
    ws: &str,
    payload: &ApplyChangesPayload,
) -> Result<MessageChangeBatch, JobError> {
    let (bytes, cut) = read_bytes(&state.blobs, ws, &payload.batch, MAX_BATCH_BYTES).await?;
    if cut {
        return Err(JobError::fatal("the change batch is larger than the limit"));
    }
    let batch: MessageChangeBatch = serde_json::from_slice(&bytes).map_err(|e| {
        JobError::fatal(format!(
            "the change batch does not parse: {}",
            redact::json(&e)
        ))
    })?;
    if batch.version != BATCH_VERSION {
        return Err(JobError::fatal(format!(
            "change batch version {} is not supported (expected {BATCH_VERSION})",
            batch.version
        )));
    }
    if batch.inbox != payload.inbox {
        return Err(JobError::fatal("the change batch names another inbox"));
    }
    Ok(batch)
}

impl JobHandler for ApplyChanges {
    type Payload = ApplyChangesPayload;
    const NAME: &'static str = "conversations.apply_changes";
    const QUEUE: &'static str = "sync";

    async fn run(
        &self,
        ctx: &mut JobContext,
        payload: ApplyChangesPayload,
    ) -> Result<(), JobError> {
        let workspace = ctx
            .workspace()
            .and_then(|w| w.parse::<WorkspaceId>().ok())
            .ok_or_else(|| JobError::fatal("apply_changes enqueued outside a workspace"))?;
        let state = ctx.state().clone();
        let ws = workspace.to_string();

        if closed(&state, workspace, payload.inbox).await? {
            // Deleted with its connection, or kept read-only: nothing syncs
            // into it any more. The batch is let go.
            let tx = ctx.tx().await?;
            state
                .blobs
                .detach(
                    tx,
                    &ws,
                    &payload.batch.hash,
                    &BlobOwner::new(BATCH_RECORD, payload.inbox),
                )
                .await?;
            tracing::info!(inbox = %payload.inbox, "change batch dropped: the inbox is closed");
            return Ok(());
        }

        let batch = read_batch(&state, &ws, &payload).await?;

        let known = survey(&state, workspace, &batch).await?;
        let prepared = self.prepare(&state, &ws, &batch, known).await?;

        let events = state.events.clone();
        let tx = ctx.tx().await?;
        // Asked again in the transaction that writes, holding the inbox
        // `FOR SHARE` until it commits: a disconnect deleting or freezing it
        // waits for this batch, or this batch sees it is closed.
        let open = sqlx::query_scalar!(
            r#"SELECT detached_at IS NULL AS "open!" FROM conversations.inboxes
                WHERE id = $1 FOR SHARE"#,
            batch.inbox as InboxId
        )
        .fetch_optional(&mut **tx)
        .await?;
        if open != Some(true) {
            state
                .blobs
                .detach(
                    tx,
                    &ws,
                    &payload.batch.hash,
                    &BlobOwner::new(BATCH_RECORD, payload.inbox),
                )
                .await?;
            tracing::info!(inbox = %payload.inbox, "change batch dropped: the inbox closed meanwhile");
            return Ok(());
        }
        let mut apply = Apply {
            state: &state,
            events: &events,
            workspace,
            ws: &ws,
            rules: self.rules.name(),
            policy: &self.policy,
            needs: &self.needs,
            inbox: batch.inbox,
            backfill: batch.backfill,
            mirroring: BTreeMap::new(),
            touched: BTreeMap::new(),
            new_in: BTreeSet::new(),
            taught: None,
            correspondents: BTreeSet::new(),
            tally: Tally::default(),
        };
        for change in &batch.changes {
            match change {
                MessageChange::Upsert { message } => {
                    apply
                        .upsert(tx, message, prepared.get(&message.provider_message_id))
                        .await?;
                }
                MessageChange::Remove {
                    provider_message_id,
                } => apply.remove(tx, provider_message_id).await?,
            }
        }
        apply.finish(tx).await?;
        // Mail stored before subject templates existed gets them in the
        // background, a page at a time.
        if rules::templates_missing(tx, batch.inbox).await? {
            crate::features::fill_templates::enqueue(tx, &ws, batch.inbox).await?;
        }
        // Applied: the batch is no longer needed (the sweep removes it once
        // nothing else holds it).
        state
            .blobs
            .detach(
                tx,
                &ws,
                &payload.batch.hash,
                &BlobOwner::new(BATCH_RECORD, payload.inbox),
            )
            .await?;
        Ok(())
    }
}

/// Which of the batch's messages the inbox already has; retries while the
/// inbox does not exist.
async fn survey(
    state: &AppState,
    workspace: WorkspaceId,
    batch: &MessageChangeBatch,
) -> Result<HashSet<String>, JobError> {
    let ids: Vec<&str> = batch
        .changes
        .iter()
        .filter_map(|change| match change {
            MessageChange::Upsert { message } => Some(message.provider_message_id.as_str()),
            MessageChange::Remove { .. } => None,
        })
        .collect();
    let mut tx = state.db.begin_for(workspace).await?;
    sqlx::query_scalar!(
        "SELECT id FROM conversations.inboxes WHERE id = $1",
        batch.inbox as InboxId
    )
    .fetch_optional(&mut *tx)
    .await?
    // The connectors module publishes the batch's inbox (capability granted)
    // before its first batch; until this module has created it, wait.
    .ok_or_else(|| JobError::retry("the change batch's inbox does not exist yet"))?;
    let known = sqlx::query_scalar!(
        "SELECT provider_message_id FROM conversations.messages
          WHERE inbox_id = $1 AND provider_message_id = ANY($2)",
        batch.inbox as InboxId,
        &ids as &[&str],
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(known.into_iter().collect())
}

/// Whether `inbox` was deleted with its connection or kept read-only.
async fn closed(
    state: &AppState,
    workspace: WorkspaceId,
    inbox: InboxId,
) -> Result<bool, JobError> {
    let mut tx = state.db.begin_for(workspace).await?;
    let closed = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM conversations.removed_inboxes WHERE inbox_id = $1)
               OR EXISTS (SELECT 1 FROM conversations.inboxes
                           WHERE id = $1 AND detached_at IS NOT NULL) AS "closed!""#,
        inbox as InboxId
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(closed)
}

/// What step 2 found out about a new message.
struct Prepared {
    participants: BlobRef,
    snippet: String,
    /// The rules' answers to the triage questions; empty for outbound mail.
    answers: Vec<Option<Decided>>,
}

impl ApplyChanges {
    /// Step 2, for every upsert of a message the inbox will not have when
    /// the upsert is applied.
    async fn prepare(
        &self,
        state: &AppState,
        ws: &str,
        batch: &MessageChangeBatch,
        mut present: HashSet<String>,
    ) -> Result<HashMap<String, Prepared>, JobError> {
        let questions = triage_questions();
        let mut prepared = HashMap::new();
        for change in &batch.changes {
            let message = match change {
                MessageChange::Remove {
                    provider_message_id,
                } => {
                    present.remove(provider_message_id);
                    continue;
                }
                MessageChange::Upsert { message } => message,
            };
            if !present.insert(message.provider_message_id.clone())
                || prepared.contains_key(&message.provider_message_id)
            {
                continue;
            }
            // Its text part, else its HTML read as text (campbooks-cloud#218).
            let body = cut(
                words(
                    &state.blobs,
                    ws,
                    (message.text.as_ref(), message.html.as_ref()),
                    ENVELOPE_BUDGET,
                    Quotes::Keep,
                )
                .await?
                .0,
                ENVELOPE_BODY_BYTES,
            );
            let document = ParticipantsDocument {
                version: 1,
                from: message.from.clone(),
                to: message.to.clone(),
                cc: message.cc.clone(),
                reply_to: message.reply_to.clone(),
                subject: message.subject.clone(),
            };
            let document =
                serde_json::to_vec(&document).map_err(|e| JobError::fatal(redact::json(&e)))?;
            let participants = state
                .blobs
                .put(ws, document, PARTICIPANTS_CONTENT_TYPE)
                .await?;
            let answers = if message.direction == Direction::Inbound {
                let attachments: Vec<(String, String)> = message
                    .attachments
                    .iter()
                    .map(|a| (a.filename.clone(), a.content_type.clone()))
                    .collect();
                let envelope = envelope(MessageParts {
                    from: &message.from,
                    to: &message.to,
                    cc: &message.cc,
                    reply_to: &message.reply_to,
                    subject: &message.subject,
                    headers: &message.headers,
                    attachments: &attachments,
                    body: &body,
                });
                self.rules
                    .decide(&envelope, &questions)
                    .await
                    .map_err(|e| redact::ai(&e))?
                    .answers
            } else {
                Vec::new()
            };
            prepared.insert(
                message.provider_message_id.clone(),
                Prepared {
                    participants,
                    snippet: snippet(&body, SNIPPET_CHARS),
                    answers,
                },
            );
        }
        Ok(prepared)
    }
}

/// Step 3's working state.
struct Apply<'a> {
    state: &'a AppState,
    events: &'a EventBus,
    workspace: WorkspaceId,
    ws: &'a str,
    /// The rules strategy's name, recorded with its answers.
    rules: &'a str,
    policy: &'a Policy,
    needs: &'a NeedSettings,
    inbox: InboxId,
    /// The batch was read by the inbox's first sync: its messages' streams
    /// are never mirrored.
    backfill: bool,
    /// Conversations whose received messages are to follow them at the
    /// provider, each once at the end of the batch, with the messages asked
    /// whatever they hold (a stale stream mark).
    mirroring: BTreeMap<ConversationId, BTreeSet<String>>,
    /// Conversations whose existing messages changed or went: what changed
    /// and which messages, announced at the end.
    touched: BTreeMap<ConversationId, (BTreeSet<Change>, BTreeSet<String>)>,
    /// Conversations that only gained new messages: recounted at the end.
    new_in: BTreeSet<ConversationId>,
    /// The inbox's taught rules, read once per batch.
    taught: Option<Vec<TaughtRule>>,
    /// Who wrote to the inbox (senders of new inbound mail) or was written
    /// to (To and Cc of new mail it sent), lower-cased: a wait on one of
    /// them in another conversation is worked out again (hearing back,
    /// campbooks-cloud#293).
    correspondents: BTreeSet<String>,
    /// What the batch changed, for its tracked event.
    tally: Tally,
}

/// What a batch changed, counted for its tracked event (campbooks-cloud#325).
#[derive(Debug, Default)]
struct Tally {
    added: usize,
    updated: usize,
    removed: usize,
    rules_acted: usize,
}

/// A stored message's mutable state.
struct Stored {
    id: MessageId,
    conversation: ConversationId,
    flags: Flags,
    provider_labels: Vec<String>,
}

/// A message's flags and mailbox state.
#[derive(Clone, Copy)]
struct Flags {
    read: bool,
    starred: bool,
    mailbox: crate::ingest::Mailbox,
}

impl Apply<'_> {
    async fn upsert(
        &mut self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        prepared: Option<&Prepared>,
    ) -> Result<(), JobError> {
        let known = sqlx::query!(
            r#"
            SELECT id AS "id: MessageId", conversation_id AS "conversation: ConversationId",
                   read, starred, in_inbox, in_spam, in_trash, provider_labels
              FROM conversations.messages
             WHERE inbox_id = $1 AND provider_message_id = $2
               FOR UPDATE
            "#,
            self.inbox as InboxId,
            message.provider_message_id,
        )
        .fetch_optional(&mut **tx)
        .await?;
        match (known, prepared) {
            (Some(known), _) => {
                let stored = Stored {
                    id: known.id,
                    conversation: known.conversation,
                    flags: Flags {
                        read: known.read,
                        starred: known.starred,
                        mailbox: crate::ingest::Mailbox {
                            in_inbox: known.in_inbox,
                            in_spam: known.in_spam,
                            in_trash: known.in_trash,
                        },
                    },
                    provider_labels: known.provider_labels,
                };
                self.update(tx, message, stored).await
            }
            (None, Some(prepared)) => self.insert(tx, message, prepared).await,
            (None, None) => Err(JobError::retry(
                "a message appeared while its batch was applied; applying again",
            )),
        }
    }

    /// Update flags, mailbox state and labels of a stored message.
    async fn update(
        &mut self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        known: Stored,
    ) -> Result<(), JobError> {
        let mut changes = BTreeSet::new();
        let was = known.flags;
        if was.read != message.read {
            changes.insert(Change::Read);
        }
        if was.starred != message.starred {
            changes.insert(Change::Starred);
        }
        if was.mailbox.in_inbox != message.mailbox.in_inbox {
            changes.insert(Change::Archived);
        }
        if was.mailbox.in_spam != message.mailbox.in_spam
            || was.mailbox.in_trash != message.mailbox.in_trash
        {
            changes.insert(Change::Messages);
        }
        let new_keys: Vec<String> = message.labels.iter().map(|l| l.key.clone()).collect();
        let old: BTreeSet<&String> = known.provider_labels.iter().collect();
        let new: BTreeSet<&String> = new_keys.iter().collect();
        if old != new {
            changes.insert(Change::Labels);
        }
        if let Some(stream) = message.provider_stream
            && self
                .provider_stream(tx, message, known.id, known.conversation, stream)
                .await?
        {
            changes.insert(Change::Stream);
        }
        if changes.is_empty() {
            return Ok(());
        }
        self.tally.updated += 1;
        sqlx::query!(
            r#"
            UPDATE conversations.messages
               SET read = $2, starred = $3, in_inbox = $4, in_spam = $5, in_trash = $6,
                   provider_labels = $7, updated_at = now()
             WHERE id = $1
            "#,
            known.id as MessageId,
            message.read,
            message.starred,
            message.mailbox.in_inbox,
            message.mailbox.in_spam,
            message.mailbox.in_trash,
            &new_keys,
        )
        .execute(&mut **tx)
        .await?;
        if changes.contains(&Change::Labels) {
            for label in message.labels.iter().filter(|l| !old.contains(&l.key)) {
                link_provider_label(tx, self.workspace, self.inbox, known.conversation, label)
                    .await?;
            }
            let removed: Vec<&String> = old.difference(&new).copied().collect();
            unlink_provider_labels(tx, known.conversation, &removed).await?;
        }
        let entry = self.touched.entry(known.conversation).or_default();
        entry.0.extend(changes);
        entry.1.insert(message.provider_message_id.clone());
        Ok(())
    }

    /// A known message's provider stream is now `now`: stored. When it
    /// changed from one said value to a stream (a move in the member's mail
    /// client), its conversation moves there as a member's move (unless
    /// someone already placed it there), the conversation's other received
    /// messages are asked to follow at the provider, and so is this one when
    /// its stream mark is stale. Returns whether the conversation moved.
    async fn provider_stream(
        &mut self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        id: MessageId,
        conversation: ConversationId,
        now: Option<Stream>,
    ) -> Result<bool, JobError> {
        // `none`: in none of the four. NULL: no upsert said (stored before
        // provider streams were carried), so a first value is no move.
        let said = now.map_or(NO_STREAM, Stream::as_str);
        let stored: Option<String> =
            sqlx::query_scalar("SELECT provider_stream FROM conversations.messages WHERE id = $1")
                .bind(id)
                .fetch_one(&mut **tx)
                .await?;
        if stored.as_deref() == Some(said) {
            return Ok(false);
        }
        sqlx::query("UPDATE conversations.messages SET provider_stream = $2 WHERE id = $1")
            .bind(id)
            .bind(said)
            .execute(&mut **tx)
            .await?;
        if stored.is_none() {
            return Ok(false);
        }
        let now = now.map(Stream::as_str);
        let (Some(now), Direction::Inbound) = (now, message.direction) else {
            return Ok(false);
        };
        // A move to where nobody placed it yet still records the member's
        // choice, so the model pass never overrides it.
        let moved = sqlx::query(
            "UPDATE conversations.conversations
                SET stream = $2, stream_source = 'member', updated_at = now()
              WHERE id = $1 AND (stream <> $2 OR stream_source IN ('default', 'rules'))",
        )
        .bind(conversation)
        .bind(now)
        .execute(&mut **tx)
        .await?
        .rows_affected()
            > 0;
        if moved || message.stream_mark_stale {
            let also = self.mirroring.entry(conversation).or_default();
            if message.stream_mark_stale {
                also.insert(message.provider_message_id.clone());
            }
        }
        Ok(moved)
    }

    /// Insert a new message, threading it into a conversation.
    async fn insert(
        &mut self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        prepared: &Prepared,
    ) -> Result<(), JobError> {
        let questions = triage_questions();
        let inbound = message.direction == Direction::Inbound;
        let placement = threading::place(tx, self.inbox, message).await?;
        let template = subject_template(&message.subject);
        let outcome = if inbound {
            self.taught_outcome(
                tx,
                message,
                prepared,
                placement.conversation,
                template.as_deref(),
            )
            .await?
        } else {
            Outcome::default()
        };
        let sort_to = if let Some(applied) = &outcome.stream {
            applied
                .value
                .parse::<Stream>()
                .ok()
                .map(|s| (s, StreamSource::Taught))
        } else if inbound {
            accepted_stream(&questions, &prepared.answers, self.policy)
                .map(|s| (s, StreamSource::Rules))
        } else {
            None
        };
        let (conversation, sorted) = self
            .conversation_for(tx, message, placement.conversation, sort_to)
            .await?;
        let id = self
            .store(tx, message, prepared, conversation, template.as_deref())
            .await?;
        if placement.conversation.is_some() && !sorted {
            self.follow_joined(tx, message, conversation).await?;
        }
        if inbound {
            let recorded = Recorded {
                message: id,
                conversation,
                source: Source::Rules,
                strategy: self.rules,
                questions: &questions,
                answers: &prepared.answers,
                policy: self.policy,
            };
            let by_signals = sorted && sort_to.is_some_and(|(_, s)| s == StreamSource::Rules);
            recorded.store(tx, self.workspace, by_signals).await?;
            if !outcome.is_empty() {
                let by_rule = sorted && sort_to.is_some_and(|(_, s)| s == StreamSource::Taught);
                let arrival = rules::Arrival {
                    workspace: *self.workspace.as_uuid(),
                    ws: self.ws,
                    inbox: self.inbox,
                    conversation,
                    message: id,
                    provider_message_id: &message.provider_message_id,
                    placed_from: by_rule.then(|| (Stream::Primary, "default".to_owned())),
                };
                let mut outcome = outcome;
                if !by_rule {
                    outcome.stream = None;
                }
                let changes =
                    rules::on_arrival(tx, &self.state.binding, self.events, &arrival, &outcome)
                        .await?;
                if !changes.is_empty() {
                    self.tally.rules_acted += 1;
                    let entry = self.touched.entry(conversation).or_default();
                    entry.0.extend(changes);
                    entry.1.insert(message.provider_message_id.clone());
                }
            }
        }
        for merged in &placement.merge {
            threading::merge(tx, *merged, conversation).await?;
            self.touched
                .entry(*merged)
                .or_default()
                .0
                .insert(Change::Removed);
            self.touched
                .entry(conversation)
                .or_default()
                .0
                .insert(Change::Messages);
        }
        self.announce(tx, message, prepared, conversation, id)
            .await?;
        self.tally.added += 1;
        self.new_in.insert(conversation);
        self.note_correspondents(message, inbound);
        Ok(())
    }

    /// Note who wrote to the inbox (`inbound`: the sender) or was written
    /// to (its To and Cc), for hearing back.
    fn note_correspondents(&mut self, message: &NormalizedMessage, inbound: bool) {
        let people: Vec<&Address> = if inbound {
            vec![&message.from]
        } else {
            message.to.iter().chain(&message.cc).collect()
        };
        self.correspondents.extend(
            people
                .into_iter()
                .map(|a| a.address.trim().to_lowercase())
                .filter(|a| !a.is_empty()),
        );
    }

    /// What the inbox's taught rules say about a new inbound message, in
    /// `found` (its conversation, when threading found one) or a new one.
    async fn taught_outcome(
        &mut self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        prepared: &Prepared,
        found: Option<ConversationId>,
        template: Option<&str>,
    ) -> Result<Outcome, JobError> {
        if self.taught.is_none() {
            self.taught = Some(rules::acting(tx, &self.state.binding, self.inbox).await?);
        }
        let all = self.taught.as_deref().unwrap_or_default();
        if all.is_empty() {
            return Ok(Outcome::default());
        }
        let (acting, stream, labels, under_review) = match found {
            Some(conversation) => {
                let acting = rules::acting_on(tx, all, conversation).await?;
                let row = sqlx::query!(
                    r#"SELECT c.stream,
                              coalesce((SELECT array_agg(cl.label_id::text)
                                          FROM conversations.conversation_labels cl
                                         WHERE cl.conversation_id = c.id), '{}') AS "labels!",
                              (EXISTS (SELECT 1 FROM conversations.needs n
                                        WHERE n.conversation_id = c.id AND n.state = 'open'
                                          AND n.reason = 'review')
                               OR EXISTS (SELECT 1 FROM conversations.decisions d
                                           WHERE d.conversation_id = c.id
                                             AND d.question = 'suspicious'
                                             AND d.value = 'true'::jsonb)) AS "under_review!"
                         FROM conversations.conversations c WHERE c.id = $1"#,
                    conversation as ConversationId,
                )
                .fetch_one(&mut **tx)
                .await?;
                (acting, row.stream, row.labels, row.under_review)
            }
            None => (
                all.to_vec(),
                Stream::Primary.as_str().to_owned(),
                Vec::new(),
                false,
            ),
        };
        let questions = triage_questions();
        // Suspicious mail, a conversation with an earlier suspicious
        // message, or one with an open Review need, is never hidden by a
        // rule (archiving would clear the Review).
        let suspicious = under_review
            || questions.iter().zip(&prepared.answers).any(|(q, a)| {
                q.id == triage::SUSPICIOUS
                    && a.as_ref()
                        .is_some_and(|d| d.answer.value == AnswerValue::Noul(true))
            });
        let sender = message.from.address.to_lowercase();
        let results = message.headers.authentication_results.as_deref();
        let impersonated = dmarc_refused(results);
        // Whether the inbox writes to them matters only for mail pretending
        // to be them ("fails its sender checks" never acts on it).
        let correspondent = impersonated && rules::corresponds(tx, self.inbox, &sender).await?;
        // Whether the member is only copied (campbooks-cloud#378): looked up
        // only when a rule asks, from the addresses alone.
        let member_not_in_to = if acting
            .iter()
            .any(|r| r.conditions.contains(&Condition::MemberNotInTo))
        {
            let own: Option<String> =
                sqlx::query_scalar("SELECT address FROM conversations.inboxes WHERE id = $1")
                    .bind(self.inbox)
                    .fetch_optional(&mut **tx)
                    .await?;
            own.is_some_and(|own| {
                let own = own.trim();
                !own.is_empty()
                    && !message
                        .to
                        .iter()
                        .any(|a| a.address.trim().eq_ignore_ascii_case(own))
            })
        } else {
            false
        };
        let facts = Facts {
            sender_address: &sender,
            subject: &message.subject,
            subject_template: template,
            list_id: message.headers.list_id.as_deref(),
            list_unsubscribe: message.headers.list_unsubscribe.as_deref(),
            precedence: message.headers.precedence.as_deref(),
            auto_submitted: message.headers.auto_submitted.as_deref(),
            labels: &labels,
            stream: &stream,
            suspicious,
            auth_failed: sender_checks_failed(&sender, results),
            impersonated,
            correspondent,
            authenticated: sender_authenticated(&sender, results),
            member_not_in_to,
        };
        Ok(evaluate(&acting, &facts))
    }

    /// The conversation a new message goes in: the one threading found
    /// (sorted by this message if it is its first inbound one), or a new
    /// one. Returns it and whether `sort_to` placed it.
    async fn conversation_for(
        &self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        found: Option<ConversationId>,
        sort_to: Option<(Stream, StreamSource)>,
    ) -> Result<(ConversationId, bool), JobError> {
        if let Some(conversation) = found {
            let sorted = match sort_to {
                Some((stream, source)) => {
                    sort_first_inbound(tx, conversation, stream, source, Inbound::Arriving).await?
                }
                None => false,
            };
            return Ok((conversation, sorted));
        }
        let (stream, source) = sort_to.unwrap_or((Stream::Primary, StreamSource::Default));
        let id = ConversationId::new();
        sqlx::query!(
            r#"
            INSERT INTO conversations.conversations
                (id, workspace_id, inbox_id, thread_key, subject, stream, stream_source,
                 last_message_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            id as ConversationId,
            self.workspace.as_uuid(),
            self.inbox as InboxId,
            message.thread_key,
            message.subject,
            stream.as_str(),
            source.as_str(),
            message.sent_at,
        )
        .execute(&mut **tx)
        .await?;
        Ok((id, sort_to.is_some()))
    }

    /// Insert the message row, its attachments and labels, and attach its
    /// blobs to it.
    async fn store(
        &self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        prepared: &Prepared,
        conversation: ConversationId,
        template: Option<&str>,
    ) -> Result<MessageId, JobError> {
        let id = MessageId::new();
        let bodies = json!({
            "raw": message.raw,
            "text": message.text,
            "html": message.html,
        });
        let recipients = json!({
            "to": message.to,
            "cc": message.cc,
            "reply_to": message.reply_to,
        });
        let categories: Vec<&str> = message.categories.iter().map(|c| c.as_str()).collect();
        let labels: Vec<&str> = message.labels.iter().map(|l| l.key.as_str()).collect();
        sqlx::query!(
            r#"
            INSERT INTO conversations.messages
                (id, workspace_id, inbox_id, conversation_id, provider_message_id, direction,
                 rfc_message_id, in_reply_to, reference_ids, sender, sender_address, recipients,
                 subject, snippet, sent_at, headers, categories, provider_labels, in_inbox,
                 in_spam, in_trash, read, starred, bodies, participants, subject_template)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                    $18, $19, $20, $21, $22, $23, $24, $25, $26)
            "#,
            id as MessageId,
            self.workspace.as_uuid(),
            self.inbox as InboxId,
            conversation as ConversationId,
            message.provider_message_id,
            message.direction.as_str(),
            message.rfc_message_id,
            message.in_reply_to,
            &message.references,
            json!(message.from),
            message.from.address.to_lowercase(),
            recipients,
            message.subject,
            prepared.snippet,
            message.sent_at,
            json!(message.headers),
            &categories as &[&str],
            &labels as &[&str],
            message.mailbox.in_inbox,
            message.mailbox.in_spam,
            message.mailbox.in_trash,
            message.read,
            message.starred,
            bodies,
            json!(prepared.participants),
            template.unwrap_or_default(),
        )
        .execute(&mut **tx)
        .await?;

        let owner = BlobOwner::new(MESSAGE_RECORD, id);
        let held = [
            Some(&prepared.participants),
            message.raw.as_ref(),
            message.text.as_ref(),
            message.html.as_ref(),
        ];
        for blob in held.into_iter().flatten() {
            self.attach(tx, blob, &owner).await?;
        }
        for (position, attachment) in (0_i32..).zip(&message.attachments) {
            let attachment_id = AttachmentId::new();
            let size = i64::try_from(attachment.blob.size).unwrap_or(i64::MAX);
            sqlx::query!(
                r#"
                INSERT INTO conversations.attachments
                    (id, workspace_id, message_id, position, filename, content_type, size,
                     inline, blob, content_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                attachment_id as AttachmentId,
                self.workspace.as_uuid(),
                id as MessageId,
                position,
                attachment.filename,
                attachment.content_type,
                size,
                attachment.inline,
                json!(attachment.blob),
                attachment
                    .content_id
                    .as_deref()
                    .filter(|id| id.chars().count() <= MAX_CONTENT_ID_CHARS),
            )
            .execute(&mut **tx)
            .await?;
            let owner = BlobOwner::new(ATTACHMENT_RECORD, attachment_id);
            self.attach(tx, &attachment.blob, &owner).await?;
        }
        for label in &message.labels {
            link_provider_label(tx, self.workspace, self.inbox, conversation, label).await?;
        }
        self.store_provider_stream(tx, message, id).await?;
        Ok(id)
    }

    /// Store where new message `id` is at the provider, and whether its
    /// stream is mirrored, when its upsert says.
    async fn store_provider_stream(
        &self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        id: MessageId,
    ) -> Result<(), JobError> {
        let Some(stream) = message.provider_stream else {
            return Ok(());
        };
        sqlx::query(
            "UPDATE conversations.messages SET provider_stream = $2, mirrors_stream = $3
              WHERE id = $1",
        )
        .bind(id)
        .bind(stream.map_or(NO_STREAM, Stream::as_str))
        .bind(self.mirrors(message))
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// A received message that joined `conversation`, already placed by
    /// someone, goes where it is at the provider too. (A placement made now
    /// asks for itself: a taught rule's on arrival, the model pass's after
    /// it.)
    async fn follow_joined(
        &mut self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        conversation: ConversationId,
    ) -> Result<(), JobError> {
        if !self.mirrors(message) {
            return Ok(());
        }
        let placed: bool = sqlx::query_scalar(
            "SELECT stream_source <> 'default' FROM conversations.conversations WHERE id = $1",
        )
        .bind(conversation)
        .fetch_one(&mut **tx)
        .await?;
        if placed {
            self.mirroring.entry(conversation).or_default();
        }
        Ok(())
    }

    /// Whether `message`'s stream is mirrored to the provider: a received
    /// message whose upsert says where it is at the provider, outside the
    /// inbox's first sync.
    fn mirrors(&self, message: &NormalizedMessage) -> bool {
        message.direction == Direction::Inbound
            && !self.backfill
            && message.provider_stream.is_some()
    }

    /// Publish `message_received` or `message_sent` for a new message.
    async fn announce(
        &self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        prepared: &Prepared,
        conversation: ConversationId,
        id: MessageId,
    ) -> Result<(), JobError> {
        let stream = sqlx::query_scalar!(
            "SELECT stream FROM conversations.conversations WHERE id = $1",
            conversation as ConversationId
        )
        .fetch_one(&mut **tx)
        .await?
        .parse::<Stream>()?;
        let facts = MessageFacts {
            workspace: self.ws.to_owned(),
            inbox: self.inbox,
            conversation,
            message: id,
            provider_message_id: message.provider_message_id.clone(),
            channel: Channel::Email,
            stream,
            sent_at: message.sent_at,
            participants: BlobPointer::from(&prepared.participants),
            text: message.text.as_ref().map(BlobPointer::from),
            attachment_count: u32::try_from(message.attachments.len()).unwrap_or(u32::MAX),
        };
        if message.direction == Direction::Inbound {
            self.events
                .publish(tx, &MessageReceived { message: facts })
                .await?;
        } else {
            self.events
                .publish(tx, &MessageSent { message: facts })
                .await?;
        }
        Ok(())
    }

    async fn attach(
        &self,
        tx: &mut Transaction<'static, Postgres>,
        blob: &BlobRef,
        owner: &BlobOwner,
    ) -> Result<(), JobError> {
        match self.state.blobs.attach(tx, self.ws, blob, owner).await {
            Ok(_) => Ok(()),
            Err(BlobError::NotStored) => Err(JobError::fatal(format!(
                "blob {} is not stored in the workspace; the connector must put it before the batch",
                blob.hash
            ))),
            Err(e) => Err(e.into()),
        }
    }

    /// Remove a message the provider no longer has.
    async fn remove(
        &mut self,
        tx: &mut Transaction<'static, Postgres>,
        provider_message_id: &str,
    ) -> Result<(), JobError> {
        let Some(gone) = sqlx::query!(
            r#"
            SELECT id AS "id: MessageId", conversation_id AS "conversation: ConversationId",
                   provider_labels
              FROM conversations.messages
             WHERE inbox_id = $1 AND provider_message_id = $2
               FOR UPDATE
            "#,
            self.inbox as InboxId,
            provider_message_id,
        )
        .fetch_optional(&mut **tx)
        .await?
        else {
            return Ok(());
        };
        let attachments = sqlx::query_scalar!(
            r#"SELECT id AS "id: AttachmentId" FROM conversations.attachments WHERE message_id = $1"#,
            gone.id as MessageId
        )
        .fetch_all(&mut **tx)
        .await?;
        sqlx::query!(
            "DELETE FROM conversations.messages WHERE id = $1",
            gone.id as MessageId
        )
        .execute(&mut **tx)
        .await?;
        let blobs = &self.state.blobs;
        blobs
            .detach_owner(tx, self.ws, &BlobOwner::new(MESSAGE_RECORD, gone.id))
            .await?;
        for attachment in attachments {
            blobs
                .detach_owner(tx, self.ws, &BlobOwner::new(ATTACHMENT_RECORD, attachment))
                .await?;
        }
        let keys: Vec<&String> = gone.provider_labels.iter().collect();
        unlink_provider_labels(tx, gone.conversation, &keys).await?;
        self.tally.removed += 1;
        let entry = self.touched.entry(gone.conversation).or_default();
        entry.0.insert(Change::Messages);
        entry.1.insert(provider_message_id.to_owned());
        Ok(())
    }

    /// Recount every conversation the batch touched and announce the
    /// changes to existing ones.
    async fn finish(self, tx: &mut Transaction<'static, Postgres>) -> Result<(), JobError> {
        // Each conversation's messages asked to follow it once, whatever
        // number of its messages the batch carried.
        for (conversation, also) in &self.mirroring {
            let also: Vec<String> = also.iter().cloned().collect();
            streams::mirror(
                &self.state.binding,
                self.events,
                tx,
                self.ws,
                *conversation,
                &also,
            )
            .await?;
        }
        for conversation in &self.new_in {
            if !self.touched.contains_key(conversation) {
                recount(tx, *conversation).await?;
            }
        }
        let mut refresh: BTreeSet<ConversationId> = self
            .new_in
            .iter()
            .chain(self.touched.keys())
            .copied()
            .collect();
        let conversations_changed = refresh.len();
        // The open waits on someone who just wrote, or was just written to,
        // in this inbox's other conversations: heard back, or nudged.
        if !self.correspondents.is_empty() {
            let addresses: Vec<&str> = self.correspondents.iter().map(String::as_str).collect();
            let waiting: Vec<ConversationId> = sqlx::query_scalar(WAITING_ON)
                .bind(self.workspace.as_uuid())
                .bind(self.inbox)
                .bind(&addresses)
                .fetch_all(&mut **tx)
                .await?;
            refresh.extend(waiting);
        }
        for (conversation, (changes, provider_ids)) in self.touched {
            Changed {
                workspace: self.ws.to_owned(),
                inbox: self.inbox,
                conversation,
                origin: Origin::Provider,
                changes: changes.into_iter().collect(),
                provider_message_ids: provider_ids.into_iter().collect(),
            }
            .publish(self.events, tx)
            .await?;
        }
        // What each conversation needs now, once its counts and state are
        // current (a conversation that is gone changes nothing).
        for conversation in refresh {
            needs::refresh(tx, self.events, self.ws, conversation, self.needs).await?;
        }
        // One summary per batch, counts only (campbooks-cloud#325): a batch
        // that changed nothing tracks nothing.
        let tally = &self.tally;
        if tally.added + tally.updated + tally.removed > 0 {
            self.state
                .tracker
                .track(
                    tx,
                    &tracking::MessageBatchApplied {
                        workspace: self.workspace,
                        inbox_id: *self.inbox.as_uuid(),
                        backfill: self.backfill,
                        messages_added: tracked::count(tally.added),
                        messages_updated: tracked::count(tally.updated),
                        messages_removed: tracked::count(tally.removed),
                        conversations_changed: tracked::count(conversations_changed),
                        rules_acted: tracked::count(tally.rules_acted),
                    },
                )
                .await?;
        }
        Ok(())
    }
}

/// The conversations of inbox `$2` (workspace `$1`) with an open wait on
/// one of `$3` (lower-cased addresses). Served by `needs_awaited_idx`.
const WAITING_ON: &str = "SELECT n.conversation_id FROM conversations.needs n \
       JOIN conversations.conversations c ON c.id = n.conversation_id \
      WHERE n.workspace_id = $1 AND n.state = 'open' AND n.reason IN ('waiting', 'nudge') \
        AND lower(n.awaited_address) = ANY($3) AND c.inbox_id = $2";

/// The stream the rules' answers may set, if the policy accepts one.
fn accepted_stream(
    questions: &[Question],
    answers: &[Option<Decided>],
    policy: &Policy,
) -> Option<Stream> {
    let recorded = Recorded {
        message: MessageId::new(),
        conversation: ConversationId::new(),
        source: crate::domain::decide::Source::Rules,
        strategy: "rules",
        questions,
        answers,
        policy,
    };
    recorded.accepted_stream()
}

/// Put the provider's label `label` on `conversation`, creating the inbox's
/// label the first time it is seen.
async fn link_provider_label(
    conn: &mut PgConnection,
    workspace: WorkspaceId,
    inbox: InboxId,
    conversation: ConversationId,
    label: &ProviderLabel,
) -> Result<(), JobError> {
    let name: String = label.name.chars().take(100).collect();
    let name = if name.trim().is_empty() {
        label.key.clone()
    } else {
        name
    };
    let label_id = sqlx::query_scalar!(
        r#"
        WITH found AS (
            SELECT id FROM conversations.labels WHERE inbox_id = $2 AND provider_key = $3
        ), named AS (
            -- A member's label of the same name becomes the provider's.
            UPDATE conversations.labels SET provider_key = $3
             WHERE inbox_id = $2 AND name = $4 AND provider_key IS NULL
               AND NOT EXISTS (SELECT 1 FROM found)
            RETURNING id
        ), created AS (
            INSERT INTO conversations.labels (id, workspace_id, inbox_id, name, provider_key)
            SELECT $1, $5, $2, $4, $3
             WHERE NOT EXISTS (SELECT 1 FROM found) AND NOT EXISTS (SELECT 1 FROM named)
            ON CONFLICT DO NOTHING
            RETURNING id
        )
        SELECT id AS "id!: LabelId" FROM found
        UNION ALL SELECT id FROM named
        UNION ALL SELECT id FROM created
        "#,
        LabelId::new() as LabelId,
        inbox as InboxId,
        label.key,
        name,
        workspace.as_uuid(),
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(label_id) = label_id else {
        // Another label already has this name: leave the conversation as is.
        return Ok(());
    };
    sqlx::query!(
        r#"
        INSERT INTO conversations.conversation_labels (workspace_id, conversation_id, label_id)
        VALUES ($1, $2, $3) ON CONFLICT DO NOTHING
        "#,
        workspace.as_uuid(),
        conversation as ConversationId,
        label_id as LabelId,
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Take provider labels `keys` off `conversation` where no message of it
/// carries them any more.
async fn unlink_provider_labels(
    conn: &mut PgConnection,
    conversation: ConversationId,
    keys: &[&String],
) -> Result<(), JobError> {
    if keys.is_empty() {
        return Ok(());
    }
    let keys: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
    sqlx::query!(
        r#"
        DELETE FROM conversations.conversation_labels cl
         USING conversations.labels l
         WHERE cl.label_id = l.id AND cl.conversation_id = $1 AND l.provider_key = ANY($2)
           AND NOT EXISTS (SELECT 1 FROM conversations.messages m
                            WHERE m.conversation_id = $1 AND l.provider_key = ANY(m.provider_labels))
        "#,
        conversation as ConversationId,
        &keys as &[&str],
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// Store `batch` as a blob of `workspace` and enqueue its application in
/// `tx` (ordered per inbox). What a connector does; used by the dev helpers
/// and tests.
pub async fn submit(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: WorkspaceId,
    batch: &MessageChangeBatch,
) -> campbooks_kernel::Result<BlobRef> {
    let ws = workspace.to_string();
    let bytes = serde_json::to_vec(batch)
        .map_err(|e| campbooks_kernel::Error::internal(redact::json(&e)))?;
    let blob = state
        .blobs
        .put(&ws, bytes, crate::ingest::BATCH_CONTENT_TYPE)
        .await?;
    enqueue(state, tx, &ws, batch.inbox, &blob).await?;
    Ok(blob)
}

/// Enqueue the application of the stored batch `blob` to `inbox`, in `tx`:
/// ordered per inbox, once per batch while pending, and with the batch held
/// until the job has applied it, so a retry still finds it.
pub async fn enqueue(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: &str,
    inbox: InboxId,
    blob: &BlobRef,
) -> campbooks_kernel::Result<()> {
    state
        .blobs
        .attach(tx, workspace, blob, &BlobOwner::new(BATCH_RECORD, inbox))
        .await?;
    let payload = ApplyChangesPayload {
        inbox,
        batch: blob.clone(),
    };
    let options = EnqueueOptions::for_workspace(workspace)
        .ordered_by(format!("conversations.inbox:{inbox}"))
        .unique(format!("{inbox}:{}", blob.hash));
    campbooks_kernel::jobs::enqueue_to::<ApplyChanges>(tx, &payload, options).await?;
    Ok(())
}
