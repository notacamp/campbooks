import { useQuery } from "@tanstack/react-query";

import { api, operationKey, unwrap } from "~/lib/api";

/** Most characters a signature may have (the server's limit). */
export const SIGNATURE_MAX_CHARS = 2000;

const signatureKey = (workspaceId: string, inboxId: string) =>
  operationKey("conversations.get_signature", { workspace_id: workspaceId, inbox_id: inboxId });

/** `conversations.get_signature`: your own signature for an inbox, as marks ("" when none). */
export function useSignature(workspaceId: string, inboxId: string) {
  return useQuery({
    queryKey: signatureKey(workspaceId, inboxId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/inboxes/{inbox_id}/signature", {
          params: { path: { workspace_id: workspaceId, inbox_id: inboxId } },
          signal,
        }),
      ).text,
  });
}

/** `conversations.set_signature`: keep `text` as your signature for the inbox (400 `signature_too_long`). */
export async function saveSignature(
  workspaceId: string,
  inboxId: string,
  text: string,
): Promise<string> {
  return unwrap(
    await api.PUT("/api/v1/w/{workspace_id}/inboxes/{inbox_id}/signature", {
      params: { path: { workspace_id: workspaceId, inbox_id: inboxId } },
      body: { text },
    }),
  ).text;
}

export { signatureKey };
