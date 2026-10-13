//! The OAuth scopes of the `conversations` module (see `campbooks_kernel::auth`).

use campbooks_kernel::auth::{ScopeInfo, ScopeName};

/// Every scope of this module, for its `Module` descriptor.
pub const ALL: &[ScopeInfo] = &[
    ScopeInfo::of::<ConversationsRead>(),
    ScopeInfo::of::<ConversationsWrite>(),
    ScopeInfo::of::<ConversationsSend>(),
];

/// `conversations:read` — read your inboxes and their conversations.
#[derive(Debug, Clone, Copy)]
pub struct ConversationsRead;

impl ScopeName for ConversationsRead {
    const NAME: &'static str = "conversations:read";
    const DESCRIPTION: &'static str = "Read your inboxes, conversations and messages.";
}

/// `conversations:write` — tidy conversations.
#[derive(Debug, Clone, Copy)]
pub struct ConversationsWrite;

impl ScopeName for ConversationsWrite {
    const NAME: &'static str = "conversations:write";
    const DESCRIPTION: &'static str =
        "Mark conversations read or unread, star, archive, label and move them between streams.";
}

/// `conversations:send` — write drafts and send messages.
#[derive(Debug, Clone, Copy)]
pub struct ConversationsSend;

impl ScopeName for ConversationsSend {
    const NAME: &'static str = "conversations:send";
    const DESCRIPTION: &'static str =
        "Write drafts and send messages, replies and forwards from your inboxes.";
}
