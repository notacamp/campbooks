import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import {
  Button,
  ChatAbout,
  ChatField,
  ChatMessage,
  ChatNotice,
  ChatPanel,
  ChatSteps,
  ChatSuggestions,
  ChatWorking,
  ThinkingTrace,
  type ChatStep as StepLine,
} from "~/lib/ui";
import {
  displayName,
  useConversation,
  useConversationsByIds,
  type ConversationSummary,
} from "~/modules/conversations";

import {
  askScout,
  deleteChat,
  useChatAbout,
  useChatCache,
  type ChatMessage as Message,
  type ChatStep,
  type ChatTool,
} from "../../api/chats";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

const CONVERSATION = "conversations.conversation";

/** Where the chat sits beside the page rather than over it. */
const WIDE = "(width >= 64rem)";

export interface ScoutChatProps {
  workspaceId: string;
  /** The conversation the chat is about. */
  conversationId: string;
  onClose: () => void;
  /** Over the whole screen (stories and tests pin it); follows the window otherwise. */
  modal?: boolean | undefined;
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

/**
 * Scout's chat about a conversation (campbooks-cloud#305, "Ask about
 * this"): opening it starts the member's chat or continues theirs; Scout
 * explains and looks things up, as the member, and changes nothing. Its
 * answers come as they are written (whole on the desktop), with what Scout
 * looked at one click away. Closing it stops a question being answered.
 */
export function ScoutChat({ workspaceId, conversationId, onClose, modal }: ScoutChatProps) {
  const { t: typed, i18n } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const chat = useChatAbout(workspaceId, conversationId);
  const cache = useChatCache(workspaceId, conversationId);
  const detail = useConversation(workspaceId, conversationId);
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
  const subject =
    head === undefined ? "" : head.subject === "" ? t("chat.noSubject") : head.subject;
  const firstName = inbound?.from.name?.trim().split(/\s+/u)[0] ?? "";

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
      const code = problemCode(error);
      if (errorStatus(error) === 429) {
        // Nothing was asked: the words stay in the field.
        setRateLimited(true);
        setDraft(question);
      } else if (code === "chat_full") {
        setAnswerProblem("full");
        setDraft(question);
      } else if (code === "ai_daily_limit_reached") {
        setAnswerProblem("limit");
      } else if (code === "chat_unavailable") {
        setAnswerProblem("unavailable");
      }
      // Anything else failed after the question was kept: the stored answer
      // says so, with Try again.
    }
    if (!controller.signal.aborted) {
      await cache.refresh();
      setPending(undefined);
    }
  };

  const startOver = async () => {
    const id = chat.data?.id;
    if (id === undefined) {
      return;
    }
    await deleteChat(workspaceId, id);
    setAnswerProblem(undefined);
    await cache.refresh();
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
      return { text: stepText(t, step, conversationId), records };
    });

  const last = messages.at(-1);
  const questionBefore = (index: number) =>
    messages.slice(0, index).findLast((message) => message.author === "member")?.text ?? "";

  const log = (
    <>
      {messages.length === 0 &&
      pending === undefined &&
      problem === undefined &&
      head !== undefined ? (
        <ChatSuggestions
          label={t("chat.suggestions")}
          intro={t("chat.intro", { sender, subject })}
          questions={[
            t("chat.why"),
            t("chat.happened"),
            ...(firstName === "" ? [] : [t("chat.else", { name: firstName })]),
          ]}
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

  const footer =
    problem === "unavailable" ? (
      <ChatNotice visor="asleep">{t("chat.unavailable")}</ChatNotice>
    ) : problem === "unreadable" ? (
      <ChatNotice visor="unsure">{t("chat.unreadable")}</ChatNotice>
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
          label={t("chat.field")}
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

  return (
    <ChatPanel
      title={t("chat.title")}
      visor={pending === undefined ? "idle" : "thinking"}
      closeLabel={t("chat.close")}
      onClose={onClose}
      modal={modal}
      follow={[messages.length, pending?.text.length, pending?.working.length, i18n.language]}
      announcement={announcement}
      about={
        head === undefined ? undefined : (
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
            {t("chat.aboutLine", { sender, subject })}
          </ChatAbout>
        )
      }
      footer={footer}
    >
      {log}
    </ChatPanel>
  );
}

/** A step as a sentence, from its tool and its count. */
function stepText(t: Translate, step: ChatStep, aboutId: string): string {
  const aboutThis = step.records.some((record) => record.id === aboutId);
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
  steps: StepLine[];
  /** The chat's last message: a failure offers Try again. */
  isLast: boolean;
  onRetry: () => void;
}

/** A stored question or answer. */
function Stored({ message, t, steps, isLast, onRetry }: StoredProps) {
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
          <ChatSteps summary={t("chat.looked")} steps={steps} />
        </>
      }
    />
  );
}
