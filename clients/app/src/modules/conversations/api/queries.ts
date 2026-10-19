import {
  infiniteQueryOptions,
  keepPreviousData,
  useInfiniteQuery,
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
  type InfiniteData,
  type QueryClient,
} from "@tanstack/react-query";

import { api, operationKey, unwrap, type OperationQuery, type Schema } from "~/lib/api";

export type Stream = Schema<"Stream">;
export type ConversationSummary = Schema<"ConversationSummary">;
export type ConversationDetail = Schema<"ConversationDetail">;
type ConversationList = Schema<"ConversationList">;
export type MessageView = Schema<"MessageView">;
export type DecisionView = Schema<"DecisionView">;
export type ReasonView = Schema<"ReasonView">;
export type NeedView = Schema<"NeedView">;
export type Sender = Schema<"SenderView">;

/** A member's verdict on what a conversation needs (`conversations.resolve_need`, …). */
export type NeedVerdict =
  "done" | "not_needed" | "confirm" | "reopen" | "already_paid" | "still_to_pay";

/** The built-in streams, in display order. */
export const STREAMS: readonly Stream[] = ["primary", "updates", "receipts", "newsletters"];

export function isStream(value: string): value is Stream {
  return (STREAMS as readonly string[]).includes(value);
}

/** A view beside the streams (`conversations.list` with `view`): never a stream. */
export type View = Exclude<NonNullable<OperationQuery<"conversations.list">["view"]>, "inbox">;

/** The views listed by `conversations.list`, in display order. */
export const VIEWS: readonly View[] = ["sent", "starred", "archive", "spam", "trash"];

/** Drafts: a place beside the views whose list is `conversations.list_drafts`, not conversations. */
export const DRAFTS = "drafts";
export type Drafts = typeof DRAFTS;

/** The places after the streams, in the order the sidebar and the chips show them. */
export const SIDE_PLACES: readonly (View | Drafts)[] = [
  "sent",
  "starred",
  DRAFTS,
  "archive",
  "spam",
  "trash",
];

export function isView(value: string): value is View {
  return (VIEWS as readonly string[]).includes(value);
}

/** A place that lists conversations: one of the streams (the inbox), or a view beside them. */
export type ConversationPlace = Stream | View;

/** Where the member is in the inbox: a list of conversations, or Drafts. */
export type Place = ConversationPlace | Drafts;

export function isPlace(value: string): value is Place {
  return isStream(value) || isView(value) || value === DRAFTS;
}

/** Whether `summary` is listed in `place`, as the server lists it. */
export function belongsIn(place: ConversationPlace, summary: ConversationSummary): boolean {
  switch (place) {
    case "starred": {
      return summary.starred && !summary.trashed;
    }
    case "sent": {
      return typeof summary.last_sent_at === "string" && !summary.trashed;
    }
    case "archive": {
      return summary.archived && !summary.trashed;
    }
    case "spam": {
      return summary.trashed && summary.spam;
    }
    case "trash": {
      return summary.trashed && !summary.spam;
    }
    default: {
      return place === summary.stream && !summary.archived && !summary.trashed;
    }
  }
}

// Keyed by operationId (`operationKey`), so the change feed refetches them
// when a conversation or message changes.
const keys = {
  inboxes: (workspaceId: string) =>
    operationKey("conversations.list_inboxes", { workspace_id: workspaceId }),
  streams: (workspaceId: string) =>
    operationKey("conversations.list_streams", { workspace_id: workspaceId }),
  list: (workspaceId: string, place: ConversationPlace) =>
    operationKey(
      "conversations.list",
      isView(place)
        ? { workspace_id: workspaceId, view: place }
        : { workspace_id: workspaceId, stream: place },
    ),
  /** Every list of the workspace, whatever the stream. */
  lists: (workspaceId: string) => operationKey("conversations.list", { workspace_id: workspaceId }),
  conversation: (workspaceId: string, conversationId: string) =>
    operationKey("conversations.get", {
      workspace_id: workspaceId,
      conversation_id: conversationId,
    }),
  senders: (workspaceId: string, q: string, reachable: boolean) =>
    operationKey("conversations.list_senders", {
      workspace_id: workspaceId,
      q,
      ...(reachable ? { reachable: "true" } : {}),
    }),
  body: (
    workspaceId: string,
    conversationId: string,
    messageId: string,
    showImages: boolean,
    spam: boolean,
  ) =>
    operationKey("conversations.get_message_body", {
      workspace_id: workspaceId,
      conversation_id: conversationId,
      message_id: messageId,
      show_images: String(showImages),
      // In spam the server holds images back: a body read before is not that one.
      ...(spam ? { spam: "true" } : {}),
    }),
  words: (workspaceId: string, conversationId: string, messageId: string) =>
    operationKey("conversations.get_message_body", {
      workspace_id: workspaceId,
      conversation_id: conversationId,
      message_id: messageId,
      words: "true",
    }),
};

/** Page size of a stream's list. */
const PAGE = 50;

/** `conversations.list_inboxes`. */
export function useInboxes(workspaceId: string) {
  return useQuery({
    queryKey: keys.inboxes(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/inboxes", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).inboxes,
  });
}

/** `conversations.list_streams`: each stream's conversations and unread ones. */
export function useStreams(workspaceId: string) {
  return useQuery({
    queryKey: keys.streams(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/streams", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).streams,
  });
}

const conversationsQuery = (workspaceId: string, place: ConversationPlace) =>
  infiniteQueryOptions({
    queryKey: keys.list(workspaceId, place),
    initialPageParam: undefined as string | undefined,
    queryFn: async ({ pageParam, signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/conversations", {
          params: {
            path: { workspace_id: workspaceId },
            query: {
              ...(isView(place) ? { view: place } : { stream: place }),
              limit: PAGE,
              ...(pageParam === undefined ? {} : { cursor: pageParam }),
            },
          },
          signal,
        }),
      ),
    getNextPageParam: (page) => page.next_cursor ?? undefined,
  });

/** `conversations.list` for one stream or view, a page at a time. */
export function useConversations(workspaceId: string, place: ConversationPlace) {
  return useInfiniteQuery(conversationsQuery(workspaceId, place));
}

/** `conversations.get`: the conversation, its messages (oldest first) and the decide answers. */
export function useConversation(workspaceId: string, conversationId: string | undefined) {
  return useQuery({
    queryKey: keys.conversation(workspaceId, conversationId ?? ""),
    enabled: conversationId !== undefined,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/conversations/{conversation_id}", {
          params: {
            path: { workspace_id: workspaceId, conversation_id: conversationId ?? "" },
          },
          signal,
        }),
      ),
  });
}

/**
 * `conversations.get_message_body`: sanitised HTML (and text), remote images
 * on request; `spam` while its conversation is in spam (asked for again then).
 */
export function useMessageBody(
  workspaceId: string,
  conversationId: string,
  messageId: string,
  showImages: boolean,
  enabled: boolean,
  spam = false,
) {
  return useQuery({
    queryKey: keys.body(workspaceId, conversationId, messageId, showImages, spam),
    enabled,
    // A body never changes once stored.
    staleTime: Number.POSITIVE_INFINITY,
    // "Show images" refetches it: keep the blocked version up meanwhile. Never
    // in spam: the version before may have had its images.
    ...(spam ? {} : { placeholderData: keepPreviousData }),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET(
          "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/body",
          {
            params: {
              path: {
                workspace_id: workspaceId,
                conversation_id: conversationId,
                message_id: messageId,
              },
              query: { show_images: showImages },
            },
            signal,
          },
        ),
      ),
  });
}

/** A message's words alone (`conversations.get_message_body` with `words`). */
function wordsQuery(workspaceId: string, conversationId: string, messageId: string) {
  return {
    queryKey: keys.words(workspaceId, conversationId, messageId),
    // A message's words never change once stored.
    staleTime: Number.POSITIVE_INFINITY,
    // As a message's text: its words, and whether they were cut.
    queryFn: async ({ signal }: { signal: AbortSignal }) => {
      const body = unwrap(
        await api.GET(
          "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/body",
          {
            params: {
              path: {
                workspace_id: workspaceId,
                conversation_id: conversationId,
                message_id: messageId,
              },
              query: { words: true },
            },
            signal,
          },
        ),
      );
      return { content: body.text ?? "", truncated: body.truncated };
    },
  };
}

/**
 * The words of a message `conversations.get` marked `text_unread` (its HTML
 * was not read in time), asked for on their own; disabled otherwise.
 */
export function useMessageWords(
  workspaceId: string,
  conversationId: string,
  message: MessageView | undefined,
) {
  return useQuery({
    ...wordsQuery(workspaceId, conversationId, message?.id ?? ""),
    enabled: message?.text_unread === true,
  });
}

/**
 * `message` with its text, for a reply's or a forward's quote: as it came,
 * or, when `conversations.get` marked it `text_unread`, with its words asked
 * for first. Rejects when they could not be read: a quote is never left
 * empty silently.
 */
export async function withWords(
  queryClient: QueryClient,
  workspaceId: string,
  conversationId: string,
  message: MessageView,
): Promise<MessageView> {
  if (message.text_unread !== true) {
    return message;
  }
  const text = await queryClient.query(wordsQuery(workspaceId, conversationId, message.id));
  return { ...message, text, text_unread: false };
}

/** The conversation-changing operations, by name. */
export type ConversationAction =
  | "archive"
  | "unarchive"
  | "read"
  | "unread"
  | "star"
  | "unstar"
  // Report and delete, and its undo (back where it was).
  | "spam"
  | "not_spam"
  | "trash"
  | "untrash"
  | { stream: Stream }
  | { need: NeedVerdict };

const PATHS = {
  archive: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/archive",
  unarchive: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/unarchive",
  read: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/read",
  unread: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/unread",
  star: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/star",
  unstar: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/unstar",
  spam: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/spam",
  not_spam: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/not-spam",
  trash: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/trash",
  untrash: "/api/v1/w/{workspace_id}/conversations/{conversation_id}/untrash",
} as const;

/** How an action changes a summary, for the optimistic update (and where it is listed after). */
export function applied(
  summary: ConversationSummary,
  action: ConversationAction,
): ConversationSummary {
  if (typeof action === "object") {
    if ("need" in action) {
      switch (action.need) {
        case "done":
        case "not_needed":
        case "already_paid": {
          return { ...summary, need: null };
        }
        case "confirm":
        case "still_to_pay": {
          return summary.need === null || summary.need === undefined
            ? summary
            : { ...summary, need: { ...summary.need, trust: "sure", basis: "member" } };
        }
        case "reopen": {
          return summary;
        }
      }
    }
    return { ...summary, stream: action.stream, stream_source: "member", suggestion: null };
  }
  switch (action) {
    case "archive": {
      return { ...summary, archived: true };
    }
    case "unarchive": {
      return { ...summary, archived: false };
    }
    case "read": {
      return { ...summary, unread_count: 0 };
    }
    case "unread": {
      return { ...summary, unread_count: Math.max(1, summary.unread_count) };
    }
    case "star": {
      return { ...summary, starred: true };
    }
    case "unstar": {
      return { ...summary, starred: false };
    }
    // In spam or the trash, it needs nothing. Spam takes every message out
    // of the inbox; the trash leaves that as it was, for Restore.
    case "spam": {
      return { ...summary, need: null, trashed: true, spam: true, archived: true };
    }
    case "trash": {
      return { ...summary, need: null, trashed: true, spam: false };
    }
    // Not spam puts received mail back in the inbox; the server's summary
    // says the rest.
    case "not_spam": {
      return { ...summary, trashed: false, spam: false, archived: false };
    }
    case "untrash": {
      return { ...summary, trashed: false, spam: false };
    }
  }
}

type ListData = InfiniteData<ConversationList, string | undefined>;

/**
 * The need operations: Done, Not needed, Yes to a question, a bill's
 * Already paid and Still to pay, and their undo.
 * Each names the message the need the member saw comes from (`seen`), so the
 * server refuses (409 `need_changed`) when the need changed meanwhile.
 */
async function needRequest(
  params: { path: { workspace_id: string; conversation_id: string } },
  verdict: NeedVerdict,
  seen: string,
): Promise<ConversationSummary> {
  switch (verdict) {
    case "done":
    case "not_needed":
    case "already_paid":
    case "still_to_pay": {
      return unwrap(
        await api.POST("/api/v1/w/{workspace_id}/conversations/{conversation_id}/need/resolve", {
          params,
          body: { outcome: verdict, message_id: seen },
        }),
      );
    }
    case "confirm": {
      return unwrap(
        await api.POST("/api/v1/w/{workspace_id}/conversations/{conversation_id}/need/confirm", {
          params,
          body: { message_id: seen },
        }),
      );
    }
    case "reopen": {
      return unwrap(
        await api.POST("/api/v1/w/{workspace_id}/conversations/{conversation_id}/need/reopen", {
          params,
        }),
      );
    }
  }
}

/**
 * `conversations.list` with `ids`: these conversations (those you can see),
 * whatever their stream, with what each needs. For screens that show
 * another module's references to conversations (Home).
 */
export function useConversationsByIds(workspaceId: string, ids: readonly string[]) {
  const sorted = [...ids].toSorted();
  return useQuery({
    queryKey: operationKey("conversations.list", {
      workspace_id: workspaceId,
      ids: sorted.join(","),
    }),
    enabled: sorted.length > 0,
    placeholderData: keepPreviousData,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/conversations", {
          params: { path: { workspace_id: workspaceId }, query: { ids: sorted.join(",") } },
          signal,
        }),
      ).conversations,
  });
}

/** The longest `ids` list `conversations.list` takes. */
const IDS_MAX = 100;

/**
 * Like {@link useConversationsByIds} for any number of ids (asked in batches
 * of 100): a map of the summaries you can see, by id. For screens that show
 * another module's hits and act on them (search results).
 */
export function useConversationSummaries(
  workspaceId: string,
  ids: readonly string[],
): ReadonlyMap<string, ConversationSummary> {
  const sorted = [...new Set(ids)].toSorted();
  const batches: string[][] = [];
  for (let start = 0; start < sorted.length; start += IDS_MAX) {
    batches.push(sorted.slice(start, start + IDS_MAX));
  }
  return useQueries({
    queries: batches.map((batch) => ({
      queryKey: operationKey("conversations.list", {
        workspace_id: workspaceId,
        ids: batch.join(","),
      }),
      placeholderData: keepPreviousData,
      queryFn: async ({ signal }: { signal: AbortSignal }) =>
        unwrap(
          await api.GET("/api/v1/w/{workspace_id}/conversations", {
            params: { path: { workspace_id: workspaceId }, query: { ids: batch.join(",") } },
            signal,
          }),
        ).conversations,
    })),
    combine: (results) =>
      new Map(
        results.flatMap((result) => result.data ?? []).map((summary) => [summary.id, summary]),
      ),
  });
}

/**
 * Put `next` in every cached list it belongs to (its stream while in the
 * inbox, and the views it is in), and out of the others. Optimistic: the
 * lists it may enter are refetched when the action settles, and the server's
 * answer and the change feed settle the rest.
 */
function placeInLists(queryClient: QueryClient, workspaceId: string, next: ConversationSummary) {
  for (const place of [...STREAMS, ...VIEWS]) {
    queryClient.setQueryData<ListData>(keys.list(workspaceId, place), (data) => {
      if (data === undefined) {
        return data;
      }
      const belongs = belongsIn(place, next);
      const present = data.pages.some((page) =>
        page.conversations.some((conversation) => conversation.id === next.id),
      );
      return {
        ...data,
        pages: data.pages.map((page) => ({
          ...page,
          conversations:
            belongs && present
              ? page.conversations.map((conversation) =>
                  conversation.id === next.id ? next : conversation,
                )
              : page.conversations.filter((conversation) => conversation.id !== next.id),
        })),
      };
    });
  }
}

/**
 * The conversation actions (`conversations.archive`, `.mark_read`, `.star`,
 * `.move_to_stream`, `.trash`, `.report_spam` and their undo pairs), applied
 * to the cache at once and reconciled with the server's summary.
 */
export function useConversationAction(workspaceId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      conversation,
      action,
    }: {
      conversation: ConversationSummary;
      action: ConversationAction;
    }) => {
      const params = { path: { workspace_id: workspaceId, conversation_id: conversation.id } };
      if (typeof action === "object" && "need" in action) {
        return needRequest(params, action.need, conversation.need?.message_id ?? "");
      }
      if (typeof action === "object") {
        return unwrap(
          await api.POST("/api/v1/w/{workspace_id}/conversations/{conversation_id}/stream", {
            params,
            body: { stream: action.stream },
          }),
        );
      }
      return unwrap(await api.POST(PATHS[action], { params }));
    },
    onMutate: ({ conversation, action }) => {
      const next = applied(conversation, action);
      placeInLists(queryClient, workspaceId, next);
      // Lists by ids (Home's rows) are plain arrays under the same key.
      queryClient.setQueriesData<unknown>({ queryKey: keys.lists(workspaceId) }, (data: unknown) =>
        Array.isArray(data)
          ? (data as ConversationSummary[]).map((summary) =>
              summary.id === next.id ? next : summary,
            )
          : data,
      );
      queryClient.setQueryData<ConversationDetail>(
        keys.conversation(workspaceId, conversation.id),
        (detail) => (detail === undefined ? detail : { ...detail, conversation: next }),
      );
    },
    onSuccess: (summary) => {
      queryClient.setQueryData<ConversationDetail>(
        keys.conversation(workspaceId, summary.id),
        (detail) => (detail === undefined ? detail : { ...detail, conversation: summary }),
      );
    },
    onSettled: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: keys.lists(workspaceId) }),
        queryClient.invalidateQueries({ queryKey: keys.streams(workspaceId) }),
        queryClient.invalidateQueries({ queryKey: keys.inboxes(workspaceId) }),
      ]);
    },
  });
}

/**
 * `conversations.list_senders`: the people the caller replies to most, or
 * those matching `q` by name or address (from two characters). With
 * `reachable`, only those it is safe to write to (Forward's picker): the
 * server leaves out no-reply addresses, senders whose mail fails their
 * checks or pretends to be them, and senders reported, under review or found
 * suspicious.
 */
export function useSenders(
  workspaceId: string,
  q = "",
  { enabled = true, reachable = false }: { enabled?: boolean; reachable?: boolean } = {},
) {
  const query = q.trim();
  return useQuery({
    queryKey: keys.senders(workspaceId, query, reachable),
    enabled: enabled && (query.length === 0 || query.length >= 2),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/senders", {
          params: {
            path: { workspace_id: workspaceId },
            query: {
              ...(query.length === 0 ? {} : { q: query }),
              ...(reachable ? { reachable: true } : {}),
            },
          },
          signal,
        }),
      ).senders,
  });
}
