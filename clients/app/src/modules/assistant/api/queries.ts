import { useMutation, useQueries, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, operationKey, unwrap, unwrapEmpty, type Schema } from "~/lib/api";

export type Proactivity = Schema<"Proactivity">;
export type CompileResult = Schema<"CompileResult">;
export type Proposal = Schema<"Proposal">;
export type Condition = Schema<"RuleCondition">;
export type Action = Schema<"RuleAction">;
export type RuleOrigin = Schema<"RuleOrigin">;

// Keyed by operationId, so the change feed refetches them.
const keys = {
  profile: (workspaceId: string) =>
    operationKey("assistant.get_profile", { workspace_id: workspaceId }),
  senders: (workspaceId: string, q: string) =>
    operationKey("conversations.list_senders", { workspace_id: workspaceId, q }),
  preview: (workspaceId: string, inboxId: string, conditions: string, actions: string) =>
    operationKey("conversations.preview_rule", {
      workspace_id: workspaceId,
      inbox_id: inboxId,
      conditions,
      actions,
    }),
  rules: (workspaceId: string) =>
    operationKey("conversations.list_rules", { workspace_id: workspaceId }),
};

/** `assistant.get_profile`: what the caller told Scout. */
export function useProfile(workspaceId: string) {
  return useQuery({
    queryKey: keys.profile(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/assistant/profile", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ),
  });
}

/** `assistant.update_profile`. */
export function useUpdateProfile(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: { proactivity: Proactivity; asked: boolean }) =>
      unwrap(
        await api.PUT("/api/v1/w/{workspace_id}/assistant/profile", {
          params: { path: { workspace_id: workspaceId } },
          body,
        }),
      ),
    onSuccess: (profile) => {
      queryClient.setQueryData(keys.profile(workspaceId), profile);
    },
  });
}

/** `assistant.compile_instruction`: a sentence, turned into rules to check. Saves nothing. */
export function useCompile(workspaceId: string) {
  return useMutation({
    mutationFn: async (text: string) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/assistant/compile", {
          params: { path: { workspace_id: workspaceId } },
          body: { text },
        }),
      ),
  });
}

/** `conversations.list_senders`: the people the caller replies to most, or those matching `q`. */
export function useSenders(workspaceId: string, q = "") {
  const query = q.trim();
  return useQuery({
    queryKey: keys.senders(workspaceId, query),
    enabled: query.length === 0 || query.length >= 2,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/senders", {
          params: {
            path: { workspace_id: workspaceId },
            query: query.length === 0 ? {} : { q: query },
          },
          signal,
        }),
      ).senders,
  });
}

/** A rule to check: what it tests and does. */
export interface RuleShape {
  conditions: Condition[];
  actions: Action[];
}

/**
 * `conversations.preview_rule` in each of `inboxIds`: what the rule would
 * match there, counted on the server. One result per inbox, in order.
 */
export function usePreviews(
  workspaceId: string,
  inboxIds: readonly string[],
  rule: RuleShape | undefined,
) {
  const conditions = JSON.stringify(rule?.conditions ?? []);
  const actions = JSON.stringify(rule?.actions ?? []);
  return useQueries({
    queries: inboxIds.map((inboxId) => ({
      queryKey: keys.preview(workspaceId, inboxId, conditions, actions),
      enabled: rule !== undefined && rule.conditions.length > 0,
      queryFn: async ({ signal }: { signal: AbortSignal }) =>
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/inboxes/{inbox_id}/rules/preview", {
            params: { path: { workspace_id: workspaceId, inbox_id: inboxId } },
            body: { conditions: rule?.conditions ?? [], actions: rule?.actions ?? [] },
            signal,
          }),
        ),
    })),
  });
}

/** A rule to create in one inbox. */
export interface NewRule extends RuleShape {
  /** The caller's key for it, to know which ones were created. */
  key: string;
  inboxId: string;
  origin: RuleOrigin;
  /** The member's own words, for an instruction. */
  originText?: string | undefined;
  applyToExisting: boolean;
  /** Shared by the rules one answer or sentence makes in several inboxes. */
  teachingId: string;
}

/** What a batch of creations did: the rules created, by key, and whether it stopped short. */
export interface Created {
  created: { key: string; id: string }[];
  /** Set when a creation failed: the ones after it were not tried. */
  failed: boolean;
}

/**
 * `conversations.create_rule` for each of `rules` in turn, active (the
 * member said yes by saving), stopping at the first failure. Resolves with
 * what was created, so a retry sends only the rest; the created ids are the
 * one Undo.
 */
export function useCreateRules(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (rules: readonly NewRule[]): Promise<Created> => {
      const created: Created["created"] = [];
      for (const rule of rules) {
        let id: string | undefined;
        try {
          const result = await api.POST("/api/v1/w/{workspace_id}/inboxes/{inbox_id}/rules", {
            params: { path: { workspace_id: workspaceId, inbox_id: rule.inboxId } },
            body: {
              conditions: rule.conditions,
              actions: rule.actions,
              origin: rule.origin,
              origin_text: rule.originText ?? null,
              state: "active",
              apply_to_existing: rule.applyToExisting,
              teaching_id: rule.teachingId,
            },
          });
          id = result.data?.rule.id;
        } catch {
          // The network gave out: what was created so far still counts.
        }
        if (id === undefined) {
          return { created, failed: true };
        }
        created.push({ key: rule.key, id });
      }
      return { created, failed: false };
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: keys.rules(workspaceId) });
    },
  });
}

/** `conversations.remove_rule` for each id, putting back what they changed (the Undo). */
export function useRemoveRules(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (ids: readonly string[]) => {
      for (const id of ids) {
        unwrapEmpty(
          await api.DELETE("/api/v1/w/{workspace_id}/rules/{rule_id}", {
            params: { path: { workspace_id: workspaceId, rule_id: id }, query: { restore: true } },
          }),
        );
      }
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: keys.rules(workspaceId) });
    },
  });
}
