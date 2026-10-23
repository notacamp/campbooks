import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";

import {
  api,
  operationKey,
  refreshBodiesAfterRules,
  unwrap,
  unwrapEmpty,
  type Schema,
} from "~/lib/api";

import type { Stream } from "./queries";

export type RuleView = Schema<"RuleView">;
export type RuleMark = Schema<"RuleMark">;
export type RuleCondition = Schema<"RuleCondition">;
export type RuleAction = Schema<"RuleAction">;
export type RuleSuggestion = Schema<"RuleSuggestion">;
export type RuleSuggestions = Schema<"RuleSuggestions">;
export type RuleException = Schema<"RuleException">;
export type RuleNotice = Schema<"RuleNotice">;

/** What a member did to one conversation, that a rule could repeat. */
export type TaughtAction =
  | { kind: "move_to_stream"; stream: Stream }
  | { kind: "archive" }
  | { kind: "mark_read" }
  | { kind: "not_needed" };

/**
 * What a member did that offers a rule for mail still to come: reported a
 * conversation as spam ("Never see emails like this again?"), or said a bill
 * is already paid ("Mark their invoices paid from now on").
 */
export type AheadAction = { kind: "report" } | { kind: "paid" };

// Keyed by operationId, so the change feed refetches them when a rule (its
// run's progress) changes.
const keys = {
  rule: (workspaceId: string, ruleId: string) =>
    operationKey("conversations.get_rule", { workspace_id: workspaceId, rule_id: ruleId }),
  rules: (workspaceId: string) =>
    operationKey("conversations.list_rules", { workspace_id: workspaceId }),
  notices: (workspaceId: string) =>
    operationKey("conversations.list_rule_notices", { workspace_id: workspaceId }),
  notice: (workspaceId: string, noticeId: string) =>
    operationKey("conversations.get_rule_notice", {
      workspace_id: workspaceId,
      notice_id: noticeId,
    }),
  preview: (workspaceId: string, inboxId: string, rule: unknown) => [
    ...operationKey("conversations.preview_rule", { workspace_id: workspaceId }),
    inboxId,
    rule,
  ],
};

/** `conversations.preview_rule`: what a rule would do in an inbox now. */
export function useRulePreview(
  workspaceId: string,
  inboxId: string,
  rule: { conditions: RuleCondition[]; actions: RuleAction[] },
  { enabled = true }: { enabled?: boolean } = {},
) {
  return useQuery({
    queryKey: keys.preview(workspaceId, inboxId, rule),
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/inboxes/{inbox_id}/rules/preview", {
          params: { path: { workspace_id: workspaceId, inbox_id: inboxId } },
          body: { conditions: rule.conditions, actions: rule.actions },
          signal,
        }),
      ),
  });
}

/**
 * `conversations.list_rule_notices`: what Scout noticed in the inboxes you
 * may teach in, the most frequent sender first. Asking also starts a fresh
 * look at an inbox not looked at lately; what it finds shows next time.
 */
export function useRuleNotices(workspaceId: string) {
  return useQuery({
    queryKey: keys.notices(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/rule-notices", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).notices,
  });
}

/**
 * `conversations.get_rule_notice`: one of Scout's suggestions, as the list
 * shows it, while it is still offered to you (404 once answered or gone).
 * Starts no look at the inbox.
 */
export function useRuleNotice(workspaceId: string, noticeId: string | undefined) {
  return useQuery({
    queryKey: keys.notice(workspaceId, noticeId ?? ""),
    enabled: noticeId !== undefined,
    retry: false,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/rule-notices/{notice_id}", {
          params: { path: { workspace_id: workspaceId, notice_id: noticeId ?? "" } },
          signal,
        }),
      ),
  });
}

/**
 * `assistant.compile_instruction`: the member's words about a sender's mail,
 * read by Scout into what a rule would do. Saves nothing; the confirmation
 * does. (Called here and not through the assistant module, which imports this
 * one.)
 */
export function useCompileGoal(workspaceId: string) {
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

/** "Not now", "Don't suggest this", and taking either back (Undo). */
export function useNoticeAnswers(workspaceId: string) {
  const queryClient = useQueryClient();
  const settle = () => queryClient.invalidateQueries({ queryKey: keys.notices(workspaceId) });
  const answer = useMutation({
    mutationFn: async ({ noticeId, answer }: { noticeId: string; answer: "not_now" | "never" }) => {
      unwrapEmpty(
        await api.POST("/api/v1/w/{workspace_id}/rule-notices/{notice_id}/answer", {
          params: { path: { workspace_id: workspaceId, notice_id: noticeId } },
          body: { answer },
        }),
      );
    },
    onSettled: settle,
  });
  const clear = useMutation({
    mutationFn: async ({ noticeId }: { noticeId: string }) => {
      unwrapEmpty(
        await api.DELETE("/api/v1/w/{workspace_id}/rule-notices/{notice_id}/answer", {
          params: { path: { workspace_id: workspaceId, notice_id: noticeId } },
        }),
      );
    },
    onSettled: settle,
  });
  return { answer, clear };
}

/** `conversations.suggest_rules`: the rules that would repeat `action` on lookalikes. */
export async function suggestRules(
  workspaceId: string,
  conversationId: string,
  action: TaughtAction | AheadAction,
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
      unwrapEmpty(
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
  /** The "Scout noticed" suggestion it answers. */
  noticeId?: string | null | undefined;
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
      // A rule saved from a suggestion (or undone) changes what Scout offers.
      queryClient.invalidateQueries({ queryKey: keys.notices(workspaceId) }),
      // Every rule of the workspace: its state and its run changed.
      queryClient.invalidateQueries({
        queryKey: operationKey("conversations.get_rule", { workspace_id: workspaceId }),
      }),
      refreshConversations(queryClient, workspaceId),
      // A rule that trusts a sender's images shows or hides them at once; a
      // body it trusted is dropped, never shown again from the cache
      // (campbooks-cloud#236).
      refreshBodiesAfterRules(queryClient, workspaceId),
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
            ...(rule.noticeId === undefined || rule.noticeId === null
              ? {}
              : { notice_id: rule.noticeId }),
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
  // A yes names the rule as it was shown (its `content_token`): one that
  // changed since answers 409 `content_changed`, and the refetch below
  // shows it as it is now.
  const confirm = useMutation({
    mutationFn: async ({ rule }: { rule: Pick<RuleView, "id" | "content_token"> }) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/rules/{rule_id}/confirm", {
          params: { path: { workspace_id: workspaceId, rule_id: rule.id } },
          body: { apply_to_existing: true, content_token: rule.content_token ?? null },
        }),
      ),
    onSettled: settle,
  });
  const decline = useMutation({
    mutationFn: async ({ ruleId }: { ruleId: string }) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/rules/{rule_id}/decline", {
          params: { path: { workspace_id: workspaceId, rule_id: ruleId } },
        }),
      ),
    onSuccess: (rule) => {
      queryClient.setQueryData(removedKey(workspaceId, rule.id), true);
      queryClient.setQueryData(keys.rule(workspaceId, rule.id), rule);
    },
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
  return { create, remove, confirm, decline, restore, wasRemoved };
}
