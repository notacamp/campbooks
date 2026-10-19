import { useTranslation } from "react-i18next";

import {
  Button,
  ConversationRow,
  EmptyState,
  initialsOf,
  SkeletonList,
  Tag,
  Visor,
  type RowLeaving,
} from "~/lib/ui";

import { isView, type ConversationPlace, type ConversationSummary } from "../../api/queries";
import {
  avatarKind,
  displayName,
  listTime,
  placeName,
  sentToLine,
  streamName,
  type Translate,
} from "../../model";
import { ruleLine } from "../../rules-text";
import { ExpiredChip, NeedChips } from "../Needs/NeedChips";

export interface ConversationListProps {
  /** The stream or view listed: Sent rows say who they went to. */
  place: ConversationPlace;
  /** `undefined` while the first page loads. */
  conversations: readonly ConversationSummary[] | undefined;
  /** Only the unread ones are asked for. */
  unreadOnly: boolean;
  failed: boolean;
  onRetry: () => void;
  hasMore: boolean;
  loadingMore: boolean;
  onMore: () => void;
  /** The conversation open in the reader. */
  openId?: string | undefined;
  /** The keyboard cursor. */
  cursorId?: string | undefined;
  onOpen: (conversation: ConversationSummary) => void;
  /**
   * Rows leaving the list (Done, Delete, Report spam, a move), by id: each
   * says what happened, then folds away and `onLeft` runs.
   */
  leaving?: ReadonlyMap<string, RowLeaving> | undefined;
  onLeft?: ((conversationId: string) => void) | undefined;
  /** For "now": tests and stories pin the clock. */
  now?: Date | undefined;
}

export const rowId = (conversationId: string) => `conversation-${conversationId}`;

/**
 * A stream's or a view's conversations, with their loading, empty and error
 * states. In Sent each row leads with who the inbox wrote to, at the time it
 * wrote, and says when they replied since. A row leaving the list says what
 * happened to it before it folds away.
 */
export function ConversationList({
  place,
  conversations,
  unreadOnly,
  failed,
  onRetry,
  hasMore,
  loadingMore,
  onMore,
  openId,
  cursorId,
  onOpen,
  leaving,
  onLeft,
  now,
}: ConversationListProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const name = placeName(t, place);
  const sent = place === "sent";

  if (conversations === undefined) {
    return failed ? (
      <EmptyState
        title={t("error.listTitle", { stream: name })}
        visor="thinking"
        visorSize="lg"
        action={<Button onClick={onRetry}>{t("error.retry")}</Button>}
      >
        {t("error.listBody")}
      </EmptyState>
    ) : (
      <SkeletonList rows={6} />
    );
  }
  if (conversations.length === 0) {
    if (isView(place) && !unreadOnly) {
      // One plain line: nothing sorts mail into a view.
      return <EmptyState title={t(`empty.${place}`)} visor="idle" visorSize="lg" />;
    }
    return (
      <EmptyState
        title={t(unreadOnly ? "empty.unreadTitle" : "empty.streamTitle", { stream: name })}
        visor="happy"
        visorSize="lg"
      >
        {unreadOnly ? t("empty.unreadBody") : t("empty.streamBody", { stream: name })}
      </EmptyState>
    );
  }
  return (
    <div className="flex flex-col gap-0-5 p-1-5">
      {conversations.map((conversation) => {
        // Scout says nothing about mail in spam or the trash.
        const scout = !conversation.trashed;
        const suggestion = scout ? conversation.suggestion : undefined;
        // Sent: who the inbox wrote to, when it last wrote.
        const at = sent
          ? (conversation.last_sent_at ?? conversation.last_message_at)
          : conversation.last_message_at;
        const to = sent ? conversation.sent_to[0] : undefined;
        return (
          <ConversationRow
            key={conversation.id}
            id={rowId(conversation.id)}
            from={sent ? sentToLine(t, conversation) : displayName(conversation.latest_from)}
            avatar={
              sent
                ? {
                    kind: "person",
                    ...(to === undefined ? {} : { initials: initialsOf(displayName(to)) }),
                  }
                : { kind: avatarKind(conversation) }
            }
            time={listTime(at, i18n.language, now)}
            dateTime={at}
            subject={conversation.subject === "" ? t("reader.noSubject") : conversation.subject}
            snippet={
              conversation.latest_from_you
                ? t("list.youPrefix", { snippet: conversation.snippet })
                : conversation.snippet
            }
            unread={conversation.unread_count > 0}
            starred={conversation.starred}
            count={conversation.message_count}
            current={conversation.id === openId}
            selected={conversation.id === cursorId && conversation.id !== openId}
            chips={
              sent && conversation.replied_since_sent ? (
                <Tag icon="reply">{t("list.replied")}</Tag>
              ) : conversation.need === null || conversation.need === undefined ? (
                conversation.lapsed_need === null ||
                conversation.lapsed_need === undefined ? undefined : (
                  <ExpiredChip />
                )
              ) : (
                <NeedChips need={conversation.need} now={now} />
              )
            }
            suggestion={
              suggestion === null || suggestion === undefined ? (
                !scout ||
                conversation.rule === null ||
                conversation.rule === undefined ? undefined : (
                  <span className="inline-flex items-center gap-1-5">
                    <Visor size="sm" />
                    {ruleLine(t, conversation.rule, i18n.language)}
                  </span>
                )
              ) : (
                <span className="inline-flex items-center gap-1-5">
                  <Visor size="sm" />
                  {t("scout.suggestion", { stream: streamName(t, suggestion.stream) })}
                </span>
              )
            }
            leaving={leaving?.get(conversation.id)}
            onLeft={() => {
              onLeft?.(conversation.id);
            }}
            onSelect={() => {
              onOpen(conversation);
            }}
          />
        );
      })}
      {hasMore ? (
        <div className="flex justify-center p-2">
          <Button variant="ghost" onClick={onMore} disabled={loadingMore}>
            {loadingMore ? t("list.loadingMore") : t("list.more")}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
