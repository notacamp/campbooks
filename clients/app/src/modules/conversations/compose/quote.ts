import type { Schema } from "~/lib/api";

type Message = Schema<"MessageView">;
type Address = Schema<"Address">;

/** The words a quote is written with, in the member's language. */
export interface QuoteWords {
  /** "On {{date}}, {{name}} wrote:" */
  wrote: (date: string, name: string) => string;
  /** "Forwarded message" */
  forwarded: string;
  from: string;
  date: string;
  subject: string;
  to: string;
}

/** "Ana Lopes <ana@example.com>", or the bare address. */
export function addressLine(address: Address): string {
  const name = address.name?.trim();
  return name === undefined || name === "" ? address.address : `${name} <${address.address}>`;
}

/** The name to show for someone: their name, else their address. */
export function displayName(address: Address): string {
  const name = address.name?.trim();
  return name === undefined || name === "" ? address.address : name;
}

function when(sentAt: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(sentAt),
  );
}

/** The message's text, or nothing when it has none. */
function textOf(message: Message): string {
  return message.text?.content.trimEnd() ?? "";
}

/** The history under a reply: an attribution line and the message quoted with `> `. */
export function replyQuote(message: Message, locale: string, words: QuoteWords): string {
  const lines = textOf(message)
    .split("\n")
    .map((line) => (line === "" ? ">" : `> ${line}`));
  return [words.wrote(when(message.sent_at, locale), displayName(message.from)), ...lines].join(
    "\n",
  );
}

/** The header block and text under a forward. */
export function forwardQuote(message: Message, locale: string, words: QuoteWords): string {
  const header = [
    `---------- ${words.forwarded} ----------`,
    `${words.from}: ${addressLine(message.from)}`,
    `${words.date}: ${when(message.sent_at, locale)}`,
    `${words.subject}: ${message.subject}`,
    `${words.to}: ${message.to.map(addressLine).join(", ")}`,
  ];
  return [...header, "", textOf(message)].join("\n").trimEnd();
}
