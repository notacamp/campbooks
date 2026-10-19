import {
  createRoute,
  Navigate,
  useNavigate,
  useParams,
  useSearch,
  type AnyRoute,
} from "@tanstack/react-router";

import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Button } from "~/lib/ui";

import { isStream } from "./api/queries";
import { InboxScreen } from "./components/InboxScreen/InboxScreen";
import { NewMessage } from "./components/NewMessage/NewMessage";
import { TeachOnceProvider } from "./components/TeachOnce/TeachOnce";
import { renderReplyComposer } from "./components/ReplyComposer/ReplyComposer";

/** A message id, as the API writes them. */
const MESSAGE_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u;

/** `/w/$workspaceId/inbox…`: a stream, and the conversation open in it. */
function InboxPage() {
  const params: { workspaceId?: string; stream?: string; conversationId?: string } = useParams({
    strict: false,
  });
  const search: { reply?: boolean; to?: string } = useSearch({ strict: false });
  const workspaceId = params.workspaceId ?? "";
  const stream = params.stream ?? "";
  const { t } = useTranslation("conversations");
  const navigate = useNavigate();
  const compose = () => {
    void navigate({ to: "/w/$workspaceId/compose", params: { workspaceId } });
  };
  useShortcut("compose.new", compose);
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
    <TeachOnceProvider workspaceId={workspaceId}>
      <InboxScreen
        key={`${workspaceId}:${stream}`}
        workspaceId={workspaceId}
        stream={stream}
        conversationId={params.conversationId}
        renderReplySlot={renderReplyComposer}
        startReply={search.reply === true}
        replyTo={search.to}
        composeAction={
          <Button variant="primary" icon="plus" shortcut="compose.new" onClick={compose}>
            {t("compose.newMessage")}
          </Button>
        }
      />
    </TeachOnceProvider>
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
        void navigate({
          to: "/w/$workspaceId/compose",
          params: { workspaceId },
          search: { draft },
        });
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
    // `?reply=true` (Home's R) opens the reply composer.
    // `reply`: open the composer on arrival; `to`: the message it answers
    // (a draft written for it), else the newest.
    validateSearch: (search: Record<string, unknown>): { reply?: boolean; to?: string } => {
      const reply = search["reply"] === true || search["reply"] === "true";
      const to = search["to"];
      if (!reply) {
        return {};
      }
      return typeof to === "string" && MESSAGE_ID.test(to) ? { reply, to } : { reply };
    },
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
