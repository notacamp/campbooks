/**
 * modules/books/api — data hook for the Books surface.
 *
 * Wraps TanStack Query around GET /api/app/money.
 * Returns the full MoneyPage shape from Money::Page.for serializer.
 *
 * Note: this endpoint may be gated (ENABLE_ACCOUNTING; may 404 in some
 * environments). TanStack Query will surface the error; the route handles it
 * with a graceful error state rather than crashing.
 */

import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { MoneyPage } from "~/modules/books/components";

// ── Query key factory ─────────────────────────────────────────────────────────

export const booksKeys = {
  all: ["books"] as const,
};

// ── Query hook ────────────────────────────────────────────────────────────────

/**
 * Fetches the Books surface payload from GET /api/app/money.
 *
 * Response shape (unwrapped by apiClient):
 *   MoneyPage — { obligations, needs_you, needs_you_count, loans,
 *                 loan_suggestions, statement_counts, selected_statement_id }
 *
 * staleTime: 30 s — reconciliation data updates on statement uploads.
 */
export const useBooksQuery = (): UseQueryResult<MoneyPage, Error> =>
  useQuery<MoneyPage, Error>({
    queryKey: booksKeys.all,
    queryFn: () => apiClient<MoneyPage>("/money"),
    staleTime: 30_000,
  });
