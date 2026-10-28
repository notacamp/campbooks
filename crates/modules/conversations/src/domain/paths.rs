//! Path parameters of the module's routes. `{workspace_id}` is read and
//! checked by the `Workspace` extractor, so these leave it out.

use serde::Deserialize;

use crate::model::{AttachmentId, ConversationId, DraftId, InboxId, LabelId, MessageId};

/// `/api/v1/w/{workspace_id}/conversations/{conversation_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ConversationPath {
    /// The conversation.
    pub conversation_id: ConversationId,
}

/// `…/conversations/{conversation_id}/messages/{message_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct MessagePath {
    /// The conversation.
    pub conversation_id: ConversationId,
    /// The message.
    pub message_id: MessageId,
}

/// `…/messages/{message_id}/attachments/{attachment_id}`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AttachmentPath {
    /// The conversation.
    #[serde(rename = "conversation_id")]
    pub conversation: ConversationId,
    /// The message.
    #[serde(rename = "message_id")]
    pub message: MessageId,
    /// The attachment.
    #[serde(rename = "attachment_id")]
    pub attachment: AttachmentId,
}

/// `…/conversations/{conversation_id}/labels/{label_id}`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct LabelPath {
    /// The conversation.
    pub conversation_id: ConversationId,
    /// The label.
    pub label_id: LabelId,
}

/// `/api/v1/w/{workspace_id}/drafts/{draft_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct DraftPath {
    /// The draft.
    pub draft_id: DraftId,
}

/// `/api/v1/w/{workspace_id}/inboxes/{inbox_id}…`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct InboxPath {
    /// The inbox.
    pub inbox_id: InboxId,
}
