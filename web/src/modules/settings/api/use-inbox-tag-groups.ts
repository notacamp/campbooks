/**
 * modules/settings/api/use-inbox-tag-groups — hooks for /api/app/inbox_settings/tag_groups
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
  TagGroup,
  CreateTagGroupParams,
  UpdateTagGroupParams,
} from "~/routes/settings/inbox/types";

// ── Query key factory ─────────────────────────────────────────────────────────

export const inboxTagGroupsKeys = {
  all: ["settings", "inbox", "tag_groups"] as const,
};

// ── List ──────────────────────────────────────────────────────────────────────

export const useTagGroupsQuery = (): UseQueryResult<TagGroup[], Error> =>
  useQuery<TagGroup[], Error>({
    queryKey: inboxTagGroupsKeys.all,
    queryFn: () => apiClient<TagGroup[]>("/inbox_settings/tag_groups"),
    staleTime: 2 * 60_000,
  });

// ── Create ────────────────────────────────────────────────────────────────────

export const useCreateTagGroupMutation = (): UseMutationResult<
  { name: string },
  Error,
  CreateTagGroupParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: CreateTagGroupParams) =>
      apiClient<{ name: string }>("/inbox_settings/tag_groups", {
        method: "POST",
        body: params,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxTagGroupsKeys.all });
    },
  });
};

// ── Update ────────────────────────────────────────────────────────────────────

export const useUpdateTagGroupMutation = (): UseMutationResult<
  { name: string },
  Error,
  UpdateTagGroupParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, ...body }: UpdateTagGroupParams) =>
      apiClient<{ name: string }>(
        `/inbox_settings/tag_groups/${encodeURIComponent(id)}`,
        { method: "PATCH", body },
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxTagGroupsKeys.all });
    },
  });
};

// ── Delete ────────────────────────────────────────────────────────────────────

export const useDeleteTagGroupMutation = (): UseMutationResult<
  void,
  Error,
  string
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (name: string) =>
      apiClient<void>(
        `/inbox_settings/tag_groups/${encodeURIComponent(name)}`,
        { method: "DELETE" },
      ),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxTagGroupsKeys.all });
    },
  });
};
