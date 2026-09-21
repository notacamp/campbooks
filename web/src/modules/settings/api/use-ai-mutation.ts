/**
 * modules/settings/api/use-ai-mutation — mode switch + embeddings update.
 *
 * POST /api/app/settings/ai/switch_mode  — "managed" | "byo"
 * PATCH /api/app/settings/ai/embeddings  — change the embedding model
 *
 * Both invalidate the ai query so the page re-fetches the current state.
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import { aiKeys } from "./use-ai-query";

// ── Switch mode ───────────────────────────────────────────────────────────────

export interface SwitchModeParams {
  mode: "managed" | "byo";
}

export interface SwitchModeResult {
  mode: "managed" | "byo";
}

export const useAiSwitchModeMutation = (): UseMutationResult<
  SwitchModeResult,
  Error,
  SwitchModeParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: SwitchModeParams) =>
      apiClient<SwitchModeResult>("/settings/ai/switch_mode", {
        method: "POST",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: aiKeys.all });
    },
  });
};

// ── Embeddings ────────────────────────────────────────────────────────────────

export interface EmbeddingsParams {
  embedding_model: string;
}

export interface EmbeddingsResult {
  embedding_model: string;
  reembed_queued: boolean;
}

export const useAiEmbeddingsMutation = (): UseMutationResult<
  EmbeddingsResult,
  Error,
  EmbeddingsParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: EmbeddingsParams) =>
      apiClient<EmbeddingsResult>("/settings/ai/embeddings", {
        method: "PATCH",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: aiKeys.all });
    },
  });
};
