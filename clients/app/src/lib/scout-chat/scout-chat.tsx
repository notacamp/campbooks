import { createContext, useContext, type ReactNode } from "react";

/**
 * Opens Scout's chat about a conversation (campbooks-cloud#305). `from` is
 * what opened it: the focus goes back there when the chat closes.
 */
export type OpenScoutChat = (conversationId: string, from?: HTMLElement | null) => void;

const ScoutChatContext = createContext<OpenScoutChat | undefined>(undefined);

export interface ScoutChatProviderProps {
  /** What opening does: the app shows its one panel. */
  open: OpenScoutChat;
  children: ReactNode;
}

/**
 * Where the app mounts Scout's chat: screens below it can open the panel
 * for a conversation without knowing the module that draws it.
 */
export function ScoutChatProvider({ open, children }: ScoutChatProviderProps) {
  return <ScoutChatContext value={open}>{children}</ScoutChatContext>;
}

/**
 * `openScoutChat(conversationId, from)`, or `undefined` where no panel is
 * mounted (tests, stories): a screen then offers nothing to open.
 */
export function useScoutChat(): OpenScoutChat | undefined {
  return useContext(ScoutChatContext);
}
