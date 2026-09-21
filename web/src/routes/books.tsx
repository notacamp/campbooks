/**
 * BooksPage — the accounting ledger surface at /books.
 *
 * Connects BooksView (design layer) to GET /api/app/money via useBooksQuery.
 *
 * States: loading skeleton / error+retry (graceful 404 if endpoint gated) / view.
 * Callbacks:
 *   onLineAction(lineId, kind) — wired to e1's bank-transaction and obligation
 *     action endpoints via useBooksLineActionMutation. Confirms with an in-place
 *     UndoDock when the inverse is available (reset undoes confirm/reject/exclude).
 *
 * Mirrors the Inbox pattern (useQuery → states → view).
 */
import { type FC, useCallback, useState } from "react";
import { createRoute } from "@tanstack/react-router";
import type { AnyRoute } from "@tanstack/react-router";
import { RotateCcw } from "lucide-react";
import { Visor } from "~/lib/ui";
import { cn } from "~/lib/utils";
import { BooksView } from "~/modules/books/components";
import { useBooksQuery } from "~/modules/books/api";
import { useBooksLineActionMutation } from "~/modules/books/api/use-books-line-action-mutation";
import { UndoDock } from "./inbox/UndoDock";

// ── Loading skeleton ───────────────────────────────────────────────────────────

const BooksSkeleton: FC = () => (
  <div
    data-testid="books.page.loading"
    className="flex flex-col gap-4 p-6 max-w-4xl mx-auto w-full animate-pulse"
    aria-busy="true"
    aria-label="Loading books"
  >
    {/* Status bar placeholder */}
    <div className="h-5 w-56 bg-surface-1 rounded-cb-1" />
    {/* Two-column placeholders */}
    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mt-2">
      <div className="h-40 bg-surface-1 rounded-cb-1" />
      <div className="h-40 bg-surface-1 rounded-cb-1" />
    </div>
    {/* Loan card placeholder */}
    <div className="h-28 bg-surface-1 rounded-cb-1 mt-2" />
  </div>
);

// ── Undo state ─────────────────────────────────────────────────────────────────

interface UndoState {
  lineId: string;
  kind: string;
  undoKind: string;
  reconciliationId: number | null;
  label: string;
}

// ── BooksPage ─────────────────────────────────────────────────────────────────

export const BooksPage: FC = () => {
  const { data, isFetching, isError, error, refetch } = useBooksQuery();
  const lineActionMutation = useBooksLineActionMutation();
  const [undoState, setUndoState] = useState<UndoState | null>(null);

  const handleLineAction = useCallback(
    (lineId: string, kind: string): void => {
      const reconciliationId = data?.selected_statement_id ?? null;

      lineActionMutation.mutate(
        { lineId, kind, reconciliationId },
        {
          onSuccess: (result) => {
            if (result.undoKind) {
              setUndoState({
                lineId,
                kind,
                undoKind: result.undoKind,
                reconciliationId,
                label: `Line ${kind}`,
              });
            }
          },
        },
      );
    },
    [data, lineActionMutation],
  );

  // Undo: call the inverse action (reset undoes confirm/reject/exclude).
  const handleUndo = useCallback((): void => {
    if (!undoState) return;
    lineActionMutation.mutate({
      lineId: undoState.lineId,
      kind: undoState.undoKind,
      reconciliationId: undoState.reconciliationId,
    });
  }, [undoState, lineActionMutation]);

  const handleDismissUndo = useCallback((): void => {
    setUndoState(null);
  }, []);

  const isPending = isFetching && !data;

  if (isPending) {
    return (
      <div className="h-full overflow-y-auto">
        <BooksSkeleton />
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="books.page.error"
        className="flex flex-col items-center justify-center h-full gap-3 p-6 text-center"
      >
        <Visor state="asleep" size={32} label="Error loading books" />
        <p className="text-[13px] text-t2">
          {error instanceof Error ? error.message : "Something went wrong"}
        </p>
        <button
          type="button"
          onClick={() => refetch()}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
            "bg-surface-1 border border-line text-[12.5px] text-t2",
            "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
          )}
          data-testid="books.page.retry"
        >
          <RotateCcw size={12} strokeWidth={2} aria-hidden="true" />
          Retry
        </button>
      </div>
    );
  }

  return (
    <div
      data-testid="books.page.root"
      className="h-full overflow-y-auto flex flex-col"
    >
      <BooksView
        data={data}
        onLineAction={handleLineAction}
        className="max-w-4xl w-full mx-auto p-6 flex-1"
      />

      {/* Undo dock — appears after confirm/reject/exclude with a Reset inverse */}
      {undoState != null && (
        <UndoDock
          archivedName={undoState.label}
          onUndo={handleUndo}
          onDismiss={handleDismissUndo}
        />
      )}
    </div>
  );
};

// ── Route builder ─────────────────────────────────────────────────────────────

export const buildBooksRoutes = (parent: AnyRoute): AnyRoute[] => [
  createRoute({
    getParentRoute: () => parent,
    path: "/books",
    component: BooksPage,
  }),
];
