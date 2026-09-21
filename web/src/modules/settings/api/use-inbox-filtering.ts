/**
 * modules/settings/api/use-inbox-filtering — hooks for /api/app/inbox_settings/filtering
 */
import {
  useQuery,
  useMutation,
  useQueryClient,
  type UseQueryResult,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { FilteringData } from "~/routes/settings/inbox/types";

// ── Query key factory ─────────────────────────────────────────────────────────

export const inboxFilteringKeys = {
  all: ["settings", "inbox", "filtering"] as const,
};

// ── Fetch ─────────────────────────────────────────────────────────────────────

export const useFilteringQuery = (): UseQueryResult<FilteringData, Error> =>
  useQuery<FilteringData, Error>({
    queryKey: inboxFilteringKeys.all,
    queryFn: () => apiClient<FilteringData>("/inbox_settings/filtering"),
    staleTime: 2 * 60_000,
  });

// ── Update strategy ───────────────────────────────────────────────────────────

export interface UpdateFilteringParams {
  inbox_filter_strategy: string;
}

export const useUpdateFilteringMutation = (): UseMutationResult<
  FilteringData,
  Error,
  UpdateFilteringParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: UpdateFilteringParams) =>
      apiClient<FilteringData>("/inbox_settings/filtering", {
        method: "PATCH",
        body: params,
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(inboxFilteringKeys.all, data);
    },
  });
};

// ── Set sender state ──────────────────────────────────────────────────────────

export interface SetSenderParams {
  contact_id: number;
  state: "block" | "allow" | "unblock" | "neutral" | "unstar";
}

export const useSetSenderFilterMutation = (): UseMutationResult<
  FilteringData,
  Error,
  SetSenderParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: SetSenderParams) =>
      apiClient<FilteringData>("/inbox_settings/filtering/set_sender", {
        method: "POST",
        body: params,
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(inboxFilteringKeys.all, data);
    },
  });
};
