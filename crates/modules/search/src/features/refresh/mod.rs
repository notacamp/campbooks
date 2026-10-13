//! Keeping the index current from the sources' events: each event names a
//! group whose current state is pulled through its `SearchSource` and
//! written in the delivery's transaction (`domain::index::refresh`). The
//! first event from a workspace also starts its backfill. Nothing is indexed
//! for a workspace being deleted.

use campbooks_kernel::{Event, EventHandler, JobContext, JobError, tenancy::WorkspaceId};

use crate::{
    domain::index,
    events::{CONVERSATION_RECORD, ConversationChanged, MessageReceived, MessageSent},
};

/// Refresh `group` (of `group_type`) in `workspace`.
async fn refresh(
    ctx: &mut JobContext,
    workspace: &str,
    group_type: &str,
    group: &str,
) -> Result<(), JobError> {
    let state = ctx.state().clone();
    let Some(source) = state.search_sources.get(group_type).cloned() else {
        // No source registered: nothing is indexed (fail closed).
        return Ok(());
    };
    let workspace: WorkspaceId = workspace
        .parse()
        .map_err(|_| JobError::fatal("event with a malformed workspace"))?;
    let tx = ctx.tx().await?;
    if index::ensure_backfill(tx, workspace, group_type).await? == index::Standing::Deleting {
        // The workspace is being deleted: its index is purged, not refreshed.
        return Ok(());
    }
    index::refresh(tx, &state.blobs, workspace, source.as_ref(), group).await?;
    Ok(())
}

/// `conversations.message_received` → refresh the conversation.
pub struct OnMessageReceived;

impl EventHandler for OnMessageReceived {
    type Event = MessageReceived;
    const NAME: &'static str = "search.on_message_received";
    const QUEUE: &'static str = "events";

    async fn handle(ctx: &mut JobContext, event: Event<MessageReceived>) -> Result<(), JobError> {
        let MessageReceived {
            workspace,
            conversation,
        } = event.payload;
        refresh(ctx, &workspace, CONVERSATION_RECORD, &conversation).await
    }
}

/// `conversations.message_sent` → refresh the conversation.
pub struct OnMessageSent;

impl EventHandler for OnMessageSent {
    type Event = MessageSent;
    const NAME: &'static str = "search.on_message_sent";
    const QUEUE: &'static str = "events";

    async fn handle(ctx: &mut JobContext, event: Event<MessageSent>) -> Result<(), JobError> {
        let MessageSent {
            workspace,
            conversation,
        } = event.payload;
        refresh(ctx, &workspace, CONVERSATION_RECORD, &conversation).await
    }
}

/// `conversations.conversation_changed` → refresh the conversation (gone
/// ones are removed).
pub struct OnConversationChanged;

impl EventHandler for OnConversationChanged {
    type Event = ConversationChanged;
    const NAME: &'static str = "search.on_conversation_changed";
    const QUEUE: &'static str = "events";

    async fn handle(
        ctx: &mut JobContext,
        event: Event<ConversationChanged>,
    ) -> Result<(), JobError> {
        let ConversationChanged {
            workspace,
            conversation,
        } = event.payload;
        refresh(ctx, &workspace, CONVERSATION_RECORD, &conversation).await
    }
}
