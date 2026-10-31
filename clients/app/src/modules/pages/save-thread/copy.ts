import type { BlockJson } from "./blocks";

/** One message of a thread as a copy needs it: its own words, quoted history already left out. */
export interface CopyMessage {
  /** Who wrote it, as the reader shows them. */
  from: string;
  /** When it was sent (RFC 3339). */
  sentAt: string;
  /** What it says. */
  text: string;
}

/**
 * Text that is safe to put in a page: no control characters, no characters
 * that reorder or hide the text around them (bidirectional overrides,
 * zero-width marks), tabs and line endings made plain, and no more than one
 * blank line in a row. It stays text: the page's schema is what decides what
 * a block can be, and nothing here is ever read as markup or as a link.
 */
export function sanitiseText(text: string): string {
  return text
    .replaceAll(/\r\n?/gu, "\n")
    .replaceAll("\t", " ")
    .replaceAll(/[\p{Cc}\p{Cf}]/gu, (character) => (kept(character) ? character : ""))
    .replaceAll(/ +\n/gu, "\n")
    .replaceAll(/\n{3,}/gu, "\n\n")
    .trim();
}

/**
 * Whether a control or format character stays: the line break, and the
 * joiners emoji and some scripts need. The rest (other controls, the
 * bidirectional overrides, zero-width marks, the byte order mark) can
 * reorder or hide text.
 */
function kept(character: string): boolean {
  return character === "\n" || character === "\u200D" || character === "\u200C";
}

const textNode = (text: string): BlockJson => ({ type: "text", text });

/** A paragraph of `text`; its line breaks stay as breaks. */
function paragraph(text: string): BlockJson {
  const content: BlockJson[] = [];
  for (const [index, line] of text.split("\n").entries()) {
    if (index > 0) {
      content.push({ type: "hardBreak" });
    }
    if (line !== "") {
      content.push(textNode(line));
    }
  }
  return { type: "paragraph", content };
}

/**
 * A copy of the messages as blocks: for each, a small heading ("Ana — 21 Oct
 * 2026") and its words as paragraphs. Only text: no HTML, no markup, no links
 * made of what was written.
 */
export function buildCopy(
  messages: readonly CopyMessage[],
  formatDate: (iso: string) => string,
): BlockJson[] {
  const blocks: BlockJson[] = [];
  for (const message of messages) {
    const from = sanitiseText(message.from).replaceAll("\n", " ");
    blocks.push({
      type: "heading",
      attrs: { level: 3 },
      content: [textNode(`${from} — ${formatDate(message.sentAt)}`)],
    });
    const body = sanitiseText(message.text);
    if (body !== "") {
      for (const part of body.split(/\n{2,}/u)) {
        blocks.push(paragraph(part));
      }
    }
  }
  return blocks;
}
