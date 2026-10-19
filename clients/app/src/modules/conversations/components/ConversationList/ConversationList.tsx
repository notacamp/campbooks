import { useTranslation } from "react-i18next";

import { Button, ConversationRow, EmptyState, SkeletonList, Visor } from "~/lib/ui";

import type { ConversationSummary, Stream } from "../../api/queries";
import { avatarKind, displayName, listTime, streamName, type Translate } from "../../model";
import { ruleLine } from "../../rules-text";
import { ExpiredChip, NeedChips } from "../Needs/NeedChips";

export interface ConversationListProps {
  stream: Stream;
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
  /** For "now": tests and stories pin the clock. */
  now?: Date | undefined;
}

export const rowId = (conversationId: string) => `conversation-${conversationId}`;

/** A stream's conversations, newest first, with their loading, empty and error states. */
export function ConversationList({
  stream,
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
  now,
}: ConversationListProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const name = streamName(t, stream);

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
        const suggestion = conversation.suggestion;
        return (
          <ConversationRow
            key={conversation.id}
            id={rowId(conversation.id)}
            from={displayName(conversation.latest_from)}
            avatar={{ kind: avatarKind(conversation) }}
            time={listTime(conversation.last_message_at, i18n.language, now)}
            dateTime={conversation.last_message_at}
            subject={conversation.subject === "" ? t("reader.noSubject") : conversation.subject}
            snippet={conversation.snippet}
            unread={conversation.unread_count > 0}
            starred={conversation.starred}
            count={conversation.message_count}
            current={conversation.id === openId}
            selected={conversation.id === cursorId && conversation.id !== openId}
            chips={
              conversation.need === null || conversation.need === undefined ? (
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
                conversation.rule === null || conversation.rule === undefined ? undefined : (
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
