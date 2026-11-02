import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, errorStatus, operationKey, problemCode, unwrap, type Schema } from "~/lib/api";

export type ExplanationView = Schema<"ExplanationView">;
export type Explanation = Schema<"Explanation">;

/** How often a card that is being written asks again, in milliseconds. */
const POLL_MS = 1500;
/** How long it keeps asking, in milliseconds; then it says Scout is taking long. */
export const POLL_LIMIT_MS = 90_000;

// Keyed by operationId: a change to the conversation (the server tells only
// the member who asked when their explanation is stored) asks again.
const keys = {
  explanation: (workspaceId: string, conversationId: string) =>
    operationKey("conversations.get_explanation", {
      workspace_id: workspaceId,
      conversation_id: conversationId,
    }),
};

/** When each explanation was first seen being written, by thread: the 90 s limit counts from here. */
const firstSeenWriting = new Map<string, number>();
const seenKey = (workspaceId: string, conversationId: string) => `${workspaceId}:${conversationId}`;

/**
 * How long to wait before asking again for an explanation in `state`: 1.5 s
 * while it is being written, until 90 s after it was first seen so; then
 * never (the card says Scout is slow). Pure for tests.
 */
export function pollDelay(
  state: string | undefined,
  key: string,
  now: number,
  seen: Map<string, number> = firstSeenWriting,
): number | false {
  if (state !== "writing") {
    seen.delete(key);
    return false;
  }
  const since = seen.get(key) ?? now;
  seen.set(key, since);
  return now - since < POLL_LIMIT_MS ? POLL_MS : false;
}

/**
 * `conversations.get_explanation`: your own explanation of a thread, or null
 * when you never asked (404). Asked once when the thread opens and the server
 * offers explaining, never again on its own, except while one is being
 * written (every 1.5 s, for up to 90 s).
 */
export function useExplanation(
  workspaceId: string,
  conversationId: string,
  { enabled }: { enabled: boolean },
) {
  return useQuery({
    queryKey: keys.explanation(workspaceId, conversationId),
    enabled,
    retry: false,
    staleTime: Infinity,
    queryFn: async ({ signal }): Promise<ExplanationView | null> => {
      const result = await api.GET(
        "/api/v1/w/{workspace_id}/conversations/{conversation_id}/explanation",
        {
          params: { path: { workspace_id: workspaceId, conversation_id: conversationId } },
          signal,
        },
      );
      if (result.response.status === 404) {
        return null;
      }
      return unwrap(result);
    },
    refetchInterval: (query) =>
      pollDelay(query.state.data?.state, seenKey(workspaceId, conversationId), Date.now()),
  });
}

/** Why an ask was refused, for what the card says. */
export type ExplainRefusal = "not_explainable" | "unavailable" | "limit" | "failed";

/** The refusal named by a failed request. */
export function refusalOf(error: unknown): ExplainRefusal {
  const status = errorStatus(error);
  if (status === 409) {
    return "not_explainable";
  }
  if (status === 429) {
    return "limit";
  }
  if (status === 503 && problemCode(error) === "explain_unavailable") {
    return "unavailable";
  }
  return "failed";
}

/**
 * `conversations.explain`: the member asks for an explanation (or asks
 * again). The answer is kept at once; the card then follows it as it is
 * written.
 */
export function useExplain(workspaceId: string, conversationId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/conversations/{conversation_id}/explanation", {
          params: { path: { workspace_id: workspaceId, conversation_id: conversationId } },
        }),
      ),
    onSuccess: (view) => {
      // A new request: the 90 s count starts again.
      firstSeenWriting.delete(seenKey(workspaceId, conversationId));
      queryClient.setQueryData(keys.explanation(workspaceId, conversationId), view);
    },
  });
}
