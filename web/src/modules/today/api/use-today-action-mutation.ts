/**
 * modules/today/api — data-driven action mutations for the Today surface.
 *
 * e1's serializer makes every action self-describing:
 *   { kind, label, primary, endpoint, method, body?, undo?: { endpoint, method, body? } }
 * So we fire `action.endpoint`/`method`/`body` directly (no source-based dispatch),
 * and on Undo fire `action.undo`. Navigational actions (no endpoint — e.g. money
 * "review") are handled by the route (it opens the surface), not here.
 *
 * No optimistic cache mutation: TodayView owns the collapse/undo strip, so
 * removing the item from needs_you would hide it. onSuccess invalidates ["today"]
 * to re-sync; the mutation promise rejects on a non-2xx so the view rolls back.
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { TodayAction } from "~/modules/today/components";
import { todayKeys } from "./index";

// ── A self-describing call (a forward action or its inverse) ────────────────────

export interface ActionCall {
  endpoint: string;
  method: string;
  body?: Record<string, unknown> | null;
}

/**
 * The runtime shape of a Today action: b4's view-facing `TodayAction`
 * ({kind,label,primary}) plus the dispatch hints e1's serializer adds. The
 * view doesn't declare these (it only needs the label), so we extend it here.
 */
export interface TodayActionHints extends TodayAction {
  endpoint?: string;
  method?: string;
  body?: Record<string, unknown> | null;
  undo?: ActionCall | null;
}

const fireCall = async ({ endpoint, method, body }: ActionCall): Promise<void> => {
  // e1's hint endpoints are absolute (they already include /api/app); apiClient
  // re-prepends /api/app, so strip the redundant prefix to avoid /api/app/api/app.
  // (A relative endpoint has no prefix to strip, so this is safe either way.)
  const path = endpoint.replace(/^\/api\/app/, "");
  await apiClient(path, {
    method: method.toUpperCase(),
    ...(body != null ? { body } : {}),
  });
};

// After a successful action, TodayView owns the optimistic collapse + the undo
// window, keeping the actioned item in the cache so Undo can restore it. We
// reconcile the cache only AFTER that window closes — refetching sooner would
// remove the item and yank the in-place "Undo" strip mid-window. Keep this
// comfortably longer than TodayView's undo window.
const UNDO_RECONCILE_MS = 5000;

// ── useTodayActionMutation ────────────────────────────────────────────────────

export const useTodayActionMutation = (): UseMutationResult<
  void,
  Error,
  ActionCall
> => {
  const queryClient = useQueryClient();

  return useMutation<void, Error, ActionCall>({
    mutationFn: fireCall,
    // Don't refetch immediately: TodayView keeps the actioned item in the cache
    // for its in-place undo window, and an immediate refetch would remove it and
    // yank the "Undo" strip mid-window. Reconcile only after the window closes
    // (so the item also doesn't reappear on remount). On failure the promise
    // rejects and TodayView rolls its own collapse back.
    onSuccess: () => {
      setTimeout(() => {
        void queryClient.invalidateQueries({ queryKey: todayKeys.all });
      }, UNDO_RECONCILE_MS);
    },
  });
};

// ── useTodayUndoMutation ──────────────────────────────────────────────────────

/** Fires an action's inverse (`action.undo`) and re-syncs Today. */
export const useTodayUndoMutation = (): UseMutationResult<
  void,
  Error,
  ActionCall
> => {
  const queryClient = useQueryClient();

  return useMutation<void, Error, ActionCall>({
    mutationFn: fireCall,
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: todayKeys.all });
    },
  });
};
