//! Path parameters of the module's routes. `{workspace_id}` is read and
//! checked by the `Workspace` extractor, so these leave it out.

use serde::Deserialize;

use crate::model::{ConversationId, DraftId, LabelId, MessageId};

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
