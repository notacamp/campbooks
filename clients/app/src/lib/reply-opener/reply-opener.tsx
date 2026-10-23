import { createContext, useContext, type ReactNode } from "react";

/** A reply to open: the conversation, and what Scout proposed it should say. */
export interface ReplyToOpen {
  conversationId: string;
  /** Its stream, for where the composer opens when the member writes their own. */
  stream: string;
  /** Who is answered, as the card named them ("Marta Ferreira"). */
  name: string;
  /** What the reply should say, in Scout's words: the dialog drafts it. */
  intent: string;
}

/**
 * Opens a conversation's Reply dialog drafting `intent` (campbooks-cloud#305):
 * nothing is sent; the member edits the draft and sends it, or not.
 */
export type OpenReply = (reply: ReplyToOpen) => void;

const ReplyOpenerContext = createContext<OpenReply | undefined>(undefined);

interface ReplyOpenerProviderProps {
  /** What opening does: the app shows the conversations module's dialog. */
  open: OpenReply;
  children: ReactNode;
}

/**
 * Where the app provides the Reply dialog: a screen below it (Scout's
 * chat) can open it for a conversation without importing the module that
 * draws it.
 */
export function ReplyOpenerProvider({ open, children }: ReplyOpenerProviderProps) {
  return <ReplyOpenerContext value={open}>{children}</ReplyOpenerContext>;
}

/**
 * `openReply(reply)`, or `undefined` where no dialog is provided (tests,
 * stories): a screen then offers no reply to open.
 */
export function useOpenReply(): OpenReply | undefined {
  return useContext(ReplyOpenerContext);
}
