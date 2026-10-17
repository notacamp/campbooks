import { useQuery } from "@tanstack/react-query";

import { api, unwrap, type Schema } from "~/lib/api";

export type ReplySuggestions = Schema<"ReplySuggestions">;
export type WriteReply = Schema<"WriteReply">;

// Not an operation key: the change feed never refetches these, since each
// asks the model again. They live for the session, one per message.
const replyKeys = {
  suggestions: (workspaceId: string, conversationId: string, messageId: string) =>
    ["reply-suggestions", workspaceId, conversationId, messageId] as const,
};

/**
 * `conversations.suggest_replies`: Scout's likely replies to `messageId`,
 * asked once per message for the session (a model call each time it is
 * asked), never retried on its own.
 */
export function useReplySuggestions(
  workspaceId: string,
  conversationId: string | undefined,
  messageId: string | undefined,
  { enabled = true }: { enabled?: boolean } = {},
) {
  return useQuery({
    queryKey: replyKeys.suggestions(workspaceId, conversationId ?? "", messageId ?? ""),
    enabled: enabled && conversationId !== undefined && messageId !== undefined,
    staleTime: Infinity,
    gcTime: 30 * 60 * 1000,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.POST(
          "/api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/suggestions",
          {
            params: {
              path: { workspace_id: workspaceId, conversation_id: conversationId ?? "" },
            },
            body: { message_id: messageId ?? null },
            signal,
          },
        ),
      ),
  });
}

/**
 * `conversations.write_reply`: Scout's reply for an intent, or the draft
 * rewritten (shorter, or as `change` asks). Plain text; nothing is saved.
 */
export async function writeReply(
  workspaceId: string,
  conversationId: string,
  body: WriteReply,
): Promise<string> {
  return unwrap(
    await api.POST("/api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/draft", {
      params: { path: { workspace_id: workspaceId, conversation_id: conversationId } },
      body,
    }),
  ).text;
}
