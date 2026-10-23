import { useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { refreshRecords } from "~/lib/api";
import { useOpenReply } from "~/lib/reply-opener";
import { Button, ChatProposal, Dialog, type ChatProposalState } from "~/lib/ui";

import {
  NotRunnable,
  refusalReason,
  runnable,
  runRequest,
  settleProposal,
  undoBody,
  withAnswer,
  type Proposal,
  type Settlement,
} from "../../api/proposals";
import { TeachScout } from "../TeachScout/TeachScout";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

interface CardsProps {
  workspaceId: string;
  chatId: string;
  /** Scout's proposals in one answer. */
  proposals: readonly Proposal[];
  /** A card was settled: the chat is read again. */
  onSettled: () => void;
}

/**
 * The cards under one of Scout's answers (campbooks-cloud#305): a change,
 * a rule or a reply, each written here from the action and the record's
 * facts. Nothing happens until the member chooses: Confirm sends their own
 * request, a rule opens Teach Scout, a reply opens the Reply dialog.
 */
export function ProposalCards({ workspaceId, chatId, proposals, onSettled }: CardsProps) {
  if (proposals.length === 0) {
    return null;
  }
  return (
    <div className="flex flex-col gap-2">
      {proposals.map((proposal) => {
        const props = { workspaceId, chatId, proposal, onSettled };
        switch (proposal.kind) {
          case "rule": {
            return <RuleCard key={proposal.id} {...props} />;
          }
          case "reply": {
            return <ReplyCard key={proposal.id} {...props} />;
          }
          default: {
            return <ChangeCard key={proposal.id} {...props} />;
          }
        }
      })}
    </div>
  );
}

interface CardProps {
  workspaceId: string;
  chatId: string;
  proposal: Proposal;
  onSettled: () => void;
}

/** What the member did, recorded for Scout's next answers; the card already says it. */
function useSettle({ workspaceId, chatId, proposal, onSettled }: CardProps) {
  return async (state: Settlement) => {
    try {
      await settleProposal(workspaceId, chatId, proposal.id, state);
    } catch {
      // The card still says what happened; Scout reads it as unanswered.
    } finally {
      onSettled();
    }
  };
}

/** Until when, in the member's language and time zone: "Monday 9:00". */
function untilText(iso: string, locale: string): string {
  const at = new Date(iso);
  const days = (at.getTime() - Date.now()) / 86_400_000;
  const options: Intl.DateTimeFormatOptions =
    days < 6
      ? { weekday: "long", hour: "numeric", minute: "2-digit" }
      : { weekday: "short", day: "numeric", month: "short", hour: "numeric", minute: "2-digit" };
  return new Intl.DateTimeFormat(locale, options).format(at);
}

/** A calendar day (`YYYY-MM-DD`) as the member reads it: "Mon, 2 Nov". */
function dayText(day: string, locale: string): string {
  const [year, month, date] = day.split("-").map(Number);
  const at = new Date(year ?? 0, (month ?? 1) - 1, date ?? 1);
  return new Intl.DateTimeFormat(locale, {
    weekday: "short",
    day: "numeric",
    month: "short",
  }).format(at);
}

/** A start as the member reads it: "Thu, 12 Nov, 15:00", or the day alone when all-day. */
function whenText(start: string, locale: string): string {
  if (/^\d{4}-\d{2}-\d{2}$/u.test(start)) {
    return dayText(start, locale);
  }
  const at = new Date(start);
  if (Number.isNaN(at.getTime())) {
    return start;
  }
  return new Intl.DateTimeFormat(locale, {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "numeric",
    minute: "2-digit",
  }).format(at);
}

/**
 * "Dentist · Thu, 12 Nov, 15:00", from the event's own record, never from
 * the model's words.
 */
function eventLine(t: Translate, proposal: Proposal, locale: string): string | undefined {
  const event = proposal.event;
  if (event === undefined || event === null) {
    return undefined;
  }
  return t("chat.proposal.eventLine", { title: event.title, when: whenText(event.starts, locale) });
}

/**
 * "Renew the van insurance · due Mon, 2 Nov", from the task's own record
 * (and, for a hand-off, where it goes), never from the model's words.
 */
function taskLine(t: Translate, proposal: Proposal, locale: string): string | undefined {
  const task = proposal.task;
  if (task === undefined || task === null) {
    return undefined;
  }
  const line =
    task.due_on === undefined || task.due_on === null
      ? t("chat.proposal.taskNoDue", { title: task.title })
      : t("chat.proposal.taskLine", { title: task.title, due: dayText(task.due_on, locale) });
  const name = proposal.detail?.member_name;
  return name === undefined || name === null
    ? line
    : `${line}. ${t("chat.proposal.handOffNote", { name })}`;
}

/** "Marta Ferreira · Walkthrough on Thursday", from the record's facts. */
function aboutLine(t: Translate, proposal: Proposal): string | undefined {
  const facts = proposal.facts;
  if (facts === undefined || facts === null) {
    return undefined;
  }
  return t("chat.aboutLine", {
    sender: facts.sender,
    subject: facts.subject === "" ? t("chat.noSubject") : facts.subject,
  });
}

/** The card's label: proposing while there is a choice, proposed after. */
function labelOf(t: Translate, state: ChatProposalState): string {
  return t(
    state === "pending" || state === "working" ? "chat.proposal.label" : "chat.proposal.labelPast",
  );
}

/** A change to a conversation: Confirm, Not now, then Undo. */
function ChangeCard(props: CardProps) {
  const { workspaceId, proposal } = props;
  const { t: typed, i18n } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const queryClient = useQueryClient();
  const settle = useSettle(props);
  const [phase, setPhase] = useState<ChatProposalState | undefined>();
  const [reason, setReason] = useState<string | undefined>();
  const [undoFailed, setUndoFailed] = useState(false);
  // The change's answer, in this session only: Undo takes fields from it.
  const [answer, setAnswer] = useState<{ value: unknown } | undefined>();
  // One request at a time, even for two quick presses.
  const acting = useRef(false);
  const request = proposal.request ?? undefined;
  const undo = proposal.undo ?? undefined;
  const ready = request === undefined ? undefined : runnable(request);
  const undoReady = undo === undefined ? undefined : runnable(undo);
  const state: ChatProposalState =
    phase ??
    (proposal.state === "pending" ? (ready === undefined ? "blocked" : "pending") : proposal.state);

  const confirm = async () => {
    if (acting.current || request === undefined || ready === undefined) {
      return;
    }
    acting.current = true;
    setPhase("working");
    try {
      const value = await runRequest(workspaceId, request, proposal.id, request.body);
      setAnswer({ value });
      setPhase("done");
      void refreshRecords(queryClient, workspaceId, ready.entry.records);
      await settle("done");
    } catch (error) {
      if (error instanceof NotRunnable) {
        setPhase("blocked");
      } else {
        setReason(refusalReason(error));
        setPhase("failed");
        await settle("failed");
      }
    } finally {
      acting.current = false;
    }
  };

  const undoIt = async () => {
    if (acting.current || undo === undefined || answer === undefined) {
      return;
    }
    acting.current = true;
    setUndoFailed(false);
    setPhase("working");
    try {
      // An undo may name the id the change's answer gave (a new event).
      const target = withAnswer(undo, answer.value);
      if (target === undefined) {
        throw new NotRunnable();
      }
      await runRequest(workspaceId, target, `${proposal.id}:undo`, undoBody(undo, answer.value));
      setPhase("undone");
      void refreshRecords(queryClient, workspaceId, undoReady?.entry.records ?? []);
      await settle("undone");
    } catch {
      setUndoFailed(true);
      setPhase("done");
    } finally {
      acting.current = false;
    }
  };

  const decline = async () => {
    if (acting.current) {
      return;
    }
    setPhase("declined");
    await settle("declined");
  };

  const action = proposal.action ?? "archive";
  const detail: Partial<NonNullable<Proposal["detail"]>> = proposal.detail ?? {};
  // A bill is paid, not done.
  const key =
    action === "task_done" && proposal.task?.verb === "pay"
      ? "task_paid"
      : action === "update_event" && detail.starts !== undefined && detail.starts !== null
        ? "update_event_moved"
        : action;
  const title = t(`chat.proposal.action.${key}`, {
    calendar: detail.calendar_name ?? proposal.event?.calendar ?? "",
    response:
      detail.response === undefined || detail.response === null
        ? ""
        : t(`chat.proposal.responses.${detail.response}`),
    when:
      detail.starts === undefined || detail.starts === null
        ? ""
        : whenText(detail.starts, i18n.language),
    day: detail.day === undefined || detail.day === null ? "" : dayText(detail.day, i18n.language),
    name: detail.member_name ?? "",
    stream:
      detail.stream === undefined || detail.stream === null
        ? ""
        : t(`chat.proposal.streams.${detail.stream}`),
    until:
      detail.until === undefined || detail.until === null
        ? ""
        : untilText(detail.until, i18n.language),
  });
  // The model's own words for an event (its new title and place), quoted.
  const eventWords = [detail.title, detail.place]
    .filter((part): part is string => typeof part === "string" && part !== "")
    .join(" · ");
  const label = detail.label ?? (eventWords === "" ? undefined : eventWords);
  const status = {
    pending: undefined,
    working: t("chat.proposal.working"),
    done: undoFailed ? t("chat.proposal.undoFailed") : t("chat.proposal.done"),
    undone: t("chat.proposal.undone"),
    declined: t("chat.proposal.declined"),
    failed:
      reason === undefined
        ? t("chat.proposal.failed")
        : t("chat.proposal.failedBecause", { reason }),
    blocked: t("chat.proposal.blocked"),
  }[state];
  const stillPending = proposal.state === "pending" && phase === undefined;
  const actions =
    state === "pending" ? (
      <>
        <Button
          onClick={() => {
            void confirm();
          }}
        >
          {t("chat.proposal.confirm")}
        </Button>
        <Button
          onClick={() => {
            void decline();
          }}
        >
          {t("chat.proposal.notNow")}
        </Button>
      </>
    ) : state === "blocked" && stillPending ? (
      <Button
        onClick={() => {
          void decline();
        }}
      >
        {t("chat.proposal.notNow")}
      </Button>
    ) : state === "done" && answer !== undefined && undoReady !== undefined ? (
      <Button
        onClick={() => {
          void undoIt();
        }}
      >
        {t("chat.proposal.undo")}
      </Button>
    ) : undefined;

  return (
    <ChatProposal
      label={labelOf(t, state)}
      title={title}
      about={
        taskLine(t, proposal, i18n.language) ??
        eventLine(t, proposal, i18n.language) ??
        aboutLine(t, proposal)
      }
      quote={label}
      quoteLabel={label === undefined ? undefined : t("chat.proposal.scoutWords")}
      state={state}
      status={status}
      actions={actions}
    />
  );
}

/** A rule for Teach Scout: Review the rule opens it with the sentence. */
function RuleCard(props: CardProps) {
  const { workspaceId, proposal } = props;
  const { t: typed } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const settle = useSettle(props);
  const [phase, setPhase] = useState<ChatProposalState | undefined>();
  const [teaching, setTeaching] = useState(false);
  const saved = useRef(false);
  const state: ChatProposalState = phase ?? proposal.state;
  const sentence = proposal.sentence ?? "";

  const close = () => {
    setTeaching(false);
    if (!saved.current) {
      setPhase("declined");
      void settle("declined");
    }
  };

  return (
    <>
      <ChatProposal
        label={labelOf(t, state)}
        title={t("chat.proposal.rule.title")}
        quote={sentence}
        quoteLabel={t("chat.proposal.scoutWords")}
        state={state}
        status={
          state === "done"
            ? t("chat.proposal.rule.done")
            : state === "declined"
              ? t("chat.proposal.declined")
              : undefined
        }
        actions={
          state === "pending" ? (
            <>
              <Button
                onClick={() => {
                  setTeaching(true);
                }}
              >
                {t("chat.proposal.rule.review")}
              </Button>
              <Button
                onClick={() => {
                  setPhase("declined");
                  void settle("declined");
                }}
              >
                {t("chat.proposal.notNow")}
              </Button>
            </>
          ) : undefined
        }
      />
      <Dialog
        open={teaching}
        onClose={close}
        title={t("chat.proposal.rule.dialog")}
        description={t("chat.proposal.rule.dialogLead")}
      >
        {teaching ? (
          <TeachScout
            workspaceId={workspaceId}
            initial={sentence}
            onSaved={() => {
              saved.current = true;
              setPhase("done");
              setTeaching(false);
              void settle("done");
            }}
          />
        ) : null}
      </Dialog>
    </>
  );
}

/** A reply: Write the draft opens the conversation's Reply dialog with the intent. */
function ReplyCard(props: CardProps) {
  const { proposal } = props;
  const { t: typed } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const settle = useSettle(props);
  const openReply = useOpenReply();
  const [phase, setPhase] = useState<ChatProposalState | undefined>();
  const record = proposal.record ?? undefined;
  const facts = proposal.facts ?? undefined;
  const intent = proposal.intent ?? "";
  const canOpen = openReply !== undefined && record !== undefined && facts !== undefined;
  const state: ChatProposalState =
    phase ?? (proposal.state === "pending" && !canOpen ? "blocked" : proposal.state);

  const write = () => {
    if (!canOpen) {
      return;
    }
    openReply({
      conversationId: record.id,
      stream: facts.stream,
      name: facts.sender,
      intent,
    });
    setPhase("done");
    void settle("done");
  };
  const decline = () => {
    setPhase("declined");
    void settle("declined");
  };

  return (
    <ChatProposal
      label={labelOf(t, state)}
      title={t("chat.proposal.reply.title", { sender: facts?.sender ?? "" })}
      about={aboutLine(t, proposal)}
      quote={intent}
      quoteLabel={t("chat.proposal.scoutWords")}
      state={state}
      status={
        state === "done"
          ? t("chat.proposal.reply.done")
          : state === "declined"
            ? t("chat.proposal.declined")
            : state === "blocked"
              ? t("chat.proposal.blocked")
              : undefined
      }
      actions={
        state === "pending" ? (
          <>
            <Button onClick={write}>{t("chat.proposal.reply.write")}</Button>
            <Button onClick={decline}>{t("chat.proposal.notNow")}</Button>
          </>
        ) : state === "blocked" && phase === undefined && proposal.state === "pending" ? (
          <Button onClick={decline}>{t("chat.proposal.notNow")}</Button>
        ) : undefined
      }
    />
  );
}
