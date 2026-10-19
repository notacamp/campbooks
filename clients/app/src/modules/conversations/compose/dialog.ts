// What the dialogs that write for the member over Home share (Reply,
// Nudge): who it goes to in words, the undo window of a send, the quote of
// the message answered. Pure helpers, no React.
import type { MessageView } from "../api/queries";
import type { Translate } from "../model";
import { displayName, type QuoteWords } from "./quote";

/** Longest intent or change the server takes, in characters. */
export const MAX_ASK_CHARS = 300;

/** Longest quote shown from the message answered, in characters. */
const QUOTE_CHARS = 280;

/** How long the send can still be undone, from its `send_at` (at least a second). */
export function undoWindow(sendAt: string | null | undefined): number | undefined {
  return sendAt === null || sendAt === undefined
    ? undefined
    : Math.max(1000, new Date(sendAt).getTime() - Date.now());
}

/** "Marta Ferreira <marta@casa-lume.example>", or the address alone. */
export function addressLine(party: { name?: string | null; address: string }): string {
  const name = party.name?.trim();
  return name === undefined || name === "" ? party.address : `${name} <${party.address}>`;
}

/** The first name, for a title ("Reply to Marta"). */
export function firstName(name: string): string {
  return name.includes("@") ? name : (name.split(/\s+/u, 1)[0] ?? name);
}

/** The words a quoted history is written with, in the member's language. */
export function quoteWords(t: Translate): QuoteWords {
  return {
    wrote: (date, who) => t("compose.quote.wrote", { date, name: who }),
    forwarded: t("compose.quote.forwarded"),
    from: t("compose.quote.from"),
    date: t("compose.quote.date"),
    subject: t("compose.quote.subject"),
    to: t("compose.quote.to"),
  };
}

/**
 * The quote a dialog shows of the message it answers: who wrote it and
 * when, its words (a long one in part) and what the preview leaves out
 * (campbooks-cloud#255): a long email's reply quotes only its first part; a
 * long quote is shown in part, and quoted whole. None without words.
 */
export function quotePreview(
  t: Translate,
  message: MessageView | undefined,
  locale: string,
): { from: string; text: string; note: string | undefined } | undefined {
  const said = message?.text?.content.trim() ?? "";
  if (message === undefined || said === "") {
    return undefined;
  }
  const shortened = said.length > QUOTE_CHARS;
  let note: string | undefined;
  if (message.quote_cut === true || message.text?.truncated === true) {
    note = t("reply.quoteCut");
  } else if (shortened) {
    note = t("reply.quoteShortened");
  }
  return {
    from: t("reply.quoteFrom", {
      name: displayName(message.from),
      date: new Intl.DateTimeFormat(locale, {
        weekday: "short",
        day: "numeric",
        month: "short",
      }).format(new Date(message.sent_at)),
    }),
    text: shortened ? `${said.slice(0, QUOTE_CHARS)}…` : said,
    note,
  };
}
