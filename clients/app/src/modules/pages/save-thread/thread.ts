import type { CopyMessage } from "./copy";

/** A thread as Save to page needs it: what the reader already holds. */
export interface SaveThread {
  /** The conversation. */
  id: string;
  subject: string;
  /** Oldest first; `text` is each message's own words, quoted history left out. */
  messages: SaveThreadMessage[];
}

interface SaveThreadMessage extends CopyMessage {
  id: string;
  /** Files attached to it (not the pictures its body shows). */
  attachments: { id: string; name: string; size: number }[];
}
