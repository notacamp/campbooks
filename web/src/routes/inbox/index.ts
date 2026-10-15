/**
 * routes/inbox — barrel + route builder.
 */
export { InboxPage } from "./InboxPage";

import { createRoute, type AnyRoute } from "@tanstack/react-router";
import { InboxRoute } from "./InboxRoute";

export const buildInboxRoutes = (parent: AnyRoute): AnyRoute[] => [
  createRoute({
    getParentRoute: () => parent,
    path: "/inbox",
    component: InboxRoute,
    // ?open=<id> — the open conversation (ids may parse as numbers).
    validateSearch: (search: Record<string, unknown>) => ({
      open:
        typeof search.open === "string" || typeof search.open === "number"
          ? String(search.open)
          : undefined,
    }),
  }),
];
