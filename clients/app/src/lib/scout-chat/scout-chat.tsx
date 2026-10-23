import { createContext, useContext, useEffect, type ReactNode } from "react";

import type { RecordRef } from "~/lib/api";

/** How the chat was opened. */
interface ScoutChatOptions {
  /** What opened it: the focus goes back there when the chat closes. */
  from?: HTMLElement | null | undefined;
  /** A first question, asked as soon as the chat is open (⌘K's "Ask Scout: <text>"). */
  ask?: string | undefined;
}

/**
 * Opens Scout's chat (campbooks-cloud#305) about a record of a type Scout
 * can read (a conversation, a rule, one of Scout's suggestions), or about
 * nothing in particular (`null`: always a new chat).
 */
export type OpenScoutChat = (about: RecordRef | null, options?: ScoutChatOptions) => void;

/** The record types a chat may be about, as the server registers them. */
export const SCOUT_CHAT_ABOUT = {
  conversation: "conversations.conversation",
  rule: "conversations.rule",
  ruleNotice: "conversations.rule_notice",
} as const;

/** A conversation, as a chat is about it. */
export function conversationRef(id: string): RecordRef {
  return { type: SCOUT_CHAT_ABOUT.conversation, id };
}

/** A rule, as a chat is about it. */
export function ruleRef(id: string): RecordRef {
  return { type: SCOUT_CHAT_ABOUT.rule, id };
}

/** One of Scout's noticed suggestions, as a chat is about it. */
export function ruleNoticeRef(id: string): RecordRef {
  return { type: SCOUT_CHAT_ABOUT.ruleNotice, id };
}

const ScoutChatContext = createContext<OpenScoutChat | undefined>(undefined);

/** Where a provider below says what opening does (the root's command menu listens). */
const HostContext = createContext<((open?: OpenScoutChat) => void) | undefined>(undefined);

interface ScoutChatProviderProps {
  /** What opening does: the app shows its one panel. */
  open: OpenScoutChat;
  children: ReactNode;
}

/**
 * Where the app mounts Scout's chat: screens below it can open the panel
 * without knowing the module that draws it. While mounted, it also tells
 * the {@link ScoutChatHost} above it, if any, how to open the panel.
 */
export function ScoutChatProvider({ open, children }: ScoutChatProviderProps) {
  const host = useContext(HostContext);
  useEffect(() => {
    if (host === undefined) {
      return;
    }
    host(open);
    return () => {
      host();
    };
  }, [host, open]);
  return <ScoutChatContext value={open}>{children}</ScoutChatContext>;
}

interface ScoutChatHostProps {
  /** The opener of the panel mounted below, or `undefined` while none is. Stable. */
  onOpener: (open?: OpenScoutChat) => void;
  children: ReactNode;
}

/**
 * Above the screens, where what sits beside them (the command menu, ⌘K)
 * learns how to open the panel a workspace mounted further down.
 */
export function ScoutChatHost({ onOpener, children }: ScoutChatHostProps) {
  return <HostContext value={onOpener}>{children}</HostContext>;
}

/**
 * `openScoutChat(about, { from, ask })`, or `undefined` where no panel is
 * mounted (tests, stories): a screen then offers nothing to open.
 */
export function useScoutChat(): OpenScoutChat | undefined {
  return useContext(ScoutChatContext);
}
