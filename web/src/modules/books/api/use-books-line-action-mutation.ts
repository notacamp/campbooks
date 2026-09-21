/**
 * modules/books/api — line action mutation for the Books surface.
 *
 * useBooksLineActionMutation:
 *   Dispatches a workbench action on a bank transaction or obligation to the
 *   correct e1 endpoint:
 *
 *   Bank transactions (nested under a reconciliation):
 *     confirm         → POST /api/app/reconciliations/:reconId/bank_transactions/:id/confirm
 *     reject          → POST /api/app/reconciliations/:reconId/bank_transactions/:id/reject
 *     exclude         → POST /api/app/reconciliations/:reconId/bank_transactions/:id/exclude
 *     reset           → POST /api/app/reconciliations/:reconId/bank_transactions/:id/reset
 *     request_invoice → POST /api/app/reconciliations/:reconId/bank_transactions/:id/request_invoice
 *
 *   Obligation actions (money surface):
 *     mark_paid       → PATCH /api/app/money/obligations/:id/settle
 *     confirm_line    → POST  /api/app/money/obligations/:id/confirm_line
 *     chase           → POST  /api/app/money/obligations/:id/chase
 *     reset_line      → POST  /api/app/money/obligations/:id/reset_line
 *
 *   Navigational / ambiguous actions (routed here as they reach the route):
 *     add_statement, reconcile_statements, hunt
 *     → TODO(actions): confirm route with e1 — these are navigational
 *
 *   onMutate: optimistically marks the item as "pending" by snapshotting cache.
 *   onError: rolls back to snapshot.
 *   onSettled: invalidates ["books"] so the server state is the final truth.
 *
 *   Returns { undoKind } so the page can offer Undo (reset is the inverse of
 *   confirm/reject/exclude for bank transactions).
 */

import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import { logger } from "~/lib/logger";
import type { MoneyPage } from "~/modules/books/components";
import { booksKeys } from "./index";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface BooksLineActionVars {
  /** transaction_id, loan_id, reconciliation_id, or obligation id. */
  lineId: string;
  /** Action kind: "confirm", "reject", "reset", "mark_paid", etc. */
  kind: string;
  /**
   * The reconciliation (statement) the transaction belongs to.
   * Required for bank-transaction actions (which are nested under a
   * reconciliation). Pass data.selected_statement_id from the page.
   */
  reconciliationId: number | null;
}

export interface BooksLineActionResult {
  /** The inverse action kind, if this action is undoable (e.g. "reset"). */
  undoKind?: string;
}

// ── Mutation ──────────────────────────────────────────────────────────────────

/** Bank-transaction action kinds that nest under /reconciliations/:id/bank_transactions/:id/. */
const BANK_TX_ACTION_KINDS = new Set([
  "confirm",
  "reject",
  "exclude",
  "reset",
  "request_invoice",
  "manual_match",
]);

/** Obligation action kinds that live under /money/obligations/:id/. */
const OBLIGATION_ACTION_MAP: Record<string, { method: string; suffix: string }> = {
  mark_paid:    { method: "PATCH", suffix: "settle" },
  confirm_line: { method: "POST",  suffix: "confirm_line" },
  chase:        { method: "POST",  suffix: "chase" },
  reset_line:   { method: "POST",  suffix: "reset_line" },
  settle:       { method: "PATCH", suffix: "settle" },
};

/** Returns the undo kind for a bank-transaction action, if one exists. */
const undoKindFor = (kind: string): string | undefined => {
  if (kind === "confirm" || kind === "reject" || kind === "exclude") return "reset";
  return undefined;
};

export const useBooksLineActionMutation = (): UseMutationResult<
  BooksLineActionResult,
  Error,
  BooksLineActionVars,
  { snapshot: MoneyPage | undefined }
> => {
  const queryClient = useQueryClient();

  return useMutation<
    BooksLineActionResult,
    Error,
    BooksLineActionVars,
    { snapshot: MoneyPage | undefined }
  >({
    mutationFn: async ({ lineId, kind, reconciliationId }) => {
      // ── Bank transaction actions (nested under reconciliation) ────────────
      if (BANK_TX_ACTION_KINDS.has(kind)) {
        if (reconciliationId == null) {
          logger.warn(
            `[books] bank_transaction action "${kind}" needs reconciliationId but got null`,
          );
          return {};
        }
        await apiClient(
          `/reconciliations/${reconciliationId}/bank_transactions/${lineId}/${kind}`,
          { method: "POST" },
        );
        return { undoKind: undoKindFor(kind) };
      }

      // ── Obligation actions ─────────────────────────────────────────────────
      const obligationAction = OBLIGATION_ACTION_MAP[kind];
      if (obligationAction) {
        await apiClient(
          `/money/obligations/${lineId}/${obligationAction.suffix}`,
          { method: obligationAction.method },
        );
        return {};
      }

      // ── Navigational / not yet wired ──────────────────────────────────────
      // TODO(actions): confirm route with e1 — add_statement, reconcile_statements, hunt
      logger.info(`[books] action not wired (navigational or ambiguous): ${kind} lineId=${lineId}`);
      return {};
    },

    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey: booksKeys.all });
      const snapshot = queryClient.getQueryData<MoneyPage>(booksKeys.all);
      // No optimistic UI update for Books — the workbench state is complex.
      // We snapshot for rollback only; the settled invalidation refreshes the view.
      return { snapshot };
    },

    onError: (_err, _vars, context) => {
      if (context?.snapshot !== undefined) {
        queryClient.setQueryData(booksKeys.all, context.snapshot);
      }
    },

    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: booksKeys.all });
    },
  });
};
