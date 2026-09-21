/**
 * lib/realtime — ActionCable-compatible WebSocket consumer.
 *
 * Implements the Rails ActionCable wire protocol over a native WebSocket
 * (no @rails/actioncable dependency). The bearer token is sent as a query
 * parameter so the Cable server can authenticate the connection.
 *
 * Usage:
 *   const unsub = subscribeUserSync("people", (data) => {
 *     queryClient.invalidateQueries({ queryKey: ["inbox"] });
 *   });
 *   // Later:
 *   unsub();
 *
 * The connection is lazy — it opens on first subscribe and closes when all
 * subscribers unsubscribe. A disconnect is attempted gracefully (code 1000).
 *
 * In test/SSR environments where WebSocket is undefined, subscriptions are
 * silently no-ops so the component tree is unaffected.
 */

import { getToken } from "~/lib/api/token";
import { logger } from "~/lib/logger";

// ── ActionCable wire types ────────────────────────────────────────────────────

type CableMessage =
  | { type: "welcome" }
  | { type: "ping"; message: number }
  | { type: "confirm_subscription"; identifier: string }
  | { type: "reject_subscription"; identifier: string }
  | { identifier: string; message: { topic: string; [key: string]: unknown } };

// ── State ─────────────────────────────────────────────────────────────────────

let socket: WebSocket | null = null;
let subscribed = false;
const listeners = new Map<string, Set<(data: unknown) => void>>();

const IDENTIFIER = JSON.stringify({ channel: "UserSyncChannel" });

// ── Helpers ───────────────────────────────────────────────────────────────────

const cableUrl = (): string => {
  const token = getToken();
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  const base = `${proto}://${window.location.host}/cable`;
  return token ? `${base}?token=${encodeURIComponent(token)}` : base;
};

const send = (payload: object): void => {
  if (socket?.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify(payload));
  }
};

const subscribe = (): void => {
  send({ command: "subscribe", identifier: IDENTIFIER });
};

const handleMessage = (raw: string): void => {
  let msg: CableMessage;
  try {
    msg = JSON.parse(raw) as CableMessage;
  } catch {
    return;
  }

  if ("type" in msg) {
    if (msg.type === "welcome") subscribe();
    if (msg.type === "confirm_subscription") {
      subscribed = true;
      logger.info("[realtime] UserSyncChannel confirmed");
    }
    return;
  }

  // Data broadcast
  if (!("message" in msg) || !msg.message?.topic) return;
  const topic: string = msg.message.topic;
  const handlers = listeners.get(topic);
  if (handlers) {
    for (const fn of handlers) {
      fn(msg.message);
    }
  }
};

const connect = (): void => {
  if (socket) return;
  if (typeof WebSocket === "undefined") return;

  socket = new WebSocket(cableUrl());

  socket.addEventListener("open", () => {
    logger.info("[realtime] WebSocket open");
    if (subscribed) subscribe(); // re-subscribe on reconnect
  });

  socket.addEventListener("message", (ev) => {
    if (typeof ev.data === "string") handleMessage(ev.data);
  });

  socket.addEventListener("close", () => {
    logger.info("[realtime] WebSocket closed");
    socket = null;
    subscribed = false;
  });

  socket.addEventListener("error", () => {
    logger.warn("[realtime] WebSocket error");
  });
};

const maybeDisconnect = (): void => {
  if (listeners.size === 0 && socket) {
    socket.close(1000, "No subscribers");
    socket = null;
    subscribed = false;
  }
};

// ── Public API ────────────────────────────────────────────────────────────────

/**
 * Subscribe to a UserSyncChannel topic.
 * Returns an unsubscribe function.
 *
 * @param topic  The topic to filter on (e.g. "people").
 * @param fn     Called with the broadcast message payload.
 */
export const subscribeUserSync = (
  topic: string,
  fn: (data: unknown) => void,
): (() => void) => {
  if (typeof WebSocket === "undefined") {
    // SSR / test env — silently no-op.
    return () => undefined;
  }

  if (!listeners.has(topic)) {
    listeners.set(topic, new Set());
  }
  listeners.get(topic)!.add(fn);

  connect();

  return () => {
    const set = listeners.get(topic);
    if (set) {
      set.delete(fn);
      if (set.size === 0) listeners.delete(topic);
    }
    maybeDisconnect();
  };
};
