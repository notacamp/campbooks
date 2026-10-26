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
    const search: { new?: 1; import?: "notion"; view?: string; row?: string } = useSearch({
      strict: false,
    });
    return (
      <PagesPage
        key={params.workspaceId}
        workspaceId={params.workspaceId ?? ""}
        pageId={params.pageId}
        creating={params.pageId === undefined && search.new === 1}
        importing={search.import === "notion"}
        view={search.view}
        row={search.row}
        notion={useNotionSource === undefined ? undefined : { useSource: useNotionSource }}
      />
    );
  }
  // `new` and `import` are present (1, notion) or not; `view` and `row` name a table's view and row.
  const validateSearch = (
    search: Record<string, unknown>,
  ): { new?: 1; import?: "notion"; view?: string; row?: string } => ({
    ...(search["new"] === 1 || search["new"] === "1" || search["new"] === true
      ? { new: 1 as const }
      : {}),
    ...(search["import"] === "notion" ? { import: "notion" as const } : {}),
    // A table page's current view and the row opened beside it.
    ...(typeof search["view"] === "string" && search["view"] !== ""
      ? { view: search["view"] }
      : {}),
    ...(typeof search["row"] === "string" && search["row"] !== "" ? { row: search["row"] } : {}),
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
