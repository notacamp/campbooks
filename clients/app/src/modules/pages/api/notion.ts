import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";

import { api, errorStatus, operationKey, unwrap } from "~/lib/api";

import type {
  ImportFromNotionBody,
  Imported,
  NotionKeep,
  NotionLink,
  NotionLinks,
  Resolved,
  Undone,
} from "../types";

// Keyed by operationId, so the change feed refetches them when a
// `pages.page` changes (a sync moved, a conflict was raised).
const LINKS = "pages.list_notion_links";

const linksKey = (workspaceId: string) => operationKey(LINKS, { workspace_id: workspaceId });

/**
 * `pages.list_notion_links`: which of the pages the caller may open are kept
 * in step with Notion, and how that is going. A server without Notion (or
 * without Pages) answers 404: that is no links, not a failure.
 */
export function useNotionLinks(workspaceId: string) {
  return useQuery({
    queryKey: linksKey(workspaceId),
    retry: false,
    queryFn: async ({ signal }): Promise<NotionLinks> => {
      try {
        return unwrap(
          await api.GET("/api/v1/w/{workspace_id}/pages/notion/links", {
            params: { path: { workspace_id: workspaceId } },
            signal,
          }),
        );
      } catch (error) {
        if (errorStatus(error) === 404) {
          return { links: [] };
        }
        throw error;
      }
    },
    select: (list: NotionLinks) => list.links,
  });
}

/** The link of one page, if it has one. */
export function useNotionLink(workspaceId: string, pageId: string): NotionLink | undefined {
  return useNotionLinks(workspaceId).data?.find((link) => link.page === pageId);
}

/** The calls Notion adds to a page. Each answers what the server made of it. */
export function useNotionCalls(workspaceId: string) {
  return useMemo(() => notionCalls(workspaceId), [workspaceId]);
}

function notionCalls(workspaceId: string) {
  const path = { workspace_id: workspaceId };
  const at = (page: string) => ({ path: { ...path, page_id: page } });
  return {
    import: async (body: ImportFromNotionBody): Promise<Imported> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/notion/imports", {
          params: { path },
          body,
        }),
      ),
    resolve: async (page: string, block: string, keep: NotionKeep): Promise<Resolved> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/notion/conflicts/{block}", {
          params: { path: { ...path, page_id: page, block } },
          body: { keep },
        }),
      ),
    undo: async (page: string, resolution: string): Promise<Undone> =>
      unwrap(
        await api.POST(
          "/api/v1/w/{workspace_id}/pages/{page_id}/notion/resolutions/{resolution}/undo",
          { params: { path: { ...path, page_id: page, resolution } } },
        ),
      ),
    stop: async (page: string): Promise<NotionLink> =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/pages/{page_id}/notion/stop", {
          params: at(page),
        }),
      ),
  };
}

/** Refetch the links, and the page itself (its body changed when a choice was made). */
export function useRefreshNotion(workspaceId: string) {
  const queryClient = useQueryClient();
  return (page?: string) => {
    void queryClient.invalidateQueries({ queryKey: linksKey(workspaceId) });
    if (page !== undefined) {
      void queryClient.invalidateQueries({
        queryKey: operationKey("pages.get", { workspace_id: workspaceId, page_id: page }),
      });
    }
  };
}
