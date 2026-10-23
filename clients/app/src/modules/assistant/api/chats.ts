import { useQuery, useQueryClient } from "@tanstack/react-query";

import {
  ApiError,
  api,
  canStream,
  readEventStream,
  unwrap,
  unwrapEmpty,
  type RecordRef,
  type Schema,
} from "~/lib/api";

export type ChatMessage = Schema<"ChatMessageView">;
export type ChatStep = Schema<"ChatStep">;
export type ChatTool = Schema<"ChatTool">;
type ChatView = Schema<"ChatView">;

/**
 * Which chat the panel shows: the member's chat about a record (started, or
 * continued), a new one about nothing (once per opening), or one chosen
 * from Recent, by its id.
 */
export type ChatSource =
  | { kind: "about"; about: RecordRef }
  | { kind: "nothing"; opening: number }
  | { kind: "chat"; id: string };

// Not operation keys: no event touches a chat, and starting one is a request
// of the member's, never refetched behind their back.
const chatKeys = {
  source: (workspaceId: string, source: ChatSource) =>
    source.kind === "about"
      ? (["scout-chat", workspaceId, "about", source.about.type, source.about.id] as const)
      : source.kind === "nothing"
        ? (["scout-chat", workspaceId, "nothing", source.opening] as const)
        : chatKeys.one(workspaceId, source.id),
  one: (workspaceId: string, chatId: string) =>
    ["scout-chat", workspaceId, "chat", chatId] as const,
  recent: (workspaceId: string) => ["scout-chat", workspaceId, "recent"] as const,
};

/**
 * The chat the panel shows, from `source`: `assistant.start_chat` about a
 * record (the member's chat about it, continued when they have one), or
 * about nothing (a new one, once per opening); `assistant.get_chat` for one
 * chosen from Recent. No model is called. The panel asks again after each
 * answer ({@link useChatCache}).
 */
export function useScoutChatOf(workspaceId: string, source: ChatSource) {
  return useQuery({
    queryKey: chatKeys.source(workspaceId, source),
    staleTime: Infinity,
    gcTime: 0,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    queryFn: async ({ signal }): Promise<ChatView> => {
      if (source.kind === "chat") {
        return unwrap(
          await api.GET("/api/v1/w/{workspace_id}/assistant/chats/{chat_id}", {
            params: { path: { workspace_id: workspaceId, chat_id: source.id } },
            signal,
          }),
        );
      }
      return unwrap(
        await api.POST("/api/v1/w/{workspace_id}/assistant/chats", {
          params: { path: { workspace_id: workspaceId } },
          body: source.kind === "about" ? { about: source.about } : {},
          signal,
        }),
      );
    },
  });
}

/** Keeping the panel's chat, and Recent, current as questions are asked. */
export function useChatCache(workspaceId: string, source: ChatSource) {
  const queryClient = useQueryClient();
  const key = chatKeys.source(workspaceId, source);
  return {
    /**
     * Ask for the chat again (after an answer is stored, or Start over). A
     * chat about nothing is asked for by its id: starting again would make
     * another.
     */
    refresh: async (chatId?: string) => {
      if (source.kind === "nothing" && chatId !== undefined) {
        const view = unwrap(
          await api.GET("/api/v1/w/{workspace_id}/assistant/chats/{chat_id}", {
            params: { path: { workspace_id: workspaceId, chat_id: chatId } },
          }),
        );
        queryClient.setQueryData(key, view);
      } else {
        await queryClient.invalidateQueries({ queryKey: key });
      }
      await queryClient.invalidateQueries({ queryKey: chatKeys.recent(workspaceId) });
    },
  };
}

/**
 * `assistant.list_chats`: the member's chats with a question, newest first,
 * by their first question. Only while Recent is open, and afresh each time.
 */
export function useRecentChats(workspaceId: string, enabled: boolean) {
  return useQuery({
    queryKey: chatKeys.recent(workspaceId),
    enabled,
    staleTime: 0,
    retry: false,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/assistant/chats", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).chats,
  });
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
interface AnswerHandlers {
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

const TOOLS: ReadonlySet<string> = new Set<ChatTool>([
  "find_conversations",
  "read_conversation",
  "conversation_history",
  "find_people",
  "what_needs_me",
  "my_inboxes",
  "my_tasks",
  "read_task",
]);

function isTool(value: unknown): value is ChatTool {
  return typeof value === "string" && TOOLS.has(value);
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
