// Pure helpers of the inbox screens: what a row, a message and the
// assistant's note say, from the API's records. No React, no fetching.
import {
  isView,
  type ConversationSummary,
  type DecisionView,
  type MessageView,
  type Place,
  type ReasonView,
  type Stream,
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

/** A stream's name, or a view's ("Sent", "Trash"). */
export function placeName(t: Translate, place: Place): string {
  return isView(place) ? t(`views.${place}`) : streamName(t, place);
}

/**
 * A Sent row's lead: who the inbox's latest message went to. "To Marta
 * Ferreira", "To Marta Ferreira and 2 others".
 */
export function sentToLine(t: Translate, summary: ConversationSummary): string {
  const [first, ...others] = summary.sent_to;
  if (first === undefined) {
    return t("list.toHidden");
  }
  const name = displayName(first);
  return others.length === 0
    ? t("list.to", { name })
    : t("list.toOthers", { name, count: others.length });
}

/**
 * A move between places, each with its Undo: Done and Move to Inbox, Delete
 * and Restore, Report spam and Not spam.
 */
export type Move = "archive" | "unarchive" | "trash" | "untrash" | "spam" | "not_spam";

/** E: Done while in the inbox, Move to Inbox once archived; nothing in spam or the trash. */
export function archiveMove(summary: ConversationSummary): Move | undefined {
  if (summary.trashed) {
    return undefined;
  }
  return summary.archived ? "unarchive" : "archive";
}

/** #: Delete, or Restore what is in the trash (back where it was). */
export function trashMove(summary: ConversationSummary): Move {
  return summary.trashed && !summary.spam ? "untrash" : "trash";
}

/** !: Report spam, or Not spam for what is in spam; nothing in the trash. */
export function spamMove(summary: ConversationSummary): Move | undefined {
  if (summary.spam) {
    return "not_spam";
  }
  return summary.trashed ? undefined : "spam";
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
  { question: "needs_reply", phrase: "scout.needsReply" },
  { question: "asks_action", phrase: "scout.asksAction" },
  { question: "has_deadline", phrase: "scout.hasDeadline" },
  { question: "urgency", phrase: "scout.urgent" },
  { question: "suspicious", phrase: "scout.suspicious" },
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

/** The note's footer line: where the conversation is and who put it there, in words. */
export function placementLine(t: Translate, conversation: ConversationSummary): string {
  const stream = streamName(t, conversation.stream);
  const suggestion = conversation.suggestion;
  if (suggestion !== null && suggestion !== undefined) {
    return t("scout.suggestion", { stream: streamName(t, suggestion.stream) });
  }
  switch (conversation.stream_source) {
    case "member": {
      return t("scout.sortedMember", { stream });
    }
    case "model": {
      return t("scout.sortedModel", { stream });
    }
    case "rules": {
      return t("scout.sortedRule", { stream });
    }
    case "taught": {
      return t("scout.sortedTaught", { stream });
    }
    case "default": {
      return t("scout.sortedDefault", { stream });
    }
  }
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
