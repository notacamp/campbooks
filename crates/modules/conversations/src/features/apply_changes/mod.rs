//! `conversations.apply_changes` — apply a batch of message changes to one
//! inbox (see [`crate::ingest`]).
//!
//! In three steps, so slow work stays out of the job's transaction:
//!
//! 1. read the batch and find which messages are new (a short read);
//! 2. for each new message, store its participants document and ask the
//!    rules strategy (blob reads and writes, no transaction);
//! 3. in the job's transaction, apply every change in order: insert and
//!    thread new messages, update flags and labels of known ones, remove
//!    deleted ones, record the rules' answers, and publish the events.
//!
//! Batches of one inbox are enqueued with the same ordering key, so they
//! apply one at a time, in order.

mod threading;

#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use campbooks_ai::decide::{
    AnswerValue, Question,
    signals::email::{triage, triage_questions},
    strategy::{Decided, Strategy},
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
        blobs::{read_bytes, read_text, snippet},
        conversation::{Changed, recount},
        decide::{ENVELOPE_BODY_BYTES, MessageParts, Source, envelope},
        decisions::Recorded,
        policy::Policy,
    },
    events::{Change, MessageFacts, MessageReceived, MessageSent, Origin},
    ingest::{
        ApplyChangesPayload, BATCH_VERSION, MessageChange, MessageChangeBatch, NormalizedMessage,
        ProviderLabel,
    },
    model::{
        AttachmentId, BlobPointer, Channel, ConversationId, Direction, InboxId, LabelId, MessageId,
        Stream, StreamSource,
    },
};

/// Largest batch document read, in bytes.
pub const MAX_BATCH_BYTES: usize = 16 * 1024 * 1024;

/// Characters of a message's snippet.
pub const SNIPPET_CHARS: usize = 200;

/// Record type of a message, for blob references.
pub const MESSAGE_RECORD: &str = "conversations.message";
/// Record type of an attachment, for blob references.
pub const ATTACHMENT_RECORD: &str = "conversations.attachment";

/// Applies change batches, sorting new conversations with the rules
/// strategy the module was composed with.
pub struct ApplyChanges {
    rules: Arc<dyn Strategy>,
    policy: Policy,
}

impl ApplyChanges {
    /// A handler sorting with `rules`, acting on their answers as `policy`
    /// allows.
    pub fn new(rules: Arc<dyn Strategy>, policy: Policy) -> Self {
        Self { rules, policy }
    }
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

        let (bytes, cut) = read_bytes(&state.blobs, &ws, &payload.batch, MAX_BATCH_BYTES).await?;
        if cut {
            return Err(JobError::fatal("the change batch is larger than the limit"));
        }
        let batch: MessageChangeBatch = serde_json::from_slice(&bytes)
            .map_err(|e| JobError::fatal(format!("the change batch does not parse: {e}")))?;
        if batch.version != BATCH_VERSION {
            return Err(JobError::fatal(format!(
                "change batch version {} is not supported (expected {BATCH_VERSION})",
                batch.version
            )));
        }
        if batch.inbox != payload.inbox {
            return Err(JobError::fatal("the change batch names another inbox"));
        }

        let known = survey(&state, workspace, &batch).await?;
        let prepared = self.prepare(&state, &ws, &batch, known).await?;

        let events = state.events.clone();
        let tx = ctx.tx().await?;
        let mut apply = Apply {
            state: &state,
            events: &events,
            workspace,
            ws: &ws,
            policy: &self.policy,
            inbox: batch.inbox,
            touched: BTreeMap::new(),
            new_in: BTreeSet::new(),
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
        apply.finish(tx).await
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
            let body = match &message.text {
                Some(text) => {
                    read_text(&state.blobs, ws, text, ENVELOPE_BODY_BYTES)
                        .await?
                        .0
                }
                None => String::new(),
            };
            let document = ParticipantsDocument {
                version: 1,
                from: message.from.clone(),
                to: message.to.clone(),
                cc: message.cc.clone(),
                reply_to: message.reply_to.clone(),
                subject: message.subject.clone(),
            };
            let document =
                serde_json::to_vec(&document).map_err(|e| JobError::fatal(e.to_string()))?;
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
                    .map_err(|e| JobError::retry(format!("rules strategy failed: {e}")))?
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
    policy: &'a Policy,
    inbox: InboxId,
    /// Conversations whose existing messages changed or went: what changed
    /// and which messages, announced at the end.
    touched: BTreeMap<ConversationId, (BTreeSet<Change>, BTreeSet<String>)>,
    /// Conversations that only gained new messages: recounted at the end.
    new_in: BTreeSet<ConversationId>,
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
        if changes.is_empty() {
            return Ok(());
        }
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

    /// Insert a new message, threading it into a conversation.
    async fn insert(
        &mut self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        prepared: &Prepared,
    ) -> Result<(), JobError> {
        let questions = triage_questions();
        let inbound = message.direction == Direction::Inbound;
        let sort_to = if inbound {
            accepted_stream(&questions, &prepared.answers, self.policy)
        } else {
            None
        };
        let placement = threading::place(tx, self.inbox, message).await?;
        let (conversation, sorted) = self
            .conversation_for(tx, message, placement.conversation, sort_to)
            .await?;
        let id = self.store(tx, message, prepared, conversation).await?;

        if inbound {
            let recorded = Recorded {
                message: id,
                conversation,
                source: Source::Rules,
                strategy: "rules",
                questions: &questions,
                answers: &prepared.answers,
                policy: self.policy,
            };
            recorded.store(tx, self.workspace, sorted).await?;
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
        self.new_in.insert(conversation);
        Ok(())
    }

    /// The conversation a new message goes in: the one threading found
    /// (sorted by this message if it is its first inbound one), or a new
    /// one. Returns it and whether `sort_to` placed it.
    async fn conversation_for(
        &self,
        tx: &mut Transaction<'static, Postgres>,
        message: &NormalizedMessage,
        found: Option<ConversationId>,
        sort_to: Option<Stream>,
    ) -> Result<(ConversationId, bool), JobError> {
        if let Some(conversation) = found {
            let sorted = match sort_to {
                Some(stream) => sort_first_inbound(tx, conversation, stream).await?,
                None => false,
            };
            return Ok((conversation, sorted));
        }
        let (stream, source) = match sort_to {
            Some(stream) => (stream, StreamSource::Rules),
            None => (Stream::Primary, StreamSource::Default),
        };
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
                 in_spam, in_trash, read, starred, bodies, participants)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                    $18, $19, $20, $21, $22, $23, $24, $25)
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
                     inline, blob)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
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
            )
            .execute(&mut **tx)
            .await?;
            let owner = BlobOwner::new(ATTACHMENT_RECORD, attachment_id);
            self.attach(tx, &attachment.blob, &owner).await?;
        }
        for label in &message.labels {
            link_provider_label(tx, self.workspace, self.inbox, conversation, label).await?;
        }
        Ok(id)
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
        let entry = self.touched.entry(gone.conversation).or_default();
        entry.0.insert(Change::Messages);
        entry.1.insert(provider_message_id.to_owned());
        Ok(())
    }

    /// Recount every conversation the batch touched and announce the
    /// changes to existing ones.
    async fn finish(self, tx: &mut Transaction<'static, Postgres>) -> Result<(), JobError> {
        for conversation in &self.new_in {
            if !self.touched.contains_key(conversation) {
                recount(tx, *conversation).await?;
            }
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
        Ok(())
    }
}

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
    match &recorded.accepted(triage::STREAM)?.answer.value {
        AnswerValue::Choice(label) => label.parse().ok(),
        _ => None,
    }
}

/// Sort an existing conversation by its first inbound message, unless a
/// member or an earlier inbound message already placed it. Returns whether
/// it did.
async fn sort_first_inbound(
    conn: &mut PgConnection,
    conversation: ConversationId,
    stream: Stream,
) -> Result<bool, JobError> {
    let sorted = sqlx::query!(
        r#"
        UPDATE conversations.conversations c
           SET stream = $2, stream_source = 'rules', updated_at = now()
         WHERE c.id = $1 AND c.stream_source = 'default'
           AND NOT EXISTS (SELECT 1 FROM conversations.messages m
                            WHERE m.conversation_id = c.id AND m.direction = 'inbound')
        "#,
        conversation as ConversationId,
        stream.as_str(),
    )
    .execute(conn)
    .await?
    .rows_affected();
    Ok(sorted > 0)
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
    let bytes = serde_json::to_vec(batch).map_err(campbooks_kernel::Error::internal)?;
    let blob = state
        .blobs
        .put(&ws, bytes, crate::ingest::BATCH_CONTENT_TYPE)
        .await?;
    let payload = ApplyChangesPayload {
        inbox: batch.inbox,
        batch: blob.clone(),
    };
    let options = EnqueueOptions::for_workspace(ws)
        .ordered_by(format!("conversations.inbox:{}", batch.inbox))
        .unique(format!("{}:{}", batch.inbox, blob.hash));
    campbooks_kernel::jobs::enqueue_to::<ApplyChanges>(tx, &payload, options).await?;
    Ok(blob)
}
