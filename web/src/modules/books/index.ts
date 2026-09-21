/**
 * modules/books — public barrel for the Books module.
 *
 * Exports the module config (for main.tsx registration) and the public
 * surface (types, hooks) that routes may consume.
 */

import type { ModuleConfig } from "~/modules";

export const moduleConfig: ModuleConfig = {
  extensions: [],
};

// Public types — re-exported from the design view (do not redefine).
export type {
  MoneyPage,
  BooksViewProps,
  Obligation,
  Loan,
  LoanSuggestion,
} from "./components";

// Data hook
export { useBooksQuery, booksKeys } from "./api";
