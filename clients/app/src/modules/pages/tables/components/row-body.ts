import { createContext, type ReactNode } from "react";

import type { usePageActions } from "../../components/use-page-actions";
import type { PageSummary } from "../../types";

export interface RowBodyProps {
  workspaceId: string;
  rowId: string;
  pages: readonly PageSummary[];
  actions: ReturnType<typeof usePageActions>;
  /** The row can be written (not a reader's). */
  editable: boolean;
  open: (href: string) => void;
}

/**
 * What draws a row's page under its properties (a function that makes it). The page editor holds tables
 * (a table can sit in a page), so the editor is handed down by the page
 * screen rather than imported here: without one a row shows its properties only.
 */
export const RowBodyContext = createContext<((props: RowBodyProps) => ReactNode) | null>(null);
