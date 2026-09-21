/**
 * routes/inbox — barrel + route builder.
 */
export { InboxPage } from "./InboxPage";

import { createRoute, type AnyRoute } from "@tanstack/react-router";
import { InboxPage } from "./InboxPage";

export const buildInboxRoutes = (parent: AnyRoute): AnyRoute[] => [
  createRoute({
    getParentRoute: () => parent,
    path: "/inbox",
    component: InboxPage,
  }),
];
