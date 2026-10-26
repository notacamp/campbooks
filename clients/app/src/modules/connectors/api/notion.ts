import { useMutation, useQuery } from "@tanstack/react-query";

import { api, errorStatus, operationKey, problemCode, unwrap } from "~/lib/api";

import type { NotionPages, StartNotion } from "../types";

const path = (workspaceId: string) => ({ path: { workspace_id: workspaceId } });

/**
 * `connectors.start_notion`: records a consent and answers where to send the
 * browser (this instance's `/auth/start/notion?ticket=...`, which goes on to
 * Notion and comes back with `connected=` or `link_error=`). Members only.
 */
export function useStartNotion(workspaceId: string) {
  return useMutation({
    mutationFn: async (body: StartNotion) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/connections/notion/authorizations", {
          params: path(workspaceId),
          body,
        }),
      ),
  });
}

/** Why a start was refused, by the problem's stable code and status. */
export type NotionStartError = "unavailable" | "not_member" | "rate_limited" | "failed";

/** What a failed `connectors.start_notion` means for the person waiting. */
export function notionStartError(error: unknown): NotionStartError {
  if (problemCode(error) === "provider_not_configured") {
    return "unavailable";
  }
  const status = errorStatus(error);
  if (problemCode(error) === "connection_needs_user" || status === 403 || status === 404) {
    return "not_member";
  }
  return status === 429 ? "rate_limited" : "failed";
}

/**
 * `connectors.list_notion_pages`: the pages a Notion connection can see
 * (what the member shared in Notion's own screen), asked when the import
 * opens. Never retried: a refusal is an answer.
 */
export function useNotionPages(workspaceId: string, connectionId: string | undefined) {
  return useQuery({
    queryKey: operationKey("connectors.list_notion_pages", {
      workspace_id: workspaceId,
      connection_id: connectionId ?? "",
    }),
    enabled: connectionId !== undefined,
    retry: false,
    // Fresh each time the import opens: what was shared in Notion can change.
    gcTime: 0,
    queryFn: async ({ signal }): Promise<NotionPages> =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/connections/{connection_id}/notion/pages", {
          params: { path: { workspace_id: workspaceId, connection_id: connectionId ?? "" } },
          signal,
        }),
      ),
  });
}
