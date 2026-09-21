/**
 * modules/inbox/components — barrel.
 *
 * Visual InboxPage components live in src/routes/inbox/ (where the HTML
 * layout layer is not restricted). This barrel re-exports the module's
 * public surface so consumers can import from "~/modules/inbox/components".
 *
 * Design rationale: the ESLint modules rule (react/forbid-elements) prevents
 * raw HTML in module component files; the layout shell lives in routes/ instead.
 */

// Re-export types and pure mappers that compose into the visual layer.
export type { InboxRow, InboxTab, InboxQueryData } from "../types";
export { annotationFor, signalPlaceFor } from "../hooks";
