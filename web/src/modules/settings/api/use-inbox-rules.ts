/**
 * modules/settings/api/use-inbox-rules — hooks for /api/app/inbox_settings/rules
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
  Rule,
  CreateRuleParams,
  UpdateRuleParams,
} from "~/routes/settings/inbox/types";

// ── Query key factory ─────────────────────────────────────────────────────────

export const inboxRulesKeys = {
  all: ["settings", "inbox", "rules"] as const,
};

// ── List ──────────────────────────────────────────────────────────────────────

export const useRulesQuery = (): UseQueryResult<Rule[], Error> =>
  useQuery<Rule[], Error>({
    queryKey: inboxRulesKeys.all,
    queryFn: () => apiClient<Rule[]>("/inbox_settings/rules"),
    staleTime: 2 * 60_000,
  });

// ── Create ────────────────────────────────────────────────────────────────────

export const useCreateRuleMutation = (): UseMutationResult<
  Rule,
  Error,
  CreateRuleParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (params: CreateRuleParams) =>
      apiClient<Rule>("/inbox_settings/rules", { method: "POST", body: params }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxRulesKeys.all });
    },
  });
};

// ── Update ────────────────────────────────────────────────────────────────────

export const useUpdateRuleMutation = (): UseMutationResult<
  Rule,
  Error,
  UpdateRuleParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, email_rule }: UpdateRuleParams) =>
      apiClient<Rule>(`/inbox_settings/rules/${id}`, {
        method: "PATCH",
        body: { email_rule },
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxRulesKeys.all });
    },
  });
};

// ── Delete ────────────────────────────────────────────────────────────────────

export const useDeleteRuleMutation = (): UseMutationResult<void, Error, number> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<void>(`/inbox_settings/rules/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxRulesKeys.all });
    },
  });
};

// ── Toggle enabled ────────────────────────────────────────────────────────────

export const useToggleRuleMutation = (): UseMutationResult<
  Rule,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<Rule>(`/inbox_settings/rules/${id}/toggle`, { method: "PATCH" }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxRulesKeys.all });
    },
  });
};

// ── Run ───────────────────────────────────────────────────────────────────────

export interface RunResult {
  run_id: number;
  status: string;
}

export const useRunRuleMutation = (): UseMutationResult<
  RunResult,
  Error,
  number
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: number) =>
      apiClient<RunResult>(`/inbox_settings/rules/${id}/run`, { method: "POST" }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: inboxRulesKeys.all });
    },
  });
};

// ── Undo run ──────────────────────────────────────────────────────────────────

export interface UndoRunParams {
  rule_id: number;
  run_id: number;
}

export const useUndoRunMutation = (): UseMutationResult<
  { undone: boolean },
  Error,
  UndoRunParams
> => {
  return useMutation({
    mutationFn: ({ rule_id, run_id }: UndoRunParams) =>
      apiClient<{ undone: boolean }>(
        `/inbox_settings/rules/${rule_id}/runs/${run_id}/undo`,
        { method: "POST" },
      ),
  });
};
