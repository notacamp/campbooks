import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";

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

/**
 * `conversations.dismiss_rule_suggestion`: "Just this one", for every likeness
 * the offer showed (whichever was chosen, none comes back). Rejects when one
 * could not be recorded.
 */
export async function dismissSuggestions(
  workspaceId: string,
  conversationId: string,
  keys: readonly string[],
): Promise<void> {
  await Promise.all(
    keys.map(async (key) => {
      unwrap(
        await api.POST(
          "/api/v1/w/{workspace_id}/conversations/{conversation_id}/rule-suggestions/dismiss",
          {
            params: { path: { workspace_id: workspaceId, conversation_id: conversationId } },
            body: { key },
          },
        ),
      );
    }),
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

/** A rule to make. */
interface RuleToMake {
  inboxId: string;
  conditions: RuleCondition[];
  actions: RuleAction[];
  fromConversation?: string | null | undefined;
  applyToExisting?: boolean | undefined;
  origin?: RuleView["origin"] | undefined;
  originText?: string | null | undefined;
  teachingId?: string | null | undefined;
  state?: "active" | "proposed" | undefined;
}

/** Whether this screen already removed `ruleId`: a stale Undo must not remove it twice. */
function removedKey(workspaceId: string, ruleId: string) {
  return ["rules", "removed", workspaceId, ruleId] as const;
}

/**
 * Refresh the conversation lists and readers: rows carry what a rule did to
 * them, which changes as its run goes or when it is removed.
 */
export async function refreshConversations(
  queryClient: QueryClient,
  workspaceId: string,
): Promise<void> {
  await Promise.all([
    queryClient.invalidateQueries({
      queryKey: operationKey("conversations.list", { workspace_id: workspaceId }),
    }),
    queryClient.invalidateQueries({
      queryKey: operationKey("conversations.get", { workspace_id: workspaceId }),
    }),
  ]);
}

/** The rule operations, each refreshing the rules and conversations it touched. */
export function useRuleMutations(workspaceId: string) {
  const queryClient = useQueryClient();
  const settle = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: keys.rules(workspaceId) }),
      // Every rule of the workspace: its state and its run changed.
      queryClient.invalidateQueries({
        queryKey: operationKey("conversations.get_rule", { workspace_id: workspaceId }),
      }),
      refreshConversations(queryClient, workspaceId),
    ]);
  };
  const create = useMutation({
    mutationFn: async (rule: RuleToMake) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/inboxes/{inbox_id}/rules", {
          params: { path: { workspace_id: workspaceId, inbox_id: rule.inboxId } },
          body: {
            conditions: rule.conditions,
            actions: rule.actions,
            apply_to_existing: rule.applyToExisting ?? true,
            ...(rule.fromConversation === undefined || rule.fromConversation === null
              ? {}
              : { from_conversation_id: rule.fromConversation }),
            ...(rule.origin === undefined ? {} : { origin: rule.origin }),
            ...(rule.originText === undefined || rule.originText === null
              ? {}
              : { origin_text: rule.originText }),
            ...(rule.teachingId === undefined || rule.teachingId === null
              ? {}
              : { teaching_id: rule.teachingId }),
            ...(rule.state === undefined ? {} : { state: rule.state }),
          },
        }),
      ),
    onSuccess: ({ rule }) => {
      queryClient.removeQueries({ queryKey: removedKey(workspaceId, rule.id) });
    },
    onSettled: settle,
  });
  const remove = useMutation({
    mutationFn: async ({ ruleId, restore }: { ruleId: string; restore: boolean }) =>
      unwrap(
        await api.DELETE("/api/v1/w/{workspace_id}/rules/{rule_id}", {
          params: { path: { workspace_id: workspaceId, rule_id: ruleId }, query: { restore } },
        }),
      ),
    onSuccess: (rule) => {
      queryClient.setQueryData(removedKey(workspaceId, rule.id), true);
      // Removed, with the run putting its mail back: seen at once.
      queryClient.setQueryData(keys.rule(workspaceId, rule.id), rule);
    },
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
  const restore = useMutation({
    mutationFn: async ({ ruleId, applyToExisting }: { ruleId: string; applyToExisting: boolean }) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/rules/{rule_id}/restore", {
          params: { path: { workspace_id: workspaceId, rule_id: ruleId } },
          body: { apply_to_existing: applyToExisting },
        }),
      ),
    onSuccess: (rule) => {
      queryClient.removeQueries({ queryKey: removedKey(workspaceId, rule.id) });
      queryClient.setQueryData(keys.rule(workspaceId, rule.id), rule);
    },
    onSettled: settle,
  });
  /** Whether `ruleId` was removed from this screen already. */
  const wasRemoved = (ruleId: string) =>
    queryClient.getQueryData<boolean>(removedKey(workspaceId, ruleId)) === true;
  return { create, remove, confirm, restore, wasRemoved };
}
