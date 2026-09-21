/**
 * modules/inbox/hooks — pure mappers and derived values for the Inbox module.
 *
 * annotationFor(row) — derives a plain-language annotation string from a row's
 *   verb and stand_line fields.
 *
 * signalPlaceFor(verb) — maps a verb to the semantic Place for Signal.Dot /
 *   Avatar tinting.
 *
 * useInboxRealtimeSync(queryClient) — wires the UserSyncChannel to invalidate
 *   the inbox query on any "people" broadcast.
 */

import { useEffect } from "react";
import { type QueryClient } from "@tanstack/react-query";
import { subscribeUserSync } from "~/lib/realtime";
import type { InboxRow } from "../types";
import type { Place } from "~/lib/ui";
import { inboxKeys } from "../api";

// ── annotationFor ─────────────────────────────────────────────────────────────

/**
 * Plain-language annotation for an inbox row.
 *
 * Maps verb → a concise phrase. Falls back to stand_line verbatim,
 * which is Scout's own prose. Never returns undefined (returns "" on nothing).
 */
export const annotationFor = (row: InboxRow): string => {
  switch (row.verb) {
    case "reply":
      return "Wants a reply";
    case "pay":
      return "Invoice · to pay";
    case "decide":
      return "Decision";
    case "chase":
    case "nudge":
      return "Waiting on you";
    case "do":
      return row.stand_line ?? "Task";
    default:
      return row.stand_line ?? "";
  }
};

// ── signalPlaceFor ────────────────────────────────────────────────────────────

/**
 * Maps an inbox row verb to the Signal place for the annotation dot and Avatar.
 *
 * reply / decide → people  (conversation)
 * pay            → money   (financial)
 * do             → time    (task/calendar)
 * chase / nudge  → now     (follow-up needed)
 * default        → none    (neutral)
 */
export const signalPlaceFor = (verb: string | null): Place => {
  switch (verb) {
    case "reply":
    case "decide":
      return "people";
    case "pay":
      return "money";
    case "do":
      return "time";
    case "chase":
    case "nudge":
      return "now";
    default:
      return "none";
  }
};

// ── useInboxRealtimeSync ──────────────────────────────────────────────────────

/**
 * Subscribes to the "people" UserSyncChannel topic and invalidates the inbox
 * query whenever the server broadcasts an update.
 *
 * No-op in test/SSR environments (subscribeUserSync returns a no-op there).
 */
export const useInboxRealtimeSync = (queryClient: QueryClient): void => {
  useEffect(() => {
    const unsub = subscribeUserSync("people", () => {
      queryClient.invalidateQueries({ queryKey: inboxKeys.all });
    });
    return unsub;
  }, [queryClient]);
};

// Re-exports for convenience
export { useInboxInfiniteQuery, useArchiveMutation, inboxKeys } from "../api";
