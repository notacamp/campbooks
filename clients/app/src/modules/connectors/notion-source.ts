import { errorStatus, problemCode } from "~/lib/api";

import { useNotionPages } from "./api/notion";
import { useNotionConnections } from "./api/queries";

/** Why the pages of a Notion connection could not be listed. */
export type NotionListError = "needs_reauth" | "rate_limited" | "not_connected" | "unavailable";

/** What a failed `connectors.list_notion_pages` means for the person importing. */
export function notionListError(error: unknown): NotionListError {
  if (problemCode(error) === "needs_reauth") {
    return "needs_reauth";
  }
  const status = errorStatus(error);
  if (status === 429) {
    return "rate_limited";
  }
  return status === 403 || status === 404 ? "not_connected" : "unavailable";
}

/** A page the connection can see. */
export interface NotionSourcePage {
  notion_page: string;
  title: string;
  edited_at: string;
  parent: string | null;
  sub_pages: number;
  linked: boolean;
}

/**
 * What the Pages import needs of Notion, handed to the `pages` module by the
 * app shell (the pages module never imports `connectors`): the caller's own
 * Notion connections, and the pages of the chosen one.
 */
export interface NotionSource {
  /** The caller's Notion connections (undefined until known). */
  connections: readonly { id: string; name: string; needsReauth: boolean }[] | undefined;
  list: {
    status: "idle" | "loading" | "ready" | "error";
    pages: readonly NotionSourcePage[];
    truncated: boolean;
    error: NotionListError | undefined;
    retry: () => void;
  };
}

export type UseNotionSource = (
  workspaceId: string,
  connectionId: string | undefined,
) => NotionSource;

/** `connections` + `connectors.list_notion_pages`, in the shape the import asks for. */
export const useNotionSource: UseNotionSource = (workspaceId, connectionId) => {
  const connections = useNotionConnections(workspaceId);
  const pages = useNotionPages(workspaceId, connectionId);
  let status: NotionSource["list"]["status"] = "idle";
  if (connectionId !== undefined) {
    if (pages.isError) {
      status = "error";
    } else {
      status = pages.isSuccess ? "ready" : "loading";
    }
  }
  return {
    connections: connections.data
      ?.filter((connection) => connection.granted_by_you)
      .map((connection) => ({
        id: connection.id,
        name: connection.display_name ?? "Notion",
        needsReauth: connection.health === "needs_reauth",
      })),
    list: {
      status,
      pages: pages.data?.pages ?? [],
      truncated: pages.data?.truncated ?? false,
      error: pages.isError ? notionListError(pages.error) : undefined,
      retry: () => {
        void pages.refetch();
      },
    },
  };
};
