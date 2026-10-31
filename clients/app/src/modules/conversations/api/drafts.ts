import { useQuery } from "@tanstack/react-query";

import { api, operationKey, problemCode, unwrap, unwrapEmpty, type Schema } from "~/lib/api";

export type Draft = Schema<"DraftView">;
export type NewDraft = Schema<"NewDraft">;
export type DraftChanges = Schema<"DraftChanges">;
export type Inbox = Schema<"InboxView">;

/** Most drafts `conversations.list_drafts` returns. */
export const DRAFTS_MAX = 50;

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

/**
 * A save or send refused because the draft is not the one the caller holds
 * (campbooks-cloud#242): saved since that version (409 `draft_changed`:
 * another tab, another device, later the assistant) or not the content
 * shown (409 `content_changed`). Nothing was saved or sent either way: read
 * it again, show it, and let the member act.
 */
export function changedElsewhere(thrown: unknown): boolean {
  const code = problemCode(thrown);
  return code === "content_changed" || code === "draft_changed";
}

const draftPath = (workspaceId: string, draftId: string) => ({
  path: { workspace_id: workspaceId, draft_id: draftId },
});

/**
 * `conversations.list_drafts`: your newest drafts across every conversation
 * (at most {@link DRAFTS_MAX}). Keyed as {@link draftKeys.all}, so every save,
 * send and delete refreshes it.
 */
export function useDrafts(workspaceId: string) {
  return useQuery({
    queryKey: draftKeys.all(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/drafts", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).drafts,
  });
}

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

/**
 * `conversations.list_inboxes`: where the caller may send from (sender and
 * manager roles), never an inbox kept read-only after its mailbox was
 * disconnected.
 */
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
      inboxes.filter(
        (inbox) => !inbox.read_only && (inbox.role === "sender" || inbox.role === "manager"),
      ),
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

/**
 * `conversations.upload_draft_attachment`: add `file` to the draft shown (its
 * `version` and `content_token`, named as a save names them). The file's own
 * bytes are the body; the type it carries is kept for the message, never
 * trusted. Answers the draft with the file and its new version and token.
 * Refused as a save is when the draft is not the one shown (409
 * `draft_changed`, `content_changed`), and with 400 `file_empty`,
 * `filename_missing`, `attachment_blocked`, `too_many_attachments`, 413
 * `attachments_too_large`, 409 `draft_not_editable`.
 */
export async function uploadAttachment(
  workspaceId: string,
  draft: Pick<Draft, "id" | "version"> & { content_token?: string | null | undefined },
  file: File,
): Promise<Draft> {
  return unwrap(
    await api.POST("/api/v1/w/{workspace_id}/drafts/{draft_id}/attachments", {
      params: {
        path: { workspace_id: workspaceId, draft_id: draft.id },
        query: { version: draft.version, filename: file.name },
        header: draft.content_token == null ? {} : { "X-Content-Token": draft.content_token },
      },
      body: file as unknown as string,
      bodySerializer: (body: unknown) => body as BodyInit,
      headers: { "Content-Type": file.type === "" ? "application/octet-stream" : file.type },
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

/**
 * `conversations.send_draft` of the saved `version` (409 `draft_changed` if
 * it is not the latest), with the `content_token` that came with the draft
 * on screen (409 `content_changed` if the draft is not that any more, or
 * there is none): scheduled, leaving at `send_at` unless cancelled. With
 * `sendAt` it leaves then, not when the undo window ends (400 `send_at_invalid`
 * if the server will not take that time).
 */
export async function sendDraft(
  workspaceId: string,
  draftId: string,
  shown: { version: number; contentToken: string | null | undefined },
  sendAt?: Date,
): Promise<Draft> {
  return unwrap(
    await api.POST("/api/v1/w/{workspace_id}/drafts/{draft_id}/send", {
      params: draftPath(workspaceId, draftId),
      body: {
        version: shown.version,
        content_token: shown.contentToken ?? null,
        ...(sendAt === undefined ? {} : { send_at: sendAt.toISOString() }),
      },
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
