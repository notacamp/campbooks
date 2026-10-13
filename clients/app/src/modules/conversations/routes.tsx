import { createRoute, type AnyRoute } from "@tanstack/react-router";

import { ConversationsHome } from "./components/ConversationsHome/ConversationsHome";

export function buildConversationsRoutes<TParent extends AnyRoute>(parent: TParent) {
  const home = createRoute({
    getParentRoute: () => parent,
    path: "/conversations",
    component: ConversationsHome,
  });
  return [home] as const;
}
