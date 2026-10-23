import { useQuery, useQueryClient } from "@tanstack/react-query";

import {
  ApiError,
  api,
  canStream,
  readEventStream,
  unwrap,
  unwrapEmpty,
  type Schema,
} from "~/lib/api";

export type ChatView = Schema<"ChatView">;
export type ChatMessage = Schema<"ChatMessageView">;
export type ChatStep = Schema<"ChatStep">;
export type ChatTool = Schema<"ChatTool">;

/** The record type a chat about a conversation names. */
const CONVERSATION = "conversations.conversation";

// Not an operation key: no event touches a chat, and starting one is a
// request of the member's, never refetched behind their back.
const chatKeys = {
  about: (workspaceId: string, conversationId: string) =>
    ["scout-chat", workspaceId, conversationId] as const,
};

/**
 * `assistant.start_chat` for a conversation: the member's chat about it,
 * continued when they have one (no model is called). Once per opening; the
 * panel puts each answer in it as it comes.
 */
export function useChatAbout(workspaceId: string, conversationId: string) {
  return useQuery({
    queryKey: chatKeys.about(workspaceId, conversationId),
    staleTime: Infinity,
    gcTime: 0,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.POST("/api/v1/w/{workspace_id}/assistant/chats", {
          params: { path: { workspace_id: workspaceId } },
          body: { about: { type: CONVERSATION, id: conversationId } },
          signal,
        }),
      ),
  });
}

/** Keeping the panel's chat current as questions are asked. */
export function useChatCache(workspaceId: string, conversationId: string) {
  const queryClient = useQueryClient();
  const key = chatKeys.about(workspaceId, conversationId);
  return {
    /** Ask for the chat again (after an answer is stored, or Start over). */
    refresh: () => queryClient.invalidateQueries({ queryKey: key }),
  };
}

/** `assistant.delete_chat`: Start over. There is no undo. */
export async function deleteChat(workspaceId: string, chatId: string): Promise<void> {
  unwrapEmpty(
    await api.DELETE("/api/v1/w/{workspace_id}/assistant/chats/{chat_id}", {
      params: { path: { workspace_id: workspaceId, chat_id: chatId } },
    }),
  );
}

/** What a streamed answer calls back with as it is written. */
export interface AnswerHandlers {
  /** Stops the request (the panel closed). */
  signal: AbortSignal;
  /** More of Scout's thinking. */
  onThinking: (delta: string) => void;
  /** A tool starting. */
  onStep: (tool: ChatTool) => void;
  /** More of the answer's words; words before a step are not the answer. */
  onText: (delta: string) => void;
}

/** A frame's JSON, or nothing. */
function frameOf(data: string): Record<string, unknown> {
  try {
    const frame: unknown = JSON.parse(data);
    if (typeof frame === "object" && frame !== null) {
      return frame as Record<string, unknown>;
    }
  } catch {
    // Not a frame we know: nothing to show.
  }
  return {};
}

const TOOLS: readonly string[] = [
  "find_conversations",
  "read_conversation",
  "conversation_history",
  "find_people",
  "what_needs_me",
  "my_inboxes",
];

function isTool(value: unknown): value is ChatTool {
  return typeof value === "string" && TOOLS.includes(value);
}

/** The member's IANA time zone, for "today" and "Thursday afternoon". */
function timeZone(): string | null {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone;
  } catch {
    return null;
  }
}

/**
 * Ask Scout in a chat: `assistant.stream_chat`, its thinking, steps and
 * words handed to `handlers` as they arrive, then the stored answer. Where
 * answers cannot stream (the desktop app's bridge), `assistant.ask_chat`,
 * whole. A refusal, or an `error` frame, throws an {@link ApiError} with the
 * problem; a stream that ends without `done` throws one with `chat_failed`.
 */
export async function askScout(
  workspaceId: string,
  chatId: string,
  text: string,
  handlers: AnswerHandlers,
): Promise<ChatMessage> {
  const body = { text, time_zone: timeZone() };
  const path = { workspace_id: workspaceId, chat_id: chatId };
  if (!canStream) {
    return unwrap(
      await api.POST("/api/v1/w/{workspace_id}/assistant/chats/{chat_id}/questions", {
        params: { path },
        body,
        signal: handlers.signal,
      }),
    );
  }
  const stream = unwrap(
    await api.POST("/api/v1/w/{workspace_id}/assistant/chats/{chat_id}/questions/stream", {
      params: { path },
      body,
      signal: handlers.signal,
      parseAs: "stream",
    }),
  );
  if (stream === null) {
    throw new ApiError(503, { code: "chat_failed" });
  }
  for await (const frame of readEventStream(stream)) {
    const data = frameOf(frame.data);
    switch (frame.event) {
      case "thinking": {
        handlers.onThinking(typeof data["delta"] === "string" ? data["delta"] : "");
        break;
      }
      case "step": {
        if (isTool(data["tool"])) {
          handlers.onStep(data["tool"]);
        }
        break;
      }
      case "text": {
        handlers.onText(typeof data["delta"] === "string" ? data["delta"] : "");
        break;
      }
      case "done": {
        return data as unknown as ChatMessage;
      }
      case "error": {
        const status = typeof data["status"] === "number" ? data["status"] : 503;
        throw new ApiError(status, data);
      }
      default: {
        break;
      }
    }
  }
  throw new ApiError(503, { code: "chat_failed" });
}
