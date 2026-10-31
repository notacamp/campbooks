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

/**
 * Reads the sentence of an email that asked for a task: the conversations
 * module's, plugged in by the app. It is the sender's own words, so it comes
 * only from the conversation, under the conversation's own access check: a
 * member who may not read the email gets nothing, however the task reached
 * them. `undefined` while loading, when the task has no such ask (an older
 * one) and when it may not be read.
 */
export type UseTaskQuote = (
  workspaceId: string,
  conversationId: string | undefined,
  ask: string | null | undefined,
) => string | undefined;

/** The record type of a conversation, the one source tasks have today. */
export const CONVERSATION = "conversations.conversation";
