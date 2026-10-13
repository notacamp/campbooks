import { createRoute, Navigate, useParams, type AnyRoute } from "@tanstack/react-router";

import { isStream } from "./api/queries";
import { InboxScreen } from "./components/InboxScreen/InboxScreen";

/** `/w/$workspaceId/inbox…`: a stream, and the conversation open in it. */
function InboxPage() {
  const params: { workspaceId?: string; stream?: string; conversationId?: string } = useParams({
    strict: false,
  });
  const workspaceId = params.workspaceId ?? "";
  const stream = params.stream ?? "";
  if (!isStream(stream)) {
    return (
      <Navigate
        to="/w/$workspaceId/inbox/$stream"
        params={{ workspaceId, stream: "primary" }}
        replace
      />
    );
  }
  return (
    <InboxScreen
      key={`${workspaceId}:${stream}`}
      workspaceId={workspaceId}
      stream={stream}
      conversationId={params.conversationId}
    />
  );
}

export function buildConversationsRoutes<TParent extends AnyRoute>(parent: TParent) {
  const inbox = createRoute({
    getParentRoute: () => parent,
    path: "/inbox",
    component: InboxPage,
  });
  const stream = createRoute({
    getParentRoute: () => parent,
    path: "/inbox/$stream",
    component: InboxPage,
  });
  const conversation = createRoute({
    getParentRoute: () => parent,
    path: "/inbox/$stream/$conversationId",
    component: InboxPage,
  });
  return [inbox, stream, conversation] as const;
}
