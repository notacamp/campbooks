import { useQuery } from "@tanstack/react-query";

import { ApiError, api, canStream, readEventStream, unwrap, type Schema } from "~/lib/api";

type WriteReply = Schema<"WriteReply">;
type ReplyText = Schema<"ReplyText">;

// Not an operation key: the change feed never refetches these, since each
// asks the model again. They live for the session, one per message.
const replyKeys = {
  suggestions: (workspaceId: string, conversationId: string, messageId: string) =>
    ["reply-suggestions", workspaceId, conversationId, messageId] as const,
};

/**
 * `conversations.suggest_replies`: Scout's likely replies to `messageId`,
 * asked once per message for the session (a model call each time it is
 * asked), never retried on its own.
 */
export function useReplySuggestions(
  workspaceId: string,
  conversationId: string | undefined,
  messageId: string | undefined,
  { enabled = true }: { enabled?: boolean } = {},
) {
  return useQuery({
    queryKey: replyKeys.suggestions(workspaceId, conversationId ?? "", messageId ?? ""),
    enabled: enabled && conversationId !== undefined && messageId !== undefined,
    staleTime: Infinity,
    gcTime: 30 * 60 * 1000,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.POST(
          "/api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/suggestions",
          {
            params: {
              path: { workspace_id: workspaceId, conversation_id: conversationId ?? "" },
            },
            body: { message_id: messageId ?? null },
            signal,
          },
        ),
      ),
  });
}

/**
 * `conversations.write_reply`: Scout's reply for an intent, or the draft
 * rewritten (shorter, or as `change` asks). Plain text; nothing is saved.
 */
async function writeReply(
  workspaceId: string,
  conversationId: string,
  body: WriteReply,
  signal?: AbortSignal,
): Promise<string> {
  return unwrap(
    await api.POST("/api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/draft", {
      params: { path: { workspace_id: workspaceId, conversation_id: conversationId } },
      body,
      ...(signal === undefined ? {} : { signal }),
    }),
  ).text;
}

/** What a streamed reply calls back with as it is written. */
interface StreamHandlers {
  /** Stops the request (a new ask, Back, the dialog closed). */
  signal: AbortSignal;
  /** More of Scout's thinking. */
  onThinking: (delta: string) => void;
  /** More of the reply's words. */
  onText: (delta: string) => void;
}

/** The `delta` of a `thinking` or `text` frame, or nothing. */
function deltaOf(data: string): string {
  try {
    const frame: unknown = JSON.parse(data);
    if (typeof frame === "object" && frame !== null && "delta" in frame) {
      return typeof frame.delta === "string" ? frame.delta : "";
    }
  } catch {
    // Not a frame we know: nothing to show.
  }
  return "";
}

/**
 * `conversations.stream_reply`: Scout's reply as it is written, its thinking
 * and words handed to `handlers` as they arrive (campbooks-cloud#288); the
 * whole reply, as `conversations.write_reply` answers it, once done. Where
 * answers cannot stream (the desktop app's bridge) it asks `write_reply`
 * and shows nothing until the reply is whole. A refusal, or an `error`
 * frame, throws an {@link ApiError} with the problem; a stream that ends
 * without `done` throws one with `reply_failed`.
 */
export async function streamReply(
  workspaceId: string,
  conversationId: string,
  body: WriteReply,
  handlers: StreamHandlers,
): Promise<string> {
  if (!canStream) {
    return writeReply(workspaceId, conversationId, body, handlers.signal);
  }
  const stream = unwrap(
    await api.POST("/api/v1/w/{workspace_id}/conversations/{conversation_id}/reply/draft/stream", {
      params: { path: { workspace_id: workspaceId, conversation_id: conversationId } },
      body,
      signal: handlers.signal,
      parseAs: "stream",
    }),
  );
  if (stream === null) {
    throw new ApiError(503, { code: "reply_failed" });
  }
  for await (const frame of readEventStream(stream)) {
    switch (frame.event) {
      case "thinking": {
        handlers.onThinking(deltaOf(frame.data));
        break;
      }
      case "text": {
        handlers.onText(deltaOf(frame.data));
        break;
      }
      case "done": {
        const done: unknown = JSON.parse(frame.data);
        return (done as ReplyText).text;
      }
      case "error": {
        const problem: unknown = JSON.parse(frame.data);
        const status =
          typeof problem === "object" &&
          problem !== null &&
          "status" in problem &&
          typeof problem.status === "number"
            ? problem.status
            : 503;
        throw new ApiError(status, problem);
      }
      default: {
        break;
      }
    }
  }
  throw new ApiError(503, { code: "reply_failed" });
}
