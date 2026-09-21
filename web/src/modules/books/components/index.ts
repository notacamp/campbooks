/**
 * modules/books/components — barrel for the Books surface view.
 *
 * BooksView is a presentational, prop-driven component. It belongs here
 * (not lib/ui) because it is the surface-level Books layout, not a
 * reusable design primitive.
 *
 * Does NOT re-export from modules/books/index.ts — another session owns
 * that barrel.
 */
export {
  BooksView,
  type BooksViewProps,
  type MoneyPage,
  type Obligation,
  type NeedsYouItem,
  type Loan,
  type LoanSuggestion,
} from "./books-view";
