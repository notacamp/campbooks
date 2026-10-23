import type { Query, QueryClient } from "@tanstack/react-query";

import { isTauri } from "~/lib/platform";

import { recordReaders } from "./records.gen";
import type { operations } from "./schema.gen";

/**
 * The change feed (ADR 0011): the server streams, per workspace, which
 * records changed (ids only, never contents), and the app's query cache
 * refetches what it shows.
 *
 * - {@link ChangeSource} is the seam between the app and a transport. The web
 *   uses `EventSource` on the BFF ({@link createWebChangeSource}). A native
 *   source will call a Rust command that holds the stream with the keychain
 *   token and emits webview events with the same frames (not built yet).
 * - {@link subscribeToChanges} keeps the last cursor, resumes from it after
 *   a disconnect or a hidden tab, and applies frames to the query cache. A
 *   cursor is an opaque resume token, new on every frame: keep the latest,
 *   never parse or compare one.
 * - Query keys follow one convention, {@link operationKey}:
 *   `[operationId, params]`. A change to a record invalidates the queries of
 *   the read operations that return its type (`records.gen.ts`, generated
 *   from the OpenAPI document).
 */

/** A record of any module: `<module>.<record>` and its id. */
export interface RecordRef {
  type: string;
  id: string;
}

/** One frame of the stream, as the server sends it. */
export type ChangeFrame =
  | { kind: "ready"; cursor: string }
  | { kind: "change"; cursor: string; record: RecordRef; op: "upsert" | "delete"; version: string }
  | { kind: "heartbeat"; cursor: string }
  | { kind: "reset"; cursor: string; reason: string }
  | { kind: "revoked" }
  | { kind: "closing"; reason: string };

export interface ChangeSourceHandlers {
  onFrame: (frame: ChangeFrame) => void;
  /**
   * The stream stopped for good (refused, or the connection failed in a way
   * the source does not retry itself). The subscription reopens it later
   * from its cursor.
   */
  onClosed: () => void;
}

/** Opens one workspace's stream after `after` (the start when undefined). */
export interface ChangeSource {
  open: (
    workspace: string,
    after: string | undefined,
    handlers: ChangeSourceHandlers,
  ) => () => void;
}

/** Frame names the server sends with an SSE `event:` line. */
const FRAME_KINDS = ["ready", "change", "heartbeat", "reset", "revoked", "closing"] as const;

/** Parse an SSE frame's `data`; `undefined` when it is not a frame we know. */
export function parseFrame(kind: string, data: string): ChangeFrame | undefined {
  if (!(FRAME_KINDS as readonly string[]).includes(kind)) {
    return undefined;
  }
  try {
    const body: unknown = JSON.parse(data);
    if (typeof body !== "object" || body === null) {
      return undefined;
    }
    return { ...body, kind } as ChangeFrame;
  } catch {
    return undefined;
  }
}

type EventSourceFactory = (url: string) => EventSource;

/**
 * The web source: `EventSource` on `/bff/api/v1/w/{workspace}/changes`. The
 * session cookie goes along (same origin) and a GET needs no CSRF header.
 * `EventSource` reconnects on its own after a dropped connection or a
 * `closing` frame, sending `Last-Event-ID`; it gives up only when refused
 * (not a member, signed out), which is reported as closed.
 */
export function createWebChangeSource(
  origin: string = globalThis.location.origin,
  connect: EventSourceFactory = (url) => new EventSource(url),
): ChangeSource {
  return {
    open(workspace, after, handlers) {
      const url = new URL(`/bff/api/v1/w/${encodeURIComponent(workspace)}/changes`, origin);
      if (after !== undefined) {
        url.searchParams.set("after", after);
      }
      const events = connect(url.href);
      const listeners = FRAME_KINDS.map((kind) => {
        const listener = (event: MessageEvent<string>) => {
          const frame = parseFrame(kind, event.data);
          if (frame !== undefined) {
            handlers.onFrame(frame);
          }
        };
        events.addEventListener(kind, listener);
        return [kind, listener] as const;
      });
      const onError = () => {
        if (events.readyState === EventSource.CLOSED) {
          handlers.onClosed();
        }
      };
      events.addEventListener("error", onError);
      return () => {
        for (const [kind, listener] of listeners) {
          events.removeEventListener(kind, listener);
        }
        events.removeEventListener("error", onError);
        events.close();
      };
    },
  };
}

type OperationId = keyof operations;

/** The query key of an operation's query: `[operationId, params]`. */
export function operationKey(
  id: OperationId,
  params: Readonly<Record<string, string>> = {},
): readonly [OperationId, Readonly<Record<string, string>>] {
  return [id, params];
}

function keyParams(query: Query): Readonly<Record<string, unknown>> {
  const params = query.queryKey[1];
  return typeof params === "object" && params !== null ? (params as Record<string, unknown>) : {};
}

/** Whether `query` belongs to `workspace` (or to no workspace in particular). */
function inWorkspace(query: Query, workspace: string): boolean {
  const owner = keyParams(query)["workspace_id"];
  return owner === undefined || owner === workspace;
}

function readsAny(query: Query, operationIds: ReadonlySet<string>): boolean {
  const id = query.queryKey[0];
  return typeof id === "string" && operationIds.has(id);
}

/**
 * Whether a message body's remote images depend on the rules: a rule trusts
 * its sender (`images_trusted`), keeps them back (`images_held`), or could
 * (`images_offer`). The bodies a rule's change can change (campbooks-cloud#236).
 */
export function holdsImageState(data: unknown): boolean {
  if (typeof data !== "object" || data === null) {
    return false;
  }
  const body = data as { images_trusted?: unknown; images_held?: unknown; images_offer?: unknown };
  return (
    body.images_trusted === true ||
    (body.images_held !== undefined && body.images_held !== null) ||
    body.images_offer === true
  );
}

/**
 * Whether a message body's remote images show because a rule trusts its
 * sender: the bodies a rule's change must drop, not merely mark stale
 * (campbooks-cloud#236, {@link refreshBodiesAfterRules}).
 */
export function trustsImages(data: unknown): boolean {
  return (
    typeof data === "object" &&
    data !== null &&
    (data as { images_trusted?: unknown }).images_trusted === true
  );
}

const RULE_RECORD = "conversations.rule";
const BODY_READER = "conversations.get_message_body";

/** Whether `query` reads a message body of `workspace`. */
function readsBodyOf(query: Query, workspace: string): boolean {
  return query.queryKey[0] === BODY_READER && keyParams(query)["workspace_id"] === workspace;
}

/**
 * Drop the cached bodies of `workspace` that a rule trusted: each goes back
 * to having no data, so no reader can show its HTML, remote images and all,
 * from the cache before a fresh read says whether the rule still trusts the
 * sender. An open one shows its skeleton while it is read again; any other
 * is read when it is next opened. A first read still on its way is dropped
 * too (resetting cancels it): it was asked before the change and may answer
 * with a trust that is gone, and a plain re-read would only wait for it.
 */
function dropTrustedBodies(queryClient: QueryClient, workspace: string): Promise<void> {
  return queryClient.resetQueries({
    predicate: (query) =>
      readsBodyOf(query, workspace) &&
      (trustsImages(query.state.data) ||
        (query.state.data === undefined && query.state.fetchStatus === "fetching")),
  });
}

/**
 * After rules changed, here or anywhere else (campbooks-cloud#236): the
 * message bodies a rule trusted are dropped ({@link trustsImages}): marked
 * stale only, a folded message or a conversation opened again would show
 * the cached body, its remote images loading and telling a sender who is no
 * longer trusted that the mail was opened, before the re-read answered. The
 * other cached bodies `only` keeps are re-read in the background, showing
 * what they showed meanwhile (their images held back, or none).
 */
export async function refreshBodiesAfterRules(
  queryClient: QueryClient,
  workspace: string,
  only: (data: unknown) => boolean = () => true,
): Promise<void> {
  // Dropped first (synchronously), so the re-read below skips them.
  const dropped = dropTrustedBodies(queryClient, workspace);
  await Promise.all([
    dropped,
    queryClient.invalidateQueries({
      predicate: (query) =>
        readsBodyOf(query, workspace) && query.state.data !== undefined && only(query.state.data),
    }),
  ]);
}

/**
 * Refresh what reads records of `types` in `workspace`: the queries of the
 * read operations that return them (`records.gen.ts`), as a change frame
 * for them would. For a change this client made itself, so what shows it
 * need not wait for the feed (or follows it where there is none).
 */
export function refreshRecords(
  queryClient: QueryClient,
  workspace: string,
  types: readonly string[],
): Promise<void> {
  const readers = new Set(types.flatMap((type) => recordReaders[type] ?? []));
  return queryClient.invalidateQueries({
    predicate: (query) => readsAny(query, readers) && inWorkspace(query, workspace),
  });
}

/**
 * Whether this client follows its workspaces' change feeds live: the web,
 * through `EventSource`. The desktop and mobile shells (Tauri) have none yet,
 * so what must follow other devices is read again when shown, on a timer or
 * on focus there (a message body: `body-freshness.ts` in conversations).
 *
 * TODO(native change source): a Tauri change source (`ChangeSource` over the
 * shell's own streaming HTTP client, with the session's token) so desktop and
 * mobile follow changes live too; then drop the timers keyed on this.
 */
export function followsChangesLive(): boolean {
  return !isTauri() && typeof EventSource !== "undefined";
}

/** How long changes are gathered before the cache is touched. */
const BATCH_MS = 50;

/**
 * Applies frames of one workspace to a query cache: invalidates the queries
 * of the operations that read each changed record type (batched), drops
 * cached details of deleted records, and invalidates the whole workspace on
 * `reset`.
 */
export function createCacheBinding(queryClient: QueryClient, workspace: string) {
  let pending = new Set<string>();
  let deleted: RecordRef[] = [];
  let timer: ReturnType<typeof setTimeout> | undefined;

  const flush = () => {
    timer = undefined;
    // Every reader of a changed type, except the message bodies on a rule's
    // change: those are dropped or re-read by what their images depend on
    // (`refreshBodiesAfterRules`); a message's own change still re-reads
    // every body of it.
    const readers = new Set<string>();
    let rules = false;
    for (const type of pending) {
      for (const operation of recordReaders[type] ?? []) {
        if (type === RULE_RECORD && operation === BODY_READER) {
          rules = true;
        } else {
          readers.add(operation);
        }
      }
    }
    const gone = deleted;
    pending = new Set();
    deleted = [];
    for (const record of gone) {
      const ofType = new Set(recordReaders[record.type]);
      queryClient.removeQueries({
        predicate: (query) =>
          readsAny(query, ofType) &&
          inWorkspace(query, workspace) &&
          Object.values(keyParams(query)).includes(record.id),
      });
    }
    if (rules) {
      void refreshBodiesAfterRules(queryClient, workspace, holdsImageState);
    }
    if (readers.size > 0) {
      void queryClient.invalidateQueries({
        predicate: (query) => readsAny(query, readers) && inWorkspace(query, workspace),
      });
    }
  };

  return {
    change(record: RecordRef, op: "upsert" | "delete") {
      pending.add(record.type);
      if (op === "delete") {
        deleted.push(record);
      }
      timer ??= setTimeout(flush, BATCH_MS);
    },
    reset() {
      // Anything may have changed since, rules too: no trusted body is kept.
      void dropTrustedBodies(queryClient, workspace);
      void queryClient.invalidateQueries({
        predicate: (query) => keyParams(query)["workspace_id"] === workspace,
      });
    },
    dispose() {
      if (timer !== undefined) {
        clearTimeout(timer);
        flush();
      }
    },
  };
}

export interface SubscribeOptions {
  queryClient: QueryClient;
  workspace: string;
  source: ChangeSource;
  /** The page whose visibility pauses the stream; the global one by default. */
  page?: Pick<Document, "visibilityState" | "addEventListener" | "removeEventListener">;
}

/** How long a hidden page keeps its stream before closing it. */
export const HIDDEN_GRACE_MS = 30_000;
/** First and longest wait before reopening a stream that closed. */
const RETRY_MIN_MS = 1000;
const RETRY_MAX_MS = 60_000;

/**
 * Follow `workspace`'s changes into `queryClient` until the returned
 * function is called. Pauses while the page is hidden (after
 * {@link HIDDEN_GRACE_MS}) and resumes from the last cursor; reopens a
 * closed stream with backoff; stops when the server says the caller no
 * longer belongs to the workspace.
 */
export function subscribeToChanges({
  queryClient,
  workspace,
  source,
  page = globalThis.document,
}: SubscribeOptions): () => void {
  const cache = createCacheBinding(queryClient, workspace);
  let cursor: string | undefined;
  let close: (() => void) | undefined;
  let stopped = false;
  let retryMs = RETRY_MIN_MS;
  let retryTimer: ReturnType<typeof setTimeout> | undefined;
  let hiddenTimer: ReturnType<typeof setTimeout> | undefined;

  const disconnect = () => {
    close?.();
    close = undefined;
  };

  const stop = () => {
    stopped = true;
    disconnect();
    clearTimeout(retryTimer);
    clearTimeout(hiddenTimer);
    page.removeEventListener("visibilitychange", onVisibility);
    cache.dispose();
  };

  const onFrame = (frame: ChangeFrame) => {
    retryMs = RETRY_MIN_MS;
    switch (frame.kind) {
      case "change": {
        cache.change(frame.record, frame.op);
        cursor = frame.cursor;
        break;
      }
      case "reset": {
        cache.reset();
        cursor = frame.cursor;
        break;
      }
      case "ready":
      case "heartbeat": {
        cursor = frame.cursor;
        break;
      }
      case "revoked": {
        stop();
        cache.reset();
        void queryClient.invalidateQueries({ queryKey: operationKey("workspaces.list") });
        break;
      }
      case "closing": {
        // The source reconnects from the cursor.
        break;
      }
    }
  };

  const connect = () => {
    if (stopped || close !== undefined) {
      return;
    }
    close = source.open(workspace, cursor, {
      onFrame,
      onClosed: () => {
        disconnect();
        if (stopped) {
          return;
        }
        // Refused: maybe no longer a member; the workspace list will tell.
        void queryClient.invalidateQueries({ queryKey: operationKey("workspaces.list") });
        retryTimer = setTimeout(connect, retryMs);
        retryMs = Math.min(retryMs * 2, RETRY_MAX_MS);
      },
    });
  };

  function onVisibility() {
    if (page.visibilityState === "hidden") {
      hiddenTimer ??= setTimeout(() => {
        hiddenTimer = undefined;
        clearTimeout(retryTimer);
        disconnect();
      }, HIDDEN_GRACE_MS);
    } else {
      clearTimeout(hiddenTimer);
      hiddenTimer = undefined;
      connect();
    }
  }

  page.addEventListener("visibilitychange", onVisibility);
  if (page.visibilityState !== "hidden") {
    connect();
  }
  return stop;
}
