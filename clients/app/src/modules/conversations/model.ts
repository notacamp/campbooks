// Pure helpers of the inbox screens: what a row, a message and the
// assistant's note say, from the API's records. No React, no fetching.
import type {
  ConversationSummary,
  DecisionView,
  MessageView,
  ReasonView,
  Stream,
} from "./api/queries";

/** `t` for keys only known at runtime (stream names, reason ids). */
export type Translate = (key: string, options?: Record<string, unknown>) => string;

const DAY_MS = 86_400_000;

/** A list's time: "09:05" today, "Fri" this week, "12 Oct" this year, "12 Oct 2025" before. */
export function listTime(iso: string, locale: string, now: Date = new Date()): string {
  const date = new Date(iso);
  const sameDay = date.toDateString() === now.toDateString();
  if (sameDay) {
    return new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(date);
  }
  if (now.getTime() - date.getTime() < 6 * DAY_MS && date < now) {
    return new Intl.DateTimeFormat(locale, { weekday: "short" }).format(date);
  }
  return new Intl.DateTimeFormat(locale, {
    day: "numeric",
    month: "short",
    ...(date.getFullYear() === now.getFullYear() ? {} : { year: "numeric" }),
  }).format(date);
}

/** A message's time: "Fri 17:42" this week, "12 Oct, 17:42" before. */
export function messageTime(iso: string, locale: string, now: Date = new Date()): string {
  const date = new Date(iso);
  const recent = now.getTime() - date.getTime() < 6 * DAY_MS;
  return new Intl.DateTimeFormat(locale, {
    ...(recent ? { weekday: "short" } : { day: "numeric", month: "short" }),
    ...(date.getFullYear() === now.getFullYear() ? {} : { year: "numeric" }),
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

/** How an address reads: its display name, or the address itself. */
export function displayName(address: { name?: string | null; address: string } | null | undefined) {
  if (address === null || address === undefined) {
    return "";
  }
  const name = address.name?.trim() ?? "";
  return name === "" ? address.address : name;
}

/** Machines and bulk senders get the squircle avatar: anything not sorted (or suggested) as Primary. */
export function avatarKind(conversation: ConversationSummary): "person" | "service" {
  const stream = conversation.suggestion?.stream ?? conversation.stream;
  return stream === "primary" ? "person" : "service";
}

/** "To Sam Rivera, cc Leo Brandt". */
export function recipientsLine(t: Translate, message: MessageView, locale: string): string {
  const list = new Intl.ListFormat(locale, { type: "conjunction" });
  const to = message.to.map((a) => displayName(a));
  const cc = message.cc.map((a) => displayName(a));
  const parts = [];
  if (to.length > 0) {
    parts.push(t("reader.to", { names: list.format(to) }));
  }
  if (cc.length > 0) {
    parts.push(t("reader.cc", { names: list.format(cc) }));
  }
  return parts.join(", ");
}

export function streamName(t: Translate, stream: Stream): string {
  return t(`streams.${stream}`);
}

/** One stored reason, as a sentence in the reader's language. */
export function reasonText(
  t: Translate,
  exists: (key: string) => boolean,
  reason: ReasonView,
): string {
  const key = `reasons.${reason.rule}`;
  if (exists(key)) {
    return t(key, { evidence: reason.evidence });
  }
  return t("reasons.other", { rule: reason.rule, evidence: reason.evidence });
}

/** Questions whose "yes" the note mentions, with their phrase and "Why?" label. */
const FLAGS = [
  { question: "needs_reply", phrase: "scout.needsReply", why: "scout.whyNeedsReply" },
  { question: "asks_action", phrase: "scout.asksAction", why: "scout.whyAsksAction" },
  { question: "has_deadline", phrase: "scout.hasDeadline", why: "scout.whyHasDeadline" },
  { question: "urgency", phrase: "scout.urgent", why: "scout.whyUrgent" },
  { question: "suspicious", phrase: "scout.suspicious", why: "scout.whySuspicious" },
] as const;

/** Urgency is a level from 0 (no hurry) to 2 (today): 2 counts as urgent. */
const URGENT_LEVEL = 2;

function holds(decision: DecisionView): boolean {
  if (decision.question === "urgency") {
    return typeof decision.value === "number" && decision.value >= URGENT_LEVEL;
  }
  return decision.value === true;
}

/** The yes answers among `decisions`, in asking order. */
function flagsOf(decisions: readonly DecisionView[]) {
  return FLAGS.flatMap((flag) => {
    const decision = decisions.find((d) => d.question === flag.question);
    return decision !== undefined && holds(decision) ? [{ ...flag, decision }] : [];
  });
}

/** The note's sentence: what the decide pass thinks, said as suggestions. */
export function noteSentence(
  t: Translate,
  decisions: readonly DecisionView[],
  locale: string,
): string {
  const flags = flagsOf(decisions);
  if (flags.length === 0) {
    return t("scout.nothing");
  }
  const list = new Intl.ListFormat(locale, { type: "conjunction" });
  return t("scout.read", { list: list.format(flags.map((flag) => t(flag.phrase))) });
}

/** The note's footer line: where the conversation is and who put it there. */
export function placementLine(
  t: Translate,
  conversation: ConversationSummary,
  decisions: readonly DecisionView[],
): string {
  const stream = streamName(t, conversation.stream);
  const suggestion = conversation.suggestion;
  if (suggestion !== null && suggestion !== undefined) {
    const suggested = streamName(t, suggestion.stream);
    return suggestion.confidence === null || suggestion.confidence === undefined
      ? t("scout.suggestion", { stream: suggested })
      : t("scout.suggestionSure", { stream: suggested, confidence: suggestion.confidence });
  }
  switch (conversation.stream_source) {
    case "member": {
      return t("scout.sortedMember", { stream });
    }
    case "model": {
      return t("scout.sortedModel", { stream });
    }
    case "rules": {
      const confidence = decisions.find((d) => d.question === "stream")?.confidence;
      return confidence === null || confidence === undefined
        ? t("scout.sortedRule", { stream })
        : t("scout.sortedRuleSure", { stream, confidence });
    }
    case "default": {
      return t("scout.sortedDefault", { stream });
    }
  }
}

/** Every stored reason behind what the note says, labelled by question. */
export function whyLines(
  t: Translate,
  exists: (key: string) => boolean,
  conversation: ConversationSummary,
  decisions: readonly DecisionView[],
): string[] {
  const streamReasons =
    conversation.suggestion?.reasons ??
    decisions.find((d) => d.question === "stream")?.reasons ??
    [];
  const lines = streamReasons.map(
    (reason) => `${t("scout.whyStream")}: ${reasonText(t, exists, reason)}`,
  );
  for (const flag of flagsOf(decisions)) {
    for (const reason of flag.decision.reasons) {
      lines.push(`${t(flag.why)}: ${reasonText(t, exists, reason)}`);
    }
  }
  return [...new Set(lines)];
}

/** Older messages fold; the newest (and any unread) stay open. */
export function openByDefault(messages: readonly MessageView[]): Set<string> {
  const open = new Set<string>();
  const newest = messages.at(-1);
  if (newest !== undefined) {
    open.add(newest.id);
  }
  for (const message of messages) {
    if (!message.read) {
      open.add(message.id);
    }
  }
  return open;
}

/** Raster images: decoded by an `<img>` and nothing else (the server's list). */
const RASTER_IMAGES = new Set([
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/avif",
  "image/bmp",
]);

/** A media type without parameters, lower-cased: `IMAGE/PNG; name=a` → `image/png`. */
function essence(contentType: string): string {
  return (contentType.split(";", 1)[0] ?? "").trim().toLowerCase();
}

/**
 * What an attachment can be shown as: a raster image or a PDF has a preview;
 * everything else (HTML and SVG included) is only downloaded.
 */
export function attachmentKind(contentType: string): "image" | "pdf" | "file" {
  const type = essence(contentType);
  if (RASTER_IMAGES.has(type)) {
    return "image";
  }
  return type === "application/pdf" ? "pdf" : "file";
}

/** The image type to show a preview as, from the client's own list only. */
export function previewType(contentType: string): string | undefined {
  const type = essence(contentType);
  return RASTER_IMAGES.has(type) ? type : undefined;
}

/**
 * A file name as the app shows and saves it: no control characters,
 * bidirectional controls (`invoice‮fdp.exe` would read as
 * `invoiceexe.pdf`) or path separators, and `attachment` when nothing is
 * left.
 */
export function cleanFileName(name: string): string {
  const cleaned = name
    .replaceAll(/[\p{Cc}‎‏‪-‮⁦-⁩/\\]/gu, "")
    .trim()
    .replace(/^\.+/u, "")
    .trim();
  return cleaned === "" ? "attachment" : cleaned;
}
