import { createRoute, useParams, useSearch, type AnyRoute } from "@tanstack/react-router";

import { PagesPage } from "./components/PagesPage";
import type { UseNotionSource } from "./notion-source";

/**
 * `/w/$workspaceId/pages` and `/pages/$pageId`: the tree, with a page open
 * beside it. `/pages?new=1` makes a page and opens it (`n g`).
 */
export interface PagesRouteOptions {
  /** Notion's pages and connections, from the app shell: the import from Notion. */
  useNotionSource?: UseNotionSource | undefined;
}

export function buildPagesRoutes<TParent extends AnyRoute>(
  parent: TParent,
  { useNotionSource }: PagesRouteOptions = {},
) {
  function PagesRoute() {
    const params: { workspaceId?: string; pageId?: string } = useParams({ strict: false });
    const search: { new?: 1; import?: "notion" } = useSearch({ strict: false });
    return (
      <PagesPage
        key={params.workspaceId}
        workspaceId={params.workspaceId ?? ""}
        pageId={params.pageId}
        creating={params.pageId === undefined && search.new === 1}
        importing={search.import === "notion"}
        notion={useNotionSource === undefined ? undefined : { useSource: useNotionSource }}
      />
    );
  }
  // `new` and `import` are the page's only state: present (1, notion) or not.
  const validateSearch = (search: Record<string, unknown>): { new?: 1; import?: "notion" } => ({
    ...(search["new"] === 1 || search["new"] === "1" || search["new"] === true ? { new: 1 as const } : {}),
    ...(search["import"] === "notion" ? { import: "notion" as const } : {}),
  });
  const list = createRoute({
    getParentRoute: () => parent,
    path: "/pages",
    validateSearch,
    component: PagesRoute,
  });
  const page = createRoute({
    getParentRoute: () => parent,
    path: "/pages/$pageId",
    validateSearch,
    component: PagesRoute,
  });
  return [list, page] as const;
}
