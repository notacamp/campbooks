import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import { conversationRef, useScoutChat } from "~/lib/scout-chat";
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
  MoreMessages,
  Note,
  OverflowActions,
  PaneHeader,
  ReaderBody,
  ReaderFooter,
  SkeletonList,
  Text,
  Visor,
  type OverflowAction,
} from "~/lib/ui";
import { MakeTaskForm, useReplyQuestion } from "~/modules/tasks";

import {
  useConversation,
  useInboxes,
  type ConversationPlace,
  type ConversationSummary,
  type MessageView,
  type NeedView,
} from "../../api/queries";
import {
  archiveMove,
  avatarKind,
  displayName,
  noteSentence,
  openByDefault,
  placeName,
  placementLine,
  repliedSince,
  spamMove,
  streamName,
  suggestsReply,
  threadLayout,
  trashMove,
  type Move,
  type Translate,
} from "../../model";
import {
  billKind,
  howRows,
  needSender,
  needSentence,
  quoteAttribution,
  unsureNote,
  unsureOf,
  warningReasons,
} from "../../needs";
import { ExpiredChip, NeedChips } from "../Needs/NeedChips";
import { ruleLine } from "../../rules-text";
import { useNeedActions } from "../Needs/use-need-actions";
import { useTeachOnce } from "../TeachOnce/TeachOnce";
import { useRuleActions } from "../TeachOnce/use-rule-actions";
import type { InboxActions } from "../InboxScreen/use-inbox-actions";

import { firstName } from "../../compose/dialog";
import { NudgeDialog } from "../NudgeDialog/NudgeDialog";
import { ReplyDialog } from "../ReplyDialog/ReplyDialog";
import { SmartRuleConfirm, type SmartRuleDraft } from "../SmartRule/SmartRuleConfirm";

import { MovePicker } from "./MovePicker";
import type { RenderReplySlot, ReplyMode, ReplyRequest, ReplySlotContext } from "./ReplySlot";
import { ThreadMessage } from "./ThreadMessage";

/** What the calendar read from a message: its "Scout noticed" strip (the schedule module's). */
export type RenderFound = (found: {
  workspaceId: string;
  conversationId: string;
  messageId: string;
  subject: string;
}) => ReactNode;

export interface ConversationReaderProps {
  workspaceId: string;
  conversationId: string;
  /** The stream or view the list shows (Back goes there). */
  stream: ConversationPlace;
  /** What Back says when the list is not a stream (search results). */
  backLabel?: string | undefined;
  actions: InboxActions;
  onBack: () => void;
  onPrevious: () => void;
  onNext: () => void;
  /**
   * Move it to another place (Done, Move to Inbox, Delete, Restore, Report
   * spam, Not spam), then whatever the screen opens next.
   */
  onMove: (conversation: ConversationSummary, move: Move) => void;
  /** The send slice's composer; Reply, Reply all and Forward are hidden without it. */
  renderReplySlot?: RenderReplySlot | undefined;
  /** The calendar's strip under each received message's header. */
  renderFound?: RenderFound | undefined;
  /** Open the reply composer once the conversation is loaded (Home's R). */
  startReply?: boolean | undefined;
  /** The message that reply answers (a draft written for it), else the newest. */
  replyTo?: string | undefined;
  /** A saved draft to open in the composer on arrival (from Drafts). */
  draftId?: string | undefined;
  now?: Date | undefined;
}

/**
 * A conversation: its actions (with their shortcuts), the assistant's note
 * (suggestions with their reasons), the thread newest first with older
 * messages folded (the middle of a long one into "N more messages"), and
 * the reply slot pinned at the bottom.
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
  onMove,
  renderReplySlot: replySlot,
  renderFound,
  startReply = false,
  replyTo,
  draftId,
  now,
}: ConversationReaderProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const { t: tShortcuts } = useTranslation("shortcuts");
  const { t: tTasks } = useTranslation("tasks");
  const query = useConversation(workspaceId, conversationId);
  const detail = query.data;
  const conversation = detail?.conversation;
  // A task of theirs from this thread that asks "You replied. Is it done?"
  // (campbooks-cloud#396): the note below puts the question.
  const replyQuestion = useReplyQuestion(workspaceId, conversationId);
  // The wait the member just asked for (campbooks-cloud#372): the nudge to that person is a
  // dialog away, and optional.
  const [nudging, setNudging] = useState<{ need: NeedView; opened: number } | undefined>();
  const needActions = useNeedActions(workspaceId, {
    onWaiting: (summary) => {
      const waiting = summary.need;
      if (waiting?.reason === "waiting" && (waiting.waiting_on ?? null) !== null) {
        setNudging((current) => ({ need: waiting, opened: (current?.opened ?? 0) + 1 }));
      }
    },
  });
  // An inbox kept as history after its mailbox was disconnected (or one you
  // only read): nothing is written to it or sent from it, so no action is
  // offered (the server would answer `inbox_read_only`).
  const inbox = useInboxes(workspaceId).data?.find(
    (candidate) => candidate.id === conversation?.inbox_id,
  );
  const kept = inbox?.read_only === true;
  // Until the inbox is known (or if it can't be), nothing is offered.
  const writable = inbox !== undefined && !kept && inbox.role !== "reader";
  // In spam or the trash: no reply, no Scout, no images (spam); the banner
  // says where it is and brings it back.
  const binned = conversation?.trashed === true;
  const inSpam = conversation?.trashed === true && conversation.spam;
  const renderReplySlot = writable && !binned ? replySlot : undefined;
  const need = writable && !binned ? (conversation?.need ?? undefined) : undefined;
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
  const choiceKey = (id: "item.choice1" | "item.choice2" | "item.choice3") =>
    offering ? {} : { shortcut: id };
  const footerLine = (summary: NonNullable<typeof conversation>) =>
    ruleMark === undefined ? placementLine(t, summary) : ruleLine(t, ruleMark, i18n.language);
  const question = need?.trust === "check";
  // Scout isn't sure and the email went to other people: whose move is it?
  const unsure = need === undefined ? undefined : unsureOf(need);
  const unsureParty = unsure?.party ?? undefined;
  // A bill: Campbooks can't see the bank, so it asks whether it is paid;
  // once still to pay, Mark paid is its Done.
  const bill = need !== undefined && billKind(need) === "ask";
  const toPay = need !== undefined && billKind(need) === "toPay";
  // Scout isn't sure whose move it is: Nudge the person it went to, Reply, or Nothing for me.
  const unsureChoice =
    conversation === undefined || need === undefined || unsure === undefined
      ? undefined
      : {
          prompt: unsureNote(t, unsure),
          options: [
            ...(unsureParty === undefined
              ? []
              : [
                  {
                    label: t("needs.actions.nudgeParty", {
                      name: displayName(unsureParty),
                    }),
                    ...choiceKey("item.choice1"),
                    onSelect: () => {
                      needActions.waitOn(conversation, {
                        address: unsureParty.address,
                        name: displayName(unsureParty),
                      });
                    },
                  },
                ]),
            {
              label: t("needs.actions.replyTo", {
                name: firstName(needSender(need, conversation)),
              }),
              ...choiceKey(unsureParty === undefined ? "item.choice1" : "item.choice2"),
              onSelect: () => {
                // Yes first, so the reply corrects what Scout was unsure of.
                needActions.confirm(conversation);
                openReply();
              },
            },
            {
              label: t("needs.actions.nothingForMe"),
              ...choiceKey(unsureParty === undefined ? "item.choice2" : "item.choice3"),
              onSelect: () => {
                needActions.notNeeded(conversation);
              },
            },
          ],
        };

  const [opened, setOpened] = useState<Set<string>>(new Set());
  // Messages "N more messages" showed (folded) in its place.
  const [revealed, setRevealed] = useState<Set<string>>(new Set());
  const [images, setImages] = useState<Set<string>>(new Set());
  const [moving, setMoving] = useState(false);
  // "Make it a task": the inline form under the subject.
  const [makingTask, setMakingTask] = useState(false);
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
  const thread = threadLayout(messages, new Set([...opened, ...revealed]));
  const newest = messages.at(-1);
  const suggestion = writable && !binned ? (conversation?.suggestion ?? undefined) : undefined;
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
      // What the checks vouch for: its domain (the registrable one, as DKIM
      // and DMARC align), not the one person (campbooks-cloud#236).
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
  // Never offered on mail that may be phishing, nor in spam: trust is not the question.
  const trustable = writable && !inSpam && conversation?.need?.question !== "suspicious";
  // The newest message from someone else: the one the key trusts.
  const newestInbound = messages.findLast((message) => message.direction === "inbound");

  /** `move` on this conversation, when there is one. */
  const moveBy = (move: Move | undefined) => () => {
    if (conversation !== undefined && move !== undefined) {
      onMove(conversation, move);
    }
  };
  const archiveTo = conversation === undefined ? undefined : archiveMove(conversation);
  const trashTo = conversation === undefined ? undefined : trashMove(conversation);
  const spamTo = conversation === undefined ? undefined : spamMove(conversation);
  useShortcut("item.done", moveBy(archiveTo), { enabled: active && archiveTo !== undefined });
  useShortcut("conversations.trash", moveBy(trashTo), { enabled: active && trashTo !== undefined });
  useShortcut("conversations.report_spam", moveBy(spamTo), {
    enabled: active && spamTo !== undefined,
  });
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
  // Ask Scout (`.`): the chat about this conversation; the focus comes
  // back to what it was asked from.
  const scout = useScoutChat();
  const askScout = () => {
    if (conversation === undefined) {
      return;
    }
    const active = document.activeElement;
    scout?.(conversationRef(conversation.id), {
      from: active instanceof HTMLElement && active !== document.body ? active : null,
    });
  };
  useShortcut(
    "scout.ask",
    () => {
      askScout();
    },
    // Asking changes nothing: a reader of the inbox asks too.
    { enabled: scout !== undefined && conversation !== undefined },
  );
  const viewing = conversation !== undefined;
  // Remote images never load in spam.
  useShortcut("inbox.show_images", showAllImages, { enabled: viewing && !inSpam });
  useShortcut(
    "inbox.next_attachment",
    () => {
      focusNextAttachment();
    },
    { enabled: viewing },
  );
  // N T: a task from this email (the same key that adds one in Tasks).
  useShortcut(
    "tasks.create",
    () => {
      setMakingTask(true);
    },
    { enabled: viewing && !binned },
  );
  useShortcut("overlay.close", onBack, {
    enabled: !moving && replying === undefined && trusting === null && !makingTask,
  });
  // J/K step between conversations; the arrows keep scrolling this one.
  useShortcut("list.next", onNext, { alternates: false });
  useShortcut("list.previous", onPrevious, { alternates: false });
  // The note offers Reply whenever it says a reply is needed (campbooks-cloud#306).
  // No key hint: the reader's own Reply carries R.
  const noteReply =
    renderReplySlot === undefined || newest === undefined ? null : (
      <Button
        variant="secondary"
        icon="reply"
        onClick={() => {
          openReply();
        }}
      >
        {tShortcuts("item.reply")}
      </Button>
    );

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
        {backLabel ?? t("reader.back", { stream: placeName(t, stream) })}
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
  // A reply since the message Scout read answered it (campbooks-cloud#355).
  const replied = repliedSince(detail?.messages ?? []);
  const suggestedReply = suggestsReply(decisions, replied) ? noteReply : null;
  // Scout's sentence when no need is open: where it sorted it, and whether
  // anything here waits.
  const standing =
    suggestion === undefined
      ? noteSentence(t, decisions, i18n.language, replied)
      : `${t("scout.suggestionNote", { stream: streamName(t, suggestion.stream) })} ${noteSentence(t, decisions, i18n.language, replied)}`;
  const exists = (key: string) => i18n.exists(`conversations:${key}`);
  const unread = conversation.unread_count > 0;
  const replyButtons =
    renderReplySlot === undefined ? null : (
      // Done's neighbours. Icons, named in their tooltips, so the header
      // stays one row; on phones Reply is the floating bar's.
      <span className="flex gap-1">
        <span className="flex max-md:hidden">
          <IconButton
            label={tShortcuts("item.reply")}
            icon="reply"
            shortcut="item.reply"
            onClick={() => {
              openReply();
            }}
          />
        </span>
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
    );

  // After Done, Reply and Move: icons while they fit, else one More menu.
  const moreActions: OverflowAction[] = writable
    ? [
        ...(trashTo === "trash"
          ? [
              {
                id: "trash",
                label: t("actions.delete"),
                icon: "trash" as const,
                shortcut: "conversations.trash" as const,
                onSelect: moveBy(trashTo),
              },
            ]
          : []),
        ...(spamTo === "spam"
          ? [
              {
                id: "spam",
                label: t("actions.reportSpam"),
                icon: "warning" as const,
                shortcut: "conversations.report_spam" as const,
                onSelect: moveBy(spamTo),
              },
            ]
          : []),
        {
          id: "star",
          label: conversation.starred ? t("actions.unstar") : t("actions.star"),
          icon: conversation.starred ? "pin-filled" : "pin",
          shortcut: "conversations.star",
          pressed: conversation.starred,
          onSelect: () => {
            actions.toggleStar(conversation);
          },
        },
        ...(binned
          ? []
          : [
              {
                id: "task",
                label: tTasks("make.button"),
                icon: "tasks" as const,
                shortcut: "tasks.create" as const,
                onSelect: () => {
                  setMakingTask(true);
                },
              },
            ]),
        {
          id: "read",
          label: unread ? t("actions.read") : t("actions.unread"),
          icon: unread ? "check" : "unread",
          shortcut: "conversations.mark_unread",
          onSelect: () => {
            actions.toggleRead(conversation);
          },
        },
      ]
    : [];
  // Asking changes nothing: offered to a reader of the inbox too.
  if (scout !== undefined) {
    moreActions.push({
      id: "scout",
      label: t("actions.askScout"),
      icon: <Visor size="xs" />,
      shortcut: "scout.ask",
      onSelect: askScout,
    });
  }
  const listActions: OverflowAction[] = [
    {
      id: "previous",
      label: tShortcuts("list.previous"),
      icon: "chevron-up",
      shortcut: "list.previous",
      onSelect: onPrevious,
    },
    {
      id: "next",
      label: tShortcuts("list.next"),
      icon: "chevron-down",
      shortcut: "list.next",
      onSelect: onNext,
    },
  ];

  const replyContext: ReplySlotContext = {
    workspaceId,
    inboxId: conversation.inbox_id,
    conversation,
    messages,
    request: reply,
    draftId,
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
            {archiveTo === undefined ? null : (
              <span className="flex max-md:hidden">
                <Button
                  icon={archiveTo === "archive" ? "archive" : "inbox"}
                  shortcut="item.done"
                  onClick={moveBy(archiveTo)}
                >
                  {archiveTo === "archive" ? tShortcuts("item.done") : t("actions.toInbox")}
                </Button>
              </span>
            )}
            {replyButtons}
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
        <OverflowActions label={t("actions.more")} actions={moreActions} end={listActions} />
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
        {makingTask ? (
          <MakeTaskForm
            key={conversation.id}
            workspaceId={workspaceId}
            conversationId={conversation.id}
            subject={conversation.subject}
            senderAddress={newestInbound?.from.address}
            now={now}
            onClose={() => {
              setMakingTask(false);
            }}
          />
        ) : null}
        {kept ? (
          <Banner icon="archive" role="status" title={t("reader.kept.title")}>
            {t("reader.kept.lead", { address: inbox.address })}
          </Banner>
        ) : binned ? (
          // Where it is, and the way back (the key that sent it there).
          <Banner
            icon={inSpam ? "warning" : "trash"}
            role="status"
            title={t(inSpam ? "reader.spam.title" : "reader.trash.title")}
            action={
              writable ? (
                <Button
                  variant="secondary"
                  icon="inbox"
                  shortcut={inSpam ? "conversations.report_spam" : "conversations.trash"}
                  onClick={moveBy(inSpam ? spamTo : trashTo)}
                >
                  {t(inSpam ? "actions.notSpam" : "actions.restore")}
                </Button>
              ) : undefined
            }
          >
            {t(inSpam ? "reader.spam.lead" : "reader.trash.lead")}
          </Banner>
        ) : need === undefined ? (
          <Note
            key={conversation.id}
            label={t("scout.label")}
            visor={suggestion === undefined ? "idle" : "unsure"}
            {...(conversation.lapsed_need === null || conversation.lapsed_need === undefined
              ? {}
              : { chips: <ExpiredChip /> })}
            footer={footerLine(conversation)}
            {...(suggestedReply === null && stopButton === null
              ? {}
              : {
                  actions: (
                    <>
                      {suggestedReply}
                      {stopButton}
                    </>
                  ),
                })}
            how={
              <HowDecided rows={howRows(t, exists, decisions)} footnote={t("needs.how.footnote")} />
            }
            {...(replyQuestion === undefined
              ? suggestion === undefined
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
                  }
              : {
                  choice: {
                    prompt: t("scout.taskPrompt"),
                    options: [
                      {
                        label: t("scout.taskDone"),
                        ...choiceKey("item.choice1"),
                        onSelect: replyQuestion.done,
                      },
                      {
                        label: t("scout.taskNotYet"),
                        ...choiceKey("item.choice2"),
                        onSelect: replyQuestion.notYet,
                      },
                    ],
                  },
                })}
          >
            {replyQuestion === undefined
              ? standing
              : `${standing} ${t("scout.taskOpen", { title: replyQuestion.task.title })}`}
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
            {...(detail?.need_read_failed === true
              ? {
                  after: (
                    <Text as="span" tone="tertiary" size="sm">
                      {t("scout.readFailed")}
                    </Text>
                  ),
                }
              : {})}
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
                    : (unsureChoice ?? {
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
                      }),
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
                      {need.reason === "reply" ? noteReply : null}
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
              <HowDecided
                rows={howRows(
                  t,
                  exists,
                  decisions,
                  unsure === undefined ? undefined : { unsure, question: need.question },
                )}
                footnote={t("needs.how.footnote")}
              />
            }
          >
            {needSentence(t, need, needSender(need, conversation), {
              locale: i18n.language,
              now,
            })}
          </Note>
        )}
        {thread.map((entry) =>
          entry.kind === "more" ? (
            <MoreMessages
              key={entry.ids.join(",")}
              label={t("reader.more", { count: entry.ids.length })}
              onOpen={() => {
                setRevealed((current) => new Set([...current, ...entry.ids]));
              }}
            />
          ) : (
            <ThreadMessage
              key={entry.message.id}
              workspaceId={workspaceId}
              conversationId={conversationId}
              message={entry.message}
              open={open.has(entry.message.id)}
              onOpen={() => {
                setOpened((current) => new Set([...current, entry.message.id]));
              }}
              showImages={!inSpam && images.has(entry.message.id)}
              onShowImages={() => {
                setImages((current) => new Set([...current, entry.message.id]));
              }}
              onTrustImages={
                trustable && entry.message.direction === "inbound"
                  ? (domain) => {
                      trustImages(entry.message, domain);
                    }
                  : undefined
              }
              trustKey={active && trusting === null && entry.message.id === newestInbound?.id}
              sharedInbox={inbox?.shared === true}
              kind={entry.message.direction === "outbound" ? "person" : kind}
              inertLinks={inSpam || conversation.need?.question === "suspicious"}
              spam={inSpam}
              found={
                entry.message.direction === "inbound"
                  ? renderFound?.({
                      workspaceId,
                      conversationId,
                      messageId: entry.message.id,
                      subject: entry.message.subject,
                    })
                  : undefined
              }
              now={now}
            />
          ),
        )}
      </ReaderBody>
      <ReaderFooter
        slot={renderReplySlot?.(replyContext)}
        // While the composer is open it owns the bottom of the screen; in
        // spam or the trash the banner holds the way back.
        phoneBar={
          reply === null && writable && archiveTo !== undefined ? (
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
                icon={archiveTo === "archive" ? "archive" : "inbox"}
                grow
                onClick={moveBy(archiveTo)}
              >
                {archiveTo === "archive" ? tShortcuts("item.done") : t("actions.toInbox")}
              </Button>
            </ActionBar>
          ) : undefined
        }
      />
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
      {nudging === undefined ? null : (
        <NudgeDialog
          key={nudging.opened}
          workspaceId={workspaceId}
          conversationId={conversation.id}
          need={nudging.need}
          onClose={() => {
            setNudging(undefined);
          }}
          onOpenDraft={(draft) => {
            setNudging(undefined);
            askReply("reply_all", draft.message_id);
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
