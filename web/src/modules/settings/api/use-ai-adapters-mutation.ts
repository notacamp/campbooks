/**
 * modules/settings/api/use-ai-adapters-mutation — BYO adapter CRUD.
 *
 * POST   /api/app/settings/ai_adapters       — create
 * PATCH  /api/app/settings/ai_adapters/:id   — update
 * DELETE /api/app/settings/ai_adapters/:id   — destroy
 *
 * All mutations invalidate the ai query so the adapter list refreshes.
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { AiAdapterData } from "./use-ai-query";
import { aiKeys } from "./use-ai-query";

// ── Create ────────────────────────────────────────────────────────────────────

export interface CreateAdapterParams {
  ai_adapter: {
    name: string;
    provider: string;
    api_key?: string;
    endpoint_url?: string;
    enabled?: boolean;
  };
}

export const useCreateAdapterMutation = (): UseMutationResult<
  AiAdapterData,
  Error,
  CreateAdapterParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: CreateAdapterParams) =>
      apiClient<AiAdapterData>("/settings/ai_adapters", {
        method: "POST",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: aiKeys.all });
    },
  });
};

// ── Update ────────────────────────────────────────────────────────────────────

export interface UpdateAdapterParams {
  id: number;
  ai_adapter: {
    name?: string;
    api_key?: string;
    endpoint_url?: string;
    enabled?: boolean;
  };
}

export const useUpdateAdapterMutation = (): UseMutationResult<
  AiAdapterData,
  Error,
  UpdateAdapterParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ai_adapter }: UpdateAdapterParams) =>
      apiClient<AiAdapterData>(`/settings/ai_adapters/${id}`, {
        method: "PATCH",
        body: { ai_adapter },
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: aiKeys.all });
    },
  });
};

// ── Destroy ───────────────────────────────────────────────────────────────────

export interface DeleteAdapterParams {
  id: number;
}

export const useDeleteAdapterMutation = (): UseMutationResult<
  void,
  Error,
  DeleteAdapterParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id }: DeleteAdapterParams) => {
      await apiClient<void>(`/settings/ai_adapters/${id}`, {
        method: "DELETE",
      });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: aiKeys.all });
    },
  });
};
