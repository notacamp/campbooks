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

import { DRAFTS, isPlace } from "./api/queries";
import { DraftsScreen } from "./components/InboxScreen/DraftsScreen";
import { InboxScreen } from "./components/InboxScreen/InboxScreen";
import { NewMessage } from "./components/NewMessage/NewMessage";
import { TeachOnceProvider } from "./components/TeachOnce/TeachOnce";
import { renderReplyComposer } from "./components/ReplyComposer/ReplyComposer";

/** A message id, as the API writes them. */
const MESSAGE_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u;

/**
 * `/w/$workspaceId/inbox…`: a stream or a place beside them (Sent, Pinned,
 * Drafts, Archive, Spam, Trash), and the conversation open in it. The
 * `$stream` param holds any; anything else goes to Primary. Drafts lists
 * drafts, not conversations, so it has a screen of its own.
 */
function InboxPage() {
  const params: { workspaceId?: string; stream?: string; conversationId?: string } = useParams({
    strict: false,
  });
  const search: { reply?: boolean; to?: string; draft?: string } = useSearch({ strict: false });
  const workspaceId = params.workspaceId ?? "";
  const stream = params.stream ?? "";
  const { t } = useTranslation("conversations");
  const navigate = useNavigate();
  const compose = () => {
    void navigate({ to: "/w/$workspaceId/compose", params: { workspaceId } });
  };
  useShortcut("compose.new", compose);
  if (!isPlace(stream)) {
    return (
      <Navigate
        to="/w/$workspaceId/inbox/$stream"
        params={{ workspaceId, stream: "primary" }}
        replace
      />
    );
  }
  const composeAction = (
    <Button variant="primary" icon="plus" shortcut="compose.new" onClick={compose}>
      {t("compose.newMessage")}
    </Button>
  );
  if (stream === DRAFTS) {
    return (
      <TeachOnceProvider workspaceId={workspaceId}>
        <DraftsScreen
          key={workspaceId}
          workspaceId={workspaceId}
          conversationId={params.conversationId}
          draftId={search.draft}
          renderReplySlot={renderReplyComposer}
          composeAction={composeAction}
        />
      </TeachOnceProvider>
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
        composeAction={composeAction}
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
    // `?reply=true` (Home's R) opens the reply composer; `reply`: open the
    // composer on arrival; `to`: the message it answers (a draft written for
    // it), else the newest. `?draft=` (Drafts) opens exactly that draft.
    validateSearch: (
      search: Record<string, unknown>,
    ): { reply?: boolean; to?: string; draft?: string } => {
      const reply = search["reply"] === true || search["reply"] === "true";
      const to = search["to"];
      const draft = search["draft"];
      return {
        ...(reply ? { reply } : {}),
        ...(reply && typeof to === "string" && MESSAGE_ID.test(to) ? { to } : {}),
        ...(typeof draft === "string" && MESSAGE_ID.test(draft) ? { draft } : {}),
      };
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
