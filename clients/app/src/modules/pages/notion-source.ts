/**
 * What the import from Notion needs of Notion, handed to this module by the
 * app shell (the `connectors` module's `useNotionSource`; this module never
 * imports it): the caller's own Notion connections, and the pages the chosen
 * one can see.
 */
export interface NotionSourcePage {
  notion_page: string;
  title: string;
  edited_at: string;
  parent?: string | null;
  sub_pages: number;
  linked: boolean;
}

export interface NotionSource {
  /** The caller's Notion connections (undefined until known). */
  connections: readonly { id: string; name: string; needsReauth: boolean }[] | undefined;
  list: {
    status: "idle" | "loading" | "ready" | "error";
    pages: readonly NotionSourcePage[];
    truncated: boolean;
    error: "needs_reauth" | "rate_limited" | "not_connected" | "unavailable" | undefined;
    retry: () => void;
  };
}

export type UseNotionSource = (
  workspaceId: string,
  connectionId: string | undefined,
) => NotionSource;
