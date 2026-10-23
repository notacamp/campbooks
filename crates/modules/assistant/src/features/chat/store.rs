//! The chats' rows: every statement runs in the caller's workspace
//! transaction, keyed by the chat's id (whose owner `authz` checked) or by
//! the caller's own user id.

use campbooks_kernel::{Error, RecordRef, Result, tenancy::UserId};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgConnection;

use super::{
    propose,
    types::{
        ChatAuthor, ChatFinish, ChatId, ChatMessageId, ChatMessageView, ChatProposal, ChatStep,
        ChatView, ProposalId,
    },
};

/// Questions a chat may hold.
pub const MAX_QUESTIONS: u32 = 40;

/// A chat's own row.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ChatRow {
    pub id: ChatId,
    pub about_type: Option<String>,
    pub about_id: Option<String>,
    pub questions: i32,
    pub created_at: DateTime<Utc>,
    pub last_message_at: DateTime<Utc>,
}

impl ChatRow {
    /// What it is about.
    pub fn about(&self) -> Option<RecordRef> {
        self.about_type
            .as_deref()
            .zip(self.about_id.as_deref())
            .map(|(record_type, id)| RecordRef::new(record_type, id))
    }

    /// Questions asked so far.
    pub fn questions(&self) -> u32 {
        u32::try_from(self.questions).unwrap_or_default()
    }
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: ChatMessageId,
    author: String,
    text: String,
    steps: Value,
    finish: Option<String>,
    proposals: Value,
    created_at: DateTime<Utc>,
}

impl MessageRow {
    fn view(self) -> ChatMessageView {
        ChatMessageView {
            id: self.id,
            author: if self.author == ChatAuthor::Scout.as_str() {
                ChatAuthor::Scout
            } else {
                ChatAuthor::Member
            },
            text: self.text,
            steps: serde_json::from_value(self.steps).unwrap_or_default(),
            finish: self.finish.as_deref().map(ChatFinish::from_db),
            proposals: serde_json::from_value(self.proposals).unwrap_or_default(),
            created_at: self.created_at,
        }
    }
}

const CHAT: &str = "SELECT id, about_type, about_id, questions, created_at, last_message_at \
     FROM assistant.chats WHERE id = $1";

/// Chat `id`, when it exists in this workspace.
pub async fn chat(conn: &mut PgConnection, id: ChatId) -> Result<Option<ChatRow>> {
    Ok(sqlx::query_as(CHAT).bind(id).fetch_optional(conn).await?)
}

/// Chat `id`, locked for a question: two questions at once in one chat wait
/// for each other, so its count holds.
pub async fn chat_for_update(conn: &mut PgConnection, id: ChatId) -> Result<ChatRow> {
    let row: Option<ChatRow> = sqlx::query_as(
        "SELECT id, about_type, about_id, questions, created_at, last_message_at \
           FROM assistant.chats WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    row.ok_or(Error::NotFound)
}

/// The caller's chat about `about`, if they have one.
pub async fn chat_about(
    conn: &mut PgConnection,
    user: UserId,
    about: &RecordRef,
) -> Result<Option<ChatRow>> {
    Ok(sqlx::query_as(
        "SELECT id, about_type, about_id, questions, created_at, last_message_at \
           FROM assistant.chats \
          WHERE user_id = $1 AND about_type = $2 AND about_id = $3",
    )
    .bind(user.as_uuid())
    .bind(&about.record_type)
    .bind(&about.id)
    .fetch_optional(conn)
    .await?)
}

/// A new chat for `user` about `about`; `None` when one about the same
/// record was made meanwhile (the unique key held).
pub async fn create(
    conn: &mut PgConnection,
    workspace: campbooks_kernel::tenancy::WorkspaceId,
    user: UserId,
    about: Option<&RecordRef>,
) -> Result<Option<ChatRow>> {
    Ok(sqlx::query_as(
        "INSERT INTO assistant.chats (id, workspace_id, user_id, about_type, about_id) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT DO NOTHING \
         RETURNING id, about_type, about_id, questions, created_at, last_message_at",
    )
    .bind(ChatId::new())
    .bind(workspace.as_uuid())
    .bind(user.as_uuid())
    .bind(about.map(|a| a.record_type.as_str()))
    .bind(about.map(|a| a.id.as_str()))
    .fetch_optional(conn)
    .await?)
}

/// The chat's messages, oldest first (a question before its answer).
pub async fn messages(conn: &mut PgConnection, chat: ChatId) -> Result<Vec<ChatMessageView>> {
    let rows: Vec<MessageRow> = sqlx::query_as(
        "SELECT id, author, text, steps, finish, proposals, created_at \
           FROM assistant.chat_messages \
          WHERE chat_id = $1 ORDER BY created_at, author = 'scout', id",
    )
    .bind(chat)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(MessageRow::view).collect())
}

/// The chat with its messages.
pub async fn view(conn: &mut PgConnection, row: &ChatRow) -> Result<ChatView> {
    Ok(ChatView {
        id: row.id,
        about: row.about(),
        questions: row.questions(),
        max_questions: MAX_QUESTIONS,
        created_at: row.created_at,
        last_message_at: row.last_message_at,
        messages: messages(conn, row.id).await?,
    })
}

/// Store the member's question and Scout's answer to come, as `failed`
/// with no text until it is written ([`finish`]): a question whose answer
/// never ends (the panel closed, the server stopped) stays failed. Counts
/// the question. The answer's id.
pub async fn ask(
    conn: &mut PgConnection,
    workspace: campbooks_kernel::tenancy::WorkspaceId,
    chat: ChatId,
    question: &str,
) -> Result<ChatMessageId> {
    let answer = ChatMessageId::new();
    sqlx::query(
        "INSERT INTO assistant.chat_messages (id, workspace_id, chat_id, author, text, created_at) \
         VALUES ($1, $2, $3, 'member', $4, clock_timestamp())",
    )
    .bind(ChatMessageId::new())
    .bind(workspace.as_uuid())
    .bind(chat)
    .bind(question)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO assistant.chat_messages \
             (id, workspace_id, chat_id, author, text, finish, created_at) \
         VALUES ($1, $2, $3, 'scout', '', 'failed', clock_timestamp())",
    )
    .bind(answer)
    .bind(workspace.as_uuid())
    .bind(chat)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "UPDATE assistant.chats SET questions = questions + 1, last_message_at = now() \
          WHERE id = $1",
    )
    .bind(chat)
    .execute(&mut *conn)
    .await?;
    Ok(answer)
}

/// Write Scout's answer: its text, steps, proposals and how it ended.
pub async fn finish(
    conn: &mut PgConnection,
    chat: ChatId,
    answer: ChatMessageId,
    text: &str,
    steps: &[ChatStep],
    proposals: &[ChatProposal],
    finish: ChatFinish,
) -> Result<Option<ChatMessageView>> {
    let steps = serde_json::to_value(steps).map_err(|e| Error::internal(e.to_string()))?;
    let proposals = serde_json::to_value(proposals).map_err(|e| Error::internal(e.to_string()))?;
    let row: Option<MessageRow> = sqlx::query_as(
        "UPDATE assistant.chat_messages SET text = $3, steps = $4, finish = $5, proposals = $6 \
          WHERE id = $1 AND chat_id = $2 AND author = 'scout' \
         RETURNING id, author, text, steps, finish, proposals, created_at",
    )
    .bind(answer)
    .bind(chat)
    .bind(text)
    .bind(steps)
    .bind(finish.as_str())
    .bind(proposals)
    .fetch_optional(&mut *conn)
    .await?;
    sqlx::query("UPDATE assistant.chats SET last_message_at = now() WHERE id = $1")
        .bind(chat)
        .execute(&mut *conn)
        .await?;
    Ok(row.map(MessageRow::view))
}

/// Delete the chat and its messages.
pub async fn delete(conn: &mut PgConnection, chat: ChatId) -> Result<()> {
    sqlx::query("DELETE FROM assistant.chat_messages WHERE chat_id = $1")
        .bind(chat)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM assistant.chats WHERE id = $1")
        .bind(chat)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// The chat's questions and answers so far, text only, oldest first: what
/// the model reads as the chat so far. Failed answers are left out (they
/// have no text). After an answer's text, a line the server writes for
/// each of its proposals: what Scout proposed and what the member did.
pub async fn transcript(
    conn: &mut PgConnection,
    chat: ChatId,
) -> Result<Vec<(ChatAuthor, String)>> {
    Ok(messages(conn, chat)
        .await?
        .into_iter()
        .filter(|message| !message.text.is_empty())
        .map(|message| {
            let mut text = message.text;
            for proposal in &message.proposals {
                text.push_str("\n\n");
                text.push_str(&propose::line(proposal));
            }
            (message.author, text)
        })
        .collect())
}

/// The Scout message of `chat` holding proposal `id`, and its proposals,
/// locked until the transaction ends.
pub async fn proposals_for_update(
    conn: &mut PgConnection,
    chat: ChatId,
    id: ProposalId,
) -> Result<Option<(ChatMessageId, Vec<ChatProposal>)>> {
    let row: Option<(ChatMessageId, Value)> = sqlx::query_as(
        "SELECT id, proposals FROM assistant.chat_messages \
          WHERE chat_id = $1 AND author = 'scout' AND proposals @> $2 FOR UPDATE",
    )
    .bind(chat)
    .bind(serde_json::json!([{ "id": id }]))
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|(message, proposals)| {
        (
            message,
            serde_json::from_value(proposals).unwrap_or_default(),
        )
    }))
}

/// Replace message `message`'s proposals.
pub async fn set_proposals(
    conn: &mut PgConnection,
    message: ChatMessageId,
    proposals: &[ChatProposal],
) -> Result<()> {
    let proposals = serde_json::to_value(proposals).map_err(|e| Error::internal(e.to_string()))?;
    sqlx::query("UPDATE assistant.chat_messages SET proposals = $2 WHERE id = $1")
        .bind(message)
        .bind(proposals)
        .execute(conn)
        .await?;
    Ok(())
}
