import { ApiError, api, unwrap, type Schema } from "~/lib/api";

export type Proposal = Schema<"ChatProposal">;
export type ProposalRequest = Schema<"ProposalRequest">;
export type ProposalAction = Schema<"ProposalAction">;
export type Settlement = Schema<"Settlement">;

const CONVERSATION = "conversations.conversation";
const ITEM = "attention.item";

/**
 * A request this client runs for one of Scout's proposals: its method, its
 * path under the workspace's API root (`{name}` one plain id), and the
 * record types it changes (whose lists are read again after it).
 */
export interface Runnable {
  method: "POST" | "DELETE";
  path: string;
  records: readonly string[];
}

const conversation = (path: string, method: Runnable["method"] = "POST"): Runnable => ({
  method,
  path: `conversations/{conversation_id}/${path}`,
  records: [CONVERSATION],
});
const item = (path: string): Runnable => ({
  method: "POST",
  path: `attention/{item_id}/${path}`,
  records: [ITEM, CONVERSATION],
});

/**
 * The client's own list of the actions Scout may propose, and their undos
 * (the same operations as the server's action table): a proposal naming
 * anything else is never sent. Calendar events and tasks add theirs here.
 */
export const RUNNABLE: Readonly<Record<string, Runnable>> = {
  "conversations.archive": conversation("archive"),
  "conversations.unarchive": conversation("unarchive"),
  "conversations.mark_read": conversation("read"),
  "conversations.mark_unread": conversation("unread"),
  "conversations.star": conversation("star"),
  "conversations.unstar": conversation("unstar"),
  "conversations.add_label": conversation("labels"),
  "conversations.remove_label": conversation("labels/{label_id}", "DELETE"),
  "conversations.move_to_stream": conversation("stream"),
  "conversations.trash": conversation("trash"),
  "conversations.untrash": conversation("untrash"),
  "conversations.report_spam": conversation("spam"),
  "conversations.not_spam": conversation("not-spam"),
  "conversations.resolve_need": conversation("need/resolve"),
  "conversations.confirm_need": conversation("need/confirm"),
  "conversations.reopen_need": conversation("need/reopen"),
  "attention.snooze": item("snooze"),
  "attention.unsnooze": item("unsnooze"),
};

/** One plain id in a path. */
const ID = /^[\w-]{1,64}$/u;

/** A proposal's request this client may send: its entry and its path's ids. */
export interface Ready {
  entry: Runnable;
  params: Record<string, string>;
}

/**
 * `request` when this client runs it: its operation is in `list`, its
 * method POST or DELETE and the one its entry says, and its path is the
 * entry's, under the workspace's root, each id one plain segment. Anything
 * else is `undefined`: the card says Scout can't do it here.
 */
export function runnable(
  request: ProposalRequest,
  list: Readonly<Record<string, Runnable>> = RUNNABLE,
): Ready | undefined {
  const entry = Object.hasOwn(list, request.operation) ? list[request.operation] : undefined;
  // Whatever the server's document says a method may be: only these two.
  const method: string = request.method;
  if (
    entry === undefined ||
    (method !== "POST" && method !== "DELETE") ||
    method !== entry.method
  ) {
    return undefined;
  }
  const want = entry.path.split("/");
  const got = request.path.split("/");
  if (want.length !== got.length) {
    return undefined;
  }
  const params: Record<string, string> = {};
  for (const [index, segment] of want.entries()) {
    const value = got[index] ?? "";
    const name = /^\{(\w+)\}$/u.exec(segment)?.[1];
    if (name === undefined) {
      if (value !== segment) {
        return undefined;
      }
    } else if (ID.test(value)) {
      params[name] = value;
    } else {
      return undefined;
    }
  }
  return { entry, params };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * An undo's body: its own, with exactly the top-level fields its
 * `from_response` names copied from the change's answer, and nothing else.
 */
export function undoBody(undo: ProposalRequest, answer: unknown): unknown {
  const fields = undo.from_response ?? [];
  if (fields.length === 0) {
    return undo.body ?? null;
  }
  const body: Record<string, unknown> = isRecord(undo.body) ? { ...undo.body } : {};
  if (isRecord(answer)) {
    for (const field of fields) {
      if (Object.hasOwn(answer, field)) {
        body[field] = answer[field];
      }
    }
  }
  return body;
}

/** A request this client may not send. */
export class NotRunnable extends Error {
  constructor() {
    super("Not one of the actions this client runs for Scout");
    this.name = "NotRunnable";
  }
}

interface LooseInit {
  params: { path: Record<string, string> };
  body?: unknown;
  headers: Record<string, string>;
}

type LooseRequest = (
  method: "post" | "delete",
  url: string,
  init: LooseInit,
) => Promise<{ data?: unknown; error?: unknown; response: Response }>;

/**
 * Send `request` through the app's API client with the member's own
 * session, `key` as its `Idempotency-Key` (a second press acts once), and
 * `body`. Its answer, or an {@link ApiError}; {@link NotRunnable} when it
 * is not one of `list`'s.
 */
export async function runRequest(
  workspaceId: string,
  request: ProposalRequest,
  key: string,
  body: unknown,
  list: Readonly<Record<string, Runnable>> = RUNNABLE,
): Promise<unknown> {
  const ready = runnable(request, list);
  if (ready === undefined) {
    throw new NotRunnable();
  }
  // The path is the entry's own template, its ids checked above: never a
  // URL the proposal wrote.
  const send = api.request as unknown as LooseRequest;
  const result = await send(
    ready.entry.method === "POST" ? "post" : "delete",
    `/api/v1/w/{workspace_id}/${ready.entry.path}`,
    {
      params: { path: { ...ready.params, workspace_id: workspaceId } },
      ...(body === null || body === undefined ? {} : { body }),
      headers: { "Idempotency-Key": key },
    },
  );
  if (!result.response.ok) {
    throw new ApiError(result.response.status, result.error);
  }
  return result.data;
}

/**
 * `assistant.settle_proposal`: what the member did with a card, for Scout's
 * next answers. Records only.
 */
export async function settleProposal(
  workspaceId: string,
  chatId: string,
  proposalId: string,
  state: Settlement,
): Promise<Proposal> {
  return unwrap(
    await api.POST("/api/v1/w/{workspace_id}/assistant/chats/{chat_id}/proposals/{proposal_id}", {
      params: {
        path: { workspace_id: workspaceId, chat_id: chatId, proposal_id: proposalId },
      },
      body: { state },
    }),
  );
}

/** The API's own reason a request was refused, when it gave one. */
export function refusalReason(error: unknown): string | undefined {
  if (!(error instanceof ApiError) || !isRecord(error.problem)) {
    return undefined;
  }
  const { detail, title } = error.problem;
  if (typeof detail === "string" && detail.trim() !== "") {
    return detail;
  }
  return typeof title === "string" && title.trim() !== "" ? title : undefined;
}
