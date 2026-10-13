import {
  createRoute,
  Navigate,
  useNavigate,
  useParams,
  useSearch,
  type AnyRoute,
} from "@tanstack/react-router";

import { useShortcut } from "~/lib/shortcuts";

import { isStream } from "./api/queries";
import { InboxScreen } from "./components/InboxScreen/InboxScreen";
import { NewMessage } from "./components/NewMessage/NewMessage";
import { renderReplyComposer } from "./components/ReplyComposer/ReplyComposer";

/** `/w/$workspaceId/inbox…`: a stream, and the conversation open in it. */
function InboxPage() {
  const params: { workspaceId?: string; stream?: string; conversationId?: string } = useParams({
    strict: false,
  });
  const workspaceId = params.workspaceId ?? "";
  const stream = params.stream ?? "";
  const navigate = useNavigate();
  useShortcut("compose.new", () => {
    void navigate({ to: "/w/$workspaceId/compose", params: { workspaceId } });
  });
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
      renderReplySlot={renderReplyComposer}
    />
  );
}

/** `/w/$workspaceId/compose`: a new message (`?draft=` reopens one). */
function ComposePage() {
  const params: { workspaceId?: string } = useParams({ strict: false });
  const search: { draft?: string } = useSearch({ strict: false });
  const workspaceId = params.workspaceId ?? "";
  const navigate = useNavigate();
  return (
    <NewMessage
      key={search.draft ?? "new"}
      workspaceId={workspaceId}
      draftId={search.draft}
      onLeave={() => {
        void navigate({ to: "/w/$workspaceId/inbox", params: { workspaceId } });
      }}
      onReopen={(draft) => {
        void navigate({ to: "/w/$workspaceId/compose", params: { workspaceId }, search: { draft } });
      }}
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
  const compose = createRoute({
    getParentRoute: () => parent,
    path: "/compose",
    validateSearch: (search: Record<string, unknown>): { draft?: string } =>
      typeof search["draft"] === "string" ? { draft: search["draft"] } : {},
    component: ComposePage,
  });
  return [inbox, stream, conversation, compose] as const;
}
