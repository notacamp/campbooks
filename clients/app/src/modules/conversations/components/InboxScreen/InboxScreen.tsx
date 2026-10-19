import { useNavigate } from "@tanstack/react-router";
import { useState, type ReactNode } from "react";
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
  type RowLeaving,
} from "~/lib/ui";

import {
  applied,
  belongsIn,
  useConversations,
  useInboxes,
  type ConversationSummary,
  type Place,
  type Stream,
} from "../../api/queries";
import {
  archiveMove,
  placeName,
  spamMove,
  streamName,
  trashMove,
  withLeaving,
  type LeavingRow,
  type Move,
  type Translate,
} from "../../model";
import { ConversationList, rowId } from "../ConversationList/ConversationList";
import { ConversationReader } from "../Reader/ConversationReader";
import type { RenderReplySlot } from "../Reader/ReplySlot";
import { StreamChips, streamPath } from "../StreamNav/StreamNav";

import { useInboxActions } from "./use-inbox-actions";

/** The keys the empty reader teaches. */
const READER_HINTS = ["list.next", "list.previous", "list.open", "item.done"] as const;

/** What a row leaving the list says, by move: spam and the trash in the danger colour. */
const LEAVING: Record<Move, { key: string; icon: RowLeaving["icon"]; tone: RowLeaving["tone"] }> = {
  archive: { key: "leaving.done", icon: "archive", tone: "done" },
  unarchive: { key: "leaving.toInbox", icon: "inbox", tone: "done" },
  trash: { key: "leaving.deleted", icon: "trash", tone: "danger" },
  untrash: { key: "leaving.restored", icon: "check", tone: "done" },
  spam: { key: "leaving.spam", icon: "warning", tone: "danger" },
  not_spam: { key: "leaving.notSpam", icon: "check", tone: "done" },
};

/** How long a row takes to leave (`--duration-confirm`, as `ConversationRow`'s fold). */
const LEAVE_MS = 900;

/** A row leaving the list, and what it says while it folds away. */
interface Farewell extends LeavingRow {
  said: RowLeaving;
}

/** Where list and reader become separate screens (the shell's large breakpoint). */
const NARROW = "(width < 64rem)";

function isNarrow(): boolean {
  return typeof globalThis.matchMedia === "function" && globalThis.matchMedia(NARROW).matches;
}

export interface InboxScreenProps {
  workspaceId: string;
  /** The stream listed, or a view beside them (Sent, Archive, Spam, Trash). */
  stream: Place;
  /** The conversation open in the reader, if any. */
  conversationId?: string | undefined;
  /**
   * The send slice's composer for Reply / Reply all / Forward (see
   * ReplySlot.ts); the reader hides those actions without it.
   */
  renderReplySlot?: RenderReplySlot | undefined;
  /** A list-header action from the send slice ("New message", C). */
  composeAction?: ReactNode | undefined;
  /** Open the reply composer once the conversation is loaded (Home's R). */
  startReply?: boolean | undefined;
  /** The message that reply answers (a draft written for it), else the newest. */
  replyTo?: string | undefined;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/**
 * The inbox (keyed by workspace and place: a new stream or view starts at
 * its top, showing everything): a stream's or a view's conversations and the
 * reader side by side, or as two screens below the large breakpoint. Every
 * action has its shortcut, and every change can be undone.
 */
export function InboxScreen({
  workspaceId,
  stream,
  conversationId,
  renderReplySlot,
  composeAction,
  startReply = false,
  replyTo,
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
  const [leaving, setLeaving] = useState<readonly Farewell[]>([]);

  const loaded = list.data?.pages.flatMap((page) => page.conversations);
  const conversations =
    show === "unread"
      ? loaded?.filter(
          (conversation) => conversation.unread_count > 0 || conversation.id === conversationId,
        )
      : loaded;
  const ids = conversations?.map((conversation) => conversation.id) ?? [];
  // The rows leaving the list stay in place while they say what happened,
  // out of the keyboard's reach; one back on the list (Undo while it
  // folded) is a plain row again.
  const folding = leaving.filter(
    (row) => !(conversations ?? []).some((conversation) => conversation.id === row.conversation.id),
  );
  const shown = conversations === undefined ? undefined : withLeaving(conversations, folding);

  /** It is leaving the list on screen: it says what happened, then folds away. */
  const farewell = (conversation: ConversationSummary, said: RowLeaving) => {
    const at = shown?.findIndex((row) => row.id === conversation.id) ?? -1;
    if (shown === undefined || at === -1) {
      return;
    }
    const before = shown[at + 1]?.id;
    const row: Farewell = { conversation, before, at, said };
    setLeaving((rows) => [
      ...rows.filter((other) => other.conversation.id !== conversation.id),
      row,
    ]);
    // Gone once it has folded, whether or not it was still drawn by then.
    setTimeout(() => {
      setLeaving((rows) => rows.filter((other) => other !== row));
    }, LEAVE_MS);
  };

  const open = (id: string) => {
    setCursor(id);
    void navigate({ to: `${streamPath(workspaceId, stream)}/${id}` });
  };
  const toList = () => {
    void navigate({ to: streamPath(workspaceId, stream) });
  };

  const current = conversationId ?? cursor ?? ids[0];
  const step = (by: number) => {
    // The first J or ↓ in a list lands on its first row.
    const index =
      current === undefined || (conversationId === undefined && cursor === undefined)
        ? -by
        : ids.indexOf(current);
    // Moving past the last row loaded asks for the next page.
    // A held key repeats faster than the list re-renders: keep the request in flight.
    if (by > 0 && index + by >= ids.length && list.hasNextPage && !list.isFetchingNextPage) {
      void list.fetchNextPage({ cancelRefetch: false });
    }
    const next = ids[Math.min(ids.length - 1, Math.max(0, index + by))];
    if (next === undefined) {
      return;
    }
    // Not every environment scrolls (tests run without layout).
    const row: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
      document.getElementById(rowId(next));
    row?.scrollIntoView?.({ block: "nearest" });
    if (conversationId === undefined) {
      setCursor(next);
    } else if (next !== conversationId) {
      open(next);
    }
  };

  /**
   * Move it to another place (Done, Delete, Report spam, or back); once it
   * leaves this list, the next conversation (or the list, on phones).
   */
  const leave = (conversation: ConversationSummary, move: Move) => {
    actions.moveTo(conversation, move);
    if (belongsIn(stream, applied(conversation, move))) {
      return;
    }
    const said = LEAVING[move];
    farewell(conversation, { label: t(said.key), icon: said.icon, tone: said.tone });
    const index = ids.indexOf(conversation.id);
    const following = ids[index + 1] ?? ids[index - 1];
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

  // In the list (no conversation open): J/K or ↓/↑ move the cursor, Enter
  // opens, and the item shortcuts act on the row under the cursor.
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
  /** The row under the cursor and `pick`'s move for it, when it has one. */
  const onCursor = (pick: (conversation: ConversationSummary) => Move | undefined) => () => {
    const conversation = listConversation(current);
    const move = conversation === undefined ? undefined : pick(conversation);
    if (conversation !== undefined && move !== undefined) {
      leave(conversation, move);
    }
  };
  useShortcut("item.done", onCursor(archiveMove), { enabled: listActive });
  useShortcut("conversations.trash", onCursor(trashMove), { enabled: listActive });
  useShortcut("conversations.report_spam", onCursor(spamMove), { enabled: listActive });
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

  /** Move it to another stream; out of this one, its row says where it went. */
  const moveToStream = (conversation: ConversationSummary, target: Stream) => {
    actions.move(conversation, target);
    if (
      target !== conversation.stream &&
      !belongsIn(stream, applied(conversation, { stream: target }))
    ) {
      farewell(conversation, {
        label: t("leaving.moved", { stream: streamName(t, target) }),
        icon: "move",
      });
    }
  };

  const name = placeName(t, stream);
  const noInbox = inboxes.data?.length === 0;

  return (
    <>
      <ListPane label={t("list.label")} narrow={conversationId === undefined ? "shown" : "hidden"}>
        <PaneHeader>
          <Heading level={1} size="pane">
            {name}
          </Heading>
          <Spacer />
          {composeAction}
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
            place={stream}
            conversations={shown}
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
            cursorId={cursor}
            onOpen={(conversation) => {
              open(conversation.id);
            }}
            leaving={new Map(folding.map((row) => [row.conversation.id, row.said]))}
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
                <Line key={id} trailing={<KbdHint shortcut={id} withAlternates announce />}>
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
            actions={{ ...actions, move: moveToStream }}
            onBack={toList}
            onPrevious={() => {
              step(-1);
            }}
            onNext={() => {
              step(1);
            }}
            onMove={leave}
            renderReplySlot={renderReplySlot}
            startReply={startReply}
            replyTo={replyTo}
            now={now}
          />
        )}
      </ReaderPane>
    </>
  );
}
