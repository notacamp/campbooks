import { createRoute, useParams, useSearch, type AnyRoute } from "@tanstack/react-router";

import { PagesPage } from "./components/PagesPage";

/**
 * `/w/$workspaceId/pages` and `/pages/$pageId`: the tree, with a page open
 * beside it. `/pages?new=1` makes a page and opens it (`n g`).
 */
export function buildPagesRoutes<TParent extends AnyRoute>(parent: TParent) {
  function PagesRoute() {
    const params: { workspaceId?: string; pageId?: string } = useParams({ strict: false });
    const search: { new?: 1 } = useSearch({ strict: false });
    return (
      <PagesPage
        key={params.workspaceId}
        workspaceId={params.workspaceId ?? ""}
        pageId={params.pageId}
        creating={params.pageId === undefined && search.new === 1}
      />
    );
  }
  // `new` is the form's only state: present (1) or not.
  const validateSearch = (search: Record<string, unknown>): { new?: 1 } =>
    search["new"] === 1 || search["new"] === "1" || search["new"] === true ? { new: 1 } : {};
  const list = createRoute({
    getParentRoute: () => parent,
    path: "/pages",
    validateSearch,
    component: PagesRoute,
  });
  const page = createRoute({
    getParentRoute: () => parent,
    path: "/pages/$pageId",
    component: PagesRoute,
  });
  return [list, page] as const;
}
