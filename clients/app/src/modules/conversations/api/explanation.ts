import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, errorStatus, operationKey, problemCode, unwrap, type Schema } from "~/lib/api";

export type ExplanationView = Schema<"ExplanationView">;
export type Explanation = Schema<"Explanation">;

/** How often a card that is being written asks again, in milliseconds. */
export const POLL_MS = 1500;
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
    refetchInterval: (query) => {
      const view = query.state.data;
      const started = query.state.dataUpdatedAt;
      return view?.state === "writing" && Date.now() - started < POLL_LIMIT_MS ? POLL_MS : false;
    },
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
      queryClient.setQueryData(keys.explanation(workspaceId, conversationId), view);
    },
  });
}
