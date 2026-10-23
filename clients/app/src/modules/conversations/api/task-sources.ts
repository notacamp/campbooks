import type { TaskSource } from "~/modules/tasks";

import { displayName } from "../model";

import { useConversationSummaries } from "./queries";

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
