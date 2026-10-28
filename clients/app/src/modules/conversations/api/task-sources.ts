import type { TaskSource, UseTaskQuote } from "~/modules/tasks";

import { displayName } from "../model";

import { useConversationSummaries, useMessageAsks } from "./queries";

/**
 * Where tasks made from conversations came from: who it is from, its
 * subject, when it last wrote and where to open it. The Tasks place reads
 * this through the app, so the tasks module never imports conversations.
 */
export function useConversationSources(
  workspaceId: string,
  conversationIds: readonly string[],
): ReadonlyMap<string, TaskSource> {
  const summaries = useConversationSummaries(workspaceId, conversationIds);
  return new Map(
    [...summaries].map(([id, conversation]) => [
      id,
      {
        name: displayName(conversation.latest_from),
        subject: conversation.subject,
        date: conversation.last_message_at,
        href: `/w/${workspaceId}/inbox/${conversation.stream}/${conversation.id}`,
      },
    ]),
  );
}

/**
 * The sentence of an email that asked for a task, by the task's ask key
 * (`<message id>:<layer>:ask:<n>`). It is read from the message's read, which
 * the server answers only to members who may read the inbox (404 otherwise),
 * so a task handed to someone who cannot read the email shows no sentence.
 * Nothing is copied into the task.
 */
export const useAskQuote: UseTaskQuote = (workspaceId, conversationId, ask) => {
  const message = ask?.split(":", 1)[0];
  const asks = useMessageAsks(
    workspaceId,
    conversationId ?? "",
    message ?? "",
    conversationId !== undefined && message !== undefined && message !== "",
  );
  return asks.data?.find((candidate) => candidate.key === ask)?.quote ?? undefined;
};
