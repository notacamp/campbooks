// What a conversation needs, in words: the sentence, the chips, the due date
// and "How Campbooks decided". Pure helpers over the API's records, shared by
// the inbox and Home. Mail text (names, the quote) is only ever interpolated
// into fixed sentences and shown as plain text: never an instruction.
import type { ConversationSummary, DecisionView, NeedView } from "./api/queries";
import { displayName, reasonText, type Translate } from "./model";

const DAY_MS = 86_400_000;

/**
 * A bill: a Pay need from the money question. `ask` while Campbooks asks
 * "Is it paid?", `toPay` once the member said it is still to pay; not a
 * bill otherwise.
 */
export function billKind(need: NeedView): "ask" | "toPay" | undefined {
  if (need.reason !== "pay" || need.question !== "money") {
    return undefined;
  }
  return need.trust === "check" ? "ask" : "toPay";
}

/**
 * "Still needed?": an old ask with no date of its own, asked again because
 * time passed (campbooks-cloud#275). Its answers teach Campbooks nothing.
 */
export function isStillNeeded(need: NeedView): boolean {
  return need.question === "still_to_do" || need.question === "still_needs_reply";
}

/**
 * How long ago `iso` was, in the user's words: "3 weeks ago", "há 3
 * semanas". Days under two weeks, weeks under two months, then months and
 * years.
 */
function agoLabel(iso: string, locale: string, now: Date = new Date()): string {
  const days = Math.max(0, Math.floor((now.getTime() - new Date(iso).getTime()) / DAY_MS));
  const words = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  if (days < 14) {
    return words.format(-days, "day");
  }
  if (days < 60) {
    return words.format(-Math.floor(days / 7), "week");
  }
  if (days < 365) {
    return words.format(-Math.floor(days / 30), "month");
  }
  return words.format(-Math.floor(days / 365), "year");
}

/** Who wrote the email a need comes from, as far as the summary says. */
function needFrom(need: NeedView, conversation: ConversationSummary) {
  return need.quote?.from ?? conversation.latest_from ?? null;
}

/** Who a need is from: the sender's name; for a warning, their address (a name is free to fake). */
export function needSender(need: NeedView, conversation: ConversationSummary): string {
  const from = needFrom(need, conversation);
  if (need.question === "suspicious") {
    return from?.address ?? "";
  }
  return displayName(from);
}

/** Whether Scout waits on someone to write (`waiting`), or it is time to nudge them (`nudge`). */
export function isWait(need: NeedView): boolean {
  return need.reason === "waiting" || need.reason === "nudge";
}

/**
 * Who a wait is on (campbooks-cloud#293): their name and address as the
 * email's headers gave them, and whether they are the one who wrote it (the
 * sender promised something) rather than someone else on it. None for any
 * other need.
 */
export function awaitedOf(
  need: NeedView,
  conversation: ConversationSummary,
): { name: string; address: string; sender: boolean } | undefined {
  const awaited = need.waiting_on;
  if (!isWait(need) || awaited === null || awaited === undefined) {
    return undefined;
  }
  const from = needFrom(need, conversation);
  return {
    name: displayName(awaited),
    address: awaited.address,
    sender:
      from !== null && from.address.trim().toLowerCase() === awaited.address.trim().toLowerCase(),
  };
}

/**
 * The need's sentence, chosen by reason and by the rule that decided it. An
 * old ask asked again says how long ago it came, in `when.locale` as of
 * `when.now`.
 */
export function needSentence(
  t: Translate,
  need: NeedView,
  name: string,
  when?: { locale: string; now?: Date | undefined },
  conversation?: ConversationSummary,
): string {
  // A wait says who has not written yet: the sender ("No word from Rui
  // Matos yet") or someone else on the email ("Ana Costa hasn't written
  // yet").
  if (isWait(need)) {
    const awaited = conversation === undefined ? undefined : awaitedOf(need, conversation);
    const person = awaited?.name ?? displayName(need.waiting_on);
    if (need.reason === "waiting") {
      return t("needs.line.waiting", { name: person });
    }
    return t(awaited?.sender === true ? "needs.line.nudgeSender" : "needs.line.nudge", {
      name: person,
    });
  }
  if (need.question === "suspicious") {
    return t("needs.line.warning");
  }
  // A rule the member taught keeps it in front of them until they open it.
  if (need.question === "worth_a_look") {
    return t("needs.line.worthALook", { name });
  }
  if (need.trust === "check") {
    if (billKind(need) === "ask") {
      // Campbooks can't see the bank: a bill is a question until answered
      // (Already paid / Still to pay).
      return t("needs.line.checkPay", { name });
    }
    const doing = need.question === "asks_action" || need.question === "still_to_do";
    const sent = need.sent_at;
    if (isStillNeeded(need) && sent !== null && sent !== undefined && when !== undefined) {
      return t(doing ? "needs.line.stillDo" : "needs.line.stillReply", {
        name,
        ago: agoLabel(sent, when.locale, when.now),
      });
    }
    return t(doing ? "needs.line.checkDo" : "needs.line.checkReply", { name });
  }
  // A rule a member taught raised it: say so, never that the sender asked.
  if (need.basis === "taught") {
    return t(need.question === "needs_reply" ? "needs.line.taughtReply" : "needs.line.taughtDo", {
      name,
    });
  }
  switch (need.reason) {
    case "pay": {
      return t("needs.line.pay", { name });
    }
    case "do": {
      return t("needs.line.do", { name });
    }
    case "reply":
    case "review": {
      return t(need.rule === "signals.question" ? "needs.line.question" : "needs.line.reply", {
        name,
      });
    }
    case "worth_a_look": {
      return t("needs.line.worthALook", { name });
    }
    // Said above; here so every reason has its sentence.
    case "waiting":
    case "nudge": {
      return t("needs.line.nudge", { name });
    }
  }
}

/** The chip naming what it needs. */
export function needChip(t: Translate, need: NeedView): string {
  if (need.question === "suspicious") {
    return t("needs.chip.suspicious");
  }
  if (isWait(need)) {
    return t(`needs.chip.${need.reason}`, { name: displayName(need.waiting_on) });
  }
  if (need.trust === "check" && billKind(need) === undefined) {
    return t("needs.chip.check");
  }
  return t(`needs.chip.${need.reason}`);
}

/** The signal colour of a need's chip. */
export function needSignal(need: NeedView): "reply" | "do" | "pay" | "review" | "look" | "nudge" {
  // A wait is never a question to the member, whatever its trust.
  if (isWait(need)) {
    return "nudge";
  }
  if (need.trust === "check" && billKind(need) === undefined) {
    return "review";
  }
  return need.reason === "worth_a_look" ? "look" : need.reason;
}

/** Days from `today` to `isoDate` (a calendar date), negative when past. */
function daysUntil(isoDate: string, today: Date): number {
  const [year, month, day] = isoDate.split("-").map(Number);
  const due = Date.UTC(year ?? 0, (month ?? 1) - 1, day ?? 1);
  const now = Date.UTC(today.getFullYear(), today.getMonth(), today.getDate());
  return Math.round((due - now) / DAY_MS);
}

function dateLabel(isoDate: string, locale: string, withWeekday: boolean): string {
  const [year, month, day] = isoDate.split("-").map(Number);
  const date = new Date(Date.UTC(year ?? 0, (month ?? 1) - 1, day ?? 1));
  return new Intl.DateTimeFormat(locale, {
    timeZone: "UTC",
    ...(withWeekday ? { weekday: "short" } : { day: "numeric", month: "short" }),
  }).format(date);
}

/** "Due today", "Due tomorrow", "Due Thu", "Due 30 Oct", "Overdue · was due Mon"; none without a date. */
export function dueLabel(
  t: Translate,
  need: NeedView,
  locale: string,
  today: Date = new Date(),
): { text: string; overdue: boolean; soon: boolean } | undefined {
  const due = need.due_on;
  // A wait's date is when Scout checks again, not something due.
  if (due === null || due === undefined || isWait(need)) {
    return undefined;
  }
  const days = daysUntil(due, today);
  // A bill still to pay says how late it is: "2 days overdue".
  if (days < 0 && billKind(need) === "toPay") {
    return { text: t("needs.due.daysOverdue", { count: -days }), overdue: true, soon: true };
  }
  if (days < 0) {
    return {
      text: t("needs.due.overdue", { date: dateLabel(due, locale, days > -7) }),
      overdue: true,
      soon: true,
    };
  }
  if (days === 0) {
    return { text: t("needs.due.today"), overdue: false, soon: true };
  }
  if (days === 1) {
    return { text: t("needs.due.tomorrow"), overdue: false, soon: true };
  }
  return {
    text: t("needs.due.on", { date: dateLabel(due, locale, days < 7) }),
    overdue: false,
    soon: false,
  };
}

/** Who wrote the quote and when: "Marta Ferreira, Mon 12 Oct". */
export function quoteAttribution(need: NeedView, name: string, locale: string): string {
  if (need.quote === null || need.quote === undefined) {
    return name;
  }
  const when = new Intl.DateTimeFormat(locale, {
    weekday: "short",
    day: "numeric",
    month: "short",
  }).format(new Date(need.quote.sent_at));
  return `${name}, ${when}`;
}

/** The tells of a warning, in words. */
export function warningReasons(
  t: Translate,
  exists: (key: string) => boolean,
  decisions: readonly DecisionView[],
): string[] {
  const suspicious = decisions.find((d) => d.question === "suspicious");
  return (suspicious?.reasons ?? []).map((reason) => reasonText(t, exists, reason));
}

const QUESTIONS = [
  { question: "stream", label: "scout.whyStream" },
  { question: "needs_reply", label: "scout.whyNeedsReply" },
  { question: "asks_action", label: "scout.whyAsksAction" },
  { question: "has_deadline", label: "scout.whyHasDeadline" },
  { question: "urgency", label: "scout.whyUrgent" },
  { question: "suspicious", label: "scout.whySuspicious" },
  { question: "money", label: "scout.whyMoney" },
] as const;

function answerWords(t: Translate, decision: DecisionView): string {
  const value = decision.value;
  if (typeof value === "boolean") {
    return t(value ? "needs.how.yes" : "needs.how.no");
  }
  if (typeof value === "number") {
    return t(`needs.how.urgency${String(Math.max(0, Math.min(2, Math.round(value))))}`);
  }
  if (decision.question === "money") {
    return t(`needs.how.money.${value}`);
  }
  return t(`streams.${value}`);
}

/** A confidence of 0 to 100, as "0.90". */
function decimal(value: number): string {
  return (value / 100).toFixed(2);
}

/**
 * "How Campbooks decided": per question, the answer in words with its
 * reasons, then where the numbers come from (source, confidence, the
 * threshold it was held to, or that it is a suggestion).
 */
export function howRows(
  t: Translate,
  exists: (key: string) => boolean,
  decisions: readonly DecisionView[],
): { question: string; answer: string; source: string }[] {
  return QUESTIONS.flatMap(({ question, label }) => {
    const decision = decisions.find((d) => d.question === question);
    if (decision === undefined) {
      return [];
    }
    const reasons = decision.reasons.map((reason) => reasonText(t, exists, reason));
    const parts = [t(`needs.how.source.${decision.source}`)];
    if (decision.confidence !== null && decision.confidence !== undefined) {
      parts.push(decimal(decision.confidence));
    }
    parts.push(
      decision.threshold === null || decision.threshold === undefined
        ? t("needs.how.suggestion")
        : t("needs.how.actsFrom", { threshold: decimal(decision.threshold) }),
    );
    return [
      {
        question: t(label),
        answer: [answerWords(t, decision), ...reasons].join(" "),
        source: parts.join(" · "),
      },
    ];
  });
}
