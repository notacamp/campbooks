/** Where a task came from, as the module that owns the record tells it. */
export interface TaskSource {
  /** Who it is from. */
  name: string;
  /** The subject of the conversation. */
  subject: string;
  /** When its newest message arrived (ISO 8601). */
  date: string;
  /** Where "Open the email" goes. */
  href: string;
}

/**
 * Reads the sources of tasks made from conversations: the conversations
 * module's, plugged in by the app so this module never imports it.
 */
export type UseTaskSources = (
  workspaceId: string,
  conversationIds: readonly string[],
) => ReadonlyMap<string, TaskSource>;

/** The record type of a conversation, the one source tasks have today. */
export const CONVERSATION = "conversations.conversation";
