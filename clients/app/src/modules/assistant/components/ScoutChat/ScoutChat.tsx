import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode, type RecordRef } from "~/lib/api";
import { SCOUT_CHAT_ABOUT } from "~/lib/scout-chat";
import {
  Button,
  ChatAbout,
  ChatField,
  ChatMessage,
  ChatNotice,
  ChatPanel,
  ChatRecent,
  ChatSteps,
  ChatSuggestions,
  ChatWorking,
  ThinkingTrace,
  type ChatStep as StepLine,
} from "~/lib/ui";
import {
  displayName,
  listTime,
  useConversation,
  useConversationsByIds,
  useRuleNoticeSubject,
  useRuleSubject,
  type ConversationSummary,
} from "~/modules/conversations";

import {
  askScout,
  deleteChat,
  useChatCache,
  useRecentChats,
  useScoutChatOf,
  type ChatMessage as Message,
  type ChatSource,
  type ChatStep,
  type ChatTool,
} from "../../api/chats";

import { ProposalCards } from "./ProposalCards";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

const CONVERSATION = SCOUT_CHAT_ABOUT.conversation;

/** Where the chat sits beside the page rather than over it. */
const WIDE = "(width >= 64rem)";

export interface ScoutChatProps {
  workspaceId: string;
  /** What the chat is about; `null` for nothing in particular (always a new chat). */
  about: RecordRef | null;
  /** Which opening this is: a chat about nothing is new for each one. */
  opening?: number | undefined;
  /** A first question, asked as soon as the chat is open (⌘K's "Ask Scout: <text>"). */
  ask?: string | undefined;
  onClose: () => void;
  /** Over the whole screen (stories and tests pin it); follows the window otherwise. */
  modal?: boolean | undefined;
  /** Opened from a dialog, which stays as it is: Esc closes the panel first. */
  over?: boolean | undefined;
}

/** A question being answered, as it comes. */
interface Pending {
  question: string;
  thinking: string;
  /** The answer's words since the last step (words before a step are not the answer). */
  text: string;
  /** The tools that started, in order. */
  working: ChatTool[];
}

/** What is said in place of the field. */
type Problem = "unavailable" | "limit" | "full" | "unreadable";

/** What kind of thing the chat is about, as its copy needs it. */
type Kind = "conversation" | "rule" | "notice" | "nothing";

function kindOf(about: RecordRef | null | undefined): Kind | undefined {
  if (about === undefined) {
    return undefined;
  }
  switch (about?.type) {
    case undefined: {
      return "nothing";
    }
    case SCOUT_CHAT_ABOUT.rule: {
      return "rule";
    }
    case SCOUT_CHAT_ABOUT.ruleNotice: {
      return "notice";
    }
    default: {
      return "conversation";
    }
  }
}

/** The id of `about` when it is of `type`. */
function idOf(about: RecordRef | null | undefined, type: string): string | undefined {
  return about?.type === type ? about.id : undefined;
}

/**
 * Scout's chat (campbooks-cloud#305): about a conversation, a rule, one of
 * Scout's suggestions, or nothing in particular. Opening it starts the
 * member's chat or continues theirs (a chat about nothing is always new);
 * Scout explains and looks things up, as the member, and may propose
 * changes as cards the member confirms; it never makes one itself. Its
 * answers come as they are written (whole on the desktop), with what Scout
 * looked at one click away. "Recent" reopens an earlier chat. Closing it
 * stops a question being answered.
 */
export function ScoutChat({
  workspaceId,
  about,
  opening = 0,
  ask: first,
  onClose,
  modal,
  over,
}: ScoutChatProps) {
  const { t: typed, i18n } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const [source, setSource] = useState<ChatSource>(
    about === null ? { kind: "nothing", opening } : { kind: "about", about },
  );
  const chat = useScoutChatOf(workspaceId, source);
  const cache = useChatCache(workspaceId, source);
  // What it is about: the chat's own word once read, else what it was opened about.
  const subject: RecordRef | null | undefined =
    chat.data === undefined
      ? source.kind === "about"
        ? source.about
        : source.kind === "nothing"
          ? null
          : undefined
      : (chat.data.about ?? null);
  const kind = kindOf(subject);
  const conversationId = idOf(subject, CONVERSATION);
  const detail = useConversation(workspaceId, conversationId);
  const rule = useRuleSubject(workspaceId, idOf(subject, SCOUT_CHAT_ABOUT.rule));
  const notice = useRuleNoticeSubject(workspaceId, idOf(subject, SCOUT_CHAT_ABOUT.ruleNotice));
  const [recentWanted, setRecentWanted] = useState(false);
  const recent = useRecentChats(workspaceId, recentWanted);
  const [draft, setDraft] = useState("");
  const [pending, setPending] = useState<Pending | undefined>();
  const [rateLimited, setRateLimited] = useState(false);
  const [answerProblem, setAnswerProblem] = useState<Problem | undefined>();
  const [announcement, setAnnouncement] = useState("");
  const abort = useRef<AbortController | undefined>(undefined);

  // Closing the panel stops a question being answered.
  useEffect(
    () => () => {
      abort.current?.abort();
    },
    [],
  );

  const messages = chat.data?.messages ?? [];
  const ids = [
    ...new Set(
      messages.flatMap((message) =>
        message.steps.flatMap((step) =>
          step.records.filter((r) => r.type === CONVERSATION).map((r) => r.id),
        ),
      ),
    ),
  ];
  const found = useConversationsByIds(workspaceId, ids);

  const startCode = chat.isError ? problemCode(chat.error) : undefined;
  const problem: Problem | undefined =
    startCode === "chat_unavailable" ? "unavailable" : chat.isError ? "unreadable" : answerProblem;

  const head = detail.data?.conversation;
  const inbound = detail.data?.messages.findLast((message) => message.direction === "inbound");
  const sender = displayName(inbound?.from ?? head?.latest_from);
  const subjectLine =
    head === undefined ? "" : head.subject === "" ? t("chat.noSubject") : head.subject;
  const firstName = inbound?.from.name?.trim().split(/\s+/u, 1)[0] ?? "";

  const openConversation = (summary: Pick<ConversationSummary, "id" | "stream">) => {
    const wide = typeof globalThis.matchMedia === "function" && globalThis.matchMedia(WIDE).matches;
    if (!wide) {
      onClose();
    }
    void navigate({
      to: "/w/$workspaceId/inbox/$stream/$conversationId",
      params: { workspaceId, stream: summary.stream, conversationId: summary.id },
    });
  };

  const ask = async (text: string) => {
    const id = chat.data?.id;
    const question = text.trim();
    if (id === undefined || pending !== undefined || question === "") {
      return;
    }
    const controller = new AbortController();
    abort.current = controller;
    setDraft("");
    setRateLimited(false);
    setAnswerProblem(undefined);
    setAnnouncement("");
    setPending({ question, thinking: "", text: "", working: [] });
    try {
      const answer = await askScout(workspaceId, id, question, {
        signal: controller.signal,
        onThinking: (delta) => {
          setPending((now) => now && { ...now, thinking: now.thinking + delta });
        },
        onStep: (tool) => {
          setPending((now) => now && { ...now, working: [...now.working, tool], text: "" });
        },
        onText: (delta) => {
          setPending((now) => now && { ...now, text: now.text + delta });
        },
      });
      setAnnouncement(answer.text);
    } catch (error) {
      if (controller.signal.aborted) {
        return;
      }
      if (errorStatus(error) === 429) {
        // Nothing was asked: the words stay in the field.
        setRateLimited(true);
        setDraft(question);
      }
      switch (problemCode(error)) {
        case "chat_full": {
          setAnswerProblem("full");
          setDraft(question);
          break;
        }
        case "ai_daily_limit_reached": {
          setAnswerProblem("limit");
          break;
        }
        case "chat_unavailable": {
          setAnswerProblem("unavailable");
          break;
        }
        default: {
          // Anything else failed after the question was kept: the stored
          // answer says so, with Try again.
          break;
        }
      }
    }
    if (!controller.signal.aborted) {
      await cache.refresh(id).catch(() => {
        // The answer was stored; the chat shows it once asked for again.
      });
      setPending(undefined);
    }
  };

  // A first question handed over (⌘K) is asked once the chat is open, as if
  // the member had typed it; after the render, as their own question would be.
  const [handingOver, setHandingOver] = useState(first !== undefined && first.trim() !== "");
  useEffect(() => {
    if (!handingOver || first === undefined) {
      return;
    }
    if (chat.data === undefined || pending !== undefined) {
      return;
    }
    const timer = setTimeout(() => {
      setHandingOver(false);
      if (chat.data?.messages.length === 0) {
        void ask(first);
      }
    }, 0);
    return () => {
      clearTimeout(timer);
    };
  });

  const startOver = async () => {
    const id = chat.data?.id;
    if (id === undefined) {
      return;
    }
    await deleteChat(workspaceId, id);
    setAnswerProblem(undefined);
    if (source.kind === "about") {
      await cache.refresh(undefined);
      return;
    }
    // A chat from Recent, or about nothing: a new one about the same thing.
    const again = chat.data?.about;
    setSource(
      again === undefined || again === null
        ? { kind: "nothing", opening: Date.now() }
        : { kind: "about", about: again },
    );
  };

  // Another chat from Recent: what was being answered stops.
  const pick = (id: string) => {
    if (id === chat.data?.id) {
      return;
    }
    abort.current?.abort();
    setPending(undefined);
    setDraft("");
    setRateLimited(false);
    setAnswerProblem(undefined);
    setAnnouncement("");
    setSource({ kind: "chat", id });
  };

  const stepLines = (steps: readonly ChatStep[]): StepLine[] =>
    steps.map((step) => {
      const records = step.records
        .filter((record) => record.type === CONVERSATION)
        .flatMap((record) => {
          const summary = found.data?.find((c) => c.id === record.id);
          return summary === undefined
            ? []
            : [
                {
                  id: record.id,
                  label: t("chat.aboutLine", {
                    sender: displayName(summary.latest_from),
                    subject: summary.subject === "" ? t("chat.noSubject") : summary.subject,
                  }),
                  onOpen: () => {
                    openConversation(summary);
                  },
                },
              ];
        });
      return { text: stepText(t, step, conversationId ?? ""), records };
    });

  const last = messages.at(-1);
  const questionBefore = (index: number) =>
    messages.slice(0, index).findLast((message) => message.author === "member")?.text ?? "";

  // The opening line and the questions to start from, by what it is about.
  const starters: { intro: string; questions: string[] } | undefined =
    kind === "conversation"
      ? head === undefined
        ? undefined
        : {
            intro: t("chat.intro", { sender, subject: subjectLine }),
            questions: [
              t("chat.why"),
              t("chat.happened"),
              ...(firstName === "" ? [] : [t("chat.else", { name: firstName })]),
            ],
          }
      : kind === "rule"
        ? { intro: t("chat.ruleIntro"), questions: [t("chat.ruleDoes"), t("chat.ruleChanged")] }
        : kind === "notice"
          ? {
              intro: t("chat.noticeIntro"),
              questions: [t("chat.noticeWhy"), t("chat.noticeChange")],
            }
          : kind === "nothing"
            ? {
                intro: t("chat.nothingIntro"),
                questions: [t("chat.nothingToday"), t("chat.nothingWaiting")],
              }
            : undefined;

  const log = (
    <>
      {messages.length === 0 &&
      pending === undefined &&
      problem === undefined &&
      chat.data !== undefined &&
      !handingOver &&
      starters !== undefined ? (
        <ChatSuggestions
          label={t("chat.suggestions")}
          intro={starters.intro}
          questions={starters.questions}
          onPick={(question) => {
            void ask(question);
          }}
        />
      ) : null}
      {messages.map((message, index) => (
        <Stored
          key={message.id}
          message={message}
          t={t}
          cards={
            chat.data === undefined ? null : (
              <ProposalCards
                workspaceId={workspaceId}
                chatId={chat.data.id}
                proposals={message.proposals}
                onSettled={() => {
                  void cache.refresh(chat.data?.id);
                }}
              />
            )
          }
          steps={stepLines(message.steps)}
          isLast={message === last && pending === undefined}
          onRetry={() => {
            void ask(questionBefore(index));
          }}
        />
      ))}
      {pending === undefined ? null : (
        <>
          <ChatMessage from="member" label={t("chat.you")} text={pending.question} />
          <ChatMessage
            from="scout"
            label={t("chat.writing")}
            writing
            text={pending.text}
            before={
              <>
                <ThinkingTrace
                  label={t("chat.thinking")}
                  text={pending.thinking}
                  live={pending.text === ""}
                  summary={t("chat.thought")}
                />
                <ChatWorking lines={pending.working.map((tool) => t(`chat.working.${tool}`))} />
              </>
            }
          />
        </>
      )}
    </>
  );

  const unreadable =
    source.kind === "chat"
      ? t("chat.gone")
      : kind === "rule"
        ? t("chat.unreadableRule")
        : kind === "notice"
          ? t("chat.unreadableNotice")
          : kind === "nothing"
            ? t("chat.unreadableNothing")
            : t("chat.unreadable");

  const footer =
    problem === "unavailable" ? (
      <ChatNotice visor="asleep">{t("chat.unavailable")}</ChatNotice>
    ) : problem === "unreadable" ? (
      <ChatNotice visor="unsure">{unreadable}</ChatNotice>
    ) : problem === "limit" ? (
      <ChatNotice visor="asleep">{t("chat.limit")}</ChatNotice>
    ) : problem === "full" ? (
      <ChatNotice
        action={
          <Button
            onClick={() => {
              void startOver();
            }}
          >
            {t("chat.startOver")}
          </Button>
        }
      >
        {t("chat.full")}
      </ChatNotice>
    ) : (
      <div className="flex flex-col gap-2">
        {rateLimited ? <ChatNotice>{t("chat.rate")}</ChatNotice> : null}
        <ChatField
          label={kind === "nothing" ? t("chat.fieldNothing") : t("chat.field")}
          value={draft}
          onChange={setDraft}
          onAsk={() => {
            void ask(draft);
          }}
          askLabel={t("chat.ask")}
          busy={pending !== undefined || chat.data === undefined}
          note={t("chat.note")}
        />
      </div>
    );

  const aboutLine: ReactNode =
    kind === "conversation" && head !== undefined ? (
      <ChatAbout
        label={t("chat.about")}
        action={
          <Button
            variant="link"
            onClick={() => {
              openConversation(head);
            }}
          >
            {t("chat.open")}
          </Button>
        }
      >
        {t("chat.aboutLine", { sender, subject: subjectLine })}
      </ChatAbout>
    ) : kind === "rule" && rule.text !== undefined ? (
      <ChatAbout label={t("chat.about")}>{rule.text}</ChatAbout>
    ) : kind === "notice" && notice.text !== undefined ? (
      <ChatAbout label={t("chat.about")}>{t("chat.noticeLine", { sender: notice.text })}</ChatAbout>
    ) : kind === "nothing" ? (
      <ChatAbout label={t("chat.about")}>{t("chat.aboutNothing")}</ChatAbout>
    ) : undefined;

  return (
    <ChatPanel
      title={t("chat.title")}
      visor={pending === undefined ? "idle" : "thinking"}
      closeLabel={t("chat.close")}
      onClose={onClose}
      modal={modal}
      over={over}
      follow={[messages.length, pending?.text.length, pending?.working.length, i18n.language]}
      announcement={announcement}
      actions={
        <ChatRecent
          label={t("chat.recent")}
          items={recent.data?.map((each) => ({
            id: each.id,
            text: each.first_question,
            time: listTime(each.last_message_at, i18n.language),
            dateTime: each.last_message_at,
          }))}
          empty={t("chat.recentEmpty")}
          loading={t("chat.recentLoading")}
          failed={recent.isError ? t("chat.recentFailed") : undefined}
          current={chat.data?.id}
          onOpen={() => {
            if (recentWanted) {
              void recent.refetch();
            } else {
              setRecentWanted(true);
            }
          }}
          onPick={pick}
        />
      }
      about={aboutLine}
      footer={footer}
    >
      {log}
    </ChatPanel>
  );
}

/** A step as a sentence, from its tool and its count. */
function stepText(t: Translate, step: ChatStep, aboutId: string): string {
  const aboutThis = aboutId !== "" && step.records.some((record) => record.id === aboutId);
  switch (step.tool) {
    case "read_conversation": {
      return t(aboutThis ? "chat.steps.readThis" : "chat.steps.read");
    }
    case "conversation_history": {
      return t(aboutThis ? "chat.steps.historyThis" : "chat.steps.history");
    }
    default: {
      return t(`chat.steps.${step.tool}`, { count: step.count });
    }
  }
}

interface StoredProps {
  message: Message;
  t: Translate;
  /** Scout's proposals in the answer, as cards. */
  cards: ReactNode;
  steps: StepLine[];
  /** The chat's last message: a failure offers Try again. */
  isLast: boolean;
  onRetry: () => void;
}

/** A stored question or answer. */
function Stored({ message, t, cards, steps, isLast, onRetry }: StoredProps) {
  if (message.author === "member") {
    return <ChatMessage from="member" label={t("chat.you")} text={message.text} />;
  }
  if (message.finish === "failed") {
    // Only the last failure asks to try again; an earlier one was asked again.
    return isLast ? (
      <ChatMessage
        from="scout"
        label={t("chat.scout")}
        text=""
        after={
          <ChatNotice
            visor="dizzy"
            action={<Button onClick={onRetry}>{t("chat.tryAgain")}</Button>}
          >
            {t("chat.failed")}
          </ChatNotice>
        }
      />
    ) : null;
  }
  return (
    <ChatMessage
      from="scout"
      label={t("chat.scout")}
      text={message.text}
      after={
        <>
          {message.finish === "cut_short" ? <ChatNotice>{t("chat.cutShort")}</ChatNotice> : null}
          {cards}
          <ChatSteps summary={t("chat.looked")} steps={steps} />
        </>
      }
    />
  );
}
