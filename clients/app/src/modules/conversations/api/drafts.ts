import { useQuery } from "@tanstack/react-query";

import { api, operationKey, unwrap, unwrapEmpty, type Schema } from "~/lib/api";

export type Draft = Schema<"DraftView">;
export type DraftKind = Schema<"DraftKind">;
export type NewDraft = Schema<"NewDraft">;
export type DraftChanges = Schema<"DraftChanges">;
export type Inbox = Schema<"InboxView">;

// Keyed by operationId (`operationKey`), so the change feed invalidates them
// when a send is requested, fails or completes.
export const draftKeys = {
  all: (workspaceId: string) =>
    operationKey("conversations.list_drafts", { workspace_id: workspaceId }),
  forConversation: (workspaceId: string, conversationId: string) =>
    operationKey("conversations.list_drafts", {
      workspace_id: workspaceId,
      conversation_id: conversationId,
    }),
  one: (workspaceId: string, draftId: string) =>
    operationKey("conversations.get_draft", { workspace_id: workspaceId, draft_id: draftId }),
  inboxes: (workspaceId: string) =>
    operationKey("conversations.list_inboxes", { workspace_id: workspaceId }),
};

const draftPath = (workspaceId: string, draftId: string) => ({
  path: { workspace_id: workspaceId, draft_id: draftId },
});

/** `conversations.list_drafts` for one conversation: the caller's own drafts answering it. */
export function useConversationDrafts(workspaceId: string, conversationId: string) {
  return useQuery({
    queryKey: draftKeys.forConversation(workspaceId, conversationId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/drafts", {
          params: {
            path: { workspace_id: workspaceId },
            query: { conversation_id: conversationId },
          },
          signal,
        }),
      ).drafts,
  });
}

/** `conversations.get_draft`. */
export function useDraft(workspaceId: string, draftId: string | undefined) {
  return useQuery({
    queryKey: draftKeys.one(workspaceId, draftId ?? ""),
    enabled: draftId !== undefined,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/drafts/{draft_id}", {
          params: draftPath(workspaceId, draftId ?? ""),
          signal,
        }),
      ),
  });
}

/** `conversations.list_inboxes`: where the caller may send from (sender and manager roles). */
export function useSendingInboxes(workspaceId: string) {
  return useQuery({
    queryKey: draftKeys.inboxes(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/inboxes", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).inboxes,
    select: (inboxes) =>
      inboxes.filter((inbox) => inbox.role === "sender" || inbox.role === "manager"),
  });
}

/** `conversations.create_draft`. */
export async function createDraft(workspaceId: string, body: NewDraft): Promise<Draft> {
  return unwrap(
    await api.POST("/api/v1/w/{workspace_id}/drafts", {
      params: { path: { workspace_id: workspaceId } },
      body,
    }),
  );
}

/** `conversations.update_draft`. */
export async function saveDraft(
  workspaceId: string,
  draftId: string,
  changes: DraftChanges,
): Promise<Draft> {
  return unwrap(
    await api.PATCH("/api/v1/w/{workspace_id}/drafts/{draft_id}", {
      params: draftPath(workspaceId, draftId),
      body: changes,
    }),
  );
}

/** `conversations.get_draft`, outside a query (after a conflict). */
export async function fetchDraft(workspaceId: string, draftId: string): Promise<Draft> {
  return unwrap(
    await api.GET("/api/v1/w/{workspace_id}/drafts/{draft_id}", {
      params: draftPath(workspaceId, draftId),
    }),
  );
}

/** `conversations.send_draft`: scheduled, leaving at `send_at` unless cancelled. */
export async function sendDraft(workspaceId: string, draftId: string): Promise<Draft> {
  return unwrap(
    await api.POST("/api/v1/w/{workspace_id}/drafts/{draft_id}/send", {
      params: draftPath(workspaceId, draftId),
    }),
  );
}

/** `conversations.cancel_send`: back to an editable draft, within the window. */
export async function cancelSend(workspaceId: string, draftId: string): Promise<Draft> {
  return unwrap(
    await api.POST("/api/v1/w/{workspace_id}/drafts/{draft_id}/cancel_send", {
      params: draftPath(workspaceId, draftId),
    }),
  );
}

/** `conversations.delete_draft`. */
export async function deleteDraft(workspaceId: string, draftId: string): Promise<void> {
  unwrapEmpty(
    await api.DELETE("/api/v1/w/{workspace_id}/drafts/{draft_id}", {
      params: draftPath(workspaceId, draftId),
    }),
  );
}
