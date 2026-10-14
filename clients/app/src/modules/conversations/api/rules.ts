import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, operationKey, unwrap, type Schema } from "~/lib/api";

import type { Stream } from "./queries";

export type RuleView = Schema<"RuleView">;
export type RuleMark = Schema<"RuleMark">;
export type RuleCondition = Schema<"RuleCondition">;
export type RuleAction = Schema<"RuleAction">;
export type RuleSuggestion = Schema<"RuleSuggestion">;
export type RuleSuggestions = Schema<"RuleSuggestions">;
export type RuleException = Schema<"RuleException">;

/** What a member did to one conversation, that a rule could repeat. */
export type TaughtAction =
  | { kind: "move_to_stream"; stream: Stream }
  | { kind: "archive" }
  | { kind: "mark_read" }
  | { kind: "not_needed" };

// Keyed by operationId, so the change feed refetches them when a rule (its
// run's progress) changes.
const keys = {
  rule: (workspaceId: string, ruleId: string) =>
    operationKey("conversations.get_rule", { workspace_id: workspaceId, rule_id: ruleId }),
  rules: (workspaceId: string) =>
    operationKey("conversations.list_rules", { workspace_id: workspaceId }),
};

/** `conversations.suggest_rules`: the rules that would repeat `action` on lookalikes. */
export async function suggestRules(
  workspaceId: string,
  conversationId: string,
  action: TaughtAction,
): Promise<RuleSuggestions> {
  return unwrap(
    await api.GET("/api/v1/w/{workspace_id}/conversations/{conversation_id}/rule-suggestions", {
      params: {
        path: { workspace_id: workspaceId, conversation_id: conversationId },
        query: {
          action: action.kind,
          ...(action.kind === "move_to_stream" ? { stream: action.stream } : {}),
        },
      },
    }),
  );
}

/** `conversations.dismiss_rule_suggestion`: "Just this one". */
export async function dismissSuggestion(
  workspaceId: string,
  conversationId: string,
  key: string,
): Promise<void> {
  await api.POST(
    "/api/v1/w/{workspace_id}/conversations/{conversation_id}/rule-suggestions/dismiss",
    {
      params: { path: { workspace_id: workspaceId, conversation_id: conversationId } },
      body: { key },
    },
  );
}

/** `conversations.get_rule`, live: its run's progress arrives through the change feed. */
export function useRule(workspaceId: string, ruleId: string | undefined) {
  return useQuery({
    queryKey: keys.rule(workspaceId, ruleId ?? ""),
    enabled: ruleId !== undefined,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/rules/{rule_id}", {
          params: { path: { workspace_id: workspaceId, rule_id: ruleId ?? "" } },
          signal,
        }),
      ),
  });
}

/** `conversations.list_rules`: every inbox's rules you can see, proposed ones first. */
export function useRules(workspaceId: string) {
  return useQuery({
    queryKey: keys.rules(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/rules", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).rules,
  });
}

/** The rule operations, each refreshing the rules it touched. */
export function useRuleMutations(workspaceId: string) {
  const queryClient = useQueryClient();
  const settle = async () => {
    await queryClient.invalidateQueries({ queryKey: keys.rules(workspaceId) });
  };
  const create = useMutation({
    mutationFn: async ({
      inboxId,
      conditions,
      actions,
      fromConversation,
      applyToExisting = true,
    }: {
      inboxId: string;
      conditions: RuleCondition[];
      actions: RuleAction[];
      fromConversation?: string | undefined;
      applyToExisting?: boolean | undefined;
    }) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/inboxes/{inbox_id}/rules", {
          params: { path: { workspace_id: workspaceId, inbox_id: inboxId } },
          body: {
            conditions,
            actions,
            apply_to_existing: applyToExisting,
            ...(fromConversation === undefined ? {} : { from_conversation_id: fromConversation }),
          },
        }),
      ),
    onSettled: settle,
  });
  const remove = useMutation({
    mutationFn: async ({ ruleId, restore }: { ruleId: string; restore: boolean }) =>
      unwrap(
        await api.DELETE("/api/v1/w/{workspace_id}/rules/{rule_id}", {
          params: { path: { workspace_id: workspaceId, rule_id: ruleId }, query: { restore } },
        }),
      ),
    onSettled: settle,
  });
  const confirm = useMutation({
    mutationFn: async ({ ruleId }: { ruleId: string }) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/rules/{rule_id}/confirm", {
          params: { path: { workspace_id: workspaceId, rule_id: ruleId } },
          body: { apply_to_existing: true },
        }),
      ),
    onSettled: settle,
  });
  return { create, remove, confirm };
}
