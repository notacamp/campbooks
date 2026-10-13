import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  ActionBar,
  Button,
  EmptyState,
  Heading,
  IconButton,
  MetaLine,
  Note,
  PaneHeader,
  ReaderBody,
  SkeletonList,
  Spacer,
} from "~/lib/ui";

import { useConversation, type ConversationSummary, type Stream } from "../../api/queries";
import {
  avatarKind,
  displayName,
  noteSentence,
  openByDefault,
  placementLine,
  streamName,
  whyLines,
  type Translate,
} from "../../model";
import type { InboxActions } from "../InboxScreen/use-inbox-actions";

import { MovePicker } from "./MovePicker";
import type { RenderReplySlot, ReplyMode, ReplyRequest } from "./ReplySlot";
import { ThreadMessage } from "./ThreadMessage";

export interface ConversationReaderProps {
  workspaceId: string;
  conversationId: string;
  /** The stream the list shows (Back goes there). */
  stream: Stream;
  actions: InboxActions;
  onBack: () => void;
  onPrevious: () => void;
  onNext: () => void;
  /** Done: archive, then whatever the screen opens next. */
  onDone: (conversation: ConversationSummary) => void;
  /** The send slice's composer; Reply, Reply all and Forward are hidden without it. */
  renderReplySlot?: RenderReplySlot | undefined;
  now?: Date | undefined;
}

/**
 * A conversation: its actions (with their shortcuts), the assistant's note
 * (suggestions with their reasons), the thread newest last with older
 * messages folded, and the reply slot.
 */
export function ConversationReader({
  workspaceId,
  conversationId,
  stream,
  actions,
  onBack,
  onPrevious,
  onNext,
  onDone,
  renderReplySlot,
  now,
}: ConversationReaderProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const { t: tShortcuts } = useTranslation("shortcuts");
  const query = useConversation(workspaceId, conversationId);
  const detail = query.data;
  const conversation = detail?.conversation;

  const [opened, setOpened] = useState<Set<string>>(new Set());
  const [images, setImages] = useState<Set<string>>(new Set());
  const [moving, setMoving] = useState(false);
  const [reply, setReply] = useState<ReplyRequest | null>(null);
  const nonce = useRef(0);

  // Opening an unread conversation reads it (once).
  const readOnce = useRef<string | null>(null);
  useEffect(() => {
    if (
      conversation !== undefined &&
      conversation.unread_count > 0 &&
      readOnce.current !== conversation.id
    ) {
      readOnce.current = conversation.id;
      actions.markRead(conversation);
    }
  }, [conversation, actions]);

  const messages = detail?.messages ?? [];
  const open = new Set([...openByDefault(messages), ...opened]);
  const newest = messages.at(-1);
  const suggestion = conversation?.suggestion ?? undefined;
  const active = conversation !== undefined;

  const askReply = (mode: ReplyMode) => {
    if (newest === undefined) {
      return;
    }
    nonce.current += 1;
    setReply({ mode, messageId: newest.id, nonce: nonce.current });
  };
  const showAllImages = () => {
    setImages(new Set(messages.map((message) => message.id)));
  };

  useShortcut(
    "item.done",
    () => {
      if (conversation !== undefined) {
        onDone(conversation);
      }
    },
    { enabled: active },
  );
  useShortcut(
    "conversations.star",
    () => {
      if (conversation !== undefined) {
        actions.toggleStar(conversation);
      }
    },
    { enabled: active },
  );
  useShortcut(
    "conversations.mark_unread",
    () => {
      if (conversation !== undefined) {
        actions.toggleRead(conversation);
      }
    },
    { enabled: active },
  );
  useShortcut(
    "conversations.move_to_stream",
    () => {
      setMoving(true);
    },
    { enabled: active },
  );
  useShortcut("inbox.show_images", showAllImages, { enabled: active });
  useShortcut("overlay.close", onBack, { enabled: !moving });
  useShortcut("list.next", onNext);
  useShortcut("list.previous", onPrevious);
  const canReply = active && renderReplySlot !== undefined;
  useShortcut(
    "item.reply",
    () => {
      askReply("reply");
    },
    { enabled: canReply },
  );
  useShortcut(
    "item.reply_all",
    () => {
      askReply("reply_all");
    },
    { enabled: canReply },
  );
  useShortcut(
    "item.forward",
    () => {
      askReply("forward");
    },
    { enabled: canReply },
  );
  useShortcut(
    "item.choice1",
    () => {
      if (conversation !== undefined && suggestion !== undefined) {
        actions.move(conversation, suggestion.stream);
      }
    },
    { enabled: suggestion !== undefined },
  );

  // Wrappers carry the responsive classes: a component's own styles win
  // over utilities.
  const back = (
    <span className="flex lg:hidden">
      <Button variant="ghost" icon="chevron-left" onClick={onBack} shortcut="overlay.close">
        {t("reader.back", { stream: streamName(t, stream) })}
      </Button>
    </span>
  );

  if (conversation === undefined) {
    const gone = errorStatus(query.error) === 404;
    return (
      <>
        <PaneHeader>{back}</PaneHeader>
        {query.isError ? (
          <EmptyState
            title={t(gone ? "error.goneTitle" : "error.conversationTitle")}
            visor="thinking"
            visorSize="lg"
            action={
              gone ? undefined : (
                <Button
                  onClick={() => {
                    void query.refetch();
                  }}
                >
                  {t("error.retry")}
                </Button>
              )
            }
          >
            {t(gone ? "error.goneBody" : "error.conversationBody")}
          </EmptyState>
        ) : (
          <ReaderBody>
            <SkeletonList rows={3} />
          </ReaderBody>
        )}
      </>
    );
  }

  const kind = avatarKind(conversation);
  const subject = conversation.subject === "" ? t("reader.noSubject") : conversation.subject;
  const decisions = detail?.decisions ?? [];
  const exists = (key: string) => i18n.exists(`conversations:${key}`);
  const unread = conversation.unread_count > 0;
  const replyButtons =
    renderReplySlot === undefined ? null : (
      <span className="flex gap-1 max-md:hidden">
        <Button
          variant="ghost"
          icon="reply"
          shortcut="item.reply"
          onClick={() => {
            askReply("reply");
          }}
        >
          {tShortcuts("item.reply")}
        </Button>
        <Button
          variant="ghost"
          icon="reply-all"
          shortcut="item.reply_all"
          onClick={() => {
            askReply("reply_all");
          }}
        >
          {t("actions.replyAll")}
        </Button>
        <Button
          variant="ghost"
          icon="forward"
          shortcut="item.forward"
          onClick={() => {
            askReply("forward");
          }}
        >
          {t("actions.forward")}
        </Button>
      </span>
    );

  return (
    <>
      <PaneHeader>
        {back}
        <span className="flex max-md:hidden">
          <Button
            icon="archive"
            shortcut="item.done"
            onClick={() => {
              onDone(conversation);
            }}
          >
            {tShortcuts("item.done")}
          </Button>
        </span>
        <IconButton
          label={conversation.starred ? t("actions.unstar") : t("actions.star")}
          icon={conversation.starred ? "star-filled" : "star"}
          shortcut="conversations.star"
          aria-pressed={conversation.starred}
          onClick={() => {
            actions.toggleStar(conversation);
          }}
        />
        <IconButton
          label={unread ? t("actions.read") : t("actions.unread")}
          icon={unread ? "check" : "unread"}
          shortcut="conversations.mark_unread"
          onClick={() => {
            actions.toggleRead(conversation);
          }}
        />
        <MovePicker
          current={conversation.stream}
          open={moving}
          onOpenChange={setMoving}
          onMove={(target) => {
            actions.move(conversation, target);
          }}
        />
        {replyButtons}
        <Spacer />
        <span className="flex gap-1 max-md:hidden">
          <IconButton
            label={tShortcuts("list.previous")}
            icon="chevron-up"
            shortcut="list.previous"
            onClick={onPrevious}
          />
          <IconButton
            label={tShortcuts("list.next")}
            icon="chevron-down"
            shortcut="list.next"
            onClick={onNext}
          />
        </span>
      </PaneHeader>
      <ReaderBody>
        <div className="flex flex-col gap-1">
          <Heading level={2} size="page">
            {subject}
          </Heading>
          <MetaLine
            parts={[
              displayName(conversation.latest_from),
              t("reader.messages", { count: conversation.message_count }),
            ]}
          />
        </div>
        <Note
          key={conversation.id}
          label={t("scout.label")}
          visor={suggestion === undefined ? "idle" : "unsure"}
          confidence={placementLine(t, conversation, decisions)}
          why={whyLines(t, exists, conversation, decisions)}
          {...(suggestion === undefined
            ? {}
            : {
                choice: {
                  prompt: t("scout.prompt"),
                  options: [
                    {
                      label: t("scout.accept", { stream: streamName(t, suggestion.stream) }),
                      shortcut: "item.choice1",
                      onSelect: () => {
                        actions.move(conversation, suggestion.stream);
                      },
                    },
                  ],
                },
              })}
        >
          {suggestion === undefined
            ? noteSentence(t, decisions, i18n.language)
            : `${t("scout.suggestionNote", { stream: streamName(t, suggestion.stream) })} ${noteSentence(t, decisions, i18n.language)}`}
        </Note>
        {messages.toReversed().map((message) => (
          <ThreadMessage
            key={message.id}
            workspaceId={workspaceId}
            conversationId={conversationId}
            message={message}
            open={open.has(message.id)}
            onOpen={() => {
              setOpened((current) => new Set([...current, message.id]));
            }}
            showImages={images.has(message.id)}
            onShowImages={() => {
              setImages((current) => new Set([...current, message.id]));
            }}
            kind={message.direction === "outbound" ? "person" : kind}
            now={now}
          />
        ))}
        {renderReplySlot?.({
          workspaceId,
          inboxId: conversation.inbox_id,
          conversation,
          messages,
          request: reply,
          onClose: () => {
            setReply(null);
          },
        })}
      </ReaderBody>
      <div className="md:hidden">
        <ActionBar label={t("reader.actions")}>
          {renderReplySlot === undefined ? null : (
            <Button
              variant="primary"
              size="lg"
              icon="reply"
              grow
              onClick={() => {
                askReply("reply");
              }}
            >
              {tShortcuts("item.reply")}
            </Button>
          )}
          <Button
            size="lg"
            icon="archive"
            grow
            onClick={() => {
              onDone(conversation);
            }}
          >
            {tShortcuts("item.done")}
          </Button>
        </ActionBar>
      </div>
    </>
  );
}
