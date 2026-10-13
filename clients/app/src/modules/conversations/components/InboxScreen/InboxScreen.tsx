import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  EmptyState,
  Heading,
  KbdHint,
  Line,
  ListPane,
  PaneHeader,
  ReaderPane,
  Segmented,
  Spacer,
} from "~/lib/ui";

import {
  useConversations,
  useInboxes,
  type ConversationSummary,
  type Stream,
} from "../../api/queries";
import { streamName, type Translate } from "../../model";
import { ConversationList, rowId } from "../ConversationList/ConversationList";
import { ConversationReader } from "../Reader/ConversationReader";
import type { RenderReplySlot } from "../Reader/ReplySlot";
import { StreamChips, streamPath } from "../StreamNav/StreamNav";

import { useInboxActions } from "./use-inbox-actions";

/** The keys the empty reader teaches. */
const READER_HINTS = ["list.next", "list.previous", "list.open", "item.done"] as const;

/** Where list and reader become separate screens (the shell's large breakpoint). */
const NARROW = "(width < 64rem)";

function isNarrow(): boolean {
  return typeof globalThis.matchMedia === "function" && globalThis.matchMedia(NARROW).matches;
}

export interface InboxScreenProps {
  workspaceId: string;
  stream: Stream;
  /** The conversation open in the reader, if any. */
  conversationId?: string | undefined;
  /**
   * The send slice's composer for Reply / Reply all / Forward (see
   * ReplySlot.ts); the reader hides those actions without it.
   */
  renderReplySlot?: RenderReplySlot | undefined;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/**
 * The inbox (keyed by workspace and stream: a new stream starts at its top,
 * showing everything): a stream's conversations and the reader side by side, or as
 * two screens below the large breakpoint. Every action has its shortcut,
 * and every change can be undone.
 */
export function InboxScreen({
  workspaceId,
  stream,
  conversationId,
  renderReplySlot,
  now,
}: InboxScreenProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const { t: tShortcuts } = useTranslation("shortcuts");
  const navigate = useNavigate();
  const inboxes = useInboxes(workspaceId);
  const list = useConversations(workspaceId, stream);
  const actions = useInboxActions(workspaceId);
  const [show, setShow] = useState<"all" | "unread">("all");
  const [cursor, setCursor] = useState<string | undefined>();

  const loaded = list.data?.pages.flatMap((page) => page.conversations);
  const conversations =
    show === "unread"
      ? loaded?.filter(
          (conversation) => conversation.unread_count > 0 || conversation.id === conversationId,
        )
      : loaded;
  const ids = conversations?.map((conversation) => conversation.id) ?? [];

  const open = (id: string) => {
    setCursor(id);
    void navigate({ to: `${streamPath(workspaceId, stream)}/${id}` });
  };
  const toList = () => {
    void navigate({ to: streamPath(workspaceId, stream) });
  };

  const current = conversationId ?? cursor ?? ids[0];
  const step = (by: number) => {
    const index = current === undefined ? -1 : ids.indexOf(current);
    const next = ids[Math.min(ids.length - 1, Math.max(0, index + by))];
    if (next === undefined) {
      return;
    }
    if (conversationId === undefined) {
      setCursor(next);
      document.getElementById(rowId(next))?.scrollIntoView({ block: "nearest" });
    } else {
      open(next);
    }
  };

  /** Done: archive, then the next conversation (or the list, on phones). */
  const done = (conversation: ConversationSummary) => {
    const index = ids.indexOf(conversation.id);
    const following = ids[index + 1] ?? ids[index - 1];
    actions.done(conversation);
    if (conversationId === conversation.id) {
      if (following === undefined || isNarrow()) {
        toList();
      } else {
        open(following);
      }
    } else {
      setCursor(following);
    }
  };

  const listConversation = (id: string | undefined) =>
    loaded?.find((conversation) => conversation.id === id);

  // In the list (no conversation open): J/K move the cursor, Enter opens,
  // and the item shortcuts act on the row under the cursor.
  const listActive = conversationId === undefined;
  useShortcut(
    "list.next",
    () => {
      step(1);
    },
    { enabled: listActive },
  );
  useShortcut(
    "list.previous",
    () => {
      step(-1);
    },
    { enabled: listActive },
  );
  useShortcut(
    "list.open",
    () => {
      if (current !== undefined) {
        open(current);
      }
    },
    { enabled: listActive },
  );
  useShortcut(
    "item.done",
    () => {
      const conversation = listConversation(current);
      if (conversation !== undefined) {
        done(conversation);
      }
    },
    { enabled: listActive },
  );
  useShortcut(
    "conversations.star",
    () => {
      const conversation = listConversation(current);
      if (conversation !== undefined) {
        actions.toggleStar(conversation);
      }
    },
    { enabled: listActive },
  );
  useShortcut(
    "conversations.mark_unread",
    () => {
      const conversation = listConversation(current);
      if (conversation !== undefined) {
        actions.toggleRead(conversation);
      }
    },
    { enabled: listActive },
  );

  const name = streamName(t, stream);
  const noInbox = inboxes.data?.length === 0;

  return (
    <>
      <ListPane label={t("list.label")} narrow={conversationId === undefined ? "shown" : "hidden"}>
        <PaneHeader>
          <Heading level={1} size="pane">
            {name}
          </Heading>
          <Spacer />
          <Segmented
            label={t("list.show")}
            value={show}
            onChange={setShow}
            options={[
              { value: "all", label: t("list.all") },
              { value: "unread", label: t("list.unread") },
            ]}
          />
        </PaneHeader>
        <div className="px-3 pt-2 md:hidden">
          <StreamChips workspaceId={workspaceId} current={stream} />
        </div>
        {actions.failedOn === undefined ? null : (
          <div className="p-2">
            <Banner
              tone="danger"
              role="alert"
              title={t("error.action", { subject: actions.failedOn })}
              action={
                <Button onClick={actions.dismissFailure}>{tShortcuts("overlay.close")}</Button>
              }
            />
          </div>
        )}
        {noInbox ? (
          <EmptyState title={t("empty.noInboxTitle")} visor="idle" visorSize="lg">
            {t("empty.noInboxBody")}
          </EmptyState>
        ) : (
          <ConversationList
            stream={stream}
            conversations={conversations}
            unreadOnly={show === "unread"}
            failed={list.isError}
            onRetry={() => {
              void list.refetch();
            }}
            hasMore={list.hasNextPage}
            loadingMore={list.isFetchingNextPage}
            onMore={() => {
              void list.fetchNextPage();
            }}
            openId={conversationId}
            cursorId={current}
            onOpen={(conversation) => {
              open(conversation.id);
            }}
            now={now}
          />
        )}
      </ListPane>
      <ReaderPane
        label={t("reader.label")}
        narrow={conversationId === undefined ? "hidden" : "shown"}
      >
        {conversationId === undefined ? (
          <div className="flex flex-col items-center gap-2 p-6">
            <EmptyState title={t("empty.readerTitle")} visor="idle" visorSize="lg">
              {t("empty.readerBody")}
            </EmptyState>
            <div className="flex w-full max-w-measure flex-col">
              {READER_HINTS.map((id) => (
                <Line key={id} trailing={<KbdHint shortcut={id} announce />}>
                  {tShortcuts(id)}
                </Line>
              ))}
            </div>
          </div>
        ) : (
          <ConversationReader
            // Each conversation opens folded, with its images blocked again.
            key={conversationId}
            workspaceId={workspaceId}
            conversationId={conversationId}
            stream={stream}
            actions={actions}
            onBack={toList}
            onPrevious={() => {
              step(-1);
            }}
            onNext={() => {
              step(1);
            }}
            onDone={done}
            renderReplySlot={renderReplySlot}
            now={now}
          />
        )}
      </ReaderPane>
    </>
  );
}
