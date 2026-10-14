import { createRoute, useParams, useSearch, type AnyRoute } from "@tanstack/react-router";

import type { SearchSort } from "./api/queries";
import { SearchScreen } from "./components/SearchScreen/SearchScreen";

interface SearchParams {
  q?: string;
  sort?: SearchSort;
}

/** `?q=` and `?sort=relevance`; the defaults (empty, newest first) stay out of the address. */
function validateSearch(search: Record<string, unknown>): SearchParams {
  const q = typeof search["q"] === "string" ? search["q"] : "";
  return {
    ...(q === "" ? {} : { q }),
    ...(search["sort"] === "relevance" ? { sort: "relevance" as const } : {}),
  };
}

/** `/w/$workspaceId/search[/$conversationId]?q=`: the results and the reader beside them. */
function SearchPage() {
  const params: { workspaceId?: string; conversationId?: string } = useParams({ strict: false });
  const search: SearchParams = useSearch({ strict: false });
  return (
    <SearchScreen
      workspaceId={params.workspaceId ?? ""}
      query={search.q ?? ""}
      sort={search.sort ?? "newest"}
      conversationId={params.conversationId}
    />
  );
}

export function buildSearchRoutes<TParent extends AnyRoute>(parent: TParent) {
  const results = createRoute({
    getParentRoute: () => parent,
    path: "/search",
    validateSearch,
    component: SearchPage,
  });
  const conversation = createRoute({
    getParentRoute: () => parent,
    path: "/search/$conversationId",
    validateSearch,
    component: SearchPage,
  });
  return [results, conversation] as const;
}
