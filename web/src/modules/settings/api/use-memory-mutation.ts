/**
 * modules/settings/api/use-memory-mutation — teach / confirm / remove memory entries.
 *
 * POST   /api/app/settings/memory/teach               — teach a new sentence
 * POST   /api/app/settings/memory/entries/:id/confirm — confirm a learned habit
 * DELETE /api/app/settings/memory/entries/:id         — remove an entry
 *
 * All mutations invalidate the full memory query so the catalog rebuilds.
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { MemoryEntryData } from "./use-memory-query";
import { memoryKeys } from "./use-memory-query";

// ── Teach ─────────────────────────────────────────────────────────────────────

export interface TeachParams {
  sentence: string;
}

export interface TeachResult {
  created: boolean;
  entry: MemoryEntryData | null;
  reason?: string;
}

export const useMemoryTeachMutation = (): UseMutationResult<
  TeachResult,
  Error,
  TeachParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: TeachParams) =>
      apiClient<TeachResult>("/settings/memory/teach", {
        method: "POST",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: memoryKeys.all });
    },
  });
};

// ── Confirm ───────────────────────────────────────────────────────────────────

export interface ConfirmEntryParams {
  id: string;
}

export const useMemoryConfirmMutation = (): UseMutationResult<
  MemoryEntryData,
  Error,
  ConfirmEntryParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id }: ConfirmEntryParams) =>
      apiClient<MemoryEntryData>(`/settings/memory/entries/${id}/confirm`, {
        method: "POST",
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: memoryKeys.all });
    },
  });
};

// ── Remove ────────────────────────────────────────────────────────────────────

export interface RemoveEntryParams {
  id: string;
}

export const useMemoryRemoveMutation = (): UseMutationResult<
  void,
  Error,
  RemoveEntryParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id }: RemoveEntryParams) => {
      await apiClient<void>(`/settings/memory/entries/${id}`, {
        method: "DELETE",
      });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: memoryKeys.all });
    },
  });
};
