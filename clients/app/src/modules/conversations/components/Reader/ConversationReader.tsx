import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  ActionBar,
  Banner,
  Button,
  EmptyState,
  focusNextAttachment,
  Heading,
  HowDecided,
  IconButton,
  MetaLine,
  Note,
  PaneHeader,
  ReaderBody,
  SkeletonList,
  Spacer,
} from "~/lib/ui";

import {
  useConversation,
  useInboxes,
  type ConversationSummary,
  type MessageView,
  type Stream,
} from "../../api/queries";
import {
  avatarKind,
  displayName,
  noteSentence,
  openByDefault,
  placementLine,
  streamName,
  type Translate,
} from "../../model";
import {
  billKind,
  howRows,
  needSender,
  needSentence,
  quoteAttribution,
  warningReasons,
} from "../../needs";
import { NeedChips } from "../Needs/NeedChips";
import { ruleLine } from "../../rules-text";
import { useNeedActions } from "../Needs/use-need-actions";
import { useTeachOnce } from "../TeachOnce/TeachOnce";
import { useRuleActions } from "../TeachOnce/use-rule-actions";
import type { InboxActions } from "../InboxScreen/use-inbox-actions";

import { ReplyDialog } from "../ReplyDialog/ReplyDialog";
import { SmartRuleConfirm, type SmartRuleDraft } from "../SmartRule/SmartRuleConfirm";

import { MovePicker } from "./MovePicker";
import type { RenderReplySlot, ReplyMode, ReplyRequest, ReplySlotContext } from "./ReplySlot";
import { ThreadMessage } from "./ThreadMessage";

export interface ConversationReaderProps {
  workspaceId: string;
  conversationId: string;
  /** The stream the list shows (Back goes there). */
  stream: Stream;
  /** What Back says when the list is not a stream (search results). */
  backLabel?: string | undefined;
  actions: InboxActions;
  onBack: () => void;
  onPrevious: () => void;
  onNext: () => void;
  /** Done: archive, then whatever the screen opens next. */
  onDone: (conversation: ConversationSummary) => void;
  /** The send slice's composer; Reply, Reply all and Forward are hidden without it. */
  renderReplySlot?: RenderReplySlot | undefined;
  /** Open the reply composer once the conversation is loaded (Home's R). */
  startReply?: boolean | undefined;
  /** The message that reply answers (a draft written for it), else the newest. */
  replyTo?: string | undefined;
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
  backLabel,
  actions,
  onBack,
  onPrevious,
  onNext,
  onDone,
  renderReplySlot: replySlot,
  startReply = false,
  replyTo,
  now,
}: ConversationReaderProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const { t: tShortcuts } = useTranslation("shortcuts");
  const query = useConversation(workspaceId, conversationId);
  const detail = query.data;
  const conversation = detail?.conversation;
  const needActions = useNeedActions(workspaceId);
  // An inbox kept as history after its mailbox was disconnected (or one you
  // only read): nothing is written to it or sent from it, so no action is
  // offered (the server would answer `inbox_read_only`).
  const inbox = useInboxes(workspaceId).data?.find(
    (candidate) => candidate.id === conversation?.inbox_id,
  );
  const kept = inbox?.read_only === true;
  // Until the inbox is known (or if it can't be), nothing is offered.
  const writable = inbox !== undefined && !kept && inbox.role !== "reader";
  const renderReplySlot = writable ? replySlot : undefined;
  const need = writable ? (conversation?.need ?? undefined) : undefined;
  const ruleActions = useRuleActions(workspaceId);
  // While a teach-once offer or question is up, it holds the choice keys:
  // the reader's own choices stay clickable but unbound and unhinted.
  const offering = useTeachOnce()?.offering ?? false;
  const ruleMark = conversation?.rule ?? undefined;
  // Only whoever may change the rule (its author, a manager) may stop it.
  const stoppable = ruleMark?.may_change === true;
  /** "Stop doing this": remove the rule that handled it; Undo brings it back. */
  const stopRule = () => {
    if (ruleMark === undefined || !stoppable) {
      return;
    }
    ruleActions.removeRule(ruleMark, { restore: false });
  };
  const stopButton = stoppable ? (
    <Button variant="ghost" disabled={ruleActions.removing} onClick={stopRule}>
      {t("rules.reader.stop")}
    </Button>
  ) : null;
  const choiceKey = (id: "item.choice1" | "item.choice2") => (offering ? {} : { shortcut: id });
  const footerLine = (summary: NonNullable<typeof conversation>) =>
    ruleMark === undefined ? placementLine(t, summary) : ruleLine(t, ruleMark, i18n.language);
  const question = need?.trust === "check";
  // A bill: Campbooks can't see the bank, so it asks whether it is paid;
  // once still to pay, Mark paid is its Done.
  const bill = need !== undefined && billKind(need) === "ask";
  const toPay = need !== undefined && billKind(need) === "toPay";

  const [opened, setOpened] = useState<Set<string>>(new Set());
  const [images, setImages] = useState<Set<string>>(new Set());
  const [moving, setMoving] = useState(false);
  const [reply, setReply] = useState<ReplyRequest | null>(null);
  // Reply first asks Scout (the reply dialog).
  const [replying, setReplying] = useState<{ opened: number } | undefined>();
  // "Always show images from <sender>": the smart rule being confirmed.
  const [trusting, setTrusting] = useState<SmartRuleDraft | null>(null);
  const nonce = useRef(0);

  // Opening an unread conversation reads it (once).
  const readOnce = useRef<string | null>(null);
  useEffect(() => {
    if (
      conversation !== undefined &&
      writable &&
      conversation.unread_count > 0 &&
      readOnce.current !== conversation.id
    ) {
      readOnce.current = conversation.id;
      actions.markRead(conversation);
    }
  }, [conversation, actions, writable]);

  const messages = detail?.messages ?? [];
  const open = new Set([...openByDefault(messages), ...opened]);
  const newest = messages.at(-1);
  const suggestion = writable ? (conversation?.suggestion ?? undefined) : undefined;
  const active = conversation !== undefined && writable;

  /** The composer on `messageId` (a draft written for it), else the newest message. */
  const askReply = (mode: ReplyMode, messageId?: string | null) => {
    const target = messages.find((m) => m.id === messageId) ?? newest;
    if (target === undefined) {
      return;
    }
    nonce.current += 1;
    setReply({ mode, messageId: target.id, nonce: nonce.current });
  };
  /** Reply: the dialog with Scout's likely replies. */
  const openReply = () => {
    nonce.current += 1;
    setReplying({ opened: nonce.current });
  };
  // The message Home's reply answers, when it is in this conversation.
  const replyTarget = messages.find((m) => m.id === replyTo)?.id;
  // Home's R: answer that message, else the newest, as soon as it is here (once).
  const repliedOnce = useRef(false);
  useEffect(() => {
    if (
      startReply &&
      !repliedOnce.current &&
      newest !== undefined &&
      renderReplySlot !== undefined
    ) {
      repliedOnce.current = true;
      nonce.current += 1;
      setReply({ mode: "reply", messageId: replyTarget ?? newest.id, nonce: nonce.current });
    }
  }, [startReply, newest, renderReplySlot, replyTarget]);
  const showAllImages = () => {
    setImages(new Set(messages.map((message) => message.id)));
  };
  /**
   * Trust `message`'s sender with its images: a smart rule for the address,
   * or anyone at `domain` when the server offers one (an owned domain, never
   * a free mail provider's), confirmed first.
   */
  const trustImages = (message: MessageView, domain: string | undefined) => {
    if (conversation === undefined || inbox === undefined) {
      return;
    }
    const address = message.from.address.trim().toLowerCase();
    const byAddress = {
      id: "address",
      heading: t("smartRule.imagesTitle", { sender: address }),
      title: t("smartRule.imagesAddress", { address }),
      description: t("smartRule.imagesAddressDetail"),
      conditions: [{ field: "sender_address" as const, value: address }],
    };
    const byDomain =
      domain === undefined
        ? undefined
        : {
            id: "domain",
            heading: t("smartRule.imagesTitle", { sender: domain }),
            title: t("smartRule.imagesDomain", { domain }),
            description: t("smartRule.imagesDomainDetail", { domain }),
            conditions: [{ field: "sender_domain" as const, value: domain }],
          };
    setTrusting({
      inboxId: inbox.id,
      conditions: byAddress.conditions,
      actions: [{ action: "show_images" }],
      title: t("smartRule.imagesTitle", { sender: address }),
      fromConversation: conversation.id,
      sharedInbox: inbox.shared ? inbox.address : undefined,
      choices: byDomain === undefined ? [byAddress] : [byAddress, byDomain],
    });
  };
  // Never offered on mail that may be phishing: trust is not the question.
  const trustable = writable && conversation?.need?.question !== "suspicious";
  // The newest message from someone else: the one the key trusts.
  const newestInbound = messages.findLast((message) => message.direction === "inbound");

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
  const viewing = conversation !== undefined;
  useShortcut("inbox.show_images", showAllImages, { enabled: viewing });
  useShortcut(
    "inbox.next_attachment",
    () => {
      focusNextAttachment();
    },
    { enabled: viewing },
  );
  useShortcut("overlay.close", onBack, {
    enabled: !moving && replying === undefined && trusting === null,
  });
  // J/K step between conversations; the arrows keep scrolling this one.
  useShortcut("list.next", onNext, { alternates: false });
  useShortcut("list.previous", onPrevious, { alternates: false });
  const canReply = active && renderReplySlot !== undefined;
  useShortcut(
    "item.reply",
    () => {
      openReply();
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
      if (conversation === undefined) {
        return;
      }
      if (bill) {
        needActions.alreadyPaid(conversation);
      } else if (question) {
        needActions.confirm(conversation);
      } else if (suggestion !== undefined) {
        actions.move(conversation, suggestion.stream);
      }
    },
    { enabled: (question || suggestion !== undefined) && !offering },
  );
  useShortcut(
    "item.choice2",
    () => {
      if (conversation !== undefined) {
        if (bill) {
          needActions.stillToPay(conversation);
        } else {
          needActions.notNeeded(conversation);
        }
      }
    },
    { enabled: question && !offering },
  );
  useShortcut(
    "conversations.resolve_need",
    () => {
      if (conversation !== undefined) {
        needActions.notNeeded(conversation);
      }
    },
    { enabled: need !== undefined && (!question || bill) },
  );

  // Wrappers carry the responsive classes: a component's own styles win
  // over utilities.
  const back = (
    <span className="flex lg:hidden">
      <Button variant="ghost" icon="chevron-left" onClick={onBack} shortcut="overlay.close">
        {backLabel ?? t("reader.back", { stream: streamName(t, stream) })}
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
      <span className="flex flex-wrap gap-1 max-md:hidden">
        <Button
          variant="ghost"
          icon="reply"
          shortcut="item.reply"
          onClick={() => {
            openReply();
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

  const replyContext: ReplySlotContext = {
    workspaceId,
    inboxId: conversation.inbox_id,
    conversation,
    messages,
    request: reply,
    onClose: () => {
      setReply(null);
    },
  };

  return (
    <>
      <PaneHeader>
        {back}
        {writable ? (
          <>
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
              icon={conversation.starred ? "pin-filled" : "pin"}
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
          </>
        ) : null}
        {replyButtons}
        {renderReplySlot === undefined ? null : (
          // Phones: Reply is in the floating bar; these two stay in the header.
          <span className="flex gap-1 md:hidden">
            <IconButton
              label={t("actions.replyAll")}
              icon="reply-all"
              shortcut="item.reply_all"
              onClick={() => {
                askReply("reply_all");
              }}
            />
            <IconButton
              label={t("actions.forward")}
              icon="forward"
              shortcut="item.forward"
              onClick={() => {
                askReply("forward");
              }}
            />
          </span>
        )}
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
        {kept ? (
          <Banner icon="archive" role="status" title={t("reader.kept.title")}>
            {t("reader.kept.lead", { address: inbox.address })}
          </Banner>
        ) : need === undefined ? (
          <Note
            key={conversation.id}
            label={t("scout.label")}
            visor={suggestion === undefined ? "idle" : "unsure"}
            footer={footerLine(conversation)}
            {...(stopButton === null ? {} : { actions: stopButton })}
            how={
              <HowDecided rows={howRows(t, exists, decisions)} footnote={t("needs.how.footnote")} />
            }
            {...(suggestion === undefined
              ? {}
              : {
                  choice: {
                    prompt: t("scout.prompt"),
                    options: [
                      {
                        label: t("scout.accept", { stream: streamName(t, suggestion.stream) }),
                        ...choiceKey("item.choice1"),
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
        ) : (
          <Note
            key={conversation.id}
            label={t("scout.label")}
            visor={need.question === "suspicious" ? "alert" : question ? "unsure" : "idle"}
            reasons={
              need.question === "suspicious"
                ? [...warningReasons(t, exists, decisions), t("needs.mail.linksOff")]
                : []
            }
            {...(need.quote === null || need.quote === undefined
              ? {}
              : {
                  quote: {
                    text: need.quote.text,
                    attribution: quoteAttribution(
                      need,
                      needSender(need, conversation),
                      i18n.language,
                    ),
                  },
                })}
            chips={<NeedChips need={need} now={now} />}
            {...(question
              ? {
                  choice: bill
                    ? {
                        prompt: t("needs.actions.billPrompt"),
                        options: [
                          {
                            label: t("needs.actions.alreadyPaid"),
                            ...choiceKey("item.choice1"),
                            onSelect: () => {
                              needActions.alreadyPaid(conversation);
                            },
                          },
                          {
                            label: t("needs.actions.stillToPay"),
                            ...choiceKey("item.choice2"),
                            onSelect: () => {
                              needActions.stillToPay(conversation);
                            },
                          },
                        ],
                      }
                    : {
                        prompt: t("needs.actions.questionPrompt"),
                        options: [
                          {
                            label: t("needs.actions.yes"),
                            ...choiceKey("item.choice1"),
                            onSelect: () => {
                              needActions.confirm(conversation);
                            },
                          },
                          {
                            label: t("needs.actions.no"),
                            ...choiceKey("item.choice2"),
                            onSelect: () => {
                              needActions.notNeeded(conversation);
                            },
                          },
                        ],
                      },
                  // A bill can also be not needed (X): shown, as every key is.
                  ...(bill
                    ? {
                        actions: (
                          <>
                            <Button
                              variant="ghost"
                              shortcut="conversations.resolve_need"
                              onClick={() => {
                                needActions.notNeeded(conversation);
                              }}
                            >
                              {t("needs.actions.notNeeded")}
                            </Button>
                            {stopButton}
                          </>
                        ),
                      }
                    : stopButton === null
                      ? {}
                      : { actions: stopButton }),
                }
              : {
                  actions: (
                    <>
                      <Button
                        variant="secondary"
                        onClick={() => {
                          if (toPay) {
                            needActions.alreadyPaid(conversation);
                          } else {
                            needActions.done(conversation);
                          }
                        }}
                      >
                        {t(toPay ? "needs.actions.markPaid" : "needs.actions.done")}
                      </Button>
                      <Button
                        variant="ghost"
                        shortcut="conversations.resolve_need"
                        onClick={() => {
                          needActions.notNeeded(conversation);
                        }}
                      >
                        {t(
                          need.question === "suspicious"
                            ? "needs.actions.safe"
                            : "needs.actions.notNeeded",
                        )}
                      </Button>
                      {stopButton}
                    </>
                  ),
                })}
            footer={footerLine(conversation)}
            how={
              <HowDecided rows={howRows(t, exists, decisions)} footnote={t("needs.how.footnote")} />
            }
          >
            {needSentence(t, need, needSender(need, conversation))}
          </Note>
        )}
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
            onTrustImages={
              trustable && message.direction === "inbound"
                ? (domain) => {
                    trustImages(message, domain);
                  }
                : undefined
            }
            trustKey={active && trusting === null && message.id === newestInbound?.id}
            sharedInbox={inbox?.shared === true}
            kind={message.direction === "outbound" ? "person" : kind}
            inertLinks={conversation.need?.question === "suspicious"}
            now={now}
          />
        ))}
        {renderReplySlot?.(replyContext)}
      </ReaderBody>
      {/* While the composer is open it owns the bottom of the screen. */}
      {reply === null && writable ? (
        <ActionBar label={t("reader.actions")} phoneOnly aboveTabBar>
          {renderReplySlot === undefined ? null : (
            <Button
              variant="primary"
              size="lg"
              icon="reply"
              grow
              onClick={() => {
                openReply();
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
      ) : null}
      {replying === undefined || renderReplySlot === undefined ? null : (
        <ReplyDialog
          key={replying.opened}
          workspaceId={workspaceId}
          conversationId={conversation.id}
          onClose={() => {
            setReplying(undefined);
          }}
          onWriteOwn={() => {
            setReplying(undefined);
            askReply("reply");
          }}
          onOpenDraft={(draft) => {
            setReplying(undefined);
            // On the message the draft answers, which may not be the newest.
            askReply("reply", draft.message_id);
          }}
          onUndone={(resume) => {
            // The reply slot reopens the undone draft in the composer.
            askReply("reply", resume.draft.message_id);
          }}
        />
      )}
      <SmartRuleConfirm
        workspaceId={workspaceId}
        draft={trusting}
        onClose={() => {
          setTrusting(null);
        }}
      />
    </>
  );
}
