/**
 * modules/today/api — action mutations for the Today surface.
 *
 * useTodayActionMutation:
 *   Dispatches the forward action to the correct e1 endpoint based on
 *   the item's source ("people" | "time" | "money").
 *   - people → POST /api/app/people/:ref_id/action { kind }
 *   - time (task) → POST /api/app/asks/:ref_id/done|snooze
 *   - time (deadline/reminder) → POST /api/app/reminders/:ref_id/confirm
 *                                 DELETE /api/app/reminders/:ref_id
 *   - money → navigational only (view/add_statement/reconcile); logged no-op.
 *
 *   No optimistic cache removal — TodayView owns the visual collapse + undo
 *   strip, so stripping needs_you would hide it. onSuccess invalidates ["today"]
 *   to re-sync (the server did the action). The mutation promise rejects on a
 *   non-2xx so the view can roll its collapse back (see routes/today.tsx).
 *
 * useTodayUndoMutation:
 *   Inverse of people actions (archive → unarchive, done → undo_done,
 *   snooze → unsnooze) using the undo_kind + undo_params returned by the
 *   people actions endpoint.
 *   For time/money items there is no server-side undo; settling ["today"]
 *   is the restore (the aggregator re-queries on each request).
 */

import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import { logger } from "~/lib/logger";
import type { NeedsYouItem } from "~/modules/today/components";
import { todayKeys } from "./index";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface TodayActionVars {
  itemId: string;
  actionKey: string;
  item: NeedsYouItem;
}

/** The relevant fields from the people-action server response. */
export interface TodayActionResult {
  undoKind?: string;
  undoParams?: Record<string, unknown>;
}

export interface TodayUndoVars {
  ref_id: string | number;
  undoKind: string;
  undoParams?: Record<string, unknown>;
}

// ── People action response shape ──────────────────────────────────────────────

interface PeopleActionResponse {
  row: unknown;
  undo_kind?: string;
  undo_params?: Record<string, unknown>;
  message?: string;
}

// ── useTodayActionMutation ────────────────────────────────────────────────────

export const useTodayActionMutation = (): UseMutationResult<
  TodayActionResult,
  Error,
  TodayActionVars
> => {
  const queryClient = useQueryClient();

  return useMutation<TodayActionResult, Error, TodayActionVars>({
    mutationFn: async ({ item, actionKey }) => {
      const { source, ref_id } = item;

      if (source === "people") {
        // POST /api/app/people/:ref_id/action { kind }
        const resp = await apiClient<PeopleActionResponse>(
          `/people/${ref_id}/action`,
          { method: "POST", body: { kind: actionKey } },
        );
        return {
          undoKind: resp.undo_kind,
          undoParams: resp.undo_params,
        };
      }

      if (source === "time") {
        const isTask = item.id.startsWith("time_task_");
        const isDeadline = item.id.startsWith("time_deadline_");

        if (isTask) {
          if (actionKey === "done") {
            await apiClient(`/asks/${ref_id}/done`, { method: "POST" });
          } else if (actionKey === "snooze") {
            await apiClient(`/asks/${ref_id}/snooze`, { method: "POST" });
          } else {
            logger.info(
              `[today] unhandled task action: ${actionKey} for item ${item.id}`,
            );
          }
        } else if (isDeadline) {
          if (actionKey === "confirm") {
            await apiClient(`/reminders/${ref_id}/confirm`, { method: "POST" });
          } else if (actionKey === "dismiss") {
            await apiClient(`/reminders/${ref_id}`, { method: "DELETE" });
          } else {
            logger.info(
              `[today] unhandled deadline action: ${actionKey} for item ${item.id}`,
            );
          }
        } else {
          logger.info(
            `[today] unknown time item type for action: ${actionKey} — id=${item.id}`,
          );
        }
        return {};
      }

      // source === "money" — actions (view, add_statement, reconcile) are navigational.
      // TODO(actions): confirm route with e1 — money Today actions are navigational
      logger.info(
        `[today] money action (navigational, no-op): ${actionKey} item=${item.id}`,
      );
      return {};
    },

    // No optimistic cache mutation: TodayView owns the collapse/undo strip and
    // the resolved→gone lifecycle. Removing the item from needs_you here would
    // hide that strip. On success the server performed the action, so refetch
    // to re-sync; on failure the promise rejects and the view rolls back.
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: todayKeys.all });
    },
  });
};

// ── useTodayUndoMutation ──────────────────────────────────────────────────────

/**
 * Calls the inverse people action (unarchive / undo_done / unsnooze) and
 * re-syncs the Today cache. For time/money items — which have no server-side
 * undo endpoint — invalidating Today is sufficient (the aggregator re-runs).
 */
export const useTodayUndoMutation = (): UseMutationResult<
  void,
  Error,
  TodayUndoVars
> => {
  const queryClient = useQueryClient();

  return useMutation<void, Error, TodayUndoVars>({
    mutationFn: async ({ ref_id, undoKind, undoParams }) => {
      await apiClient(`/people/${ref_id}/action`, {
        method: "POST",
        body: { kind: undoKind, ...undoParams },
      });
    },

    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: todayKeys.all });
    },
  });
};
