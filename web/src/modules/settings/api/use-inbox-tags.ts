/**
 * modules/settings/api/use-inbox-tags — hooks for /api/app/inbox_settings/tags
 *
 * Kept separate from the shared barrel (index.ts) so parallel build sessions
 * can add new inbox hooks without merge conflicts.
 */
import {
  useQuery,
  useMutation,
  useQueryClient,
  type UseQueryResult,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type {
  Tag,
  TagsData,
  CreateTagParams,
  UpdateTagParams,
} from "~/routes/settings/inbox/types";

// ── Query key factory ─────────────────────────────────────────────────────────

export const inboxTagsKeys = {
  all: ["settings", "inbox", "tags"] as const,
};

// ── List ──────────────────────────────────────────────────────────────────────

export const useTagsQuery = (): UseQueryResult<TagsData, Error> =>
  useQuery<TagsData, Error>({
    queryKey: inboxTagsKeys.all,
    queryFn: () => apiClient<TagsData>("/inbox_settings/tags"),
    staleTime: 2 * 60_000,
  });

// ── Create ────────────────────────────────────────────────────────────────────

export const useCreateTagMutation = (): UseMutationResult<
  Tag,
  Error,
  CreateTagParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: CreateTagParams) =>
      apiClient<Tag>("/inbox_settings/tags", { method: "POST", body: params }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxTagsKeys.all });
    },
  });
};

// ── Update ────────────────────────────────────────────────────────────────────

export const useUpdateTagMutation = (): UseMutationResult<
  Tag,
  Error,
  UpdateTagParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: UpdateTagParams) =>
      apiClient<Tag>(`/inbox_settings/tags/${id}`, { method: "PATCH", body }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxTagsKeys.all });
    },
  });
};

// ── Delete ────────────────────────────────────────────────────────────────────

export const useDeleteTagMutation = (): UseMutationResult<void, Error, number> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<void>(`/inbox_settings/tags/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxTagsKeys.all });
    },
  });
};

// ── Toggle hidden ─────────────────────────────────────────────────────────────

export const useToggleTagHiddenMutation = (): UseMutationResult<
  Tag,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<Tag>(`/inbox_settings/tags/${id}/toggle_hidden`, {
        method: "PATCH",
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxTagsKeys.all });
    },
  });
};

// ── Merge ─────────────────────────────────────────────────────────────────────

export interface MergeTagParams {
  id: number;
  into_tag_id: number;
}

export const useMergeTagMutation = (): UseMutationResult<
  { merged: boolean; into_tag_id: number },
  Error,
  MergeTagParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, into_tag_id }: MergeTagParams) =>
      apiClient<{ merged: boolean; into_tag_id: number }>(
        `/inbox_settings/tags/${id}/merge`,
        { method: "POST", body: { into_tag_id } },
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxTagsKeys.all });
    },
  });
};
